use std::sync::atomic::{AtomicU16, Ordering};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use serde_json::Value;
use crate::data::AccountStatus;
use crate::logger;

static BOUND_PORT: AtomicU16 = AtomicU16::new(8080);
const MINI_APP_HTML: &str = include_str!("web/index.html");

pub fn get_bound_port() -> u16 {
    BOUND_PORT.load(Ordering::Relaxed)
}

pub fn start_web_server(start_port: u16) {
    crate::steam::get_runtime().spawn(async move {
        let mut port = if start_port == 0 { 8080 } else { start_port };
        let mut listener = None;

        // Try ports if default is already occupied
        for p in port..=(port + 15) {
            match TcpListener::bind(format!("0.0.0.0:{}", p)).await {
                Ok(l) => {
                    port = p;
                    listener = Some(l);
                    break;
                }
                Err(_) => continue,
            }
        }

        let listener = match listener {
            Some(l) => l,
            None => {
                logger::log_error("Failed to start WebApp server: All candidate ports occupied");
                return;
            }
        };

        BOUND_PORT.store(port, Ordering::SeqCst);
        logger::log_success(&format!("🌐 Telegram Mini App Web Server running at: http://127.0.0.1:{}", port));

        loop {
            match listener.accept().await {
                Ok((mut socket, _)) => {
                    tokio::spawn(async move {
                        let mut buf = vec![0u8; 16384];
                        let n = match socket.read(&mut buf).await {
                            Ok(n) if n > 0 => n,
                            _ => return,
                        };

                        let req_str = String::from_utf8_lossy(&buf[..n]);
                        let response = handle_http_request(&req_str).await;

                        let _ = socket.write_all(response.as_bytes()).await;
                        let _ = socket.flush().await;
                    });
                }
                Err(_) => {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        }
    });
}

async fn handle_http_request(req: &str) -> String {
    let mut lines = req.lines();
    let request_line = match lines.next() {
        Some(l) => l,
        None => return build_response(400, "text/plain", "Bad Request"),
    };

    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 2 {
        return build_response(400, "text/plain", "Bad Request");
    }

    let method = parts[0];
    let full_uri = parts[1];

    // CORS preflight
    if method == "OPTIONS" {
        return "HTTP/1.1 204 No Content\r\n\
                Access-Control-Allow-Origin: *\r\n\
                Access-Control-Allow-Methods: GET, POST, OPTIONS\r\n\
                Access-Control-Allow-Headers: Content-Type\r\n\
                Connection: close\r\n\r\n".to_string();
    }

    let (path, query) = if let Some(idx) = full_uri.find('?') {
        (&full_uri[..idx], &full_uri[idx + 1..])
    } else {
        (full_uri, "")
    };

    // Body parsing for POST
    let body = if let Some(idx) = req.find("\r\n\r\n") {
        &req[idx + 4..]
    } else if let Some(idx) = req.find("\n\n") {
        &req[idx + 2..]
    } else {
        ""
    };

    // Routing
    match (method, path) {
        ("GET", "/") | ("GET", "/app") | ("GET", "/webapp") | ("GET", "/index.html") => {
            build_response(200, "text/html; charset=utf-8", MINI_APP_HTML)
        }

        ("GET", "/api/status") => {
            let accounts = crate::telegram::get_runtime_accounts();
            let mut acc_list = Vec::new();

            for arc in accounts {
                let a = arc.lock().unwrap();
                let status_str = match &a.status {
                    AccountStatus::Boosting => "Boosting".to_string(),
                    AccountStatus::Connecting => "Connecting".to_string(),
                    AccountStatus::AwaitingSteamGuard { .. } => "AwaitingSteamGuard".to_string(),
                    AccountStatus::PausedForGame { .. } => "InGame".to_string(),
                    AccountStatus::Reconnecting { .. } => "Reconnecting".to_string(),
                    AccountStatus::Error(e) => format!("Error: {}", e),
                    AccountStatus::Idle => "Idle".to_string(),
                };

                acc_list.push(serde_json::json!({
                    "id": a.id,
                    "username": a.username,
                    "custom_title": a.custom_title,
                    "status": status_str,
                    "hours_boosted": a.hours_boosted,
                    "games": a.games.iter().map(|g| serde_json::json!({
                        "app_id": g.app_id,
                        "name": g.name
                    })).collect::<Vec<_>>()
                }));
            }

            let resp = serde_json::json!({
                "ok": true,
                "accounts": acc_list
            });
            build_json_response(&resp)
        }

        ("GET", "/api/games/search") => {
            let mut search_query = String::new();
            for param in query.split('&') {
                if let Some(val) = param.strip_prefix("q=") {
                    search_query = urlencoding_decode(val);
                    break;
                }
            }

            let results = crate::telegram::search_steam_games(&search_query).await;
            let resp = serde_json::json!(results);
            build_json_response(&resp)
        }

        ("POST", "/api/boost/start_all") => {
            crate::telegram::start_all_remote();
            build_json_response(&serde_json::json!({ "ok": true }))
        }

        ("POST", "/api/boost/stop_all") => {
            crate::telegram::stop_all_remote();
            build_json_response(&serde_json::json!({ "ok": true }))
        }

        ("POST", "/api/boost/restart_all") => {
            crate::telegram::restart_all_remote();
            build_json_response(&serde_json::json!({ "ok": true }))
        }

        ("POST", "/api/boost/start") => {
            if let Ok(json) = serde_json::from_str::<Value>(body) {
                if let Some(id) = json.get("account_id").and_then(|v| v.as_u64()) {
                    let _ = crate::telegram::start_account_remote(&id.to_string());
                    return build_json_response(&serde_json::json!({ "ok": true }));
                }
            }
            build_json_response(&serde_json::json!({ "ok": false, "error": "Invalid account_id" }))
        }

        ("POST", "/api/boost/stop") => {
            if let Ok(json) = serde_json::from_str::<Value>(body) {
                if let Some(id) = json.get("account_id").and_then(|v| v.as_u64()) {
                    let _ = crate::telegram::stop_account_remote(&id.to_string());
                    return build_json_response(&serde_json::json!({ "ok": true }));
                }
            }
            build_json_response(&serde_json::json!({ "ok": false, "error": "Invalid account_id" }))
        }

        ("POST", "/api/accounts/add") => {
            if let Ok(json) = serde_json::from_str::<Value>(body) {
                let user = json.get("username").and_then(|v| v.as_str()).unwrap_or("");
                let pass = json.get("password").and_then(|v| v.as_str()).unwrap_or("");
                let title = json.get("custom_title").and_then(|v| v.as_str());
                let secret = json.get("shared_secret").and_then(|v| v.as_str());

                match crate::telegram::add_account_remote(user, pass, title, secret) {
                    Ok(id) => return build_json_response(&serde_json::json!({ "ok": true, "account_id": id })),
                    Err(e) => return build_json_response(&serde_json::json!({ "ok": false, "error": e })),
                }
            }
            build_json_response(&serde_json::json!({ "ok": false, "error": "Invalid JSON body" }))
        }

        ("POST", "/api/accounts/delete") => {
            if let Ok(json) = serde_json::from_str::<Value>(body) {
                if let Some(id) = json.get("account_id").and_then(|v| v.as_u64()) {
                    match crate::telegram::delete_account_remote(&id.to_string()) {
                        Ok(_) => return build_json_response(&serde_json::json!({ "ok": true })),
                        Err(e) => return build_json_response(&serde_json::json!({ "ok": false, "error": e })),
                    }
                }
            }
            build_json_response(&serde_json::json!({ "ok": false, "error": "Invalid account_id" }))
        }

        ("POST", "/api/accounts/games/add") => {
            if let Ok(json) = serde_json::from_str::<Value>(body) {
                let acc_id = json.get("account_id").and_then(|v| v.as_u64()).unwrap_or(0);
                let app_id = json.get("app_id").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                let name = json.get("name").and_then(|v| v.as_str()).unwrap_or("Game");

                if acc_id > 0 && app_id > 0 {
                    match crate::telegram::add_game_remote(&acc_id.to_string(), app_id, name) {
                        Ok(_) => return build_json_response(&serde_json::json!({ "ok": true })),
                        Err(e) => return build_json_response(&serde_json::json!({ "ok": false, "error": e })),
                    }
                }
            }
            build_json_response(&serde_json::json!({ "ok": false, "error": "Invalid parameters" }))
        }

        ("POST", "/api/accounts/games/remove") => {
            if let Ok(json) = serde_json::from_str::<Value>(body) {
                let acc_id = json.get("account_id").and_then(|v| v.as_u64()).unwrap_or(0);
                let app_id = json.get("app_id").and_then(|v| v.as_u64()).unwrap_or(0) as u32;

                if acc_id > 0 && app_id > 0 {
                    match crate::telegram::remove_game_remote(&acc_id.to_string(), app_id) {
                        Ok(_) => return build_json_response(&serde_json::json!({ "ok": true })),
                        Err(e) => return build_json_response(&serde_json::json!({ "ok": false, "error": e })),
                    }
                }
            }
            build_json_response(&serde_json::json!({ "ok": false, "error": "Invalid parameters" }))
        }

        ("POST", "/api/accounts/guard") => {
            if let Ok(json) = serde_json::from_str::<Value>(body) {
                let acc_id = json.get("account_id").and_then(|v| v.as_u64()).unwrap_or(0);
                let code = json.get("code").and_then(|v| v.as_str()).unwrap_or("");

                if acc_id > 0 && !code.is_empty() {
                    match crate::telegram::submit_guard_remote(&acc_id.to_string(), code) {
                        Ok(_) => return build_json_response(&serde_json::json!({ "ok": true })),
                        Err(e) => return build_json_response(&serde_json::json!({ "ok": false, "error": e })),
                    }
                }
            }
            build_json_response(&serde_json::json!({ "ok": false, "error": "Invalid parameters" }))
        }

        ("GET", "/api/cards") => {
            let cards = crate::inventory::get_recent_card_drops();
            build_json_response(&serde_json::json!(cards))
        }

        ("GET", "/api/logs") => {
            let logs = crate::logger::get_logs();
            let recent: Vec<_> = logs.iter().rev().take(30).collect();
            build_json_response(&serde_json::json!(recent))
        }

        _ => build_response(404, "text/plain", "Not Found"),
    }
}

fn build_response(status_code: u16, content_type: &str, body: &str) -> String {
    let status_text = match status_code {
        200 => "OK",
        204 => "No Content",
        400 => "Bad Request",
        404 => "Not Found",
        _ => "Status",
    };
    format!(
        "HTTP/1.1 {} {}\r\n\
        Content-Type: {}\r\n\
        Content-Length: {}\r\n\
        Access-Control-Allow-Origin: *\r\n\
        Connection: close\r\n\r\n{}",
        status_code,
        status_text,
        content_type,
        body.len(),
        body
    )
}

fn build_json_response(val: &Value) -> String {
    let body = val.to_string();
    build_response(200, "application/json", &body)
}

fn urlencoding_decode(val: &str) -> String {
    let mut bytes = Vec::new();
    let mut chars = val.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' {
            let h1 = chars.next().unwrap_or('0');
            let h2 = chars.next().unwrap_or('0');
            if let Ok(b) = u8::from_str_radix(&format!("{}{}", h1, h2), 16) {
                bytes.push(b);
            }
        } else if c == '+' {
            bytes.push(b' ');
        } else {
            bytes.extend_from_slice(c.to_string().as_bytes());
        }
    }
    String::from_utf8_lossy(&bytes).to_string()
}
