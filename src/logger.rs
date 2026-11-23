use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};
use chrono::Local;

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
pub enum LogLevel {
    Info,
    Success,
    Warn,
    Error,
}

impl LogLevel {
    #[allow(dead_code)]
    pub fn badge(self) -> &'static str {
        match self {
            LogLevel::Info => "INFO",
            LogLevel::Success => "SUCCESS",
            LogLevel::Warn => "WARN",
            LogLevel::Error => "ERROR",
        }
    }
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: LogLevel,
    pub message: String,
}

static LOG_BUFFER: OnceLock<Mutex<VecDeque<LogEntry>>> = OnceLock::new();

fn get_buffer() -> &'static Mutex<VecDeque<LogEntry>> {
    LOG_BUFFER.get_or_init(|| Mutex::new(VecDeque::with_capacity(600)))
}

pub fn log(level: LogLevel, message: &str) {
    let now = Local::now().format("%H:%M:%S").to_string();
    let entry = LogEntry {
        timestamp: now,
        level,
        message: message.to_string(),
    };
    let mut buf = get_buffer().lock().unwrap();
    if buf.len() >= 500 {
        buf.pop_front();
    }
    buf.push_back(entry);
}

pub fn log_info(message: &str) {
    log(LogLevel::Info, message);
}

pub fn log_success(message: &str) {
    log(LogLevel::Success, message);
}

pub fn log_warn(message: &str) {
    log(LogLevel::Warn, message);
}

pub fn log_error(message: &str) {
    log(LogLevel::Error, message);
}

pub fn get_logs() -> Vec<LogEntry> {
    let buf = get_buffer().lock().unwrap();
    buf.iter().cloned().collect()
}

pub fn clear_logs() {
    let mut buf = get_buffer().lock().unwrap();
    buf.clear();
}
