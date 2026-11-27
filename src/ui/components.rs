use eframe::egui::{self, Color32, FontId, Pos2, Rect, Response, RichText, Rounding, Sense, Stroke, Ui, Vec2};
use crate::data::{CustomThemeColors, ThemeMode};

pub fn surface_panel_fill(mode: ThemeMode, colors: &CustomThemeColors, alpha_boost: u8) -> Color32 {
    match mode {
        ThemeMode::Light => Color32::from_rgb(245, 247, 250),
        ThemeMode::Blur => Color32::from_rgba_unmultiplied(
            colors.surface[0],
            colors.surface[1],
            colors.surface[2],
            colors.glass_alpha.saturating_add(alpha_boost),
        ),
        _ => Color32::from_rgb(
            colors.surface[0].saturating_add(6),
            colors.surface[1].saturating_add(8),
            colors.surface[2].saturating_add(14),
        ),
    }
}

pub fn glass_stroke(mode: ThemeMode) -> Stroke {
    match mode {
        ThemeMode::Light => Stroke::new(1.0_f32, Color32::from_rgb(215, 222, 232)),
        ThemeMode::Blur => Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 34)),
        _ => Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 22)),
    }
}

pub fn muted_text(mode: ThemeMode) -> Color32 {
    match mode {
        ThemeMode::Light => Color32::from_rgb(60, 70, 85),
        _ => Color32::from_rgb(148, 158, 178),
    }
}

pub fn prism_primary_button(ui: &mut Ui, label: &str, accent: Color32) -> Response {
    let btn = egui::Button::new(
        RichText::new(label)
            .font(FontId::proportional(12.5))
            .color(Color32::WHITE)
            .strong(),
    )
    .fill(accent)
    .stroke(Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 45)))
    .rounding(Rounding::same(8.0))
    .min_size(Vec2::new(0.0, 30.0));
    ui.add(btn)
}

pub fn prism_filter_pill(
    ui: &mut Ui,
    label: &str,
    count: Option<usize>,
    active: bool,
    accent: Color32,
    mode: ThemeMode,
) -> Response {
    let text = match count {
        Some(c) => format!("{}  {}", label, c),
        None => label.to_string(),
    };
    let (fill, stroke, text_col) = if active {
        (
            Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 45),
            Stroke::new(1.0_f32, accent),
            Color32::WHITE,
        )
    } else if mode == ThemeMode::Light {
        (
            Color32::from_rgb(232, 236, 242),
            Stroke::new(1.0_f32, Color32::from_rgb(205, 212, 222)),
            Color32::from_rgb(45, 55, 70),
        )
    } else {
        (
            Color32::from_rgba_unmultiplied(255, 255, 255, 10),
            Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 22)),
            Color32::from_rgb(180, 190, 210),
        )
    };

    let btn = egui::Button::new(RichText::new(text).font(FontId::proportional(11.5)).color(text_col))
        .fill(fill)
        .stroke(stroke)
        .rounding(Rounding::same(14.0))
        .min_size(Vec2::new(0.0, 26.0));
    ui.add(btn)
}

pub fn prism_progress_bar(ui: &mut Ui, fraction: f32, width: f32, height: f32, fill_color: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    ui.painter().rect_filled(
        rect,
        Rounding::same(height / 2.0),
        Color32::from_rgba_unmultiplied(255, 255, 255, 18),
    );
    let clamped = fraction.clamp(0.0, 1.0);
    if clamped > 0.0 {
        let fill_rect = Rect::from_min_size(rect.min, Vec2::new(rect.width() * clamped, rect.height()));
        ui.painter().rect_filled(fill_rect, Rounding::same(height / 2.0), fill_color);
    }
}

pub fn prism_brand_badge(ui: &mut Ui, accent: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(30.0), Sense::hover());
    ui.painter().rect_filled(rect, Rounding::same(12.0), accent);
    ui.painter().rect_stroke(
        rect,
        Rounding::same(12.0),
        Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 70)),
    );
    ui.painter().text(
        Pos2::new(rect.center().x, rect.center().y),
        egui::Align2::CENTER_CENTER,
        "⚡",
        FontId::proportional(16.0),
        Color32::WHITE,
    );
}
