use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Screen,
    Solid,
    Breathe,
    Rainbow,
    Aurora,
}

impl Mode {
    pub const ALL: [Mode; 5] = [Mode::Screen, Mode::Solid, Mode::Breathe, Mode::Rainbow, Mode::Aurora];

    pub fn label(self) -> &'static str {
        match self {
            Mode::Screen => "Экран",
            Mode::Solid => "Цвет",
            Mode::Breathe => "Дыхание",
            Mode::Rainbow => "Радуга",
            Mode::Aurora => "Аврора",
        }
    }

    pub fn index(self) -> usize {
        Mode::ALL.iter().position(|m| *m == self).unwrap_or(0)
    }

    pub fn shifted(self, step: isize) -> Mode {
        let n = Mode::ALL.len() as isize;
        Mode::ALL[((self.index() as isize + step).rem_euclid(n)) as usize]
    }
}

/// LED order: starts at the bottom-left corner and runs clockwise —
/// up the left side, across the top, down the right side, back along the bottom.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Layout {
    pub left: u16,
    pub top: u16,
    pub right: u16,
    pub bottom: u16,
    pub reverse: bool,
}

impl Default for Layout {
    // Skydimo SK0124 (24"): 14 + 26 + 14, no bottom run.
    fn default() -> Self {
        Self { left: 14, top: 26, right: 14, bottom: 0, reverse: false }
    }
}

impl Layout {
    pub fn total(&self) -> usize {
        (self.left + self.top + self.right + self.bottom) as usize
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScreenSettings {
    pub monitor: u32,
    /// How far into the picture each LED looks, as a fraction of screen height.
    pub depth: f32,
    pub saturation: f32,
    /// 0 = instant, 1 = very soft.
    pub smoothing: f32,
    pub fps: u32,
    /// Crop letterbox/pillarbox bars so films light the strip edge to edge.
    pub letterbox: bool,
}

impl Default for ScreenSettings {
    fn default() -> Self {
        Self { monitor: 0, depth: 0.12, saturation: 1.15, smoothing: 0.45, fps: 30, letterbox: true }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    pub brightness: f32,
    pub mode: Mode,
    pub color: [u8; 3],
    pub speed: f32,
    pub screen: ScreenSettings,
    pub layout: Layout,
    pub autostart: bool,
    pub port_hint: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            brightness: 0.6,
            mode: Mode::Screen,
            color: [255, 170, 90],
            speed: 1.0,
            screen: ScreenSettings::default(),
            layout: Layout::default(),
            autostart: false,
            port_hint: None,
        }
    }
}

impl Config {
    pub fn load() -> Self {
        fs::read_to_string(config_path())
            .ok()
            .and_then(|s| serde_json::from_str::<Config>(&s).ok())
            .unwrap_or_default()
            .sanitized()
    }

    pub fn save(&self) -> io::Result<()> {
        let path = config_path();
        let tmp = path.with_extension("json.tmp");
        let body = serde_json::to_string_pretty(self).map_err(io::Error::other)?;
        fs::write(&tmp, body)?;
        fs::rename(tmp, path)
    }

    pub fn sanitized(self) -> Self {
        let clamp_side = |v: u16| v.min(300);
        Self {
            brightness: self.brightness.clamp(0.0, 1.0),
            speed: self.speed.clamp(0.1, 4.0),
            screen: ScreenSettings {
                depth: self.screen.depth.clamp(0.03, 0.45),
                saturation: self.screen.saturation.clamp(0.0, 2.0),
                smoothing: self.screen.smoothing.clamp(0.0, 1.0),
                fps: self.screen.fps.clamp(10, 60),
                ..self.screen
            },
            layout: Layout {
                left: clamp_side(self.layout.left),
                top: clamp_side(self.layout.top),
                right: clamp_side(self.layout.right),
                bottom: clamp_side(self.layout.bottom),
                ..self.layout
            },
            ..self
        }
    }
}

pub fn data_dir() -> PathBuf {
    let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    let dir = base.join("Halo");
    let _ = fs::create_dir_all(&dir);
    dir
}

pub fn config_path() -> PathBuf {
    data_dir().join("config.json")
}
