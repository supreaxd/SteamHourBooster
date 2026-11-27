use eframe::egui::{Color32, Context, Frame, Margin, Rounding, Stroke, Visuals};
use crate::data::{CustomThemeColors, ThemeMode};

/// Manages themes and color schemes including Transparent Acrylic & Liquid Glass refraction
pub struct ThemeEngine;

impl ThemeEngine {
    pub fn apply_theme(ctx: &Context, mode: ThemeMode, colors: &CustomThemeColors) {
        let mut visuals = match mode {
            ThemeMode::Light => Visuals::light(),
            ThemeMode::Dark | ThemeMode::System => Visuals::dark(),
            ThemeMode::Blur => {
                let mut v = Visuals::dark();
                let blur = colors.blur_level.clamp(0.0, 10.0);
                let g = colors.glass_alpha as f32;
                let a = (g * 0.25 + blur * 11.0).clamp(if blur <= 0.05 { 1.0 } else { 8.0 }, 210.0) as u8;
                v.panel_fill = Color32::from_rgba_unmultiplied(colors.surface[0], colors.surface[1], colors.surface[2], a);
                v.window_fill = Color32::from_rgba_unmultiplied(colors.surface[0], colors.surface[1], colors.surface[2], 220);
                v.extreme_bg_color = Color32::from_rgba_unmultiplied(colors.surface[0], colors.surface[1], colors.surface[2], 120);
                v
            }
            ThemeMode::LiquidGlass => {
                let mut v = Visuals::dark();
                let blur = colors.blur_level.clamp(0.0, 10.0);
                let g = colors.glass_alpha as f32;
                let panel_a = (g * 0.30 + blur * 1.2).clamp(if g < 5.0 { 2.0 } else { 8.0 }, 185.0) as u8;
                v.panel_fill = Color32::from_rgba_unmultiplied(colors.surface[0], colors.surface[1], colors.surface[2], panel_a);
                v.window_fill = Color32::from_rgba_unmultiplied(colors.surface[0], colors.surface[1], colors.surface[2], 220);
                v.window_stroke = Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 38));
                v.extreme_bg_color = Color32::from_rgba_unmultiplied(colors.surface[0], colors.surface[1], colors.surface[2], 80);
                v.widgets.noninteractive.bg_fill = Color32::from_rgba_unmultiplied(255, 255, 255, 8);
                v.widgets.noninteractive.bg_stroke = Stroke::new(0.8_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 20));
                v.widgets.inactive.bg_fill = Color32::from_rgba_unmultiplied(255, 255, 255, 14);
                v.widgets.inactive.bg_stroke = Stroke::new(0.9_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 28));
                v.widgets.hovered.bg_fill = Color32::from_rgba_unmultiplied(255, 255, 255, 28);
                v.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 62));
                v.widgets.active.bg_fill = Color32::from_rgba_unmultiplied(255, 255, 255, 42);
                v.widgets.active.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 85));
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
                let blur = colors.blur_level.clamp(0.0, 10.0);
                let g = colors.glass_alpha as f32;
                if is_sidebar {
                    // Shared subtle chrome fill for Sidebar, TopBar, and StatusBar
                    let alpha = (g * 0.42 + blur * 1.5).clamp(if g < 5.0 { 4.0 } else { 10.0 }, 205.0) as u8;
                    (
                        Color32::from_rgba_unmultiplied(colors.surface[0], colors.surface[1], colors.surface[2], alpha),
                        Stroke::NONE,
                    )
                } else {
                    let alpha = (g * 0.22 + blur * 1.0).clamp(if g < 5.0 { 2.0 } else { 6.0 }, 175.0) as u8;
                    (
                        Color32::from_rgba_unmultiplied(colors.surface[0], colors.surface[1], colors.surface[2], alpha),
                        Stroke::NONE,
                    )
                }
            }
            ThemeMode::Blur => {
                let blur = colors.blur_level.clamp(0.0, 10.0);
                let g = colors.glass_alpha as f32;
                let alpha = if is_sidebar {
                    (g * 0.50 + blur * 11.5).clamp(if g < 5.0 && blur <= 0.05 { 2.0 } else { 10.0 }, 225.0) as u8
                } else {
                    (g * 0.25 + blur * 11.0).clamp(if blur <= 0.05 { 1.0 } else { 4.0 }, 200.0) as u8
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
                let blur = colors.blur_level.clamp(0.0, 10.0);
                let g = colors.glass_alpha as f32;
                let a = (g * 0.32 + blur * 1.2).clamp(if g < 5.0 { 8.0 } else { 14.0 }, 190.0) as u8;
                (
                    Color32::from_rgba_unmultiplied(colors.surface[0], colors.surface[1], colors.surface[2], a),
                    // Subtle whispered glass border matching dark translucent reference (never thick white wireframes)
                    Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 34)),
                    Rounding::same(14.0),
                )
            }
            ThemeMode::Blur => {
                let blur = colors.blur_level.clamp(0.0, 10.0);
                let g = colors.glass_alpha as f32;
                let a = (g * 0.38 + blur * 11.5).clamp(12.0, 215.0) as u8;
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
            inner_margin: Margin::same(14.0),
            outer_margin: Margin::symmetric(2.0, 5.0),
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
            ThemeMode::Blur => (
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

    /// Draws balanced 4-side Apple Liquid Glass edge refraction on UI cards & panels
    pub fn paint_liquid_refraction(painter: &eframe::egui::Painter, rect: eframe::egui::Rect, rounding: f32) {
        if rect.width() < 16.0 || rect.height() < 16.0 {
            return;
        }
        let inset = (rounding * 0.75).max(6.0).min(rect.width().min(rect.height()) * 0.32);
        // Top specular edge
        painter.line_segment(
            [
                eframe::egui::pos2(rect.left() + inset, rect.top() + 0.6),
                eframe::egui::pos2(rect.right() - inset, rect.top() + 0.6),
            ],
            Stroke::new(0.9_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 34)),
        );
        // Bottom cool refraction edge
        painter.line_segment(
            [
                eframe::egui::pos2(rect.left() + inset, rect.bottom() - 0.6),
                eframe::egui::pos2(rect.right() - inset, rect.bottom() - 0.6),
            ],
            Stroke::new(0.8_f32, Color32::from_rgba_unmultiplied(205, 235, 255, 26)),
        );
        // Left edge refraction
        painter.line_segment(
            [
                eframe::egui::pos2(rect.left() + 0.6, rect.top() + inset),
                eframe::egui::pos2(rect.left() + 0.6, rect.bottom() - inset),
            ],
            Stroke::new(0.8_f32, Color32::from_rgba_unmultiplied(245, 250, 255, 28)),
        );
        // Right edge refraction
        painter.line_segment(
            [
                eframe::egui::pos2(rect.right() - 0.6, rect.top() + inset),
                eframe::egui::pos2(rect.right() - 0.6, rect.bottom() - inset),
            ],
            Stroke::new(0.8_f32, Color32::from_rgba_unmultiplied(205, 235, 255, 26)),
        );
    }

    /// Paints Apple-style curved glass light bending around ALL 4 window edges and corners.
    /// Uses a translucent 5-ring optical meniscus (soft specular -> cool refraction -> subtle dark lip -> inner caustic)
    /// so all 4 sides and rounded corners have a clean 3D curve effect without harsh milky white brightness.
    pub fn paint_window_glass_shimmer(painter: &eframe::egui::Painter, rect: eframe::egui::Rect) {
        if rect.width() < 40.0 || rect.height() < 40.0 {
            return;
        }
        // Match Windows 11 DWMWCP_ROUND 8px corner radius cleanly (no corner pixel overhang)
        let r = 8.0_f32.min(rect.width() * 0.25).min(rect.height() * 0.25);
        let bevel_w = 9.0_f32;

        // Sample points clockwise around the rounded-rectangle perimeter: (pos, inward_normal, light_weight)
        let mut samples: Vec<(eframe::egui::Pos2, eframe::egui::Vec2, f32)> = Vec::with_capacity(160);

        let push_edge = |samples: &mut Vec<(eframe::egui::Pos2, eframe::egui::Vec2, f32)>,
                         p0: eframe::egui::Pos2,
                         p1: eframe::egui::Pos2,
                         inward: eframe::egui::Vec2,
                         steps: usize| {
            for i in 0..steps {
                let t = i as f32 / steps as f32;
                let pos = p0.lerp(p1, t);
                let arch = (t * std::f32::consts::PI).sin(); // smooth lens curve along full edge
                let w = 0.55 + 0.45 * arch;
                samples.push((pos, inward, w));
            }
        };

        let push_corner = |samples: &mut Vec<(eframe::egui::Pos2, eframe::egui::Vec2, f32)>,
                           center: eframe::egui::Pos2,
                           start_ang: f32,
                           end_ang: f32,
                           steps: usize| {
            for i in 0..steps {
                let t = i as f32 / steps as f32;
                let ang = start_ang + (end_ang - start_ang) * t;
                let (s, c) = ang.sin_cos();
                let out_dir = eframe::egui::vec2(c, s);
                let pos = center + out_dir * r;
                let inward = -out_dir;
                let corner_peak = (t * std::f32::consts::PI).sin();
                let w = 0.55 + 0.45 * corner_peak;
                samples.push((pos, inward, w));
            }
        };

        let edge_steps = 28usize;
        let corner_steps = 10usize;
        let pi = std::f32::consts::PI;

        // Top edge (left -> right)
        push_edge(
            &mut samples,
            eframe::egui::pos2(rect.left() + r, rect.top()),
            eframe::egui::pos2(rect.right() - r, rect.top()),
            eframe::egui::vec2(0.0, 1.0),
            edge_steps,
        );
        // Top-right corner
        push_corner(
            &mut samples,
            eframe::egui::pos2(rect.right() - r, rect.top() + r),
            -pi * 0.5,
            0.0,
            corner_steps,
        );
        // Right edge (top -> bottom)
        push_edge(
            &mut samples,
            eframe::egui::pos2(rect.right(), rect.top() + r),
            eframe::egui::pos2(rect.right(), rect.bottom() - r),
            eframe::egui::vec2(-1.0, 0.0),
            edge_steps,
        );
        // Bottom-right corner
        push_corner(
            &mut samples,
            eframe::egui::pos2(rect.right() - r, rect.bottom() - r),
            0.0,
            pi * 0.5,
            corner_steps,
        );
        // Bottom edge (right -> left)
        push_edge(
            &mut samples,
            eframe::egui::pos2(rect.right() - r, rect.bottom()),
            eframe::egui::pos2(rect.left() + r, rect.bottom()),
            eframe::egui::vec2(0.0, -1.0),
            edge_steps,
        );
        // Bottom-left corner
        push_corner(
            &mut samples,
            eframe::egui::pos2(rect.left() + r, rect.bottom() - r),
            pi * 0.5,
            pi,
            corner_steps,
        );
        // Left edge (bottom -> top)
        push_edge(
            &mut samples,
            eframe::egui::pos2(rect.left(), rect.bottom() - r),
            eframe::egui::pos2(rect.left(), rect.top() + r),
            eframe::egui::vec2(1.0, 0.0),
            edge_steps,
        );
        // Top-left corner
        push_corner(
            &mut samples,
            eframe::egui::pos2(rect.left() + r, rect.top() + r),
            pi,
            pi * 1.5,
            corner_steps,
        );

        let n = samples.len();
        if n < 8 {
            return;
        }

        // 5-ring translucent optical meniscus profile batched into a single Mesh:
        // Ring 0 (0.0px): crisp subtle outer glass catchlight (translucent, never milky white)
        // Ring 1 (1.1px): cool glass dispersion tint
        // Ring 2 (2.8px): faint dark refraction groove (gives physical 3D curved bevel depth on all 4 sides)
        // Ring 3 (5.2px): soft inner caustic reflection
        // Ring 4 (9.0px): smooth fade to 0
        let mut mesh = eframe::egui::epaint::Mesh::default();
        for &(pos, inward, w) in &samples {
            let rim_a = (18.0 + w * 34.0).clamp(0.0, 56.0) as u8;
            let sheen_a = (8.0 + w * 14.0).clamp(0.0, 24.0) as u8;
            let groove_a = (22.0 + w * 18.0).clamp(0.0, 44.0) as u8;
            let caustic_a = (6.0 + w * 11.0).clamp(0.0, 18.0) as u8;

            mesh.colored_vertex(pos, Color32::from_rgba_unmultiplied(255, 255, 255, rim_a));
            mesh.colored_vertex(pos + inward * 1.1, Color32::from_rgba_unmultiplied(205, 232, 255, sheen_a));
            mesh.colored_vertex(pos + inward * 2.8, Color32::from_rgba_unmultiplied(0, 8, 18, groove_a));
            mesh.colored_vertex(pos + inward * 5.2, Color32::from_rgba_unmultiplied(215, 238, 255, caustic_a));
            mesh.colored_vertex(pos + inward * bevel_w, Color32::from_rgba_unmultiplied(215, 238, 255, 0));
        }
        for i in 0..n {
            let next = (i + 1) % n;
            let b0 = (i * 5) as u32;
            let b1 = (next * 5) as u32;
            for band in 0..4u32 {
                let a0 = b0 + band;
                let a1 = a0 + 1;
                let c0 = b1 + band;
                let c1 = c0 + 1;
                mesh.add_triangle(a0, c0, a1);
                mesh.add_triangle(a1, c0, c1);
            }
        }
        painter.add(eframe::egui::Shape::mesh(mesh));
    }
}

/// Sets Windows 11/10 backdrop:
/// - LiquidGlass uses SetWindowCompositionAttribute(ACCENT_ENABLE_ACRYLICBLURBEHIND) directly with custom RGBA tint
///   (bypassing Win11's thick opaque-gray DWMSBT_TRANSIENTWINDOW luminosity layer).
/// - Transparent keeps standard Windows 11 Acrylic (DWMSBT_TRANSIENTWINDOW).
#[cfg(target_os = "windows")]
unsafe fn set_win_composition_accent(hwnd: isize, accent_state: u32, color: Option<(u8, u8, u8, u8)>) {
    use std::ffi::c_void;

    #[repr(C)]
    struct AccentPolicy {
        accent_state: u32,
        accent_flags: u32,
        gradient_color: u32,
        animation_id: u32,
    }
    #[repr(C)]
    struct WindowCompositionAttribData {
        attrib: u32,
        pv_data: *mut c_void,
        cb_data: usize,
    }

    extern "system" {
        fn LoadLibraryA(lp_lib_file_name: *const u8) -> *mut c_void;
        fn GetProcAddress(h_module: *mut c_void, lp_proc_name: *const u8) -> *mut c_void;
    }

    let user32 = LoadLibraryA(b"user32.dll\0".as_ptr());
    if user32.is_null() {
        return;
    }
    let proc = GetProcAddress(user32, b"SetWindowCompositionAttribute\0".as_ptr());
    if proc.is_null() {
        return;
    }

    let func: unsafe extern "system" fn(isize, *mut WindowCompositionAttribData) -> i32 =
        std::mem::transmute(proc);

    let (r, g, b, mut a) = color.unwrap_or((0, 0, 0, 0));
    if accent_state == 4 && a == 0 {
        a = 1;
    }
    let mut policy = AccentPolicy {
        accent_state,
        accent_flags: if accent_state == 4 { 0 } else { 2 },
        gradient_color: (r as u32) | ((g as u32) << 8) | ((b as u32) << 16) | ((a as u32) << 24),
        animation_id: 0,
    };
    let mut data = WindowCompositionAttribData {
        attrib: 0x13, // WCA_ACCENT_POLICY
        pv_data: &mut policy as *mut _ as *mut c_void,
        cb_data: std::mem::size_of::<AccentPolicy>(),
    };
    let _ = func(hwnd, &mut data);
}

#[cfg(target_os = "windows")]
unsafe fn set_win11_rounded_corners(hwnd: isize) {
    use std::ffi::c_void;
    extern "system" {
        fn LoadLibraryA(lp_lib_file_name: *const u8) -> *mut c_void;
        fn GetProcAddress(h_module: *mut c_void, lp_proc_name: *const u8) -> *mut c_void;
    }
    let dwmapi = LoadLibraryA(b"dwmapi.dll\0".as_ptr());
    if dwmapi.is_null() {
        return;
    }
    let proc = GetProcAddress(dwmapi, b"DwmSetWindowAttribute\0".as_ptr());
    if proc.is_null() {
        return;
    }
    let func: unsafe extern "system" fn(isize, u32, *const c_void, u32) -> i32 = std::mem::transmute(proc);
    let corner_round: u32 = 2; // DWMWCP_ROUND
    let _ = func(hwnd, 33, &corner_round as *const u32 as *const c_void, 4); // DWMWA_WINDOW_CORNER_PREFERENCE
}

#[cfg(target_os = "windows")]
pub fn apply_platform_vibrancy(window: &impl raw_window_handle::HasWindowHandle, mode: ThemeMode, colors: &CustomThemeColors) {
    let hwnd_opt = window.window_handle().ok().and_then(|h| match h.as_raw() {
        raw_window_handle::RawWindowHandle::Win32(w) => Some(w.hwnd.get() as isize),
        _ => None,
    });

    if let Some(hwnd) = hwnd_opt {
        unsafe {
            set_win11_rounded_corners(hwnd);
        }
    }

    match mode {
        ThemeMode::LiquidGlass => {
            // Liquid Glass uses DesktopBackdrop smooth variable-sigma blur when blur_level > 0,
            // and pure sharp DWM transparency when blur_level == 0 (never Win11's abrupt 35px jump!)
            if let Some(hwnd) = hwnd_opt {
                unsafe {
                    set_win_composition_accent(hwnd, 0, None);
                }
            }
            let _ = window_vibrancy::clear_blur(window);
            let _ = window_vibrancy::clear_mica(window);
            let _ = window_vibrancy::clear_acrylic(window);
        }
        ThemeMode::Blur => {
            let blur = colors.blur_level.clamp(0.0, 10.0);
            if blur <= 0.05 {
                if let Some(hwnd) = hwnd_opt {
                    unsafe {
                        set_win_composition_accent(hwnd, 0, None);
                    }
                }
                let _ = window_vibrancy::clear_blur(window);
                let _ = window_vibrancy::clear_mica(window);
                let _ = window_vibrancy::clear_acrylic(window);
            } else if blur < 7.0 {
                let _ = window_vibrancy::clear_mica(window);
                let _ = window_vibrancy::clear_acrylic(window);
                let t = (blur / 6.9).clamp(0.0, 1.0);
                let a = (2.0 + t * 150.0).max(colors.glass_alpha as f32 * 0.35).clamp(2.0, 210.0) as u8;
                if let Some(hwnd) = hwnd_opt {
                    unsafe {
                        set_win_composition_accent(hwnd, 4, Some((colors.surface[0], colors.surface[1], colors.surface[2], a)));
                    }
                } else {
                    let _ = window_vibrancy::apply_acrylic(window, Some((colors.surface[0], colors.surface[1], colors.surface[2], a)));
                }
            } else {
                if let Some(hwnd) = hwnd_opt {
                    unsafe {
                        set_win_composition_accent(hwnd, 0, None);
                    }
                }
                let _ = window_vibrancy::clear_mica(window);
                let t = ((blur - 7.0) / 3.0).clamp(0.0, 1.0);
                let a = (120.0 + t * 105.0).max(colors.glass_alpha as f32 * 0.6).clamp(120.0, 235.0) as u8;
                let color = Some((colors.surface[0], colors.surface[1], colors.surface[2], a));
                if window_vibrancy::apply_acrylic(window, color).is_err() {
                    let _ = window_vibrancy::apply_mica(window, Some(true));
                }
            }
        }
        _ => {
            if let Some(hwnd) = hwnd_opt {
                unsafe {
                    set_win_composition_accent(hwnd, 0, None);
                }
            }
            let _ = window_vibrancy::clear_blur(window);
            let _ = window_vibrancy::clear_mica(window);
            let _ = window_vibrancy::clear_acrylic(window);
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub fn apply_platform_vibrancy(_window: &impl raw_window_handle::HasWindowHandle, _mode: ThemeMode, _colors: &CustomThemeColors) {}

#[cfg(target_os = "windows")]
static ORIG_WNDPROC: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);

#[cfg(target_os = "windows")]
#[repr(C)]
struct HitRect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn borderless_resize_wndproc(
    hwnd: isize,
    msg: u32,
    wparam: usize,
    lparam: isize,
) -> isize {
    use std::ffi::c_void;
    const WM_NCHITTEST: u32 = 0x0084;
    const HTCLIENT: isize = 1;
    const HTLEFT: isize = 10;
    const HTRIGHT: isize = 11;
    const HTTOP: isize = 12;
    const HTTOPLEFT: isize = 13;
    const HTTOPRIGHT: isize = 14;
    const HTBOTTOM: isize = 15;
    const HTBOTTOMLEFT: isize = 16;
    const HTBOTTOMRIGHT: isize = 17;

    extern "system" {
        fn CallWindowProcW(lp_prev_wnd_func: *const c_void, h_wnd: isize, msg: u32, w_param: usize, l_param: isize) -> isize;
        fn GetWindowRect(h_wnd: isize, lp_rect: *mut HitRect) -> i32;
        fn IsZoomed(h_wnd: isize) -> i32;
        fn GetDpiForWindow(hwnd: isize) -> u32;
    }

    let orig = ORIG_WNDPROC.load(std::sync::atomic::Ordering::Acquire);
    let orig_fn: *const c_void = orig as *const c_void;
    let res = CallWindowProcW(orig_fn, hwnd, msg, wparam, lparam);

    if msg == WM_NCHITTEST && res == HTCLIENT && IsZoomed(hwnd) == 0 {
        let mut wr = HitRect { left: 0, top: 0, right: 0, bottom: 0 };
        if GetWindowRect(hwnd, &mut wr) != 0 {
            let x = (lparam & 0xFFFF) as i16 as i32;
            let y = ((lparam >> 16) & 0xFFFF) as i16 as i32;
            let dpi = GetDpiForWindow(hwnd).max(96) as f32;
            let scale = dpi / 96.0;
            let border = (7.0 * scale).round() as i32;
            let corner = (14.0 * scale).round() as i32;

            let in_win_btns = y >= wr.top + (8.0 * scale) as i32
                && y <= wr.top + (40.0 * scale) as i32
                && x >= wr.right - (110.0 * scale) as i32
                && x <= wr.right - (8.0 * scale) as i32;

            if !in_win_btns {
                let on_left = x >= wr.left && x < wr.left + border;
                let on_right = x <= wr.right && x > wr.right - border;
                let on_top = y >= wr.top && y < wr.top + border;
                let on_bottom = y <= wr.bottom && y > wr.bottom - border;

                if (on_left && y < wr.top + corner) || (on_top && x < wr.left + corner) {
                    return HTTOPLEFT;
                }
                if (on_right && y < wr.top + corner) || (on_top && x > wr.right - corner) {
                    return HTTOPRIGHT;
                }
                if (on_left && y > wr.bottom - corner) || (on_bottom && x < wr.left + corner) {
                    return HTBOTTOMLEFT;
                }
                if (on_right && y > wr.bottom - corner) || (on_bottom && x > wr.right - corner) {
                    return HTBOTTOMRIGHT;
                }
                if on_left {
                    return HTLEFT;
                }
                if on_right {
                    return HTRIGHT;
                }
                if on_top {
                    return HTTOP;
                }
                if on_bottom {
                    return HTBOTTOM;
                }
            }
        }
    }

    res
}

/// Subclasses the borderless window once on Windows so all 4 edges & corners support
/// native WM_NCHITTEST resizing and Aero Snap without interfering with TopBar controls.
#[cfg(target_os = "windows")]
pub fn install_borderless_resize_subclass(hwnd: isize) {
    use std::ffi::c_void;
    const GWLP_WNDPROC: i32 = -4;

    extern "system" {
        fn LoadLibraryA(lp_lib_file_name: *const u8) -> *mut c_void;
        fn GetProcAddress(h_module: *mut c_void, lp_proc_name: *const u8) -> *mut c_void;
    }

    if ORIG_WNDPROC.load(std::sync::atomic::Ordering::Acquire) != 0 {
        return;
    }

    unsafe {
        let user32 = LoadLibraryA(b"user32.dll\0".as_ptr());
        if user32.is_null() {
            return;
        }
        let p_set = GetProcAddress(user32, b"SetWindowLongPtrW\0".as_ptr());
        if p_set.is_null() {
            return;
        }
        let set_wnd_long_ptr: unsafe extern "system" fn(isize, i32, isize) -> isize = std::mem::transmute(p_set);
        let new_proc = borderless_resize_wndproc as *const () as isize;
        let prev = set_wnd_long_ptr(hwnd, GWLP_WNDPROC, new_proc);
        if prev != 0 && prev != new_proc {
            ORIG_WNDPROC.store(prev, std::sync::atomic::Ordering::Release);
        }
    }
}
