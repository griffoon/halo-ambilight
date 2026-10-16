//! Low-level drawing: the type scale, anti-aliased gradients, glows and the glass material.

use egui::epaint::{Mesh, Shadow};
use egui::text::{LayoutJob, TextFormat};
use egui::{pos2, vec2, Align2, Color32, FontFamily, FontId, Painter, Pos2, Rect, RichText, Rounding, Shape, Stroke, Ui, Vec2};
use std::f32::consts::{FRAC_PI_2, TAU};

pub const TEXT: Color32 = Color32::from_rgba_premultiplied(244, 244, 248, 244);
pub const TEXT2: Color32 = Color32::from_rgba_premultiplied(158, 158, 166, 166);
pub const TEXT3: Color32 = Color32::from_rgba_premultiplied(104, 104, 112, 112);
pub const ACCENT: Color32 = Color32::from_rgb(128, 116, 255);
pub const ACCENT_HI: Color32 = Color32::from_rgb(168, 150, 255);
pub const OK: Color32 = Color32::from_rgb(52, 211, 120);
pub const WARN: Color32 = Color32::from_rgb(255, 184, 64);
pub const BAD: Color32 = Color32::from_rgb(255, 86, 96);

// ---------------------------------------------------------------- type

#[derive(Clone, Copy)]
pub enum Face {
    Regular,
    Medium,
    Semibold,
    /// Display cut for large sizes: tighter spacing, finer details.
    Display,
}

impl Face {
    pub fn family(self) -> FontFamily {
        match self {
            Face::Regular => FontFamily::Proportional,
            Face::Medium => FontFamily::Name("medium".into()),
            Face::Semibold => FontFamily::Name("semibold".into()),
            Face::Display => FontFamily::Name("display".into()),
        }
    }
}

#[derive(Clone, Copy)]
pub struct Type {
    pub size: f32,
    pub face: Face,
    pub color: Color32,
}

impl Type {
    pub const fn color(self, color: Color32) -> Self {
        Self { color, ..self }
    }
}

pub const TITLE: Type = Type { size: 22.0, face: Face::Display, color: TEXT };
pub const HEADLINE: Type = Type { size: 17.0, face: Face::Semibold, color: TEXT };
pub const SECTION: Type = Type { size: 15.0, face: Face::Semibold, color: TEXT };
pub const BODY: Type = Type { size: 14.0, face: Face::Regular, color: TEXT };
pub const VALUE: Type = Type { size: 14.0, face: Face::Medium, color: TEXT2 };
pub const CONTROL: Type = Type { size: 13.0, face: Face::Medium, color: TEXT };
pub const SUB: Type = Type { size: 13.0, face: Face::Regular, color: TEXT2 };
pub const STATUS: Type = Type { size: 12.0, face: Face::Medium, color: TEXT2 };
pub const FOOT: Type = Type { size: 11.0, face: Face::Regular, color: TEXT3 };

/// Size-specific tracking (Inter's dynamic metrics): tight for large text, slightly open for small.
pub fn tracking(size: f32) -> f32 {
    size * (-0.0223 + 0.185 * (-0.1745 * size).exp())
}

pub fn job(text: &str, t: Type) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.append(
        text,
        0.0,
        TextFormat {
            font_id: FontId::new(t.size, t.face.family()),
            color: t.color,
            extra_letter_spacing: tracking(t.size),
            ..Default::default()
        },
    );
    job
}

pub fn rich(text: impl Into<String>, t: Type) -> RichText {
    RichText::new(text).size(t.size).family(t.face.family()).color(t.color).extra_letter_spacing(tracking(t.size))
}

/// Paints text anchored at `pos` and returns where it landed.
pub fn text(p: &Painter, pos: Pos2, align: Align2, s: &str, t: Type) -> Rect {
    let galley = p.layout_job(job(s, t));
    let rect = align.anchor_size(pos, galley.size());
    p.galley(rect.min, galley, t.color);
    rect
}

pub fn text_width(ui: &Ui, s: &str, t: Type) -> f32 {
    ui.fonts(|f| f.layout_job(job(s, t))).size().x
}

pub fn section(ui: &mut Ui, title: &str) {
    ui.label(rich(title, SECTION));
}

// ---------------------------------------------------------------- colour

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

// ---------------------------------------------------------------- geometry

/// Rounded-rectangle outline, clockwise in screen space, without duplicate points.
pub fn rrect_path(rect: Rect, radius: f32) -> Vec<Pos2> {
    const SEG: usize = 10;
    let r = radius.min(rect.width() * 0.5).min(rect.height() * 0.5).max(0.0);
    let corners = [
        (pos2(rect.right() - r, rect.bottom() - r), 0.0),
        (pos2(rect.left() + r, rect.bottom() - r), FRAC_PI_2),
        (pos2(rect.left() + r, rect.top() + r), 2.0 * FRAC_PI_2),
        (pos2(rect.right() - r, rect.top() + r), 3.0 * FRAC_PI_2),
    ];
    let mut pts: Vec<Pos2> = Vec::with_capacity(4 * (SEG + 1));
    for (c, start) in corners {
        for i in 0..=SEG {
            let a = start + i as f32 / SEG as f32 * FRAC_PI_2;
            let pt = pos2(c.x + r * a.cos(), c.y + r * a.sin());
            if pts.last().map_or(true, |last| last.distance(pt) > 0.01) {
                pts.push(pt);
            }
        }
    }
    if pts.len() > 1 && pts[0].distance(pts[pts.len() - 1]) <= 0.01 {
        pts.pop();
    }
    pts
}

fn outward(a: Pos2, b: Pos2) -> Vec2 {
    let d = b - a;
    let len = d.length();
    if len < 1e-4 {
        Vec2::ZERO
    } else {
        vec2(d.y, -d.x) / len
    }
}

fn feather(p: &Painter) -> f32 {
    1.0 / p.ctx().pixels_per_point()
}

/// Anti-aliased convex fill: a fan plus a one-pixel feathered rim, the way egui feathers its own shapes.
fn shaded_mesh(pts: &[Pos2], center: Pos2, feather: f32, shade: impl Fn(Pos2) -> Color32) -> Mesh {
    let n = pts.len();
    let mut mesh = Mesh::default();
    mesh.colored_vertex(center, shade(center));
    for i in 0..n {
        let (prev, cur, next) = (pts[(i + n - 1) % n], pts[i], pts[(i + 1) % n]);
        let sum = outward(prev, cur) + outward(cur, next);
        let normal = if sum.length() < 1e-4 { Vec2::ZERO } else { sum.normalized() };
        let c = shade(cur);
        mesh.colored_vertex(cur - normal * feather * 0.5, c);
        mesh.colored_vertex(cur + normal * feather * 0.5, Color32::TRANSPARENT);
    }
    for i in 0..n as u32 {
        let j = (i + 1) % n as u32;
        let (ii, io, ji, jo) = (1 + 2 * i, 2 + 2 * i, 1 + 2 * j, 2 + 2 * j);
        mesh.add_triangle(0, ii, ji);
        mesh.add_triangle(ii, io, jo);
        mesh.add_triangle(ii, jo, ji);
    }
    mesh
}

fn gradient_mesh(rect: Rect, radius: f32, top: Color32, bottom: Color32, feather: f32) -> Mesh {
    let pts = rrect_path(rect, radius);
    let h = rect.height().max(1.0);
    shaded_mesh(&pts, rect.center(), feather, |pt| lerp_color(top, bottom, (pt.y - rect.top()) / h))
}

/// Vertical gradient inside a rounded rectangle, with smooth edges.
pub fn fill_gradient(p: &Painter, rect: Rect, radius: f32, top: Color32, bottom: Color32) {
    p.add(Shape::mesh(gradient_mesh(rect, radius, top, bottom, feather(p))));
}

/// Horizontal strip whose colour follows x, feathered top and bottom (used by the hue track).
pub fn fill_horizontal(p: &Painter, rect: Rect, steps: usize, color_at: impl Fn(f32) -> Color32) {
    let f = feather(p) * 0.5;
    let mut mesh = Mesh::default();
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let x = rect.left() + rect.width() * t;
        let c = color_at(t);
        mesh.colored_vertex(pos2(x, rect.top() - f), Color32::TRANSPARENT);
        mesh.colored_vertex(pos2(x, rect.top() + f), c);
        mesh.colored_vertex(pos2(x, rect.bottom() - f), c);
        mesh.colored_vertex(pos2(x, rect.bottom() + f), Color32::TRANSPARENT);
        if i > 0 {
            let b = 4 * i as u32;
            let a = b - 4;
            for k in 0..3 {
                mesh.add_triangle(a + k, a + k + 1, b + k);
                mesh.add_triangle(a + k + 1, b + k + 1, b + k);
            }
        }
    }
    p.add(Shape::mesh(mesh));
}

/// Soft radial glow fading to transparent.
pub fn radial(p: &Painter, center: Pos2, radius: f32, color: Color32) {
    const SEG: u32 = 40;
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

// ---------------------------------------------------------------- material

/// The glass material as shapes: soft shadow, faint white body, hairline border, light-catching top edge.
pub fn glass_shapes(rect: Rect, radius: f32, weight: f32, feather: f32) -> Vec<Shape> {
    let s = Shadow { offset: vec2(0.0, 10.0), blur: 30.0, spread: 0.0, color: Color32::from_black_alpha((36.0 * weight) as u8) };
    let body = gradient_mesh(
        rect,
        radius,
        Color32::from_white_alpha((19.0 * weight) as u8),
        Color32::from_white_alpha((6.0 * weight) as u8),
        feather,
    );
    let y = rect.top() + 0.75;
    vec![
        Shape::from(s.as_shape(rect, Rounding::same(radius))),
        Shape::mesh(body),
        Shape::rect_stroke(rect, Rounding::same(radius), Stroke::new(1.0_f32, Color32::from_white_alpha(20))),
        Shape::line_segment(
            [pos2(rect.left() + radius * 0.9, y), pos2(rect.right() - radius * 0.9, y)],
            Stroke::new(1.0_f32, Color32::from_white_alpha(44)),
        ),
    ]
}

pub fn glass(p: &Painter, rect: Rect, radius: f32, weight: f32) {
    p.extend(glass_shapes(rect, radius, weight, feather(p)));
}

/// Wraps content in a glass card.
pub fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let slot = ui.painter().add(Shape::Noop);
    let inner = egui::Frame::none().inner_margin(egui::Margin::same(16.0)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        add(ui)
    });
    // The slot was reserved before the content, so the card lands behind it.
    let f = feather(ui.painter());
    ui.painter().set(slot, Shape::Vec(glass_shapes(inner.response.rect, 20.0, 1.0, f)));
    inner.inner
}

// ---------------------------------------------------------------- glyphs

pub fn sun(p: &Painter, c: Pos2, size: f32, color: Color32) {
    p.circle_filled(c, size * 0.24, color);
    for i in 0..8 {
        let a = i as f32 / 8.0 * TAU;
        let d = vec2(a.cos(), a.sin());
        p.line_segment([c + d * size * 0.40, c + d * size * 0.54], Stroke::new(size * 0.1, color));
    }
}

/// The halo logo: a conic ring with a soft glow.
pub fn logo(p: &Painter, c: Pos2, r: f32) {
    radial(p, c, r * 2.2, Color32::from_rgba_unmultiplied(140, 110, 255, 60));
    const SEG: usize = 96;
    let mut mesh = Mesh::default();
    let (inner, outer) = (r * 0.86, r * 1.14);
    let f = feather(p) * 0.5;
    for i in 0..=SEG {
        let a = i as f32 / SEG as f32 * TAU;
        let col = crate::art::conic(a);
        let c32 = Color32::from_rgb((col[0] * 255.0) as u8, (col[1] * 255.0) as u8, (col[2] * 255.0) as u8);
        let d = vec2(a.cos(), a.sin());
        mesh.colored_vertex(c + d * (inner - f), Color32::TRANSPARENT);
        mesh.colored_vertex(c + d * (inner + f), c32);
        mesh.colored_vertex(c + d * (outer - f), c32);
        mesh.colored_vertex(c + d * (outer + f), Color32::TRANSPARENT);
        if i > 0 {
            let b = 4 * i as u32;
            let a0 = b - 4;
            for k in 0..3 {
                mesh.add_triangle(a0 + k, a0 + k + 1, b + k);
                mesh.add_triangle(a0 + k + 1, b + k + 1, b + k);
            }
        }
    }
    p.add(Shape::mesh(mesh));
}
