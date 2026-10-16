//! The light loop: finds the strip, renders the current mode, smooths it and streams frames.

use crate::capture::{self, Capturer};
use crate::config::{Config, Mode};
use crate::device::{self, Device};
use crate::effects::{self, Rgb, Zones};
use crate::letterbox;
use crate::log;
use crate::proto::Status;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

const PROBE_INTERVAL: Duration = Duration::from_secs(3);
const CAPTURE_RETRY: Duration = Duration::from_millis(1500);
/// Every mode streams at 60 fps. That is the ceiling of the link — a 54-LED frame is 168 bytes,
/// about 15 ms at 115200 baud — and the strip blanks itself if frames stop, so even a static
/// colour keeps flowing.
const FRAME_INTERVAL: Duration = Duration::from_micros(16_667);
const SAVE_DELAY: Duration = Duration::from_millis(800);
const FADE_SECONDS: f32 = 0.35;

type Waker = Box<dyn Fn() + Send>;

pub struct Shared {
    config: Mutex<Config>,
    pub config_rev: AtomicU64,
    status: Mutex<Status>,
    pub status_rev: AtomicU64,
    leds: Mutex<Vec<[u8; 3]>>,
    ambient: Mutex<[u8; 3]>,
    pub leds_rev: AtomicU64,
    pub rescan: AtomicBool,
    pub quit: AtomicBool,
    dirty_since: Mutex<Option<Instant>>,
    waker: Mutex<Option<Waker>>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl Shared {
    pub fn new(config: Config) -> Arc<Self> {
        Arc::new(Self {
            config: Mutex::new(config),
            config_rev: AtomicU64::new(1),
            status: Mutex::new(Status::default()),
            status_rev: AtomicU64::new(1),
            leds: Mutex::new(Vec::new()),
            ambient: Mutex::new([0; 3]),
            leds_rev: AtomicU64::new(0),
            rescan: AtomicBool::new(false),
            quit: AtomicBool::new(false),
            dirty_since: Mutex::new(None),
            waker: Mutex::new(None),
        })
    }

    pub fn config(&self) -> Config {
        lock(&self.config).clone()
    }

    pub fn status(&self) -> Status {
        lock(&self.status).clone()
    }

    pub fn leds(&self) -> Vec<[u8; 3]> {
        lock(&self.leds).clone()
    }

    /// Average colour of what the strip is showing right now, brightness included.
    pub fn ambient(&self) -> [u8; 3] {
        *lock(&self.ambient)
    }

    pub fn set_waker(&self, waker: Waker) {
        *lock(&self.waker) = Some(waker);
    }

    fn wake(&self) {
        if let Some(w) = lock(&self.waker).as_ref() {
            w();
        }
    }

    pub fn update_config(&self, f: impl FnOnce(&Config) -> Config) {
        {
            let mut current = lock(&self.config);
            let next = f(&current).sanitized();
            if *current == next {
                return;
            }
            *current = next;
        }
        self.config_rev.fetch_add(1, Ordering::SeqCst);
        *lock(&self.dirty_since) = Some(Instant::now());
        self.wake();
    }

    pub fn update_status(&self, f: impl FnOnce(&Status) -> Status) {
        {
            let mut current = lock(&self.status);
            let next = f(&current);
            if *current == next {
                return;
            }
            *current = next;
        }
        self.status_rev.fetch_add(1, Ordering::SeqCst);
        self.wake();
    }

    fn publish_leds(&self, leds: &[[u8; 3]]) {
        let mut current = lock(&self.leds);
        if current.as_slice() != leds {
            current.clear();
            current.extend_from_slice(leds);
            self.leds_rev.fetch_add(1, Ordering::SeqCst);
        }
    }

    pub fn request_quit(&self) {
        self.quit.store(true, Ordering::SeqCst);
        self.wake();
    }

    fn save_if_due(&self) {
        let due = lock(&self.dirty_since).is_some_and(|t| t.elapsed() >= SAVE_DELAY);
        if due {
            *lock(&self.dirty_since) = None;
            if let Err(e) = self.config().save() {
                log::line(&format!("config save failed: {e}"));
            }
        }
    }
}

struct Link {
    device: Option<Device>,
    next_probe: Instant,
}

impl Link {
    fn maintain(&mut self, shared: &Shared, hint: Option<String>) {
        if self.device.is_some() || Instant::now() < self.next_probe {
            return;
        }
        match device::discover(hint.as_deref()) {
            Ok(dev) => {
                let info = dev.info.clone();
                log::line(&format!("connected {} {} on {}", info.model, info.serial, info.port));
                shared.update_config(|c| Config { port_hint: Some(info.port.clone()), ..c.clone() });
                shared.update_status(|s| Status { device: Some(info), note: String::new(), ..s.clone() });
                self.device = Some(dev);
            }
            Err(note) => {
                shared.update_status(|s| Status { device: None, note, ..s.clone() });
                self.next_probe = Instant::now() + PROBE_INTERVAL;
            }
        }
    }

    fn push(&mut self, shared: &Shared, frame: &[[u8; 3]]) {
        let Some(dev) = self.device.as_mut() else { return };
        if let Err(e) = dev.send(frame) {
            log::line(&format!("write failed: {e}"));
            self.device = None;
            self.next_probe = Instant::now() + Duration::from_millis(800);
            shared.update_status(|s| Status { device: None, note: "Лента отключилась".into(), ..s.clone() });
        }
    }
}

struct Screen {
    capturer: Option<(u32, Capturer)>,
    retry_at: Instant,
    zones: Zones,
    bars: letterbox::Tracker,
}

impl Screen {
    /// Returns true if `target` was refreshed from a captured frame.
    fn sample(&mut self, shared: &Shared, cfg: &Config, target: &mut [Rgb]) -> bool {
        let monitor = cfg.screen.monitor;
        if self.capturer.as_ref().map(|(m, _)| *m) != Some(monitor) {
            self.capturer = None;
            if Instant::now() < self.retry_at {
                return false;
            }
            match Capturer::new(monitor) {
                Ok(c) => {
                    self.capturer = Some((monitor, c));
                    shared.update_status(|s| Status { capture_error: None, ..s.clone() });
                }
                Err(e) => {
                    self.retry_at = Instant::now() + CAPTURE_RETRY;
                    shared.update_status(|s| Status { capture_error: Some(e), ..s.clone() });
                    return false;
                }
            }
        }
        let Some((_, cap)) = self.capturer.as_mut() else { return false };
        if cap.grab().is_err() {
            // Access lost (UAC prompt, mode switch, exclusive fullscreen): rebuild shortly.
            self.capturer = None;
            self.retry_at = Instant::now() + Duration::from_millis(300);
            return false;
        }
        let Some((pixels, w, h)) = cap.frame() else { return false };
        let area = if cfg.screen.letterbox { self.bars.update(pixels, w, h) } else { [0, 0, w, h] };
        self.zones.ensure(area, &cfg.layout, cfg.screen.depth);
        self.zones.sample(pixels, w, target);
        let sat = cfg.screen.saturation;
        target.iter_mut().for_each(|c| *c = effects::saturate(*c, sat));
        true
    }
}

fn average(frame: &[[u8; 3]]) -> [u8; 3] {
    let n = frame.len().max(1) as u32;
    let sum = frame.iter().fold([0u32; 3], |acc, c| [acc[0] + c[0] as u32, acc[1] + c[1] as u32, acc[2] + c[2] as u32]);
    sum.map(|v| (v / n) as u8)
}

fn ease(x: f32) -> f32 {
    x * x * (3.0 - 2.0 * x)
}

pub fn spawn(shared: Arc<Shared>) -> thread::JoinHandle<()> {
    thread::Builder::new().name("halo-engine".into()).spawn(move || run(&shared)).expect("engine thread")
}

fn run(shared: &Shared) {
    shared.update_status(|s| Status { monitors: capture::list_monitors(), ..s.clone() });

    let mut link = Link { device: None, next_probe: Instant::now() };
    let mut screen = Screen { capturer: None, retry_at: Instant::now(), zones: Zones::default(), bars: letterbox::Tracker::default() };
    let mut target: Vec<Rgb> = Vec::new();
    let mut current: Vec<Rgb> = Vec::new();
    let mut frame: Vec<[u8; 3]> = Vec::new();
    let mut preview: Vec<[u8; 3]> = Vec::new();
    let mut fade = 0.0f32;
    let mut effect_t = 0.0f32;
    let mut last = Instant::now();

    while !shared.quit.load(Ordering::SeqCst) {
        let tick_start = Instant::now();
        let dt = (tick_start - last).as_secs_f32().min(0.25);
        last = tick_start;
        let cfg = shared.config();

        if shared.rescan.swap(false, Ordering::SeqCst) {
            link.device = None;
            link.next_probe = Instant::now();
            screen.capturer = None;
            screen.retry_at = Instant::now();
            shared.update_status(|s| Status { monitors: capture::list_monitors(), ..s.clone() });
        }
        link.maintain(shared, cfg.port_hint.clone());

        let n = cfg.layout.total().max(1);
        target.resize(n, [0.0; 3]);
        current.resize(n, [0.0; 3]);

        let screen_mode = cfg.enabled && cfg.mode == Mode::Screen;
        if screen_mode {
            screen.sample(shared, &cfg, &mut target);
        } else {
            screen.capturer = None;
            effect_t += dt * cfg.speed;
            effects::render(&cfg, effect_t, &mut target);
        }

        let fade_target = if cfg.enabled { 1.0 } else { 0.0 };
        let step = dt / FADE_SECONDS;
        fade = if fade < fade_target { (fade + step).min(1.0) } else { (fade - step).max(0.0) };

        let tau = if screen_mode { cfg.screen.smoothing * 0.3 } else { 0.12 };
        let alpha = if tau < 0.005 { 1.0 } else { 1.0 - (-dt / tau).exp() };
        for (c, t) in current.iter_mut().zip(&target) {
            for k in 0..3 {
                let delta = t[k] - c[k];
                c[k] += delta * alpha;
            }
        }

        let master = ease(fade);
        let gain = cfg.brightness * cfg.brightness * master;
        frame.clear();
        frame.extend(current.iter().map(|c| effects::to8(effects::scale(*c, gain))));
        preview.clear();
        preview.extend(current.iter().map(|c| effects::to8(effects::scale(*c, master))));

        link.push(shared, &frame);
        *lock(&shared.ambient) = average(&frame);
        shared.publish_leds(&preview);
        shared.save_if_due();

        if let Some(rest) = FRAME_INTERVAL.checked_sub(tick_start.elapsed()) {
            thread::sleep(rest);
        }
    }

    // Leave the strip dark rather than frozen on the last frame.
    if let Some(dev) = link.device.as_mut() {
        let _ = dev.send(&vec![[0, 0, 0]; frame.len().max(1)]);
    }
    if let Err(e) = shared.config().save() {
        log::line(&format!("config save failed: {e}"));
    }
}
