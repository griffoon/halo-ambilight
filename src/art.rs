//! The app icon, drawn procedurally: a glowing halo ring floating on a dark glass squircle.
//! Shared by the tray, the window and the packaging tool.

use std::f32::consts::TAU;

const SUPERSAMPLE: u32 = 4;
const RING_RADIUS: f32 = 0.50;

/// Straight (non-premultiplied) RGBA, row-major, `size * size * 4` bytes.
pub fn render(size: u32) -> Vec<u8> {
    let mut out = vec![0u8; (size * size * 4) as usize];
    let step = 2.0 / (size * SUPERSAMPLE) as f32;
    let samples = (SUPERSAMPLE * SUPERSAMPLE) as f32;
    for py in 0..size {
        for px in 0..size {
            let mut acc = [0f32; 4];
            for sy in 0..SUPERSAMPLE {
                for sx in 0..SUPERSAMPLE {
                    let u = ((px * SUPERSAMPLE + sx) as f32 + 0.5) * step - 1.0;
                    let v = ((py * SUPERSAMPLE + sy) as f32 + 0.5) * step - 1.0;
                    let [r, g, b, a] = shade(u, v);
                    acc = [acc[0] + r * a, acc[1] + g * a, acc[2] + b * a, acc[3] + a];
                }
            }
            let i = ((py * size + px) * 4) as usize;
            if acc[3] > 0.0 {
                out[i] = to8(acc[0] / acc[3]);
                out[i + 1] = to8(acc[1] / acc[3]);
                out[i + 2] = to8(acc[2] / acc[3]);
                out[i + 3] = to8(acc[3] / samples);
            }
        }
    }
    out
}

fn to8(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

/// Conic palette around the ring: cyan → violet → pink → amber → cyan.
pub fn conic(angle: f32) -> [f32; 3] {
    const STOPS: [[f32; 3]; 5] = [
        [0.32, 0.86, 1.00],
        [0.60, 0.45, 1.00],
        [1.00, 0.36, 0.78],
        [1.00, 0.68, 0.32],
        [0.32, 0.86, 1.00],
    ];
    let f = (angle / TAU + 0.75).rem_euclid(1.0) * 4.0;
    let i = (f.floor() as usize).min(3);
    mix(STOPS[i], STOPS[i + 1], f - i as f32)
}

fn shade(u: f32, v: f32) -> [f32; 4] {
    let extent = 0.94;
    let sq = (u.abs() / extent).powi(5) + (v.abs() / extent).powi(5);
    if sq > 1.0 {
        return [0.0; 4];
    }
    let down = (v + 1.0) * 0.5;
    let mut c = mix([0.15, 0.13, 0.26], [0.035, 0.035, 0.07], down);

    let r = (u * u + v * v).sqrt();
    let d = (r - RING_RADIUS).abs();
    let hue = conic(v.atan2(u));

    let glow = (-(d / 0.19).powi(2)).exp() * 0.55;
    c = [c[0] + hue[0] * glow, c[1] + hue[1] * glow, c[2] + hue[2] * glow];
    let core = smoothstep(0.075, 0.03, d);
    c = mix(c, mix(hue, [1.0, 1.0, 1.0], 0.3), core);

    // Inside the ring the glass reads a touch deeper.
    let inner = smoothstep(RING_RADIUS - 0.08, RING_RADIUS - 0.3, r) * 0.35;
    c = mix(c, [0.02, 0.02, 0.05], inner);

    // Specular sheen across the upper half, and a light-catching rim.
    let sheen = ((-v - 0.05) / 0.95).clamp(0.0, 1.0).powf(1.6) * 0.10;
    c = mix(c, [1.0, 1.0, 1.0], sheen);
    let rim = smoothstep(0.78, 1.0, sq) * 0.22 * (1.0 - down * 0.75);
    c = mix(c, [1.0, 1.0, 1.0], rim);

    [c[0], c[1], c[2], 1.0]
}
