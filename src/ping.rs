use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionQuality {
    Good,
    Fair,
    Poor,
    Disconnected,
}

#[derive(Debug, Clone)]
pub struct PingStatus {
    pub ping_ms: Option<u32>,
    pub quality: ConnectionQuality,
    pub endpoint: String,
    pub last_checked: String,
    pub is_checking: bool,
}

impl Default for PingStatus {
    fn default() -> Self {
        Self {
            ping_ms: None,
            quality: ConnectionQuality::Disconnected,
            endpoint: "Steam CM".to_string(),
            last_checked: "--:--:--".to_string(),
            is_checking: false,
        }
    }
}

static PING_STATE: OnceLock<Mutex<PingStatus>> = OnceLock::new();
static MONITOR_STARTED: OnceLock<()> = OnceLock::new();

fn state() -> &'static Mutex<PingStatus> {
    PING_STATE.get_or_init(|| Mutex::new(PingStatus::default()))
}

pub fn get_ping_status() -> PingStatus {
    state().lock().map(|g| g.clone()).unwrap_or_default()
}

pub fn init_ping_monitor() {
    MONITOR_STARTED.get_or_init(|| {
        crate::steam::get_runtime().spawn(async {
            loop {
                measure_steam_cm_ping().await;
                tokio::time::sleep(Duration::from_secs(12)).await;
            }
        });
    });
}

pub fn trigger_ping_now() {
    crate::steam::get_runtime().spawn(async {
        measure_steam_cm_ping().await;
    });
}

async fn measure_steam_cm_ping() {
    if let Ok(mut s) = state().lock() {
        s.is_checking = true;
    }

    let targets = [
        ("cmp1-fra1.steamserver.net:27017", "Steam CM (EU)"),
        ("ext1-iad1.steamserver.net:27017", "Steam CM (US)"),
        ("api.steampowered.com:443", "Steam Web API"),
    ];

    let mut best_ms: Option<u32> = None;
    let mut best_label = "Steam CM".to_string();

    for (addr, label) in targets {
        let start = Instant::now();
        let conn = tokio::time::timeout(
            Duration::from_secs(3),
            tokio::net::TcpStream::connect(addr),
        )
        .await;

        if let Ok(Ok(_stream)) = conn {
            let ms = start.elapsed().as_millis().max(1) as u32;
            if best_ms.map_or(true, |prev| ms < prev) {
                best_ms = Some(ms);
                best_label = label.to_string();
            }
            break;
        }
    }

    let quality = match best_ms {
        Some(ms) if ms <= 80 => ConnectionQuality::Good,
        Some(ms) if ms <= 180 => ConnectionQuality::Fair,
        Some(_) => ConnectionQuality::Poor,
        None => ConnectionQuality::Disconnected,
    };

    let now = chrono::Local::now().format("%H:%M:%S").to_string();
    if let Ok(mut s) = state().lock() {
        s.ping_ms = best_ms;
        s.quality = quality;
        s.endpoint = best_label;
        s.last_checked = now;
        s.is_checking = false;
    }
}
