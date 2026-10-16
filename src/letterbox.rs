//! Black-bar detection for films: finds letterbox (top/bottom) and pillarbox (left/right) bars
//! so the strip samples the picture itself instead of glowing black.

use std::time::{Duration, Instant};

/// A channel at or below this counts as black (encoders rarely output a clean 0).
const DARK: u8 = 18;
/// A row may contain this share of brighter pixels and still count as a bar (subtitles, noise).
const NOISE_SHARE: f32 = 0.02;
/// Bars must hold this long before the crop switches, so dark scenes don't make it flicker.
const SETTLE: Duration = Duration::from_millis(1500);

fn is_dark(px: &[u8]) -> bool {
    px[0] <= DARK && px[1] <= DARK && px[2] <= DARK
}

fn line_is_bar(bgra: &[u8], w: u32, pixels: impl Iterator<Item = (u32, u32)>, len: u32) -> bool {
    let allowed = (len as f32 * NOISE_SHARE) as u32;
    let mut lit = 0;
    for (x, y) in pixels {
        let i = ((y * w + x) * 4) as usize;
        if !is_dark(&bgra[i..i + 4]) {
            lit += 1;
            if lit > allowed {
                return false;
            }
        }
    }
    true
}

/// Symmetric bar sizes `(horizontal, vertical)` in pixels, or `None` when the frame is too dark to tell.
pub fn detect(bgra: &[u8], w: u32, h: u32) -> Option<(u32, u32)> {
    let row = |y: u32| line_is_bar(bgra, w, (0..w).map(move |x| (x, y)), w);
    let col = |x: u32| line_is_bar(bgra, w, (0..h).map(move |y| (x, y)), h);

    let max_y = h / 3;
    let max_x = w / 4;
    let top = (0..max_y).take_while(|&y| row(y)).count() as u32;
    let bottom = (0..max_y).take_while(|&y| row(h - 1 - y)).count() as u32;
    let left = (0..max_x).take_while(|&x| col(x)).count() as u32;
    let right = (0..max_x).take_while(|&x| col(w - 1 - x)).count() as u32;

    // Hitting the search limit means a black (or nearly black) scene, not bars.
    if top >= max_y || bottom >= max_y || left >= max_x || right >= max_x {
        return None;
    }
    Some((left.min(right), top.min(bottom)))
}

#[derive(Default)]
pub struct Tracker {
    active: (u32, u32),
    candidate: Option<((u32, u32), Instant)>,
}

impl Tracker {
    /// Feeds one frame and returns the picture area `[x0, y0, x1, y1]` to sample from.
    pub fn update(&mut self, bgra: &[u8], w: u32, h: u32) -> [u32; 4] {
        if let Some(found) = detect(bgra, w, h) {
            match self.candidate {
                Some((c, since)) if c == found => {
                    if since.elapsed() >= SETTLE {
                        self.active = found;
                    }
                }
                _ if found == self.active => self.candidate = None,
                _ => self.candidate = Some((found, Instant::now())),
            }
        }
        let (bx, by) = self.active;
        let bx = bx.min(w.saturating_sub(2) / 2);
        let by = by.min(h.saturating_sub(2) / 2);
        [bx, by, w - bx, h - by]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(w: u32, h: u32, bar_y: u32) -> Vec<u8> {
        (0..h)
            .flat_map(|y| {
                (0..w).flat_map(move |_| if y < bar_y || y >= h - bar_y { [0, 0, 0, 255] } else { [90, 140, 200, 255] })
            })
            .collect()
    }

    #[test]
    fn finds_letterbox_bars() {
        let (w, h) = (120, 67);
        assert_eq!(detect(&frame(w, h, 8), w, h), Some((0, 8)));
    }

    #[test]
    fn full_picture_has_no_bars() {
        let (w, h) = (120, 67);
        assert_eq!(detect(&frame(w, h, 0), w, h), Some((0, 0)));
    }

    #[test]
    fn black_scene_is_undecided() {
        let (w, h) = (120, 67);
        let black = vec![0u8; (w * h * 4) as usize];
        assert_eq!(detect(&black, w, h), None);
    }
}
