use eframe::egui::{self, Color32, FontId, Pos2, Rect, Response, RichText, Rounding, Sense, Ui, Vec2};
use crate::data::CustomThemeColors;
use crate::i18n::Strings;
use crate::updater::{UpdateManager, UpdateState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavTab {
    Dashboard,
    ActiveDrops,
    Inventory,
    Accounts,
    Telegram,
    Settings,
}

pub struct Sidebar;

impl Sidebar {
    pub fn draw(
        ui: &mut Ui,
        current_tab: &mut NavTab,
        colors: &CustomThemeColors,
        strings: &Strings,
        updater: &UpdateManager,
        show_changelogs: &mut bool,
    ) {
        let accent = Color32::from_rgb(colors.accent[0], colors.accent[1], colors.accent[2]);
        let text_color = Color32::from_rgb(colors.text[0], colors.text[1], colors.text[2]);

        ui.vertical(|ui| {
            ui.add_space(8.0);

            // Tab items
            let tabs = [
                (NavTab::Dashboard, strings.tab_dashboard, "📊"),
                (NavTab::ActiveDrops, strings.tab_active_drops, "🃏"),
                (NavTab::Inventory, strings.tab_inventory, "🎒"),
                (NavTab::Accounts, strings.tab_accounts, "👤"),
                (NavTab::Telegram, strings.tab_telegram, "🤖"),
                (NavTab::Settings, strings.tab_settings, "⚙️"),
            ];

            for (tab, label, icon) in tabs {
                let is_active = *current_tab == tab;
                if draw_nav_button(ui, icon, label, is_active, accent, text_color).clicked() {
                    *current_tab = tab;
                }
                ui.add_space(3.0);
            }

            ui.add_space(16.0);

            // Push the rest of the content to the bottom
            ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                ui.add_space(10.0);

                // Small, non-intrusive Changelogs indicator at the very bottom
                let update_state = updater.state.lock().unwrap().clone();
                let has_update = matches!(update_state, UpdateState::UpdateAvailable { .. });

                ui.horizontal(|ui| {
                    ui.add_space(12.0);

                    // Indicator badge/dot
                    if has_update {
                        let (rect, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
                        ui.painter().circle_filled(rect.center(), 4.0, Color32::from_rgb(255, 175, 50));
                        ui.add_space(4.0);
                    } else {
                        let (rect, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
                        ui.painter().circle_filled(rect.center(), 3.5, Color32::from_rgb(70, 85, 105));
                        ui.add_space(4.0);
                    }

                    // Clickable changelogs link
                    let changelog_btn = ui.link(
                        RichText::new(strings.changelogs_text)
                            .font(FontId::proportional(11.5))
                            .color(if has_update {
                                Color32::from_rgb(255, 190, 70)
                            } else {
                                Color32::from_rgb(120, 140, 165)
                            }),
                    );

                    if changelog_btn.clicked() {
                        *show_changelogs = true;
                    }
                });

                ui.separator();
            });
        });
    }
}

fn draw_nav_button(
    ui: &mut Ui,
    icon: &str,
    label: &str,
    is_active: bool,
    accent: Color32,
    text_color: Color32,
) -> Response {
    let desired_size = Vec2::new(ui.available_width() - 4.0, 38.0);
    let (rect, response) = ui.allocate_exact_size(desired_size, Sense::click());

    if ui.is_rect_visible(rect) {
        let is_hovered = response.hovered();

        // Background fill
        if is_active {
            ui.painter().rect_filled(
                rect,
                Rounding::same(7.0),
                Color32::from_rgba_premultiplied(accent.r(), accent.g(), accent.b(), 45),
            );
            // Left border accent line
            let indicator_rect = Rect::from_min_size(
                Pos2::new(rect.min.x + 1.5, rect.min.y + 6.0),
                Vec2::new(3.5, rect.height() - 12.0),
            );
            ui.painter().rect_filled(indicator_rect, Rounding::same(2.0), accent);
        } else if is_hovered {
            ui.painter().rect_filled(
                rect,
                Rounding::same(7.0),
                Color32::from_rgba_premultiplied(255, 255, 255, 15),
            );
        }

        // Icon + Label (ensuring exactly one icon is rendered)
        let text_pos = Pos2::new(rect.min.x + 16.0, rect.center().y - 8.0);
        let display_color = if is_active {
            accent
        } else if is_hovered {
            Color32::WHITE
        } else {
            text_color
        };

        let clean_label = label.trim_start_matches(|c: char| !c.is_alphabetic()).trim();
        let display_text = if !icon.is_empty() {
            format!("{}  {}", icon, clean_label)
        } else {
            label.to_string()
        };

        ui.painter().text(
            text_pos,
            egui::Align2::LEFT_TOP,
            display_text,
            FontId::proportional(13.5),
            display_color,
        );
    }

    response
}
