//! Dark "liquid glass" theme with orange-glow accents (colors taken from the
//! app icon). egui can't do true backdrop blur, so the glass look is simulated
//! with translucent fills, soft shadows, rounded corners and a light top edge.

use std::f32::consts::TAU;

use egui::{Color32, CornerRadius, Frame, Margin, Painter, Pos2, Response, RichText, Shape, Stroke, Ui};

// --- Palette ---------------------------------------------------------------
pub const BG: Color32 = Color32::from_rgb(8, 8, 10);
pub const BG_DEEP: Color32 = Color32::from_rgb(4, 4, 6);
/// Translucent panel surface (the "glass").
pub const PANEL: Color32 = Color32::from_rgba_premultiplied(28, 28, 34, 210);
pub const PANEL_SOLID: Color32 = Color32::from_rgb(24, 24, 28);
pub const PANEL_RAISED: Color32 = Color32::from_rgba_premultiplied(44, 44, 52, 220);
pub const BORDER: Color32 = Color32::from_rgb(64, 60, 70);
pub const HIGHLIGHT: Color32 = Color32::from_rgba_premultiplied(255, 255, 255, 22);

pub const ACCENT: Color32 = Color32::from_rgb(255, 122, 26); // #FF7A1A
pub const ACCENT_HOT: Color32 = Color32::from_rgb(255, 77, 46); // #FF4D2E
pub const ACCENT_SOFT: Color32 = Color32::from_rgba_premultiplied(255, 122, 26, 38);

pub const TEXT: Color32 = Color32::from_rgb(240, 240, 244);
pub const MUTED: Color32 = Color32::from_rgb(150, 150, 160);
pub const SUCCESS: Color32 = Color32::from_rgb(46, 204, 113);
pub const DANGER: Color32 = Color32::from_rgb(231, 76, 60);

/// Install the global egui style/visuals.
pub fn apply(ctx: &egui::Context) {
    ctx.set_theme(egui::ThemePreference::Dark);
    ctx.all_styles_mut(style_setup);
}

fn style_setup(style: &mut egui::Style) {
    let mut v = egui::Visuals::dark();

    v.dark_mode = true;
    v.override_text_color = Some(TEXT);
    v.panel_fill = BG;
    v.window_fill = PANEL_SOLID;
    v.extreme_bg_color = Color32::from_rgb(18, 18, 22); // text edit background
    v.faint_bg_color = Color32::from_rgb(20, 20, 24);
    v.window_stroke = Stroke::new(1.0, BORDER);
    v.window_corner_radius = CornerRadius::same(16);

    v.selection.bg_fill = ACCENT_SOFT;
    v.selection.stroke = Stroke::new(1.0, ACCENT);
    v.hyperlink_color = ACCENT;

    let radius = CornerRadius::same(10);
    v.widgets.noninteractive.corner_radius = radius;
    v.widgets.noninteractive.bg_fill = PANEL;
    v.widgets.noninteractive.weak_bg_fill = PANEL;
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);

    v.widgets.inactive.corner_radius = radius;
    v.widgets.inactive.bg_fill = PANEL_RAISED;
    v.widgets.inactive.weak_bg_fill = PANEL_RAISED;
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER);

    v.widgets.hovered.corner_radius = radius;
    v.widgets.hovered.bg_fill = Color32::from_rgb(58, 58, 66);
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(58, 58, 66);
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT);

    v.widgets.active.corner_radius = radius;
    v.widgets.active.bg_fill = Color32::from_rgb(70, 70, 80);
    v.widgets.active.weak_bg_fill = Color32::from_rgb(70, 70, 80);
    v.widgets.active.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);

    style.visuals = v;
    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.button_padding = egui::vec2(12.0, 7.0);

    use egui::{FontFamily, FontId, TextStyle};
    style.text_styles = [
        (TextStyle::Heading, FontId::new(24.0, FontFamily::Proportional)),
        (TextStyle::Body, FontId::new(14.0, FontFamily::Proportional)),
        (TextStyle::Button, FontId::new(14.0, FontFamily::Proportional)),
        (TextStyle::Monospace, FontId::new(13.0, FontFamily::Monospace)),
        (TextStyle::Small, FontId::new(11.0, FontFamily::Proportional)),
    ]
    .into();
}

// --- Frames ----------------------------------------------------------------

/// A translucent glass panel frame with a soft shadow.
pub fn glass_frame() -> Frame {
    Frame::default()
        .fill(PANEL)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(CornerRadius::same(14))
        .inner_margin(Margin::same(14))
        .shadow(egui::epaint::Shadow {
            offset: [0, 10],
            blur: 28,
            spread: 0,
            color: Color32::from_black_alpha(120),
        })
}

/// Frame used for floating windows (Bulk, Help).
pub fn window_frame() -> Frame {
    Frame::default()
        .fill(PANEL_SOLID)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(CornerRadius::same(16))
        .inner_margin(Margin::same(18))
        .shadow(egui::epaint::Shadow {
            offset: [0, 14],
            blur: 36,
            spread: 0,
            color: Color32::from_black_alpha(160),
        })
}

/// Show some content inside a glass panel.
pub fn glass_group<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    glass_frame().show(ui, add).inner
}

// --- Buttons ---------------------------------------------------------------

/// Primary, orange-glow action button with a glossy top highlight.
pub fn accent_button(ui: &mut Ui, text: &str) -> Response {
    let btn = egui::Button::new(RichText::new(text).color(Color32::WHITE).strong().size(14.0))
        .fill(ACCENT)
        .corner_radius(CornerRadius::same(10))
        .min_size(egui::vec2(130.0, 38.0));
    let resp = ui.add(btn);
    let rect = resp.rect;
    let painter = ui.painter();
    // Glossy sheen along the top edge.
    let sheen = egui::Rect::from_min_max(
        egui::pos2(rect.min.x + 6.0, rect.min.y + 1.0),
        egui::pos2(rect.max.x - 6.0, rect.min.y + 3.0),
    );
    painter.rect_filled(sheen, CornerRadius::same(2), HIGHLIGHT);
    // Hot outline + soft glow when hovered.
    if resp.hovered() {
        painter.rect_stroke(
            rect.expand(1.0),
            CornerRadius::same(11),
            Stroke::new(1.5, ACCENT_HOT),
            egui::StrokeKind::Outside,
        );
        painter.rect_stroke(
            rect.expand(4.0),
            CornerRadius::same(13),
            Stroke::new(3.0, ACCENT_SOFT),
            egui::StrokeKind::Outside,
        );
    }
    resp
}

/// Secondary, glassy surface button.
pub fn surface_button(ui: &mut Ui, text: &str) -> Response {
    let btn = egui::Button::new(RichText::new(text).color(TEXT).size(13.0))
        .fill(PANEL_RAISED)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(CornerRadius::same(10))
        .min_size(egui::vec2(110.0, 34.0));
    ui.add(btn)
}

// --- Text helpers ----------------------------------------------------------

pub fn title_text(s: &str) -> RichText {
    RichText::new(s).size(18.0).strong().color(TEXT)
}

pub fn section_title(s: &str) -> RichText {
    RichText::new(s).size(16.0).strong().color(TEXT)
}

pub fn body_text(s: &str) -> RichText {
    RichText::new(s).size(14.0).color(TEXT)
}

pub fn muted_text(s: &str) -> RichText {
    RichText::new(s).size(12.0).color(MUTED)
}

pub fn label_text(s: &str) -> RichText {
    RichText::new(s).size(11.0).color(MUTED)
}

// --- "Factory" animation (gears + conveyor belt) ---------------------------

fn polar(center: Pos2, r: f32, a: f32) -> Pos2 {
    egui::pos2(center.x + r * a.cos(), center.y + r * a.sin())
}

/// Draw a rotating gear silhouette (outline + hub) at `angle` radians.
fn paint_gear(painter: &Painter, center: Pos2, radius: f32, teeth: usize, angle: f32, color: Color32, width: f32) {
    let root = radius * 0.74;
    let step = TAU / teeth as f32;
    let mut pts: Vec<Pos2> = Vec::with_capacity(teeth * 4);
    for i in 0..teeth {
        let a = angle + i as f32 * step;
        pts.push(polar(center, root, a));
        pts.push(polar(center, radius, a + step * 0.18));
        pts.push(polar(center, radius, a + step * 0.34));
        pts.push(polar(center, root, a + step * 0.5));
    }
    // Faint inner disk for depth.
    painter.circle_filled(center, root, Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 18));
    painter.add(Shape::closed_line(pts, Stroke::new(width, color)));
    painter.circle_stroke(center, radius * 0.34, Stroke::new(width, color));
    painter.circle_filled(center, width * 1.4, color);
}

/// Draw a conveyor belt with turning end-rollers and scrolling tread marks.
fn paint_conveyor(painter: &Painter, left: Pos2, right: Pos2, roller_r: f32, t: f32, color: Color32, width: f32) {
    let top_y = left.y - roller_r;
    let bot_y = left.y + roller_r;

    // Belt bands.
    painter.line_segment([egui::pos2(left.x, top_y), egui::pos2(right.x, top_y)], Stroke::new(width, color));
    painter.line_segment([egui::pos2(left.x, bot_y), egui::pos2(right.x, bot_y)], Stroke::new(width, color));

    // End rollers as small turning gears.
    let roller_angle = t * 1.4;
    paint_gear(painter, left, roller_r, 8, roller_angle, color, width);
    paint_gear(painter, right, roller_r, 8, roller_angle, color, width);

    // Scrolling treads: top moves right, bottom moves left (a looping belt).
    let spacing = 26.0;
    let speed = roller_r * 1.4;
    let tread = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 110);
    let offset = (t * speed).rem_euclid(spacing);

    let mut x = left.x + roller_r + offset;
    while x < right.x - roller_r {
        painter.line_segment([egui::pos2(x, top_y + 2.0), egui::pos2(x, top_y + 10.0)], Stroke::new(width * 0.8, tread));
        x += spacing;
    }
    let mut x = right.x - roller_r - offset;
    while x > left.x + roller_r {
        painter.line_segment([egui::pos2(x, bot_y - 10.0), egui::pos2(x, bot_y - 2.0)], Stroke::new(width * 0.8, tread));
        x -= spacing;
    }
}

/// Paint a looping factory scene (interlocking gears + a conveyor belt) inside
/// `rect`, sized/positioned to frame a centered icon. `t` is seconds.
pub fn paint_factory_scene(painter: &Painter, rect: egui::Rect, t: f32) {
    let c = rect.center();
    let icon_cy = c.y - rect.height() * 0.06;
    let s = (rect.height() * 0.42).min(190.0);
    let half = s / 2.0;

    let gear = Color32::from_rgba_unmultiplied(ACCENT.r(), ACCENT.g(), ACCENT.b(), 150);
    let belt = Color32::from_rgba_unmultiplied(255, 150, 70, 135);
    let w = 2.5;

    // Two meshing gears upper-left, one upper-right, flanking the icon.
    paint_gear(painter, egui::pos2(c.x - half - 18.0, icon_cy - half + 14.0), 44.0, 10, t * 0.6, gear, w);
    paint_gear(painter, egui::pos2(c.x - half + 30.0, icon_cy - half + 70.0), 28.0, 8, -t * 0.95, gear, w);
    paint_gear(painter, egui::pos2(c.x + half + 14.0, icon_cy - half + 40.0), 36.0, 9, t * 0.78, gear, w);

    // Conveyor belt beneath the icon.
    let belt_y = icon_cy + half + 50.0;
    let left = egui::pos2(c.x - half - 36.0, belt_y);
    let right = egui::pos2(c.x + half + 36.0, belt_y);
    paint_conveyor(painter, left, right, 26.0, t, belt, w);
}

/// Paint the app's dark background with a warm glow behind the glass, echoing
/// the icon's amber light.
pub fn paint_background(ui: &Ui, rect: egui::Rect) {
    let painter = ui.painter();
    painter.rect_filled(rect, 0.0, BG_DEEP);
    // Lighter core for depth.
    let core = egui::Rect::from_center_size(rect.center(), rect.size() * 0.92);
    painter.rect_filled(core, CornerRadius::same(0), BG);
    // Soft concentric warm glow near the upper-center (the icon's light source).
    let origin = egui::pos2(rect.center().x, rect.top() + rect.height() * 0.30);
    for i in (1..=6).rev() {
        let t = i as f32;
        let size = egui::vec2(120.0 * t, 90.0 * t);
        let alpha = (16.0 / t) as u8;
        let warm = if i <= 2 { ACCENT_HOT } else { ACCENT };
        let color = Color32::from_rgba_unmultiplied(warm.r(), warm.g(), warm.b(), alpha);
        painter.rect_filled(egui::Rect::from_center_size(origin, size), CornerRadius::same(40), color);
    }
}
