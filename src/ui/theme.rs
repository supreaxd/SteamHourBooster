use eframe::egui::{Color32, Context, Frame, Margin, Rounding, Stroke, Visuals};
use crate::data::{CustomThemeColors, ThemeMode};

/// Manages themes and color schemes including Transparent Blur (Acrylic/Mica)
pub struct ThemeEngine;

impl ThemeEngine {
    /// Applies the selected theme mode and custom colors to the egui context
    pub fn apply_theme(ctx: &Context, mode: ThemeMode, colors: &CustomThemeColors) {
        let mut visuals = match mode {
            ThemeMode::Light => Visuals::light(),
            ThemeMode::Dark => Visuals::dark(),
            ThemeMode::System => {
                // Default to dark for standard gaming aesthetics
                Visuals::dark()
            }
            ThemeMode::Transparent => {
                let mut v = Visuals::dark();
                // Semi-transparent window background
                v.panel_fill = Color32::from_rgba_unmultiplied(
                    colors.surface[0],
                    colors.surface[1],
                    colors.surface[2],
                    colors.glass_alpha,
                );
                v.window_fill = Color32::from_rgba_unmultiplied(
                    colors.surface[0],
                    colors.surface[1],
                    colors.surface[2],
                    colors.glass_alpha,
                );
                v
            }
        };

        let accent_c32 = Color32::from_rgb(colors.accent[0], colors.accent[1], colors.accent[2]);
        let surface_c32 = Color32::from_rgb(colors.surface[0], colors.surface[1], colors.surface[2]);

        // Contrast-safe text color: in Light theme, default to dark/black text so it is always readable
        let text_c32 = if mode == ThemeMode::Light {
            let lum = (colors.text[0] as u32 * 299 + colors.text[1] as u32 * 587 + colors.text[2] as u32 * 114) / 1000;
            if lum > 140 {
                Color32::from_rgb(25, 30, 36)
            } else {
                Color32::from_rgb(colors.text[0], colors.text[1], colors.text[2])
            }
        } else {
            let lum = (colors.text[0] as u32 * 299 + colors.text[1] as u32 * 587 + colors.text[2] as u32 * 114) / 1000;
            if lum < 60 {
                Color32::from_rgb(215, 225, 235)
            } else {
                Color32::from_rgb(colors.text[0], colors.text[1], colors.text[2])
            }
        };

        if mode == ThemeMode::Light {
            let lum = (colors.surface[0] as u32 * 299 + colors.surface[1] as u32 * 587 + colors.surface[2] as u32 * 114) / 1000;
            if lum < 140 {
                visuals.panel_fill = Color32::from_rgb(245, 247, 250);
                visuals.window_fill = Color32::from_rgb(255, 255, 255);
            } else {
                visuals.panel_fill = surface_c32;
                visuals.window_fill = Color32::from_rgb(
                    surface_c32.r().saturating_add(6),
                    surface_c32.g().saturating_add(6),
                    surface_c32.b().saturating_add(6),
                );
            }
        } else if mode != ThemeMode::Transparent {
            visuals.panel_fill = surface_c32;
            visuals.window_fill = Color32::from_rgb(
                surface_c32.r().saturating_add(6),
                surface_c32.g().saturating_add(8),
                surface_c32.b().saturating_add(12),
            );
        }

        visuals.override_text_color = Some(text_c32);
        visuals.hyperlink_color = accent_c32;
        visuals.selection.bg_fill = accent_c32;
        visuals.selection.stroke = Stroke::new(1.0_f32, accent_c32);

        // Rounded widgets
        visuals.widgets.noninteractive.rounding = Rounding::same(6.0);
        visuals.widgets.inactive.rounding = Rounding::same(6.0);
        visuals.widgets.hovered.rounding = Rounding::same(6.0);
        visuals.widgets.active.rounding = Rounding::same(6.0);
        visuals.window_rounding = Rounding::same(10.0);
        visuals.menu_rounding = Rounding::same(8.0);

        ctx.set_visuals(visuals);
    }

    /// Card frame with high contrast border and subtle glass backing
    pub fn card_frame(mode: ThemeMode, colors: &CustomThemeColors) -> Frame {
        let (fill, stroke_color) = match mode {
            ThemeMode::Transparent => (
                Color32::from_rgba_unmultiplied(
                    colors.surface[0],
                    colors.surface[1],
                    colors.surface[2],
                    colors.glass_alpha.saturating_add(20),
                ),
                Color32::from_rgba_unmultiplied(255, 255, 255, 38), // 1.0px glass highlight
            ),
            ThemeMode::Light => (
                Color32::from_rgb(245, 247, 250),
                Color32::from_rgb(215, 222, 230),
            ),
            _ => (
                Color32::from_rgb(
                    colors.surface[0].saturating_add(8),
                    colors.surface[1].saturating_add(10),
                    colors.surface[2].saturating_add(16),
                ),
                Color32::from_rgb(42, 56, 75),
            ),
        };

        Frame {
            inner_margin: Margin::same(12.0),
            outer_margin: Margin::symmetric(0.0, 4.0),
            rounding: Rounding::same(8.0),
            fill,
            stroke: Stroke::new(1.0, stroke_color),
            ..Default::default()
        }
    }

    /// Interactive card frame with accent highlight when active/hovered
    pub fn active_card_frame(colors: &CustomThemeColors) -> Frame {
        let accent = Color32::from_rgb(colors.accent[0], colors.accent[1], colors.accent[2]);
        Frame {
            inner_margin: Margin::same(12.0),
            outer_margin: Margin::symmetric(0.0, 4.0),
            rounding: Rounding::same(8.0),
            fill: Color32::from_rgba_unmultiplied(colors.surface[0], colors.surface[1], colors.surface[2], 220),
            stroke: Stroke::new(1.5, accent),
            ..Default::default()
        }
    }

    /// Floating bottom bar frame for inventory bulk selling
    pub fn floating_bar_frame(mode: ThemeMode, colors: &CustomThemeColors) -> Frame {
        let (fill, stroke_color) = match mode {
            ThemeMode::Transparent => (
                Color32::from_rgba_unmultiplied(
                    colors.surface[0].saturating_add(5),
                    colors.surface[1].saturating_add(5),
                    colors.surface[2].saturating_add(8),
                    235,
                ),
                Color32::from_rgb(colors.accent[0], colors.accent[1], colors.accent[2]),
            ),
            ThemeMode::Light => (
                Color32::from_rgb(238, 242, 246),
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
            rounding: Rounding::same(10.0),
            fill,
            stroke: Stroke::new(1.5, stroke_color),
            shadow: Default::default(),
        }
    }
}

/// Applies Acrylic / Mica / Blur via window-vibrancy when running on Windows
#[cfg(target_os = "windows")]
pub fn apply_platform_vibrancy(window: &impl raw_window_handle::HasWindowHandle, mode: ThemeMode, colors: &CustomThemeColors) {
    match mode {
        ThemeMode::Transparent => {
            // First attempt Acrylic effect
            let color = Some((colors.surface[0], colors.surface[1], colors.surface[2], colors.glass_alpha));
            if window_vibrancy::apply_acrylic(window, color).is_err() {
                // If Acrylic fails (e.g. Windows 11 build variation), attempt Mica
                if window_vibrancy::apply_mica(window, Some(true)).is_err() {
                    // Fallback to Blur
                    let _ = window_vibrancy::apply_blur(window, color);
                }
            }
        }
        _ => {
            // Clear vibrancy for solid dark/light themes
            let _ = window_vibrancy::clear_acrylic(window);
            let _ = window_vibrancy::clear_mica(window);
            let _ = window_vibrancy::clear_blur(window);
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub fn apply_platform_vibrancy(_window: &impl raw_window_handle::HasWindowHandle, _mode: ThemeMode, _colors: &CustomThemeColors) {}
