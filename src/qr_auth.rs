use qrcode::QrCode;
use serde::Deserialize;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::watch;
use crate::steam::get_runtime;

#[derive(Debug, Clone, PartialEq)]
pub enum QrAuthStatus {
    Idle,
    Loading,
    WaitingForScan {
        challenge_url: String,
        modules: Vec<qrcode::Color>,
        width: usize,
    },
    Success {
        account_name: String,
        refresh_token: String,
    },
    Error(String),
}

#[derive(Deserialize)]
struct BeginQrResponseWrapper {
    response: Option<BeginQrResponse>,
}

#[derive(Deserialize)]
struct BeginQrResponse {
    client_id: Option<serde_json::Value>,
    challenge_url: Option<String>,
    request_id: Option<String>,
    interval: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct PollStatusResponseWrapper {
    response: Option<PollStatusResponse>,
}

#[derive(Deserialize)]
struct PollStatusResponse {
    new_challenge_url: Option<String>,
    refresh_token: Option<String>,
    access_token: Option<String>,
    account_name: Option<String>,
}

pub struct QrAuthManager {
    pub status: Arc<Mutex<QrAuthStatus>>,
    cancel_tx: Option<watch::Sender<bool>>,
}

impl QrAuthManager {
    pub fn new() -> Self {
        Self {
            status: Arc::new(Mutex::new(QrAuthStatus::Idle)),
            cancel_tx: None,
        }
    }

    pub fn start(&mut self) {
        self.stop();

        let (cancel_tx, mut cancel_rx) = watch::channel(false);
        self.cancel_tx = Some(cancel_tx);
        let status_arc = self.status.clone();

        {
            let mut s = status_arc.lock().unwrap();
            *s = QrAuthStatus::Loading;
        }

        get_runtime().spawn(async move {
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(15))
                .build()
                .unwrap_or_default();

            // 1. BeginAuthSessionViaQR çağrısı (platform_type=1 -> SteamClient oturumu)
            let body_str = "device_friendly_name=SteamBooster&platform_type=1&website_id=Client";

            let res_result = client
                .post("https://api.steampowered.com/IAuthenticationService/BeginAuthSessionViaQR/v1/")
                .header("Content-Type", "application/x-www-form-urlencoded")
                .body(body_str)
                .send()
                .await;

            let res = match res_result {
                Ok(r) => r,
                Err(e) => {
                    let mut s = status_arc.lock().unwrap();
                    *s = QrAuthStatus::Error(format!("Steam API connection error: {}", e));
                    return;
                }
            };

            let begin_data = match res.json::<BeginQrResponseWrapper>().await {
                Ok(d) => d,
                Err(e) => {
                    let mut s = status_arc.lock().unwrap();
                    *s = QrAuthStatus::Error(format!("Failed to parse response: {}", e));
                    return;
                }
            };

            let begin = match begin_data.response {
                Some(b) => b,
                None => {
                    let mut s = status_arc.lock().unwrap();
                    *s = QrAuthStatus::Error("Steam failed to initiate QR session.".to_string());
                    return;
                }
            };

            let client_id_str = match begin.client_id {
                Some(serde_json::Value::String(s)) => s,
                Some(serde_json::Value::Number(n)) => n.to_string(),
                _ => {
                    let mut s = status_arc.lock().unwrap();
                    *s = QrAuthStatus::Error("Invalid client_id received.".to_string());
                    return;
                }
            };

            let mut challenge_url = match begin.challenge_url {
                Some(url) => url,
                None => {
                    let mut s = status_arc.lock().unwrap();
                    *s = QrAuthStatus::Error("challenge_url not received.".to_string());
                    return;
                }
            };

            let request_id = match begin.request_id {
                Some(req) => req,
                None => {
                    let mut s = status_arc.lock().unwrap();
                    *s = QrAuthStatus::Error("request_id not received.".to_string());
                    return;
                }
            };

            // QR kod oluştur
            if let Ok(code) = QrCode::new(challenge_url.as_bytes()) {
                let width = code.width();
                let modules = code.into_colors();
                let mut s = status_arc.lock().unwrap();
                *s = QrAuthStatus::WaitingForScan {
                    challenge_url: challenge_url.clone(),
                    modules,
                    width,
                };
            }

            let poll_interval = match begin.interval {
                Some(serde_json::Value::Number(n)) => n.as_f64().unwrap_or(3.0) as f32,
                _ => 3.0,
            }.clamp(1.5, 5.0);
            let mut timer = tokio::time::interval(Duration::from_secs_f32(poll_interval));

            // 2. Poll döngüsü
            loop {
                tokio::select! {
                    _ = timer.tick() => {
                        let poll_body = format!(
                            "client_id={}&request_id={}",
                            url_encode(&client_id_str),
                            url_encode(&request_id)
                        );

                        let poll_res_result = client
                            .post("https://api.steampowered.com/IAuthenticationService/PollAuthSessionStatus/v1/")
                            .header("Content-Type", "application/x-www-form-urlencoded")
                            .body(poll_body)
                            .send()
                            .await;

                        if let Ok(poll_res) = poll_res_result {
                            if let Ok(poll_data) = poll_res.json::<PollStatusResponseWrapper>().await {
                                if let Some(p) = poll_data.response {
                                    // Yenilenen QR kod var mı?
                                    if let Some(new_url) = p.new_challenge_url {
                                        if new_url != challenge_url {
                                            challenge_url = new_url.clone();
                                            if let Ok(code) = QrCode::new(challenge_url.as_bytes()) {
                                                let width = code.width();
                                                let modules = code.into_colors();
                                                let mut s = status_arc.lock().unwrap();
                                                *s = QrAuthStatus::WaitingForScan {
                                                    challenge_url: challenge_url.clone(),
                                                    modules,
                                                    width,
                                                };
                                            }
                                        }
                                    }

                                    // Başarılı giriş
                                    let token_opt = p.refresh_token.or(p.access_token);
                                    if let (Some(token), Some(acc_name)) = (token_opt, p.account_name) {
                                        let mut s = status_arc.lock().unwrap();
                                        *s = QrAuthStatus::Success {
                                            account_name: acc_name,
                                            refresh_token: token,
                                        };
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    _ = cancel_rx.changed() => {
                        if *cancel_rx.borrow() {
                            break;
                        }
                    }
                }
            }
        });
    }

    pub fn stop(&mut self) {
        if let Some(tx) = self.cancel_tx.take() {
            let _ = tx.send(true);
        }
        let mut s = self.status.lock().unwrap();
        *s = QrAuthStatus::Idle;
    }
}

fn url_encode(input: &str) -> String {
    let mut encoded = String::with_capacity(input.len());
    for b in input.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(b as char);
            }
            _ => {
                encoded.push_str(&format!("%{:02X}", b));
            }
        }
    }
    encoded
}
