use std::sync::{Mutex, OnceLock};
use chrono::Local;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacketDirection {
    Inbound,
    Outbound,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacketCategory {
    Chat,
    Presence,
    Session,
    License,
    Economy,
    System,
}

impl PacketCategory {
    pub fn label(&self) -> &'static str {
        match self {
            PacketCategory::Chat => "💬 Chat",
            PacketCategory::Presence => "🎮 Presence",
            PacketCategory::Session => "🔒 Session",
            PacketCategory::License => "🎁 License",
            PacketCategory::Economy => "🃏 Economy",
            PacketCategory::System => "⚙️ System",
        }
    }
}

#[derive(Debug, Clone)]
pub struct PacketLogEntry {
    pub timestamp: String,
    pub account: String,
    pub direction: PacketDirection,
    pub msg_kind: String,
    pub summary: String,
    pub category: PacketCategory,
}

static PACKET_LOGS: OnceLock<Mutex<Vec<PacketLogEntry>>> = OnceLock::new();
const MAX_PACKET_LOGS: usize = 500;

fn get_packet_logs_lock() -> &'static Mutex<Vec<PacketLogEntry>> {
    PACKET_LOGS.get_or_init(|| Mutex::new(Vec::with_capacity(MAX_PACKET_LOGS)))
}

pub fn log_packet(
    account: &str,
    direction: PacketDirection,
    msg_kind: &str,
    summary: &str,
    category: PacketCategory,
) {
    let now = Local::now().format("%H:%M:%S%.3f").to_string();
    let entry = PacketLogEntry {
        timestamp: now,
        account: account.to_string(),
        direction,
        msg_kind: msg_kind.to_string(),
        summary: summary.to_string(),
        category,
    };

    let lock = get_packet_logs_lock();
    if let Ok(mut logs) = lock.lock() {
        if logs.len() >= MAX_PACKET_LOGS {
            logs.remove(0);
        }
        logs.push(entry);
    }
}

pub fn get_packet_logs() -> Vec<PacketLogEntry> {
    let lock = get_packet_logs_lock();
    if let Ok(logs) = lock.lock() {
        logs.clone()
    } else {
        Vec::new()
    }
}

pub fn clear_packet_logs() {
    let lock = get_packet_logs_lock();
    if let Ok(mut logs) = lock.lock() {
        logs.clear();
    }
}
