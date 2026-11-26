use eframe::egui::{Color32, Context, Frame, Margin, Rounding, Stroke, Visuals};
use crate::data::{CustomThemeColors, ThemeMode};

/// Manages themes and color schemes including Transparent Acrylic & Liquid Glass refraction
pub struct ThemeEngine;

impl ThemeEngine {
    pub fn apply_theme(ctx: &Context, mode: ThemeMode, colors: &CustomThemeColors) {
        let mut visuals = match mode {
            ThemeMode::Light => Visuals::light(),
            ThemeMode::Dark | ThemeMode::System => Visuals::dark(),
            ThemeMode::Transparent => {
                let mut v = Visuals::dark();
                let a = (colors.glass_alpha as f32 * 0.32).clamp(32.0, 130.0) as u8;
                v.panel_fill = Color32::from_rgba_unmultiplied(colors.surface[0], colors.surface[1], colors.surface[2], a);
                v.window_fill = Color32::from_rgba_unmultiplied(colors.surface[0], colors.surface[1], colors.surface[2], 150);
                v.extreme_bg_color = Color32::from_rgba_unmultiplied(8, 10, 16, 110);
                v
            }
            ThemeMode::LiquidGlass => {
                let mut v = Visuals::dark();
                let a = (colors.glass_alpha as f32 * 0.14).clamp(18.0, 68.0) as u8;
                v.panel_fill = Color32::from_rgba_unmultiplied(colors.surface[0], colors.surface[1], colors.surface[2], a);
                v.window_fill = Color32::from_rgba_unmultiplied(colors.surface[0], colors.surface[1], colors.surface[2], 105);
                v.extreme_bg_color = Color32::from_rgba_unmultiplied(8, 10, 16, 75);
                v
            }
        };

        let accent_c32 = Color32::from_rgb(colors.accent[0], colors.accent[1], colors.accent[2]);
        let surface_c32 = Color32::from_rgb(colors.surface[0], colors.surface[1], colors.surface[2]);

        if mode == ThemeMode::Light {
            // Always force dark/black text on Light theme so nothing ever becomes invisible
            let black = Color32::from_rgb(16, 20, 26);
            visuals.override_text_color = Some(black);
            visuals.widgets.noninteractive.fg_stroke.color = black;
            visuals.widgets.inactive.fg_stroke.color = black;
            visuals.widgets.hovered.fg_stroke.color = Color32::BLACK;
            visuals.widgets.active.fg_stroke.color = Color32::BLACK;
            visuals.widgets.hovered.bg_fill = Color32::from_rgb(214, 220, 230);
            visuals.widgets.inactive.bg_fill = Color32::from_rgb(230, 235, 242);
            visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(242, 245, 249);

            let lum = (colors.surface[0] as u32 * 299 + colors.surface[1] as u32 * 587 + colors.surface[2] as u32 * 114) / 1000;
            if lum < 160 {
                visuals.panel_fill = Color32::from_rgb(244, 246, 250);
                visuals.window_fill = Color32::from_rgb(252, 253, 255);
            } else {
                visuals.panel_fill = surface_c32;
                visuals.window_fill = Color32::from_rgb(
                    surface_c32.r().saturating_add(6),
                    surface_c32.g().saturating_add(6),
                    surface_c32.b().saturating_add(6),
                );
            }
        } else {
            let lum = (colors.text[0] as u32 * 299 + colors.text[1] as u32 * 587 + colors.text[2] as u32 * 114) / 1000;
            let text_c32 = if lum < 60 {
                Color32::from_rgb(220, 228, 238)
            } else {
                Color32::from_rgb(colors.text[0], colors.text[1], colors.text[2])
            };
            visuals.override_text_color = Some(text_c32);
            if mode == ThemeMode::Dark || mode == ThemeMode::System {
                visuals.panel_fill = surface_c32;
                visuals.window_fill = Color32::from_rgb(
                    surface_c32.r().saturating_add(6),
                    surface_c32.g().saturating_add(8),
                    surface_c32.b().saturating_add(12),
                );
            }
        }

        visuals.hyperlink_color = accent_c32;
        visuals.selection.bg_fill = accent_c32;
        visuals.selection.stroke = Stroke::new(1.0_f32, accent_c32);

        let r = if mode == ThemeMode::LiquidGlass { 10.0 } else { 6.0 };
        visuals.widgets.noninteractive.rounding = Rounding::same(r);
        visuals.widgets.inactive.rounding = Rounding::same(r);
        visuals.widgets.hovered.rounding = Rounding::same(r);
        visuals.widgets.active.rounding = Rounding::same(r);
        visuals.window_rounding = Rounding::same(12.0);
        visuals.menu_rounding = Rounding::same(10.0);

        ctx.set_visuals(visuals);
    }

    /// Panel frame for Sidebar, TopBar, StatusBar, and CentralPanel
    pub fn panel_frame(mode: ThemeMode, colors: &CustomThemeColors, is_sidebar: bool) -> Frame {
        let (fill, stroke) = match mode {
            ThemeMode::LiquidGlass => {
                let alpha = if is_sidebar {
                    (colors.glass_alpha as f32 * 0.16).clamp(22.0, 72.0) as u8
                } else {
                    (colors.glass_alpha as f32 * 0.10).clamp(18.0, 54.0) as u8
                };
                let edge_stroke = if is_sidebar {
                    Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(235, 245, 255, 85))
                } else {
                    Stroke::NONE
                };
                (
                    Color32::from_rgba_unmultiplied(colors.surface[0], colors.surface[1], colors.surface[2], alpha),
                    edge_stroke,
                )
            }
            ThemeMode::Transparent => {
                let alpha = if is_sidebar {
                    (colors.glass_alpha as f32 * 0.30).clamp(24.0, 120.0) as u8
                } else {
                    (colors.glass_alpha as f32 * 0.18).clamp(14.0, 90.0) as u8
                };
                (
                    Color32::from_rgba_unmultiplied(colors.surface[0], colors.surface[1], colors.surface[2], alpha),
                    Stroke::NONE,
                )
            }
            ThemeMode::Light => (
                Color32::from_rgb(244, 246, 250),
                Stroke::new(1.0_f32, Color32::from_rgb(220, 226, 235)),
            ),
            _ => {
                let s = if is_sidebar {
                    Color32::from_rgb(
                        colors.surface[0],
                        colors.surface[1],
                        colors.surface[2].saturating_add(4),
                    )
                } else {
                    Color32::from_rgb(
                        colors.surface[0].saturating_add(4),
                        colors.surface[1].saturating_add(5),
                        colors.surface[2].saturating_add(9),
                    )
                };
                (s, Stroke::NONE)
            }
        };

        Frame {
            fill,
            stroke,
            inner_margin: Margin::same(6.0),
            ..Default::default()
        }
    }

    /// Card frame with specular Liquid Glass edge refraction or crisp solid/acrylic backing
    pub fn card_frame(mode: ThemeMode, colors: &CustomThemeColors) -> Frame {
        let (fill, stroke, rounding) = match mode {
            ThemeMode::LiquidGlass => {
                let a = (colors.glass_alpha as f32 * 0.18).clamp(20.0, 76.0) as u8;
                (
                    Color32::from_rgba_unmultiplied(colors.surface[0], colors.surface[1], colors.surface[2], a),
                    // Crisp specular edge refraction border (bright frosted glass rim)
                    Stroke::new(1.35_f32, Color32::from_rgba_unmultiplied(235, 245, 255, 130)),
                    Rounding::same(14.0),
                )
            }
            ThemeMode::Transparent => {
                let a = (colors.glass_alpha as f32 * 0.34).clamp(28.0, 125.0) as u8;
                (
                    Color32::from_rgba_unmultiplied(colors.surface[0], colors.surface[1], colors.surface[2], a),
                    Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 48)),
                    Rounding::same(10.0),
                )
            }
            ThemeMode::Light => (
                Color32::from_rgb(252, 253, 255),
                Stroke::new(1.0_f32, Color32::from_rgb(212, 220, 232)),
                Rounding::same(8.0),
            ),
            _ => (
                Color32::from_rgb(
                    colors.surface[0].saturating_add(8),
                    colors.surface[1].saturating_add(10),
                    colors.surface[2].saturating_add(16),
                ),
                Stroke::new(1.0_f32, Color32::from_rgb(42, 56, 75)),
                Rounding::same(8.0),
            ),
        };

        Frame {
            inner_margin: Margin::same(12.0),
            outer_margin: Margin::symmetric(0.0, 4.0),
            rounding,
            fill,
            stroke,
            ..Default::default()
        }
    }

    /// Interactive card frame with accent highlight when active/hovered
    #[allow(dead_code)]
    pub fn active_card_frame(colors: &CustomThemeColors) -> Frame {
        let accent = Color32::from_rgb(colors.accent[0], colors.accent[1], colors.accent[2]);
        Frame {
            inner_margin: Margin::same(12.0),
            outer_margin: Margin::symmetric(0.0, 4.0),
            rounding: Rounding::same(10.0),
            fill: Color32::from_rgba_unmultiplied(colors.surface[0], colors.surface[1], colors.surface[2], 140),
            stroke: Stroke::new(1.5_f32, accent),
            ..Default::default()
        }
    }

    /// Floating bottom bar frame for inventory bulk selling
    pub fn floating_bar_frame(mode: ThemeMode, colors: &CustomThemeColors) -> Frame {
        let (fill, stroke_color) = match mode {
            ThemeMode::LiquidGlass => (
                Color32::from_rgba_unmultiplied(colors.surface[0], colors.surface[1], colors.surface[2], 65),
                Color32::from_rgba_unmultiplied(235, 245, 255, 130),
            ),
            ThemeMode::Transparent => (
                Color32::from_rgba_unmultiplied(
                    colors.surface[0].saturating_add(5),
                    colors.surface[1].saturating_add(5),
                    colors.surface[2].saturating_add(8),
                    155,
                ),
                Color32::from_rgb(colors.accent[0], colors.accent[1], colors.accent[2]),
            ),
            ThemeMode::Light => (
                Color32::from_rgb(238, 242, 248),
                Color32::from_rgb(colors.accent[0], colors.accent[1], colors.accent[2]),
            ),
            _ => (
                Color32::from_rgb(28, 36, 48),
                Color32::from_rgb(colors.accent[0], colors.accent[1], colors.accent[2]),
            ),
        };

        Frame {
            inner_margin: Margin::symmetric(16.0, 10.0),
            outer_margin: Margin::same(4.0),
            rounding: Rounding::same(12.0),
            fill,
            stroke: Stroke::new(1.5_f32, stroke_color),
            shadow: Default::default(),
        }
    }
}

/// Applies Acrylic / Mica via window-vibrancy when running on Windows (avoids broken Win11 apply_blur black screen)
#[cfg(target_os = "windows")]
pub fn apply_platform_vibrancy(window: &impl raw_window_handle::HasWindowHandle, mode: ThemeMode, colors: &CustomThemeColors) {
    let _ = window_vibrancy::clear_blur(window);
    let _ = window_vibrancy::clear_mica(window);

    match mode {
        ThemeMode::LiquidGlass => {
            // Smooth, lighter frosted Acrylic blur (Windows 10/11 safe — never goes solid black or flickers on scroll)
            let a = (colors.glass_alpha as f32 * 0.18).clamp(24.0, 72.0) as u8;
            let color = Some((colors.surface[0], colors.surface[1], colors.surface[2], a));
            if window_vibrancy::apply_acrylic(window, color).is_err() {
                let _ = window_vibrancy::apply_mica(window, Some(true));
            }
        }
        ThemeMode::Transparent => {
            let a = (colors.glass_alpha as f32 * 0.42).clamp(45.0, 145.0) as u8;
            let color = Some((colors.surface[0], colors.surface[1], colors.surface[2], a));
            if window_vibrancy::apply_acrylic(window, color).is_err() {
                let _ = window_vibrancy::apply_mica(window, Some(true));
            }
        }
        _ => {
            let _ = window_vibrancy::clear_acrylic(window);
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub fn apply_platform_vibrancy(_window: &impl raw_window_handle::HasWindowHandle, _mode: ThemeMode, _colors: &CustomThemeColors) {}
