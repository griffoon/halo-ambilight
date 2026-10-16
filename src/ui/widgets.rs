//! Controls. Every moving part is a spring, so motion always starts from where it is now
//! and can be re-targeted mid-flight. Drags track the pointer 1:1 relative to where it was grabbed.

use super::paint::{self, ACCENT, ACCENT_HI, TEXT, TEXT2};
use egui::epaint::Mesh;
use egui::{pos2, vec2, Align2, Color32, Context, FontFamily, FontId, Id, Rect, Response, Rounding, Sense, Shape, Stroke, Ui};
use std::hash::Hash;
use std::ops::RangeInclusive;

#[derive(Clone, Copy)]
struct SpringState {
    pos: f32,
    vel: f32,
    frame: u64,
}

pub struct Motion {
    pub pos: f32,
    pub vel: f32,
}

/// Damped spring towards `target`. `response` ≈ seconds to get there; `damping` 1.0 = no overshoot.
pub fn spring(ctx: &Context, id: Id, target: f32, response: f32, damping: f32) -> Motion {
    spring_from(ctx, id, target, target, response, damping)
}

pub fn spring_from(ctx: &Context, id: Id, initial: f32, target: f32, response: f32, damping: f32) -> Motion {
    let frame = ctx.cumulative_pass_nr();
    let mut s = ctx.data_mut(|d| *d.get_temp_mut_or_insert_with(id, || SpringState { pos: initial, vel: 0.0, frame: u64::MAX }));
    if s.frame != frame {
        s.frame = frame;
        let dt = ctx.input(|i| i.stable_dt).clamp(1.0 / 240.0, 1.0 / 20.0);
        let omega = std::f32::consts::TAU / response.max(0.02);
        let (k, c) = (omega * omega, 2.0 * damping * omega);
        let steps = ((dt * 240.0).ceil() as usize).clamp(1, 12);
        let h = dt / steps as f32;
        for _ in 0..steps {
            let a = -k * (s.pos - target) - c * s.vel;
            s.vel += a * h;
            s.pos += s.vel * h;
        }
        if (s.pos - target).abs() < 1e-4 && s.vel.abs() < 1e-3 {
            s.pos = target;
            s.vel = 0.0;
        } else {
            ctx.request_repaint();
        }
        ctx.data_mut(|d| d.insert_temp(id, s));
    }
    Motion { pos: s.pos, vel: s.vel }
}

/// Pins a spring to a value (used while the pointer drives it directly).
pub fn spring_set(ctx: &Context, id: Id, value: f32) {
    ctx.data_mut(|d| d.insert_temp(id, SpringState { pos: value, vel: 0.0, frame: ctx.cumulative_pass_nr() }));
}

fn press(ui: &Ui, id: Id, resp: &Response) -> f32 {
    let down = resp.is_pointer_button_down_on();
    // Pressing reacts fast, releasing settles a little slower.
    spring(ui.ctx(), id.with("press"), if down { 1.0 } else { 0.0 }, if down { 0.12 } else { 0.28 }, 1.0).pos
}

pub fn toggle(ui: &mut Ui, salt: impl Hash, on: &mut bool) -> Response {
    let id = ui.make_persistent_id(salt);
    let (rect, _) = ui.allocate_exact_size(vec2(52.0, 32.0), Sense::hover());
    let mut resp = ui.interact(rect, id, Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let t = spring(ui.ctx(), id.with("knob"), if *on { 1.0 } else { 0.0 }, 0.34, 0.8).pos;
    let p = press(ui, id, &resp);
    let painter = ui.painter();
    let track = paint::lerp_color(Color32::from_white_alpha(30), ACCENT, t);
    painter.rect_filled(rect, Rounding::same(16.0), track);
    painter.rect_stroke(rect, Rounding::same(16.0), Stroke::new(1.0_f32, Color32::from_white_alpha(18)));
    let kw = 26.0 + 7.0 * p;
    let x = rect.left() + 3.0 + (rect.width() - 6.0 - kw) * t;
    let knob = Rect::from_min_size(pos2(x, rect.top() + 3.0), vec2(kw, 26.0));
    paint::shadow(painter, knob, 13.0, 2.0, 6.0, 60);
    painter.rect_filled(knob, Rounding::same(13.0), Color32::WHITE);
    resp
}

/// Large brightness bar: drag anywhere to adjust relative to the grab point, tap to jump.
pub fn level(ui: &mut Ui, salt: impl Hash, value: &mut f32) -> Response {
    let id = ui.make_persistent_id(salt);
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 44.0), Sense::hover());
    let mut resp = ui.interact(rect, id, Sense::click_and_drag());
    let shown_id = id.with("shown");
    if resp.dragged() {
        *value = (*value + resp.drag_delta().x / rect.width()).clamp(0.0, 1.0);
        spring_set(ui.ctx(), shown_id, *value);
        resp.mark_changed();
    } else if resp.clicked() {
        if let Some(pos) = resp.interact_pointer_pos() {
            *value = ((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
            resp.mark_changed();
        }
    }
    let shown = spring(ui.ctx(), shown_id, *value, 0.3, 1.0).pos.clamp(0.0, 1.0);
    let p = press(ui, id, &resp);
    let r = rect.expand2(vec2(p * 2.0, p * 2.0));
    let radius = r.height() * 0.5;
    let painter = ui.painter();

    painter.rect_filled(r, Rounding::same(radius), Color32::from_white_alpha(22));
    let fill_clip = Rect::from_min_max(r.min, pos2(r.left() + r.width() * shown, r.bottom()));
    let fill = painter.with_clip_rect(fill_clip.intersect(ui.clip_rect()));
    paint::fill_gradient(&fill, r, radius, Color32::from_white_alpha(236), Color32::from_white_alpha(206));
    painter.rect_stroke(r, Rounding::same(radius), Stroke::new(1.0_f32, Color32::from_white_alpha(20)));

    let icon_pos = pos2(r.left() + 22.0, r.center().y);
    let icon_col = if fill_clip.right() > icon_pos.x + 8.0 { Color32::from_black_alpha(150) } else { TEXT2 };
    paint::sun(painter, icon_pos, 18.0, icon_col);
    let label = format!("{:.0}%", *value * 100.0);
    let label_x = r.right() - 16.0;
    let label_col = if fill_clip.right() > label_x - 20.0 { Color32::from_black_alpha(170) } else { TEXT };
    painter.text(pos2(label_x, r.center().y), Align2::RIGHT_CENTER, label, FontId::new(14.0, paint::semibold()), label_col);
    resp
}

/// Compact labelled slider with a knob.
pub fn slider(
    ui: &mut Ui,
    salt: impl Hash,
    label: &str,
    value: &mut f32,
    range: RangeInclusive<f32>,
    fmt: impl Fn(f32) -> String,
) -> Response {
    let id = ui.make_persistent_id(salt);
    let (lo, hi) = (*range.start(), *range.end());
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 46.0), Sense::hover());
    let track = Rect::from_min_max(pos2(rect.left() + 2.0, rect.top() + 29.0), pos2(rect.right() - 2.0, rect.top() + 35.0));
    let hit = Rect::from_min_max(pos2(rect.left(), rect.top() + 18.0), rect.max);
    let mut resp = ui.interact(hit, id, Sense::click_and_drag());
    let shown_id = id.with("shown");
    let norm = |v: f32| ((v - lo) / (hi - lo)).clamp(0.0, 1.0);
    if resp.dragged() {
        *value = (*value + resp.drag_delta().x / track.width() * (hi - lo)).clamp(lo, hi);
        spring_set(ui.ctx(), shown_id, norm(*value));
        resp.mark_changed();
    } else if resp.clicked() {
        if let Some(pos) = resp.interact_pointer_pos() {
            *value = lo + ((pos.x - track.left()) / track.width()).clamp(0.0, 1.0) * (hi - lo);
            resp.mark_changed();
        }
    }
    let t = spring(ui.ctx(), shown_id, norm(*value), 0.28, 1.0).pos.clamp(0.0, 1.0);
    let p = press(ui, id, &resp);
    let painter = ui.painter();

    painter.text(rect.left_top() + vec2(0.0, 2.0), Align2::LEFT_TOP, label, FontId::proportional(13.5), TEXT2);
    painter.text(rect.right_top() + vec2(0.0, 2.0), Align2::RIGHT_TOP, fmt(*value), FontId::new(13.5, paint::semibold()), TEXT);

    painter.rect_filled(track, Rounding::same(3.0), Color32::from_white_alpha(26));
    let knob_x = track.left() + track.width() * t;
    let filled = Rect::from_min_max(track.min, pos2(knob_x, track.bottom()));
    paint::fill_gradient(painter, filled, 3.0, ACCENT_HI, ACCENT);
    let r = 10.0 * (1.0 + 0.14 * p);
    let c = pos2(knob_x, track.center().y);
    paint::shadow(painter, Rect::from_center_size(c, vec2(r * 2.0, r * 2.0)), r, 2.0, 8.0, 70);
    painter.circle_filled(c, r, Color32::WHITE);
    resp
}

/// Segmented control with a liquid pill: it overshoots a touch and stretches with its speed.
pub fn segmented(ui: &mut Ui, salt: impl Hash, labels: &[String], selected: &mut usize, height: f32) -> bool {
    let id = ui.make_persistent_id(salt);
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    let n = labels.len().max(1);
    let cell_w = rect.width() / n as f32;
    let mut changed = false;
    let cells: Vec<Rect> =
        (0..n).map(|i| Rect::from_min_size(pos2(rect.left() + cell_w * i as f32, rect.top()), vec2(cell_w, height))).collect();
    let responses: Vec<Response> = cells.iter().enumerate().map(|(i, c)| ui.interact(*c, id.with(i), Sense::click())).collect();
    for (i, r) in responses.iter().enumerate() {
        if r.clicked() && *selected != i {
            *selected = i;
            changed = true;
        }
    }

    let m = spring(ui.ctx(), id.with("pill"), *selected as f32, 0.4, 0.78);
    let stretch = (m.vel.abs() * 0.035).min(0.28) * cell_w;
    let painter = ui.painter();
    let radius = height * 0.5;
    painter.rect_filled(rect, Rounding::same(radius), Color32::from_black_alpha(46));
    painter.rect_stroke(rect, Rounding::same(radius), Stroke::new(1.0_f32, Color32::from_white_alpha(14)));

    let pill = Rect::from_min_size(pos2(rect.left() + m.pos * cell_w + 3.0 - stretch * 0.5, rect.top() + 3.0), vec2(cell_w - 6.0 + stretch, height - 6.0));
    paint::shadow(painter, pill, radius - 3.0, 3.0, 10.0, 70);
    paint::fill_gradient(painter, pill, radius - 3.0, Color32::from_white_alpha(62), Color32::from_white_alpha(34));
    painter.rect_stroke(pill, Rounding::same(radius - 3.0), Stroke::new(1.0_f32, Color32::from_white_alpha(40)));
    painter.line_segment(
        [pos2(pill.left() + radius, pill.top() + 0.8), pos2(pill.right() - radius, pill.top() + 0.8)],
        Stroke::new(1.0_f32, Color32::from_white_alpha(90)),
    );

    for (i, (cell, r)) in cells.iter().zip(&responses).enumerate() {
        let near = (1.0 - (m.pos - i as f32).abs()).clamp(0.0, 1.0);
        let dim = if r.is_pointer_button_down_on() { 0.7 } else { 1.0 };
        let color = paint::lerp_color(TEXT2, TEXT, near).gamma_multiply(dim);
        let family = if near > 0.5 { paint::semibold() } else { FontFamily::Proportional };
        painter.text(cell.center(), Align2::CENTER_CENTER, &labels[i], FontId::new(13.5, family), color);
    }
    changed
}

pub fn swatches(ui: &mut Ui, salt: impl Hash, colors: &[[u8; 3]], current: &mut [u8; 3]) -> bool {
    let id = ui.make_persistent_id(salt);
    let n = colors.len().max(1);
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 40.0), Sense::hover());
    let step = rect.width() / n as f32;
    let mut changed = false;
    for (i, c) in colors.iter().enumerate() {
        let center = pos2(rect.left() + step * (i as f32 + 0.5), rect.center().y);
        let hit = Rect::from_center_size(center, vec2(step, 40.0));
        let resp = ui.interact(hit, id.with(i), Sense::click());
        if resp.clicked() {
            *current = *c;
            changed = true;
        }
        let selected = *current == *c;
        let grow = spring(ui.ctx(), id.with(("s", i)), if selected { 1.0 } else if resp.hovered() { 0.4 } else { 0.0 }, 0.3, 0.75).pos;
        let p = press(ui, id.with(i), &resp);
        let r = 12.0 * (1.0 + 0.1 * grow - 0.08 * p);
        let painter = ui.painter();
        paint::radial(painter, center, r * 2.2, paint::with_alpha(*c, (70.0 * grow) as u8));
        painter.circle_filled(center, r, paint::rgb(*c));
        painter.circle_stroke(center, r, Stroke::new(1.0_f32, Color32::from_white_alpha(40)));
        if grow > 0.01 {
            painter.circle_stroke(center, r + 4.0, Stroke::new(2.0_f32, Color32::from_white_alpha((230.0 * grow.min(1.0)) as u8)));
        }
    }
    changed
}

/// Rainbow track for picking a hue, 0..1.
pub fn hue(ui: &mut Ui, salt: impl Hash, value: &mut f32, preview: [u8; 3]) -> Response {
    let id = ui.make_persistent_id(salt);
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::hover());
    let track = rect.shrink2(vec2(11.0, 8.0));
    let mut resp = ui.interact(rect, id, Sense::click_and_drag());
    if resp.dragged() || resp.clicked() {
        if let Some(pos) = resp.interact_pointer_pos() {
            *value = ((pos.x - track.left()) / track.width()).clamp(0.0, 1.0);
            resp.mark_changed();
        }
    }
    let painter = ui.painter();
    const SEG: usize = 36;
    // Hue wraps red → red, so both rounded caps are plain red discs.
    let cap = track.height() * 0.5;
    painter.circle_filled(pos2(track.left(), track.center().y), cap, Color32::from_rgb(255, 0, 0));
    painter.circle_filled(pos2(track.right(), track.center().y), cap, Color32::from_rgb(255, 0, 0));
    let mut mesh = Mesh::default();
    for i in 0..=SEG {
        let t = i as f32 / SEG as f32;
        let c = crate::effects::to8(crate::effects::hsv(t, 1.0, 1.0));
        let x = track.left() + track.width() * t;
        mesh.colored_vertex(pos2(x, track.top()), paint::rgb(c));
        mesh.colored_vertex(pos2(x, track.bottom()), paint::rgb(c));
        if i > 0 {
            let b = (i as u32) * 2;
            mesh.add_triangle(b - 2, b - 1, b);
            mesh.add_triangle(b - 1, b + 1, b);
        }
    }
    painter.add(Shape::mesh(mesh));
    let p = press(ui, id, &resp);
    let c = pos2(track.left() + track.width() * *value, track.center().y);
    let r = 11.0 * (1.0 + 0.12 * p);
    paint::shadow(painter, Rect::from_center_size(c, vec2(r * 2.0, r * 2.0)), r, 2.0, 8.0, 80);
    painter.circle_filled(c, r, Color32::WHITE);
    painter.circle_filled(c, r - 3.5, paint::rgb(preview));
    resp
}

pub fn stepper(ui: &mut Ui, salt: impl Hash, label: &str, value: &mut u16, max: u16) -> bool {
    let id = ui.make_persistent_id(salt);
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 34.0), Sense::hover());
    let painter = ui.painter().clone();
    painter.text(pos2(rect.left(), rect.center().y), Align2::LEFT_CENTER, label, FontId::proportional(14.0), TEXT);
    let plus = Rect::from_center_size(pos2(rect.right() - 15.0, rect.center().y), vec2(30.0, 30.0));
    let minus = Rect::from_center_size(pos2(rect.right() - 95.0, rect.center().y), vec2(30.0, 30.0));
    let mut changed = false;
    for (key, r, delta) in [("-", minus, -1i32), ("+", plus, 1)] {
        let resp = ui.interact(r, id.with(key), Sense::click());
        if resp.clicked() {
            let next = (*value as i32 + delta).clamp(0, max as i32) as u16;
            changed |= next != *value;
            *value = next;
        }
        let p = press(ui, id.with(key), &resp);
        let rr = r.shrink(p * 2.0);
        let fill = if resp.hovered() { 34 } else { 22 };
        painter.circle_filled(rr.center(), rr.width() * 0.5, Color32::from_white_alpha(fill));
        painter.circle_stroke(rr.center(), rr.width() * 0.5, Stroke::new(1.0_f32, Color32::from_white_alpha(22)));
        let c = rr.center();
        let s = Stroke::new(1.8_f32, TEXT);
        painter.line_segment([c - vec2(5.0, 0.0), c + vec2(5.0, 0.0)], s);
        if delta > 0 {
            painter.line_segment([c - vec2(0.0, 5.0), c + vec2(0.0, 5.0)], s);
        }
    }
    painter.text(
        pos2((minus.center().x + plus.center().x) * 0.5, rect.center().y),
        Align2::CENTER_CENTER,
        value.to_string(),
        FontId::new(15.0, paint::semibold()),
        TEXT,
    );
    changed
}

/// Full-width row with a label on the left and a toggle on the right.
pub fn toggle_row(ui: &mut Ui, salt: impl Hash, label: &str, on: &mut bool) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.set_min_height(34.0);
        ui.label(egui::RichText::new(label).size(14.0).color(TEXT));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            changed = toggle(ui, salt, on).changed();
        });
    });
    changed
}

pub fn pill_button(ui: &mut Ui, salt: impl Hash, label: &str) -> Response {
    let id = ui.make_persistent_id(salt);
    let galley = paint::galley(ui, label, 13.5, paint::semibold(), TEXT, 0.0);
    let size = vec2(galley.size().x + 32.0, 34.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let resp = ui.interact(rect, id, Sense::click());
    let p = press(ui, id, &resp);
    let r = rect.shrink(p * 1.5);
    let painter = ui.painter();
    let base = if resp.hovered() { 40 } else { 28 };
    paint::fill_gradient(painter, r, 17.0, Color32::from_white_alpha(base + 10), Color32::from_white_alpha(base - 8));
    painter.rect_stroke(r, Rounding::same(17.0), Stroke::new(1.0_f32, Color32::from_white_alpha(32)));
    painter.galley(r.center() - galley.size() * 0.5, galley, TEXT);
    resp
}

pub enum Glyph {
    Close,
    Minimize,
}

pub fn round_icon(ui: &mut Ui, salt: impl Hash, glyph: Glyph) -> Response {
    let id = ui.make_persistent_id(salt);
    let (rect, _) = ui.allocate_exact_size(vec2(30.0, 30.0), Sense::hover());
    let resp = ui.interact(rect, id, Sense::click());
    let p = press(ui, id, &resp);
    let hover = spring(ui.ctx(), id.with("hover"), if resp.hovered() { 1.0 } else { 0.0 }, 0.2, 1.0).pos;
    let c = rect.center();
    let radius = 14.0 - p * 1.5;
    let painter = ui.painter();
    let fill = match glyph {
        Glyph::Close => paint::lerp_color(Color32::from_white_alpha(22), Color32::from_rgb(232, 72, 84), hover),
        Glyph::Minimize => paint::lerp_color(Color32::from_white_alpha(22), Color32::from_white_alpha(52), hover),
    };
    painter.circle_filled(c, radius, fill);
    painter.circle_stroke(c, radius, Stroke::new(1.0_f32, Color32::from_white_alpha(24)));
    let s = Stroke::new(1.6_f32, TEXT);
    match glyph {
        Glyph::Close => {
            painter.line_segment([c + vec2(-4.0, -4.0), c + vec2(4.0, 4.0)], s);
            painter.line_segment([c + vec2(4.0, -4.0), c + vec2(-4.0, 4.0)], s);
        }
        Glyph::Minimize => {
            painter.line_segment([c + vec2(-4.5, 0.0), c + vec2(4.5, 0.0)], s);
        }
    }
    resp
}

/// Clickable section header with a rotating chevron. Returns the animated openness, 0..1.
pub fn disclosure(ui: &mut Ui, salt: impl Hash, title: &str, open: &mut bool) -> f32 {
    let id = ui.make_persistent_id(salt);
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 28.0), Sense::hover());
    let resp = ui.interact(rect, id, Sense::click());
    if resp.clicked() {
        *open = !*open;
    }
    let t = spring(ui.ctx(), id.with("open"), if *open { 1.0 } else { 0.0 }, 0.38, 1.0).pos;
    let painter = ui.painter();
    painter.text(pos2(rect.left(), rect.center().y), Align2::LEFT_CENTER, title, FontId::new(15.0, paint::semibold()), TEXT);
    let c = pos2(rect.right() - 8.0, rect.center().y);
    let a = t * std::f32::consts::FRAC_PI_2;
    let rot = |v: egui::Vec2| vec2(v.x * a.cos() - v.y * a.sin(), v.x * a.sin() + v.y * a.cos());
    let s = Stroke::new(1.8_f32, TEXT2);
    painter.line_segment([c + rot(vec2(-2.5, -5.0)), c + rot(vec2(2.5, 0.0))], s);
    painter.line_segment([c + rot(vec2(2.5, 0.0)), c + rot(vec2(-2.5, 5.0))], s);
    t
}
