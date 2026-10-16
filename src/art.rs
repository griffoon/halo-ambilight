//! The app icon, drawn procedurally in the Windows 11 app-icon style: a front-facing object with
//! soft gradients and no background plate. Here, a light monitor with a dark screen and the
//! ambient glow it throws on the wall behind it.
//! Shared by the tray and the packaging tool.

const SUPERSAMPLE: u32 = 4;

// Geometry in normalised icon space, x and y in -1..1 (y grows downwards).
const SCREEN_HALF: (f32, f32) = (0.74, 0.46);
const SCREEN_CENTER_Y: f32 = -0.12;
const SCREEN_RADIUS: f32 = 0.10;
const BEZEL: f32 = 0.065;

type Rgba = [f32; 4];

/// Straight (non-premultiplied) RGBA, row-major, `size * size * 4` bytes.
pub fn render(size: u32) -> Vec<u8> {
    let mut out = vec![0u8; (size * size * 4) as usize];
    let step = 2.0 / (size * SUPERSAMPLE) as f32;
    let samples = (SUPERSAMPLE * SUPERSAMPLE) as f32;
    // Small sizes drop the glow's outer reach so the monitor stays readable at 16 px.
    let glow_reach = if size <= 24 { 0.10 } else { 0.17 };
    for py in 0..size {
        for px in 0..size {
            let mut acc = [0f32; 4];
            for sy in 0..SUPERSAMPLE {
                for sx in 0..SUPERSAMPLE {
                    let u = ((px * SUPERSAMPLE + sx) as f32 + 0.5) * step - 1.0;
                    let v = ((py * SUPERSAMPLE + sy) as f32 + 0.5) * step - 1.0;
                    let [r, g, b, a] = shade(u, v, glow_reach);
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

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0);
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

/// Signed distance to a rounded rectangle centred at the origin.
fn rounded_rect(x: f32, y: f32, half_w: f32, half_h: f32, r: f32) -> f32 {
    let qx = x.abs() - half_w + r;
    let qy = y.abs() - half_h + r;
    let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
    outside + qx.max(qy).min(0.0) - r
}

/// Paints `top` over `bottom` (straight alpha).
fn over(bottom: Rgba, top: Rgba) -> Rgba {
    let a = top[3] + bottom[3] * (1.0 - top[3]);
    if a <= 0.0 {
        return [0.0; 4];
    }
    let c = |k: usize| (top[k] * top[3] + bottom[k] * bottom[3] * (1.0 - top[3])) / a;
    [c(0), c(1), c(2), a]
}

fn solid(c: [f32; 3], inside: bool) -> Rgba {
    if inside {
        [c[0], c[1], c[2], 1.0]
    } else {
        [0.0; 4]
    }
}

/// Glow colour sweeps blue → violet → pink from left to right, like light spilling off a scene.
fn glow_color(u: f32) -> [f32; 3] {
    const BLUE: [f32; 3] = [0.25, 0.55, 1.00];
    const VIOLET: [f32; 3] = [0.56, 0.38, 1.00];
    const PINK: [f32; 3] = [1.00, 0.42, 0.72];
    let t = (u + 1.0) * 0.5;
    if t < 0.5 {
        mix(BLUE, VIOLET, t * 2.0)
    } else {
        mix(VIOLET, PINK, (t - 0.5) * 2.0)
    }
}

fn shade(u: f32, v: f32, glow_reach: f32) -> Rgba {
    let (hw, hh) = SCREEN_HALF;
    let y = v - SCREEN_CENTER_Y;
    let body = rounded_rect(u, y, hw, hh, SCREEN_RADIUS);

    // Ambient glow on the wall behind the monitor.
    let glow = if body > 0.0 { (-(body / glow_reach).powi(2)).exp() * 0.85 } else { 0.85 };
    let mut px: Rgba = { let c = glow_color(u); [c[0], c[1], c[2], glow] };

    // Stand: a short neck and a pill-shaped foot, in a slightly darker aluminium.
    let neck = rounded_rect(u, v - 0.47, 0.08, 0.13, 0.02) <= 0.0;
    let foot = rounded_rect(u, v - 0.62, 0.34, 0.045, 0.045) <= 0.0;
    let metal_dark = mix([0.80, 0.83, 0.89], [0.64, 0.68, 0.76], (v - 0.35) / 0.3);
    px = over(px, solid(metal_dark, neck || foot));

    // Bezel: light aluminium, lit from the top.
    if body <= 0.0 {
        let t = (y + hh) / (2.0 * hh);
        let bezel = mix([0.97, 0.98, 1.00], [0.80, 0.84, 0.91], t);
        px = over(px, solid(bezel, true));
        // A hairline on the outer edge gives the shape definition on light backgrounds.
        let edge = (1.0 - (-body / 0.025)).clamp(0.0, 1.0) * 0.35;
        px = over(px, [0.45, 0.50, 0.60, edge]);
    }

    // Screen: deep navy with a soft diagonal sheen.
    let screen = rounded_rect(u, y, hw - BEZEL, hh - BEZEL, SCREEN_RADIUS - BEZEL * 0.5);
    if screen <= 0.0 {
        let t = ((u + hw) / (2.0 * hw) + (y + hh) / (2.0 * hh)) * 0.5;
        let mut c = mix([0.13, 0.20, 0.40], [0.04, 0.06, 0.15], t);
        let sheen = ((0.35 - (u * 0.6 + y)).clamp(0.0, 0.5)) * 0.18;
        c = mix(c, [0.55, 0.65, 0.95], sheen);
        px = over(px, solid(c, true));
    }
    px
}
