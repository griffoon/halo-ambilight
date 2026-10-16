//! Live preview: a monitor with the strip's real colours glowing around it,
//! and the window backdrop tinted by the same light.

use super::paint::{self, TEXT2, TEXT3};
use crate::config::Layout;
use crate::effects::{self, Side, Slot};
use egui::{pos2, vec2, Align2, Color32, FontId, Painter, Pos2, Rect, Rounding, Sense, Stroke, Ui, Vec2};

const FALLBACK: [u8; 3] = [120, 96, 255];

fn edge_point(screen: Rect, slot: &Slot) -> (Pos2, Vec2) {
    let (cx, cy) = slot.center();
    let pos = pos2(screen.left() + cx * screen.width(), screen.top() + cy * screen.height());
    let normal = match slot.side {
        Side::Left => vec2(-1.0, 0.0),
        Side::Top => vec2(0.0, -1.0),
        Side::Right => vec2(1.0, 0.0),
        Side::Bottom => vec2(0.0, 1.0),
    };
    (pos, normal)
}

pub fn hero(ui: &mut Ui, leds: &[[u8; 3]], layout: &Layout, caption: &str) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 206.0), Sense::hover());
    let p = ui.painter();
    let screen = Rect::from_center_size(rect.center() - vec2(0.0, 14.0), vec2(252.0, 144.0));
    let slots = effects::slots(layout);

    for (slot, c) in slots.iter().zip(leds) {
        let (pos, n) = edge_point(screen, slot);
        paint::radial(p, pos + n * 12.0, 34.0, paint::with_alpha(*c, 115));
    }

    // Stand
    let neck = Rect::from_center_size(pos2(screen.center().x, screen.bottom() + 12.0), vec2(26.0, 20.0));
    paint::fill_gradient(p, neck, 3.0, Color32::from_white_alpha(26), Color32::from_white_alpha(10));
    let foot = Rect::from_center_size(pos2(screen.center().x, screen.bottom() + 24.0), vec2(96.0, 7.0));
    paint::glass(p, foot, 3.5, 1.4);

    // Panel
    paint::shadow(p, screen, 12.0, 10.0, 26.0, 120);
    paint::fill_gradient(p, screen, 12.0, Color32::from_rgb(24, 24, 32), Color32::from_rgb(9, 9, 13));
    p.rect_stroke(screen, Rounding::same(12.0), Stroke::new(1.0_f32, Color32::from_white_alpha(30)));
    p.line_segment(
        [pos2(screen.left() + 14.0, screen.top() + 1.0), pos2(screen.right() - 14.0, screen.top() + 1.0)],
        Stroke::new(1.0_f32, Color32::from_white_alpha(60)),
    );
    p.text(screen.center(), Align2::CENTER_CENTER, caption, FontId::proportional(13.0), TEXT2);

    // LED chips sit on the bezel.
    for (slot, c) in slots.iter().zip(leds) {
        let (pos, n) = edge_point(screen, slot);
        p.circle_filled(pos + n * 3.5, 2.1, paint::rgb(*c));
    }
    if leds.is_empty() {
        p.text(pos2(screen.center().x, screen.bottom() + 44.0), Align2::CENTER_CENTER, "нет данных от ленты", FontId::proportional(11.5), TEXT3);
    }
}

fn average(leds: &[[u8; 3]], slots: &[Slot], side: Side) -> [u8; 3] {
    let (mut sum, mut n) = ([0u32; 3], 0u32);
    for (s, c) in slots.iter().zip(leds) {
        if s.side == side {
            (0..3).for_each(|k| sum[k] += c[k] as u32);
            n += 1;
        }
    }
    if n == 0 {
        return FALLBACK;
    }
    let avg = sum.map(|v| (v / n) as u8);
    // Near-black (lights off or a dark scene) falls back to the house violet, dimmed.
    if avg.iter().map(|v| *v as u32).sum::<u32>() < 30 {
        FALLBACK.map(|v| v / 3)
    } else {
        avg
    }
}

/// Dark glass tint over the system acrylic, with three soft blobs of the strip's light.
pub fn backdrop(p: &Painter, rect: Rect, leds: &[[u8; 3]], layout: &Layout) {
    p.rect_filled(rect, 0.0, Color32::from_rgba_premultiplied(9, 9, 15, 190));
    let slots = effects::slots(layout);
    let left = average(leds, &slots, Side::Left);
    let top = average(leds, &slots, Side::Top);
    let right = average(leds, &slots, Side::Right);
    paint::radial(p, rect.left_top() + vec2(40.0, 150.0), 300.0, paint::with_alpha(left, 58));
    paint::radial(p, rect.right_top() + vec2(-40.0, 170.0), 300.0, paint::with_alpha(right, 58));
    paint::radial(p, pos2(rect.center().x, rect.top() + 40.0), 260.0, paint::with_alpha(top, 46));
}
