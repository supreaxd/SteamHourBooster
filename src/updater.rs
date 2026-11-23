use std::sync::{Arc, Mutex};
use std::time::Duration;
use crate::logger;

#[derive(Debug, Clone, PartialEq)]
pub enum UpdateState {
    Idle,
    Checking,
    UpToDate {
        checked_at: String,
    },
    UpdateAvailable {
        version: String,
        tag_name: String,
        release_title: String,
        release_notes: String,
        published_at: String,
        download_url: String,
        asset_name: String,
        asset_size: u64,
    },
    Downloading {
        progress: f32,
        downloaded_bytes: u64,
        total_bytes: u64,
    },
    ReadyToRestart,
    Error(String),
}

#[derive(Debug, Clone)]
pub struct ReleaseInfo {
    pub tag_name: String,
    pub title: String,
    pub version: String,
    pub body: String,
    pub published_at: String,
    pub asset_name: String,
    pub asset_size: u64,
    pub download_url: String,
}

pub struct UpdateManager {
    pub state: Arc<Mutex<UpdateState>>,
    pub dismissed: bool,
}

impl UpdateManager {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(UpdateState::Idle)),
            dismissed: false,
        }
    }
}

/// On application boot, silently clean up any leftover .old or .new.exe from a previous update
pub fn cleanup_old_executable() {
    if let Ok(current_exe) = std::env::current_exe() {
        let old_backup = current_exe.with_extension("old");
        if old_backup.exists() {
            let _ = std::fs::remove_file(&old_backup);
            logger::log_info("🧹 Previous executable backup (.old) cleaned up.");
        }
        let temp_exe = current_exe.with_extension("new.exe");
        if temp_exe.exists() {
            let _ = std::fs::remove_file(&temp_exe);
        }
    }
}

/// Extract semantic version (major, minor, patch) from strings like "v0.3.0", "0.3.0", "SteamBooster.v0.3.0.exe"
pub fn extract_semver(s: &str) -> Option<(u32, u32, u32)> {
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_ascii_digit() {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            let sub: String = chars[start..i].iter().collect();
            let parts: Vec<&str> = sub.split('.').collect();
            if parts.len() >= 2 {
                if let (Ok(maj), Ok(min)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>()) {
                    let patch = if parts.len() >= 3 {
                        parts[2].parse::<u32>().unwrap_or(0)
                    } else {
                        0
                    };
                    return Some((maj, min, patch));
                }
            }
        } else {
            i += 1;
        }
    }
    None
}

/// Check if remote version is strictly greater than current version
pub fn is_newer(remote_ver: &str, current_ver: &str) -> bool {
    let remote_tuple = extract_semver(remote_ver);
    let current_tuple = extract_semver(current_ver);
    match (remote_tuple, current_tuple) {
        (Some(r), Some(c)) => r > c,
        (Some(_), None) => true,
        _ => remote_ver != current_ver && !remote_ver.is_empty(),
    }
}

/// Query GitHub Releases API for the latest published release
pub async fn fetch_latest_release(repo: &str) -> Result<ReleaseInfo, String> {
    let clean_repo = repo.trim().trim_start_matches("https://github.com/").trim_end_matches(".git");
    let url = format!("https://api.github.com/repos/{}/releases/latest", clean_repo);

    let client = reqwest::Client::builder()
        .user_agent("SteamBooster-App-Updater")
        .timeout(Duration::from_secs(12))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client.get(&url).send().await.map_err(|e| format!("Bağlantı hatası: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("GitHub API hatası (HTTP {})", resp.status()));
    }

    let json: serde_json::Value = resp.json().await.map_err(|e| format!("JSON ayrıştırma hatası: {}", e))?;

    let tag_name = json.get("tag_name").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let title = json.get("name").and_then(|v| v.as_str()).unwrap_or(&tag_name).to_string();
    let body = json.get("body").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let published_at = json.get("published_at").and_then(|v| v.as_str()).unwrap_or("").to_string();

    let assets = json.get("assets").and_then(|v| v.as_array());
    let mut exe_asset = None;

    if let Some(assets_arr) = assets {
        for a in assets_arr {
            let name = a.get("name").and_then(|v| v.as_str()).unwrap_or("");
            if name.to_lowercase().ends_with(".exe") {
                let download_url = a.get("browser_download_url").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let size = a.get("size").and_then(|v| v.as_u64()).unwrap_or(0);
                exe_asset = Some((name.to_string(), download_url, size));
                break;
            }
        }
    }

    let (asset_name, download_url, asset_size) = exe_asset.ok_or_else(|| {
        "Bu release içinde indirilebilir .exe dosyası bulunamadı.".to_string()
    })?;

    // Determine version
    let version = if let Some((maj, min, pat)) = extract_semver(&tag_name) {
        format!("{}.{}.{}", maj, min, pat)
    } else if let Some((maj, min, pat)) = extract_semver(&asset_name) {
        format!("{}.{}.{}", maj, min, pat)
    } else if let Some((maj, min, pat)) = extract_semver(&title) {
        format!("{}.{}.{}", maj, min, pat)
    } else {
        tag_name.clone()
    };

    Ok(ReleaseInfo {
        tag_name,
        title,
        version,
        body,
        published_at,
        asset_name,
        asset_size,
        download_url,
    })
}

/// Asynchronously check GitHub for new updates
pub fn check_for_updates(state_arc: Arc<Mutex<UpdateState>>, repo: String) {
    {
        let mut s = state_arc.lock().unwrap();
        *s = UpdateState::Checking;
    }

    crate::steam::get_runtime().spawn(async move {
        logger::log_info(&format!("🔍 Checking GitHub repository `{}` for updates...", repo));
        match fetch_latest_release(&repo).await {
            Ok(release) => {
                let current_version = env!("CARGO_PKG_VERSION");
                if is_newer(&release.version, current_version) {
                    logger::log_success(&format!(
                        "🚀 Yeni sürüm bulundu: v{} (Mevcut: v{}) — {}",
                        release.version, current_version, release.title
                    ));
                    let mut s = state_arc.lock().unwrap();
                    *s = UpdateState::UpdateAvailable {
                        version: release.version,
                        tag_name: release.tag_name,
                        release_title: release.title,
                        release_notes: release.body,
                        published_at: release.published_at,
                        download_url: release.download_url,
                        asset_name: release.asset_name,
                        asset_size: release.asset_size,
                    };
                } else {
                    let now = chrono::Local::now().format("%H:%M:%S").to_string();
                    logger::log_info(&format!("✅ Uygulama güncel (v{}). Son kontrol: {}", current_version, now));
                    let mut s = state_arc.lock().unwrap();
                    *s = UpdateState::UpToDate { checked_at: now };
                }
            }
            Err(e) => {
                logger::log_warn(&format!("⚠️ Güncelleme denetimi tamamlanamadı: {}", e));
                let mut s = state_arc.lock().unwrap();
                *s = UpdateState::Error(e);
            }
        }
    });
}

/// Download new release binary and apply in-place replacement
pub fn start_download_and_update(
    download_url: String,
    expected_size: u64,
    state_arc: Arc<Mutex<UpdateState>>,
) {
    {
        let mut s = state_arc.lock().unwrap();
        *s = UpdateState::Downloading {
            progress: 0.0,
            downloaded_bytes: 0,
            total_bytes: expected_size,
        };
    }

    crate::steam::get_runtime().spawn(async move {
        logger::log_info(&format!("📥 Downloading new version from: {}", download_url));
        match download_binary(&download_url, expected_size, state_arc.clone()).await {
            Ok(temp_path) => {
                logger::log_success("✅ Yeni sürüm indirildi. Uygulanıyor ve yeniden başlatılıyor...");
                {
                    let mut s = state_arc.lock().unwrap();
                    *s = UpdateState::ReadyToRestart;
                }
                // Small sleep to ensure UI draws "Yeniden başlatılıyor..."
                tokio::time::sleep(Duration::from_millis(500)).await;

                if let Err(e) = apply_update_and_restart(&temp_path) {
                    logger::log_error(&format!("Güncelleme uygulanamadı: {}", e));
                    let mut s = state_arc.lock().unwrap();
                    *s = UpdateState::Error(format!("Güncelleme uygulanamadı: {}", e));
                }
            }
            Err(e) => {
                logger::log_error(&format!("İndirme hatası: {}", e));
                let mut s = state_arc.lock().unwrap();
                *s = UpdateState::Error(format!("İndirme hatası: {}", e));
            }
        }
    });
}

async fn download_binary(
    download_url: &str,
    expected_size: u64,
    state_arc: Arc<Mutex<UpdateState>>,
) -> Result<std::path::PathBuf, String> {
    use tokio::io::AsyncWriteExt;

    let client = reqwest::Client::builder()
        .user_agent("SteamBooster-App-Updater")
        .timeout(Duration::from_secs(180))
        .build()
        .map_err(|e| e.to_string())?;

    let mut resp = client.get(download_url).send().await.map_err(|e| format!("İndirme başlatılamadı: {}", e))?;
    let total_size = resp.content_length().unwrap_or(expected_size);

    let current_exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let temp_exe_path = current_exe.with_extension("new.exe");

    if temp_exe_path.exists() {
        let _ = tokio::fs::remove_file(&temp_exe_path).await;
    }

    let mut file = tokio::fs::File::create(&temp_exe_path).await
        .map_err(|e| format!("Hedef dosya açılamadı: {}", e))?;

    let mut downloaded: u64 = 0;
    while let Some(chunk) = resp.chunk().await.map_err(|e| format!("Veri transfer hatası: {}", e))? {
        file.write_all(&chunk).await.map_err(|e| format!("Diske yazma hatası: {}", e))?;
        downloaded += chunk.len() as u64;
        let progress = if total_size > 0 {
            (downloaded as f32 / total_size as f32).clamp(0.0, 1.0)
        } else {
            0.5
        };

        if let Ok(mut s) = state_arc.lock() {
            *s = UpdateState::Downloading {
                progress,
                downloaded_bytes: downloaded,
                total_bytes: total_size,
            };
        }
    }

    file.flush().await.map_err(|e| format!("Dosya kaydedilemedi: {}", e))?;
    Ok(temp_exe_path)
}

/// Seamless in-place binary swap for Windows
pub fn apply_update_and_restart(temp_path: &std::path::Path) -> Result<(), String> {
    let current_exe = std::env::current_exe().map_err(|e| format!("Çalışan exe yolu bulunamadı: {}", e))?;
    let old_backup = current_exe.with_extension("old");

    // 1. Remove any previous .old file
    if old_backup.exists() {
        let _ = std::fs::remove_file(&old_backup);
    }

    // 2. Windows allows renaming a running executable!
    std::fs::rename(&current_exe, &old_backup)
        .map_err(|e| format!("Mevcut uygulama yedeklenemedi (.old): {}", e))?;

    // 3. Move newly downloaded executable to current_exe
    if let Err(e) = std::fs::rename(temp_path, &current_exe) {
        // Fallback to copy if rename across volumes fails
        if let Err(copy_err) = std::fs::copy(temp_path, &current_exe) {
            // Rollback on failure
            let _ = std::fs::rename(&old_backup, &current_exe);
            return Err(format!("Yeni sürüm uygulanamadı: {} (Taşıma: {})", copy_err, e));
        }
        let _ = std::fs::remove_file(temp_path);
    }

    logger::log_success("🚀 Uygulama güncellendi! Yeni sürüm başlatılıyor...");

    // 4. Spawn new process
    let mut cmd = std::process::Command::new(&current_exe);
    let args: Vec<String> = std::env::args().skip(1).collect();
    cmd.args(&args);
    cmd.spawn().map_err(|e| format!("Yeni sürüm çalıştırılamadı: {}", e))?;

    // 5. Exit current process
    std::process::exit(0);
}
