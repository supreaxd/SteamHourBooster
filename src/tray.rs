use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use eframe::egui;
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem},
    Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
};
use crate::data::{Account, AccountStatus};

const TRAY_ICON_RGBA: &[u8] = include_bytes!("../assets/icon_32.rgba");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayAction {
    OpenWindow,
    StartAll,
    StopAll,
    Quit,
}

pub struct TrayManager {
    _tray_icon: TrayIcon,
    pending_actions: Arc<Mutex<VecDeque<TrayAction>>>,
    last_tooltip: String,
}

impl TrayManager {
    pub fn new(ctx: &egui::Context) -> Option<Self> {
        let icon = Icon::from_rgba(TRAY_ICON_RGBA.to_vec(), 32, 32).ok()?;

        let tray_menu = Menu::new();
        let item_open = MenuItem::new("Open SupBooster", true, None);
        let item_start_all = MenuItem::new("Start All Boosting", true, None);
        let item_stop_all = MenuItem::new("Stop All Boosting", true, None);
        let item_quit = MenuItem::new("Exit", true, None);

        let id_open = item_open.id().clone();
        let id_start = item_start_all.id().clone();
        let id_stop = item_stop_all.id().clone();
        let id_quit = item_quit.id().clone();

        let _ = tray_menu.append(&item_open);
        let _ = tray_menu.append(&item_start_all);
        let _ = tray_menu.append(&item_stop_all);
        let _ = tray_menu.append(&item_quit);

        let initial_tooltip = "SupBooster - Active".to_string();

        let tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(tray_menu))
            .with_tooltip(&initial_tooltip)
            .with_icon(icon)
            .build()
            .ok()?;

        let pending_actions = Arc::new(Mutex::new(VecDeque::new()));

        // Event handler for right-click menu items (works even when window is hidden/minimized)
        let q_menu = pending_actions.clone();
        let ctx_menu = ctx.clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            let act = if event.id == id_open {
                Some(TrayAction::OpenWindow)
            } else if event.id == id_start {
                Some(TrayAction::StartAll)
            } else if event.id == id_stop {
                Some(TrayAction::StopAll)
            } else if event.id == id_quit {
                Some(TrayAction::Quit)
            } else {
                None
            };
            if let Some(a) = act {
                if a == TrayAction::Quit {
                    std::process::exit(0);
                }
                if a == TrayAction::OpenWindow {
                    ctx_menu.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx_menu.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                if let Ok(mut lock) = q_menu.lock() {
                    lock.push_back(a);
                }
                ctx_menu.request_repaint();
            }
        }));

        // Event handler for left-click / double-click on tray icon
        let q_icon = pending_actions.clone();
        let ctx_icon = ctx.clone();
        TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
            match event {
                TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. }
                | TrayIconEvent::DoubleClick { button: MouseButton::Left, .. } => {
                    ctx_icon.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx_icon.send_viewport_cmd(egui::ViewportCommand::Focus);
                    if let Ok(mut lock) = q_icon.lock() {
                        lock.push_back(TrayAction::OpenWindow);
                    }
                    ctx_icon.request_repaint();
                }
                _ => {}
            }
        }));

        Some(Self {
            _tray_icon: tray_icon,
            pending_actions,
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
        lines.push("SupBooster".to_string());

        if !boosting.is_empty() {
            let names = boosting.join(", ");
            lines.push(format!("Active ({}): {}", boosting.len(), names));
        } else {
            lines.push(format!("Idle ({} accounts)", idle_count));
        }

        if !awaiting_guard.is_empty() {
            lines.push(format!("Guard: {}", awaiting_guard.join(", ")));
        }

        let mut tooltip = lines.join("\n");
        if tooltip.chars().count() > 120 {
            tooltip = tooltip.chars().take(117).collect::<String>() + "...";
        }

        if tooltip != self.last_tooltip {
            let _ = self._tray_icon.set_tooltip(Some(&tooltip));
            self.last_tooltip = tooltip;
        }
    }

    pub fn poll_events(&self) -> Option<TrayAction> {
        if let Ok(mut lock) = self.pending_actions.lock() {
            lock.pop_front()
        } else {
            None
        }
    }
}
