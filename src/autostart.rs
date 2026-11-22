use std::path::PathBuf;
use winreg::enums::{HKEY_CURRENT_USER, KEY_READ};
use winreg::RegKey;

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const APP_NAME: &str = "SteamBooster";
const TASK_NAME: &str = "SteamBoosterBackground";

pub fn get_exe_path() -> PathBuf {
    std::env::current_exe().unwrap_or_else(|_| PathBuf::from("SteamBooster.exe"))
}

pub fn is_registry_autostart_enabled() -> bool {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(key) = hkcu.open_subkey_with_flags(RUN_KEY, KEY_READ) {
        let val: Result<String, _> = key.get_value(APP_NAME);
        val.is_ok()
    } else {
        false
    }
}

pub fn set_registry_autostart(enable: bool, start_minimized: bool) -> Result<(), String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu.create_subkey(RUN_KEY).map_err(|e| e.to_string())?;

    if enable {
        let exe = get_exe_path();
        let cmd = if start_minimized {
            format!("\"{}\" --minimized", exe.display())
        } else {
            format!("\"{}\"", exe.display())
        };
        key.set_value(APP_NAME, &cmd).map_err(|e| e.to_string())?;
    } else {
        let _ = key.delete_value(APP_NAME);
    }
    Ok(())
}

pub fn is_task_scheduler_enabled() -> bool {
    let output = std::process::Command::new("schtasks")
        .args(["/Query", "/TN", TASK_NAME])
        .output();
    if let Ok(out) = output {
        out.status.success()
    } else {
        false
    }
}

pub fn register_task_scheduler(start_minimized: bool) -> Result<(), String> {
    let exe = get_exe_path();
    let tr_arg = if start_minimized {
        format!("\"{}\" --minimized", exe.display())
    } else {
        format!("\"{}\"", exe.display())
    };

    let output = std::process::Command::new("schtasks")
        .args([
            "/Create",
            "/TN", TASK_NAME,
            "/TR", &tr_arg,
            "/SC", "ONLOGON",
            "/RL", "HIGHEST",
            "/F",
        ])
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

pub fn unregister_task_scheduler() -> Result<(), String> {
    let output = std::process::Command::new("schtasks")
        .args(["/Delete", "/TN", TASK_NAME, "/F"])
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}
