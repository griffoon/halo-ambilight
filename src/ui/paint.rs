//! Low-level drawing: gradients, glows and the glass material.

use egui::epaint::{Mesh, Shadow};
use egui::text::{LayoutJob, TextFormat};
use egui::{pos2, vec2, Color32, FontFamily, FontId, Galley, Painter, Pos2, Rect, Rounding, Shape, Stroke, Ui};
use std::f32::consts::{FRAC_PI_2, TAU};
use std::sync::Arc;

pub const TEXT: Color32 = Color32::from_rgba_premultiplied(242, 242, 247, 242);
pub const TEXT2: Color32 = Color32::from_rgba_premultiplied(150, 150, 158, 158);
pub const TEXT3: Color32 = Color32::from_rgba_premultiplied(96, 96, 104, 104);
pub const ACCENT: Color32 = Color32::from_rgb(128, 116, 255);
pub const ACCENT_HI: Color32 = Color32::from_rgb(168, 150, 255);
pub const OK: Color32 = Color32::from_rgb(52, 211, 120);
pub const WARN: Color32 = Color32::from_rgb(255, 184, 64);
pub const BAD: Color32 = Color32::from_rgb(255, 86, 96);

pub fn semibold() -> FontFamily {
    FontFamily::Name("sb".into())
}

pub fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_premultiplied(mix(a.r(), b.r()), mix(a.g(), b.g()), mix(a.b(), b.b()), mix(a.a(), b.a()))
}

pub fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

pub fn with_alpha(c: [u8; 3], a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c[0], c[1], c[2], a)
}

/// Rounded-rectangle outline, clockwise in screen space.
pub fn rrect_path(rect: Rect, radius: f32) -> Vec<Pos2> {
    const SEG: usize = 8;
    let r = radius.min(rect.width() * 0.5).min(rect.height() * 0.5).max(0.0);
    let corners = [
        (pos2(rect.right() - r, rect.bottom() - r), 0.0),
        (pos2(rect.left() + r, rect.bottom() - r), FRAC_PI_2),
        (pos2(rect.left() + r, rect.top() + r), 2.0 * FRAC_PI_2),
        (pos2(rect.right() - r, rect.top() + r), 3.0 * FRAC_PI_2),
    ];
    corners
        .iter()
        .flat_map(|(c, start)| {
            (0..=SEG).map(move |i| {
                let a = start + i as f32 / SEG as f32 * FRAC_PI_2;
                pos2(c.x + r * a.cos(), c.y + r * a.sin())
            })
        })
        .collect()
}

/// Vertical gradient inside a rounded rectangle (triangle fan, convex shape).
pub fn fill_gradient(p: &Painter, rect: Rect, radius: f32, top: Color32, bottom: Color32) {
    let pts = rrect_path(rect, radius);
    let shade = |y: f32| lerp_color(top, bottom, (y - rect.top()) / rect.height().max(1.0));
    let mut mesh = Mesh::default();
    mesh.colored_vertex(rect.center(), shade(rect.center().y));
    pts.iter().for_each(|pt| mesh.colored_vertex(*pt, shade(pt.y)));
    let n = pts.len() as u32;
    (0..n).for_each(|i| mesh.add_triangle(0, 1 + i, 1 + (i + 1) % n));
    p.add(Shape::mesh(mesh));
}

/// Soft radial glow fading to transparent.
pub fn radial(p: &Painter, center: Pos2, radius: f32, color: Color32) {
    const SEG: u32 = 36;
    let mid = color.gamma_multiply(0.36);
    let mut mesh = Mesh::default();
    mesh.colored_vertex(center, color);
    for i in 0..SEG {
        let a = i as f32 / SEG as f32 * TAU;
        let d = vec2(a.cos(), a.sin());
        mesh.colored_vertex(center + d * radius * 0.45, mid);
        mesh.colored_vertex(center + d * radius, Color32::TRANSPARENT);
    }
    for i in 0..SEG {
        let j = (i + 1) % SEG;
        let (m0, o0, m1, o1) = (1 + 2 * i, 2 + 2 * i, 1 + 2 * j, 2 + 2 * j);
        mesh.add_triangle(0, m0, m1);
        mesh.add_triangle(m0, o0, o1);
        mesh.add_triangle(m0, o1, m1);
    }
    p.add(Shape::mesh(mesh));
}

pub fn shadow(p: &Painter, rect: Rect, radius: f32, y: f32, blur: f32, alpha: u8) {
    let s = Shadow { offset: vec2(0.0, y), blur, spread: 0.0, color: Color32::from_black_alpha(alpha) };
    p.add(s.as_shape(rect, Rounding::same(radius)));
}

/// The glass material: faint white body, light-catching top edge, hairline border.
pub fn glass(p: &Painter, rect: Rect, radius: f32, weight: f32) {
    shadow(p, rect, radius, 8.0, 24.0, (36.0 * weight) as u8);
    fill_gradient(
        p,
        rect,
        radius,
        Color32::from_white_alpha((20.0 * weight) as u8),
        Color32::from_white_alpha((7.0 * weight) as u8),
    );
    p.rect_stroke(rect, Rounding::same(radius), Stroke::new(1.0_f32, Color32::from_white_alpha(20)));
    let y = rect.top() + 0.8;
    p.line_segment(
        [pos2(rect.left() + radius * 0.9, y), pos2(rect.right() - radius * 0.9, y)],
        Stroke::new(1.0_f32, Color32::from_white_alpha(46)),
    );
}

pub fn galley(ui: &Ui, text: &str, size: f32, family: FontFamily, color: Color32, tracking: f32) -> Arc<Galley> {
    let mut job = LayoutJob::default();
    job.append(
        text,
        0.0,
        TextFormat { font_id: FontId::new(size, family), color, extra_letter_spacing: tracking, ..Default::default() },
    );
    ui.fonts(|f| f.layout_job(job))
}

/// Small uppercase section caption.
pub fn caption(ui: &mut Ui, text: &str) {
    let g = galley(ui, &text.to_uppercase(), 11.0, semibold(), TEXT3, 0.9);
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), g.size().y), egui::Sense::hover());
    ui.painter().galley(rect.min, g, TEXT3);
}

/// Wraps content in a glass card.
pub fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let slot = ui.painter().add(Shape::Noop);
    let inner = egui::Frame::none().inner_margin(egui::Margin::same(16.0)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        add(ui)
    });
    // The slot was reserved before the content, so the card lands behind it.
    ui.painter().set(slot, Shape::Vec(glass_shapes(inner.response.rect, 20.0)));
    inner.inner
}

/// Same look as [`glass`], returned as shapes so it can fill a reserved slot.
pub fn glass_shapes(rect: Rect, radius: f32) -> Vec<Shape> {
    let s = Shadow { offset: vec2(0.0, 10.0), blur: 30.0, spread: 0.0, color: Color32::from_black_alpha(38) };
    let pts = rrect_path(rect, radius);
    let top = Color32::from_white_alpha(19);
    let bottom = Color32::from_white_alpha(6);
    let shade = |y: f32| lerp_color(top, bottom, (y - rect.top()) / rect.height().max(1.0));
    let mut mesh = Mesh::default();
    mesh.colored_vertex(rect.center(), shade(rect.center().y));
    pts.iter().for_each(|pt| mesh.colored_vertex(*pt, shade(pt.y)));
    let n = pts.len() as u32;
    (0..n).for_each(|i| mesh.add_triangle(0, 1 + i, 1 + (i + 1) % n));
    let y = rect.top() + 0.8;
    vec![
        Shape::from(s.as_shape(rect, Rounding::same(radius))),
        Shape::mesh(mesh),
        Shape::rect_stroke(rect, Rounding::same(radius), Stroke::new(1.0_f32, Color32::from_white_alpha(20))),
        Shape::line_segment(
            [pos2(rect.left() + radius * 0.9, y), pos2(rect.right() - radius * 0.9, y)],
            Stroke::new(1.0_f32, Color32::from_white_alpha(44)),
        ),
    ]
}

pub fn sun(p: &Painter, c: Pos2, size: f32, color: Color32) {
    p.circle_filled(c, size * 0.26, color);
    for i in 0..8 {
        let a = i as f32 / 8.0 * TAU;
        let d = vec2(a.cos(), a.sin());
        p.line_segment([c + d * size * 0.42, c + d * size * 0.58], Stroke::new(size * 0.11, color));
    }
}

/// The halo logo: a conic ring with a soft glow.
pub fn logo(p: &Painter, c: Pos2, r: f32) {
    radial(p, c, r * 2.2, Color32::from_rgba_unmultiplied(140, 110, 255, 60));
    const SEG: usize = 72;
    for i in 0..SEG {
        let a0 = i as f32 / SEG as f32 * TAU;
        let a1 = (i + 1) as f32 / SEG as f32 * TAU;
        let col = crate::art::conic(a0);
        let color = Color32::from_rgb((col[0] * 255.0) as u8, (col[1] * 255.0) as u8, (col[2] * 255.0) as u8);
        p.line_segment(
            [c + vec2(a0.cos(), a0.sin()) * r, c + vec2(a1.cos(), a1.sin()) * r],
            Stroke::new(r * 0.24, color),
        );
    }
}
