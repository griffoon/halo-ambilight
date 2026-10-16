use crate::config::{Config, Layout, Mode};
use std::f32::consts::TAU;

/// Linear RGB, 0..1.
pub type Rgb = [f32; 3];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Left,
    Top,
    Right,
    Bottom,
}

/// One LED's span along a screen edge, in normalised screen coordinates (y grows downwards).
#[derive(Clone, Copy, Debug)]
pub struct Slot {
    pub side: Side,
    pub a: f32,
    pub b: f32,
}

impl Slot {
    pub fn center(&self) -> (f32, f32) {
        let m = (self.a + self.b) * 0.5;
        match self.side {
            Side::Left => (0.0, m),
            Side::Top => (m, 0.0),
            Side::Right => (1.0, m),
            Side::Bottom => (m, 1.0),
        }
    }
}

/// LEDs in strip order: clockwise from the bottom-left corner.
pub fn slots(layout: &Layout) -> Vec<Slot> {
    let run = |side: Side, n: u16, rising: bool| {
        let n = n as usize;
        (0..n).map(move |i| {
            let (a, b) = (i as f32 / n as f32, (i + 1) as f32 / n as f32);
            if rising {
                Slot { side, a, b }
            } else {
                Slot { side, a: 1.0 - b, b: 1.0 - a }
            }
        })
    };
    let mut all: Vec<Slot> = run(Side::Left, layout.left, false)
        .chain(run(Side::Top, layout.top, true))
        .chain(run(Side::Right, layout.right, true))
        .chain(run(Side::Bottom, layout.bottom, false))
        .collect();
    if layout.reverse {
        all.reverse();
    }
    all
}

pub fn hsv(h: f32, s: f32, v: f32) -> Rgb {
    let h = h.rem_euclid(1.0) * 6.0;
    let i = h.floor();
    let f = h - i;
    let (p, q, t) = (v * (1.0 - s), v * (1.0 - s * f), v * (1.0 - s * (1.0 - f)));
    match i as i32 {
        0 => [v, t, p],
        1 => [q, v, p],
        2 => [p, v, t],
        3 => [p, q, v],
        4 => [t, p, v],
        _ => [v, p, q],
    }
}

pub fn from8(c: [u8; 3]) -> Rgb {
    [c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0]
}

pub fn to8(c: Rgb) -> [u8; 3] {
    let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
    [q(c[0]), q(c[1]), q(c[2])]
}

pub fn scale(c: Rgb, k: f32) -> Rgb {
    [c[0] * k, c[1] * k, c[2] * k]
}

pub fn saturate(c: Rgb, amount: f32) -> Rgb {
    let luma = 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
    c.map(|v| (luma + (v - luma) * amount).clamp(0.0, 1.0))
}

/// Fills `out` for every mode except screen sync. `t` is effect time, already scaled by speed.
pub fn render(cfg: &Config, t: f32, out: &mut [Rgb]) {
    let n = out.len().max(1) as f32;
    let base = from8(cfg.color);
    match cfg.mode {
        Mode::Screen | Mode::Solid => out.fill(base),
        Mode::Breathe => {
            // Eased sine, never fully dark so it reads as breathing rather than blinking.
            let phase = 0.5 - 0.5 * (t * TAU * 0.22).cos();
            out.fill(scale(base, 0.12 + 0.88 * phase * phase));
        }
        Mode::Rainbow => {
            for (i, c) in out.iter_mut().enumerate() {
                *c = hsv(i as f32 / n + t * 0.08, 1.0, 1.0);
            }
        }
        Mode::Aurora => {
            for (i, c) in out.iter_mut().enumerate() {
                let x = i as f32 / n * TAU;
                let wave = (x * 1.3 + t * 0.35).sin() * 0.5 + (x * 2.7 - t * 0.21).sin() * 0.3;
                let hue = 0.47 + wave * 0.16; // teal → violet → magenta band
                let glow = 0.55 + 0.45 * ((x * 1.9 - t * 0.5).sin() * 0.5 + 0.5);
                *c = hsv(hue, 0.85, glow);
            }
        }
    }
}

/// Pixel rectangles of each LED inside the downsampled frame.
#[derive(Default)]
pub struct Zones {
    key: Option<([u32; 4], u32, Layout)>,
    rects: Vec<[u32; 4]>,
}

impl Zones {
    /// `area` is the visible picture `[x0, y0, x1, y1]` inside the frame (smaller than the frame
    /// when black bars are cropped away).
    pub fn ensure(&mut self, area: [u32; 4], layout: &Layout, depth: f32) {
        let depth_key = (depth * 1000.0) as u32;
        let key = Some((area, depth_key, *layout));
        if self.key == key {
            return;
        }
        self.key = key;
        let [ax, ay, ax1, ay1] = area;
        let (w, h) = ((ax1 - ax).max(1), (ay1 - ay).max(1));
        let d = ((depth * h as f32).round() as u32).clamp(1, (w.min(h) / 2).max(1));
        let span = |a: f32, b: f32, len: u32| {
            let lo = ((a * len as f32).floor() as u32).min(len - 1);
            let hi = ((b * len as f32).ceil() as u32).clamp(lo + 1, len);
            (lo, hi)
        };
        self.rects = slots(layout)
            .iter()
            .map(|s| match s.side {
                Side::Left => {
                    let (y0, y1) = span(s.a, s.b, h);
                    [0, y0, d, y1]
                }
                Side::Right => {
                    let (y0, y1) = span(s.a, s.b, h);
                    [w - d, y0, w, y1]
                }
                Side::Top => {
                    let (x0, x1) = span(s.a, s.b, w);
                    [x0, 0, x1, d]
                }
                Side::Bottom => {
                    let (x0, x1) = span(s.a, s.b, w);
                    [x0, h - d, x1, h]
                }
            })
            .map(|[x0, y0, x1, y1]| [x0 + ax, y0 + ay, x1 + ax, y1 + ay])
            .collect();
    }

    /// Averages each zone of a BGRA frame into `out`.
    pub fn sample(&self, bgra: &[u8], w: u32, out: &mut [Rgb]) {
        for (rect, dst) in self.rects.iter().zip(out.iter_mut()) {
            let [x0, y0, x1, y1] = *rect;
            let mut sum = [0u32; 3];
            for y in y0..y1 {
                let row = &bgra[((y * w + x0) * 4) as usize..((y * w + x1) * 4) as usize];
                for px in row.chunks_exact(4) {
                    sum[0] += px[2] as u32;
                    sum[1] += px[1] as u32;
                    sum[2] += px[0] as u32;
                }
            }
            let count = ((x1 - x0) * (y1 - y0)).max(1) as f32 * 255.0;
            *dst = [sum[0] as f32 / count, sum[1] as f32 / count, sum[2] as f32 / count];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_layout_starts_bottom_left_and_runs_clockwise() {
        let s = slots(&Layout::default());
        assert_eq!(s.len(), 54);
        assert_eq!(s[0].side, Side::Left);
        assert!(s[0].center().1 > 0.9);
        assert_eq!(s[14].side, Side::Top);
        assert!(s[14].center().0 < 0.05);
        assert_eq!(s[53].side, Side::Right);
        assert!(s[53].center().1 > 0.9);
    }

    #[test]
    fn reverse_flips_the_order() {
        let layout = Layout { reverse: true, ..Layout::default() };
        assert_eq!(slots(&layout)[0].side, Side::Right);
    }

    #[test]
    fn zones_sample_solid_frame() {
        let (w, h) = (16, 9);
        let frame: Vec<u8> = (0..w * h).flat_map(|_| [0u8, 128, 255, 255]).collect();
        let layout = Layout::default();
        let mut zones = Zones::default();
        zones.ensure([0, 0, w, h], &layout, 0.2);
        let mut out = vec![[0.0; 3]; layout.total()];
        zones.sample(&frame, w, &mut out);
        assert!(out.iter().all(|c| (c[0] - 1.0).abs() < 1e-3 && c[2].abs() < 1e-3));
    }
}
