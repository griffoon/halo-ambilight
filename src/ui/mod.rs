//! Settings window. Lives in its own process so the service stays tiny while it is closed.

mod hero;
mod link;
mod paint;
mod widgets;

use crate::config::{Config, Mode};
use crate::daemon::WINDOW_TITLE;
use crate::effects;
use crate::proto::{Status, ToDaemon, ToUi};
use egui::{pos2, vec2, Align2, Color32, FontData, FontDefinitions, FontFamily, FontId, Id, Rect, RichText, Sense, Ui, ViewportCommand};
use link::Link;
use paint::{TEXT, TEXT2, TEXT3};
use std::sync::Arc;
use widgets::Glyph;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
use windows::Win32::Graphics::Dwm::{
    DwmExtendFrameIntoClientArea, DwmSetWindowAttribute, DWMSBT_TRANSIENTWINDOW, DWMWA_SYSTEMBACKDROP_TYPE,
    DWMWA_USE_IMMERSIVE_DARK_MODE, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DWM_SYSTEMBACKDROP_TYPE,
    DWM_WINDOW_CORNER_PREFERENCE,
};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::Controls::MARGINS;
use windows::Win32::UI::WindowsAndMessaging::FindWindowW;

/// Ignore echoes from the service for this long after a local edit, so drags never jump back.
const ECHO_GUARD: f64 = 1.0;

const PRESETS: [[u8; 3]; 10] = [
    [255, 170, 90],
    [255, 236, 214],
    [255, 44, 32],
    [255, 112, 24],
    [255, 204, 40],
    [60, 230, 96],
    [40, 212, 255],
    [52, 92, 255],
    [150, 72, 255],
    [255, 62, 170],
];

pub fn run() -> eframe::Result<()> {
    // SAFETY: named mutex held for the life of the window process.
    let already = unsafe {
        let m = CreateMutexW(None, true, w!("Local\\HaloWindowMutex"));
        m.is_err() || GetLastError() == ERROR_ALREADY_EXISTS
    };
    if already {
        // Never spawn from here: the other window may still be starting up.
        crate::daemon::focus_ui();
        return Ok(());
    }
    let size = 64;
    let icon = egui::IconData { rgba: crate::art::render(size), width: size, height: size };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(WINDOW_TITLE)
            .with_inner_size([440.0, 780.0])
            .with_min_inner_size([440.0, 480.0])
            .with_decorations(false)
            .with_transparent(true)
            .with_icon(Arc::new(icon)),
        centered: true,
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(WINDOW_TITLE, options, Box::new(|cc| Ok(Box::new(HaloApp::new(cc)))))
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain([0]).collect()
}

/// Win11 acrylic behind the whole client area, dark title chrome and rounded corners.
fn apply_backdrop() -> bool {
    let title = wide(WINDOW_TITLE);
    // SAFETY: attribute pointers reference locals of the declared size; hwnd comes from FindWindowW.
    unsafe {
        let Ok(hwnd) = FindWindowW(PCWSTR::null(), PCWSTR(title.as_ptr())) else { return false };
        if hwnd.is_invalid() {
            return false;
        }
        let dark: i32 = 1;
        let _ = DwmSetWindowAttribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, (&dark as *const i32).cast(), 4);
        let corner = DWMWCP_ROUND;
        let _ = DwmSetWindowAttribute(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, (&corner as *const DWM_WINDOW_CORNER_PREFERENCE).cast(), 4);
        let margins = MARGINS { cxLeftWidth: -1, cxRightWidth: -1, cyTopHeight: -1, cyBottomHeight: -1 };
        let _ = DwmExtendFrameIntoClientArea(hwnd, &margins);
        let backdrop = DWMSBT_TRANSIENTWINDOW;
        DwmSetWindowAttribute(hwnd, DWMWA_SYSTEMBACKDROP_TYPE, (&backdrop as *const DWM_SYSTEMBACKDROP_TYPE).cast(), 4).is_ok()
    }
}

fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let dir = std::path::PathBuf::from(std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into())).join("Fonts");
    let load = |names: &[&str]| names.iter().find_map(|n| std::fs::read(dir.join(n)).ok());
    let mut sb_family = Vec::new();
    if let Some(bytes) = load(&["segoeui.ttf"]) {
        fonts.font_data.insert("ui".into(), FontData::from_owned(bytes));
        if let Some(list) = fonts.families.get_mut(&FontFamily::Proportional) {
            list.insert(0, "ui".into());
        }
    }
    if let Some(bytes) = load(&["seguisb.ttf", "segoeuib.ttf"]) {
        fonts.font_data.insert("ui_sb".into(), FontData::from_owned(bytes));
        sb_family.push("ui_sb".to_string());
    }
    sb_family.extend(fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default());
    fonts.families.insert(paint::semibold(), sb_family);
    ctx.set_fonts(fonts);
}

fn install_style(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = Color32::TRANSPARENT;
    visuals.window_fill = Color32::TRANSPARENT;
    visuals.override_text_color = Some(TEXT);
    visuals.selection.bg_fill = paint::ACCENT;
    ctx.set_visuals(visuals);
    ctx.style_mut(|s| {
        s.spacing.item_spacing = vec2(8.0, 10.0);
        s.spacing.scroll = egui::style::ScrollStyle::floating();
        s.animation_time = 0.2;
    });
}

fn plural_leds(n: usize) -> String {
    let word = match (n % 10, n % 100) {
        (1, x) if x != 11 => "светодиод",
        (2..=4, x) if !(12..=14).contains(&x) => "светодиода",
        _ => "светодиодов",
    };
    format!("{n} {word}")
}

fn rgb_to_hs(c: [u8; 3]) -> (f32, f32) {
    let [r, g, b] = effects::from8(c);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let s = if max > 0.0 { d / max } else { 0.0 };
    let h = if d < 1e-5 {
        0.0
    } else if max == r {
        ((g - b) / d).rem_euclid(6.0) / 6.0
    } else if max == g {
        ((b - r) / d + 2.0) / 6.0
    } else {
        ((r - g) / d + 4.0) / 6.0
    };
    (h, s)
}

struct HaloApp {
    link: Link,
    config: Config,
    status: Status,
    leds: Vec<[u8; 3]>,
    last_edit: f64,
    backdrop: bool,
    hue: f32,
    device_open: bool,
}

impl HaloApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        install_fonts(&cc.egui_ctx);
        install_style(&cc.egui_ctx);
        let config = Config::load();
        let (hue, _) = rgb_to_hs(config.color);
        Self {
            link: Link::start(cc.egui_ctx.clone()),
            config,
            status: Status::default(),
            leds: Vec::new(),
            last_edit: f64::NEG_INFINITY,
            backdrop: apply_backdrop(),
            hue,
            device_open: false,
        }
    }

    fn absorb(&mut self, now: f64) {
        for msg in self.link.drain() {
            match msg {
                ToUi::State { config, status } => {
                    if now - self.last_edit > ECHO_GUARD {
                        self.config = config;
                    }
                    self.status = status;
                }
                ToUi::Leds { c } => self.leds = c,
            }
        }
    }

    fn status_line(&self) -> (Color32, String) {
        if !self.link.online() {
            return (paint::BAD, "Служба не отвечает".into());
        }
        match &self.status.device {
            Some(d) => (paint::OK, format!("{} · {} · {} LED", d.model, d.port, self.config.layout.total())),
            None if self.status.note.is_empty() => (paint::WARN, "Ищу ленту…".into()),
            None => (paint::WARN, self.status.note.clone()),
        }
    }

    fn header(&mut self, ui: &mut Ui) {
        let rect = ui.max_rect();
        let drag = ui.interact(rect, Id::new("drag-area"), Sense::click_and_drag());
        if drag.drag_started() {
            ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
        }
        let p = ui.painter();
        let logo_c = pos2(rect.left() + 18.0, rect.center().y);
        paint::logo(p, logo_c, 11.0);
        let title = paint::galley(ui, "Halo", 21.0, paint::semibold(), TEXT, -0.4);
        p.galley(pos2(rect.left() + 42.0, rect.center().y - 22.0), title, TEXT);
        let (dot, line) = self.status_line();
        p.circle_filled(pos2(rect.left() + 46.0, rect.center().y + 10.0), 3.5, dot);
        p.text(pos2(rect.left() + 55.0, rect.center().y + 10.0), Align2::LEFT_CENTER, line, FontId::proportional(12.5), TEXT2);

        let close_rect = Rect::from_center_size(pos2(rect.right() - 15.0, rect.center().y), vec2(30.0, 30.0));
        let min_rect = close_rect.translate(vec2(-38.0, 0.0));
        if widgets::round_icon(&mut ui.new_child(egui::UiBuilder::new().max_rect(min_rect)), "min", Glyph::Minimize).clicked() {
            ui.ctx().send_viewport_cmd(ViewportCommand::Minimized(true));
        }
        if widgets::round_icon(&mut ui.new_child(egui::UiBuilder::new().max_rect(close_rect)), "close", Glyph::Close)
            .on_hover_text("Свернуть в трей")
            .clicked()
        {
            ui.ctx().send_viewport_cmd(ViewportCommand::Close);
        }
    }

    fn power_card(&mut self, ui: &mut Ui) {
        paint::card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    ui.label(RichText::new("Подсветка").size(17.0).family(paint::semibold()).color(TEXT));
                    let sub = if self.config.enabled { "Включена".to_string() } else { "Выключена".to_string() };
                    ui.label(RichText::new(sub).size(13.0).color(TEXT2));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    widgets::toggle(ui, "power", &mut self.config.enabled);
                });
            });
            ui.add_space(6.0);
            widgets::level(ui, "brightness", &mut self.config.brightness);
        });
    }

    fn mode_picker(&mut self, ui: &mut Ui) {
        let labels: Vec<String> = Mode::ALL.iter().map(|m| m.label().to_string()).collect();
        let mut index = self.config.mode.index();
        if widgets::segmented(ui, "mode", &labels, &mut index, 40.0) {
            self.config.mode = Mode::ALL[index];
            self.config.enabled = true;
        }
    }

    fn mode_card(&mut self, ui: &mut Ui) {
        paint::card(ui, |ui| match self.config.mode {
            Mode::Screen => self.screen_settings(ui),
            Mode::Solid => self.color_settings(ui, false),
            Mode::Breathe => self.color_settings(ui, true),
            Mode::Rainbow => self.speed_settings(ui, "Радуга бежит по кругу экрана"),
            Mode::Aurora => self.speed_settings(ui, "Медленные переливы бирюзы и фиолета"),
        });
    }

    fn screen_settings(&mut self, ui: &mut Ui) {
        paint::caption(ui, "Синхронизация с экраном");
        let monitors = self.status.monitors.clone();
        if monitors.len() > 1 {
            let labels: Vec<String> = monitors.iter().map(|m| format!("{} · {}×{}", m.index + 1, m.width, m.height)).collect();
            let mut i = monitors.iter().position(|m| m.index == self.config.screen.monitor).unwrap_or(0);
            if widgets::segmented(ui, "monitor", &labels, &mut i, 34.0) {
                self.config.screen.monitor = monitors[i].index;
            }
        }
        let s = &mut self.config.screen;
        let pct = |v: f32| format!("{:.0}%", v * 100.0);
        widgets::slider(ui, "smooth", "Плавность", &mut s.smoothing, 0.0..=1.0, pct);
        widgets::slider(ui, "sat", "Насыщенность", &mut s.saturation, 0.0..=2.0, pct);
        widgets::slider(ui, "depth", "Глубина захвата", &mut s.depth, 0.03..=0.45, pct);
        let labels = vec!["30 кадров".to_string(), "60 кадров".to_string()];
        let mut fps_i = usize::from(s.fps >= 45);
        if widgets::segmented(ui, "fps", &labels, &mut fps_i, 34.0) {
            s.fps = if fps_i == 1 { 60 } else { 30 };
        }
        widgets::toggle_row(ui, "letterbox", "Убирать чёрные полосы", &mut s.letterbox);
        if let Some(err) = &self.status.capture_error {
            ui.label(RichText::new(err).size(12.5).color(paint::WARN));
        }
    }

    fn color_settings(&mut self, ui: &mut Ui, breathing: bool) {
        paint::caption(ui, if breathing { "Дыхание" } else { "Цвет" });
        if widgets::swatches(ui, "presets", &PRESETS, &mut self.config.color) {
            self.hue = rgb_to_hs(self.config.color).0;
        }
        let (_, mut sat) = rgb_to_hs(self.config.color);
        let hue_changed = widgets::hue(ui, "hue", &mut self.hue, self.config.color).changed();
        let sat_changed =
            widgets::slider(ui, "csat", "Насыщенность", &mut sat, 0.0..=1.0, |v| format!("{:.0}%", v * 100.0)).changed();
        if hue_changed || sat_changed {
            self.config.color = effects::to8(effects::hsv(self.hue, sat, 1.0));
        }
        if breathing {
            widgets::slider(ui, "speed", "Темп", &mut self.config.speed, 0.2..=3.0, |v| format!("{v:.1}×"));
        }
    }

    fn speed_settings(&mut self, ui: &mut Ui, hint: &str) {
        paint::caption(ui, self.config.mode.label());
        ui.label(RichText::new(hint).size(13.0).color(TEXT2));
        widgets::slider(ui, "speed", "Скорость", &mut self.config.speed, 0.1..=4.0, |v| format!("{v:.1}×"));
    }

    fn device_card(&mut self, ui: &mut Ui) {
        paint::card(ui, |ui| {
            let openness = widgets::disclosure(ui, "device", "Лента и система", &mut self.device_open);
            if openness < 0.01 {
                return;
            }
            ui.scope(|ui| {
                ui.set_opacity(openness);
                ui.add_space(4.0);
                paint::caption(ui, &format!("Разметка · {}", plural_leds(self.config.layout.total())));
                let l = &mut self.config.layout;
                widgets::stepper(ui, "left", "Слева", &mut l.left, 300);
                widgets::stepper(ui, "top", "Сверху", &mut l.top, 300);
                widgets::stepper(ui, "right", "Справа", &mut l.right, 300);
                widgets::stepper(ui, "bottom", "Снизу", &mut l.bottom, 300);
                widgets::toggle_row(ui, "reverse", "Обратное направление", &mut l.reverse);
                widgets::toggle_row(ui, "autostart", "Запускать вместе с Windows", &mut self.config.autostart);
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if widgets::pill_button(ui, "rescan", "Найти ленту заново").clicked() {
                        self.link.send(&ToDaemon::Rescan);
                    }
                    if widgets::pill_button(ui, "quit", "Выключить Halo").clicked() {
                        self.link.send(&ToDaemon::Quit);
                        ui.ctx().send_viewport_cmd(ViewportCommand::Close);
                    }
                });
            });
        });
    }

    fn offline_card(&mut self, ui: &mut Ui) {
        paint::card(ui, |ui| {
            ui.label(RichText::new("Фоновая служба Halo не запущена").size(14.0).color(TEXT));
            ui.label(RichText::new("Без неё лента не получает цвета.").size(13.0).color(TEXT2));
            ui.add_space(4.0);
            if widgets::pill_button(ui, "start", "Запустить").clicked() {
                if let Ok(exe) = std::env::current_exe() {
                    let _ = std::process::Command::new(exe).arg("--background").spawn();
                }
            }
        });
    }

    fn footer(&self, ui: &mut Ui) {
        ui.add_space(2.0);
        ui.vertical_centered(|ui| {
            ui.label(RichText::new("Ctrl Shift Alt  ·  ↑ ↓ яркость  ·  ← → режим  ·  F10 вкл/выкл").size(11.5).color(TEXT3));
        });
        ui.add_space(14.0);
    }

    fn hero_caption(&self) -> String {
        if !self.config.enabled {
            return "Подсветка выключена".into();
        }
        match self.config.mode {
            Mode::Screen => {
                let m = self.status.monitors.iter().find(|m| m.index == self.config.screen.monitor);
                m.map_or("Экран".into(), |m| format!("Экран {} · {}×{}", m.index + 1, m.width, m.height))
            }
            other => other.label().into(),
        }
    }
}

impl eframe::App for HaloApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0; 4]
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let now = ctx.input(|i| i.time);
        if !self.backdrop {
            self.backdrop = apply_backdrop();
        }
        self.absorb(now);
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
        let before = self.config.clone();

        egui::CentralPanel::default().frame(egui::Frame::none()).show(ctx, |ui| {
            let full = ui.max_rect();
            hero::backdrop(ui.painter(), full, &self.leds, &self.config.layout);

            // Materialise on open: fade in while rising a few points.
            let appear = widgets::spring_from(ctx, Id::new("appear"), 0.0, 1.0, 0.55, 1.0).pos.clamp(0.0, 1.0);
            ui.set_opacity(appear);
            let lift = (1.0 - appear) * 14.0;

            let header = Rect::from_min_size(pos2(full.left() + 20.0, full.top() + 14.0 + lift), vec2(full.width() - 40.0, 50.0));
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(header), |ui| self.header(ui));

            let body = Rect::from_min_max(pos2(full.left(), header.bottom() + 6.0), full.max);
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(body), |ui| {
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    egui::Frame::none().inner_margin(egui::Margin::symmetric(20.0, 0.0)).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        let caption = self.hero_caption();
                        hero::hero(ui, &self.leds, &self.config.layout, &caption);
                        if !self.link.online() {
                            self.offline_card(ui);
                        }
                        self.power_card(ui);
                        self.mode_picker(ui);
                        self.mode_card(ui);
                        self.device_card(ui);
                        self.footer(ui);
                    });
                });
            });
        });

        self.config = std::mem::take(&mut self.config).sanitized();
        if self.config != before {
            self.last_edit = now;
            self.link.send(&ToDaemon::Set { config: self.config.clone() });
        }
    }
}
