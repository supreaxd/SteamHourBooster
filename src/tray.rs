use std::sync::{Arc, Mutex};
use tray_icon::{
    menu::{Menu, MenuItem, MenuEvent},
    Icon, TrayIcon, TrayIconBuilder, TrayIconEvent, MouseButton, MouseButtonState,
};
use crate::data::{Account, AccountStatus};

const TRAY_ICON_RGBA: &[u8] = include_bytes!("../assets/icon_32.rgba");

pub enum TrayAction {
    OpenWindow,
    StartAll,
    StopAll,
    Quit,
}

pub struct TrayManager {
    _tray_icon: TrayIcon,
    item_open: MenuItem,
    item_start_all: MenuItem,
    item_stop_all: MenuItem,
    item_quit: MenuItem,
    last_tooltip: String,
}

impl TrayManager {
    pub fn new() -> Option<Self> {
        let icon = Icon::from_rgba(TRAY_ICON_RGBA.to_vec(), 32, 32).ok()?;

        let tray_menu = Menu::new();
        let item_open = MenuItem::new("⚡ Open SteamBooster", true, None);
        let item_start_all = MenuItem::new("▶️ Start All Boosting", true, None);
        let item_stop_all = MenuItem::new("⏹️ Stop All Boosting", true, None);
        let item_quit = MenuItem::new("❌ Exit", true, None);

        let _ = tray_menu.append(&item_open);
        let _ = tray_menu.append(&item_start_all);
        let _ = tray_menu.append(&item_stop_all);
        let _ = tray_menu.append(&item_quit);

        let initial_tooltip = "⚡ SteamBooster v0.2.0 - Active".to_string();

        let tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(tray_menu))
            .with_tooltip(&initial_tooltip)
            .with_icon(icon)
            .build()
            .ok()?;

        Some(Self {
            _tray_icon: tray_icon,
            item_open,
            item_start_all,
            item_stop_all,
            item_quit,
            last_tooltip: initial_tooltip,
        })
    }

    pub fn update_status(&mut self, accounts: &[Arc<Mutex<Account>>]) {
        let mut boosting = Vec::new();
        let mut awaiting_guard = Vec::new();
        let mut idle_count = 0;

        for acc in accounts {
            if let Ok(a) = acc.lock() {
                match &a.status {
                    AccountStatus::Boosting => boosting.push(a.username.clone()),
                    AccountStatus::AwaitingSteamGuard { .. } => awaiting_guard.push(a.username.clone()),
                    _ => idle_count += 1,
                }
            }
        }

        let mut lines = Vec::new();
        lines.push("⚡ SteamBooster".to_string());

        if !boosting.is_empty() {
            let names = boosting.join(", ");
            lines.push(format!("🟢 Active ({}): {}", boosting.len(), names));
        } else {
            lines.push(format!("⚪ Idle ({} accounts)", idle_count));
        }

        if !awaiting_guard.is_empty() {
            lines.push(format!("🛡️ Guard: {}", awaiting_guard.join(", ")));
        }

        let mut tooltip = lines.join("\n");
        // Windows tooltips are most reliable within 120 chars
        if tooltip.chars().count() > 120 {
            tooltip = tooltip.chars().take(117).collect::<String>() + "...";
        }

        if tooltip != self.last_tooltip {
            let _ = self._tray_icon.set_tooltip(Some(&tooltip));
            self.last_tooltip = tooltip;
        }
    }

    pub fn poll_events(&self) -> Option<TrayAction> {
        // 1. Menu item clicks
        if let Ok(event) = MenuEvent::receiver().try_recv() {
            if event.id == self.item_open.id() {
                return Some(TrayAction::OpenWindow);
            } else if event.id == self.item_start_all.id() {
                return Some(TrayAction::StartAll);
            } else if event.id == self.item_stop_all.id() {
                return Some(TrayAction::StopAll);
            } else if event.id == self.item_quit.id() {
                return Some(TrayAction::Quit);
            }
        }

        // 2. Tray icon left-click or double-click -> restore window
        if let Ok(event) = TrayIconEvent::receiver().try_recv() {
            match event {
                TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. }
                | TrayIconEvent::DoubleClick { button: MouseButton::Left, .. } => {
                    return Some(TrayAction::OpenWindow);
                }
                _ => {}
            }
        }

        None
    }
}
