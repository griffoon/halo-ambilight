//! Background service: tray icon, global hotkeys, autostart, and the engine/IPC threads.

use crate::config::{Config, Mode};
use crate::engine::{self, Shared};
use crate::{art, ipc, log, openrgb};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HWND, LPARAM, WPARAM};
use windows::Win32::System::Registry::{RegDeleteKeyValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ};
use windows::Win32::System::Threading::{CreateMutexW, GetCurrentThreadId};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, VK_DOWN, VK_F10, VK_LEFT, VK_RIGHT, VK_UP,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, FindWindowW, GetMessageW, PeekMessageW, PostThreadMessageW, SetForegroundWindow, ShowWindow,
    TranslateMessage, MSG, PM_NOREMOVE, SW_RESTORE, WM_APP, WM_HOTKEY,
};

const WM_WAKE: u32 = WM_APP + 7;
const RUN_KEY: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
pub const WINDOW_TITLE: &str = "Halo";
const BRIGHTNESS_STEP: f32 = 0.05;
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
const REPLACE_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Copy)]
enum Hotkey {
    Brighter = 1,
    Dimmer,
    NextMode,
    PrevMode,
    Power,
}

impl Hotkey {
    fn from_id(id: usize) -> Option<Self> {
        [Hotkey::Brighter, Hotkey::Dimmer, Hotkey::NextMode, Hotkey::PrevMode, Hotkey::Power]
            .into_iter()
            .find(|h| *h as usize == id)
    }
}

/// Takes the single-instance mutex; `false` if another service already holds it.
fn claim_instance() -> bool {
    // SAFETY: on success the handle is kept for the life of the process; on a clash it is closed
    // so the mutex can vanish once the other service exits.
    unsafe {
        match CreateMutexW(None, true, w!("Local\\HaloServiceMutex")) {
            Ok(handle) if GetLastError() == ERROR_ALREADY_EXISTS => {
                let _ = CloseHandle(handle);
                false
            }
            Ok(_) => true,
            Err(_) => false,
        }
    }
}

/// A different build of the service is running (e.g. after an update): ask it to step aside.
fn replace_running_service() -> bool {
    if ipc::running_version().as_deref() == Some(VERSION) {
        return false;
    }
    ipc::request_quit();
    let deadline = Instant::now() + REPLACE_TIMEOUT;
    while Instant::now() < deadline {
        thread::sleep(Duration::from_millis(150));
        if claim_instance() {
            return true;
        }
    }
    false
}

pub fn run(open_window: bool) {
    if !claim_instance() && !replace_running_service() {
        open_ui();
        return;
    }

    let shared = Shared::new(Config::load());
    if let Err(e) = ipc::serve(shared.clone()) {
        log::line(&format!("ipc failed: {e}"));
    }
    let engine = engine::spawn(shared.clone());
    openrgb::spawn(shared.clone());
    if open_window {
        open_ui();
    }
    tray_loop(&shared);
    shared.request_quit();
    let _ = engine.join();
}

/// Brings an existing settings window forward. Returns false if there is none.
pub fn focus_ui() -> bool {
    let title: Vec<u16> = WINDOW_TITLE.encode_utf16().chain([0]).collect();
    // SAFETY: `title` is NUL-terminated and outlives the call.
    unsafe {
        match FindWindowW(PCWSTR::null(), PCWSTR(title.as_ptr())) {
            Ok(hwnd) if !hwnd.is_invalid() => {
                let _ = ShowWindow(hwnd, SW_RESTORE);
                let _ = SetForegroundWindow(hwnd);
                true
            }
            _ => false,
        }
    }
}

/// Brings the settings window forward, or launches it.
pub fn open_ui() {
    if focus_ui() {
        return;
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Err(e) = std::process::Command::new(exe).arg("--ui").spawn() {
            log::line(&format!("cannot start window: {e}"));
        }
    }
}

pub fn set_autostart(on: bool) {
    // SAFETY: every pointer passed is a NUL-terminated wide string that outlives the call.
    unsafe {
        if on {
            let Ok(exe) = std::env::current_exe() else { return };
            let cmd: Vec<u16> = format!("\"{}\" --background", exe.display()).encode_utf16().chain([0]).collect();
            let _ = RegSetKeyValueW(
                HKEY_CURRENT_USER,
                RUN_KEY,
                w!("Halo"),
                REG_SZ.0,
                Some(cmd.as_ptr().cast()),
                (cmd.len() * 2) as u32,
            );
        } else {
            let _ = RegDeleteKeyValueW(HKEY_CURRENT_USER, RUN_KEY, w!("Halo"));
        }
    }
}

struct Tray {
    icon: TrayIcon,
    open: MenuItem,
    power: CheckMenuItem,
    modes: Vec<(Mode, CheckMenuItem)>,
    quit: MenuItem,
}

impl Tray {
    fn build(cfg: &Config) -> Option<Self> {
        let open = MenuItem::new("Открыть Halo", true, None);
        let power = CheckMenuItem::new("Подсветка", true, cfg.enabled, None);
        let modes: Vec<(Mode, CheckMenuItem)> =
            Mode::ALL.iter().map(|m| (*m, CheckMenuItem::new(m.label(), true, cfg.mode == *m, None))).collect();
        let mode_menu = Submenu::new("Режим", true);
        for (_, item) in &modes {
            let _ = mode_menu.append(item);
        }
        let quit = MenuItem::new("Выйти", true, None);
        let menu = Menu::new();
        let _ = menu.append_items(&[&open, &PredefinedMenuItem::separator(), &power, &mode_menu, &PredefinedMenuItem::separator(), &quit]);

        let size = 32;
        let icon = Icon::from_rgba(art::render(size), size, size).ok()?;
        let icon = TrayIconBuilder::new()
            .with_icon(icon)
            .with_tooltip("Halo")
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false)
            .build()
            .map_err(|e| log::line(&format!("tray failed: {e}")))
            .ok()?;
        Some(Self { icon, open, power, modes, quit })
    }

    fn sync(&self, shared: &Shared) {
        let cfg = shared.config();
        self.power.set_checked(cfg.enabled);
        for (mode, item) in &self.modes {
            item.set_checked(cfg.mode == *mode);
        }
        let status = shared.status();
        let tip = match &status.device {
            Some(d) => format!("Halo · {} · {}", d.model, d.port),
            None if status.note.is_empty() => "Halo".to_string(),
            None => format!("Halo · {}", status.note),
        };
        let _ = self.icon.set_tooltip(Some(tip));
    }
}

fn register_hotkeys() {
    let mods = MOD_CONTROL | MOD_SHIFT | MOD_ALT;
    let keys = [
        (Hotkey::Brighter, VK_UP, mods),
        (Hotkey::Dimmer, VK_DOWN, mods),
        (Hotkey::NextMode, VK_RIGHT, mods | MOD_NOREPEAT),
        (Hotkey::PrevMode, VK_LEFT, mods | MOD_NOREPEAT),
        (Hotkey::Power, VK_F10, mods | MOD_NOREPEAT),
    ];
    for (id, vk, m) in keys {
        // SAFETY: thread-bound hotkey (null HWND); WM_HOTKEY lands in this thread's queue.
        if unsafe { RegisterHotKey(HWND::default(), id as i32, m, vk.0 as u32) }.is_err() {
            log::line(&format!("hotkey {} is taken by another app", id as i32));
        }
    }
}

fn on_hotkey(shared: &Shared, key: Hotkey) {
    shared.update_config(|c| match key {
        Hotkey::Brighter => Config { brightness: c.brightness + BRIGHTNESS_STEP, enabled: true, ..c.clone() },
        Hotkey::Dimmer => Config { brightness: c.brightness - BRIGHTNESS_STEP, ..c.clone() },
        Hotkey::NextMode => Config { mode: c.mode.shifted(1), ..c.clone() },
        Hotkey::PrevMode => Config { mode: c.mode.shifted(-1), ..c.clone() },
        Hotkey::Power => Config { enabled: !c.enabled, ..c.clone() },
    });
}

fn tray_loop(shared: &Arc<Shared>) {
    let mut msg = MSG::default();
    // SAFETY: forces creation of this thread's message queue before anyone posts to it.
    let thread_id = unsafe {
        let _ = PeekMessageW(&mut msg, HWND::default(), 0, 0, PM_NOREMOVE);
        GetCurrentThreadId()
    };
    shared.set_waker(Box::new(move || {
        // SAFETY: posting a plain message to a thread id that stays valid while the loop runs.
        let _ = unsafe { PostThreadMessageW(thread_id, WM_WAKE, WPARAM(0), LPARAM(0)) };
    }));

    let initial = shared.config();
    let Some(tray) = Tray::build(&initial) else { return };
    register_hotkeys();
    set_autostart(initial.autostart);
    let mut autostart = initial.autostart;
    tray.sync(shared);

    loop {
        // SAFETY: standard Win32 message pump on the thread that owns the tray window.
        let got = unsafe { GetMessageW(&mut msg, HWND::default(), 0, 0) };
        if got.0 <= 0 {
            break;
        }
        match msg.message {
            WM_HOTKEY => {
                if let Some(key) = Hotkey::from_id(msg.wParam.0) {
                    on_hotkey(shared, key);
                }
            }
            WM_WAKE => {
                if shared.quit.load(Ordering::SeqCst) {
                    break;
                }
                let cfg = shared.config();
                if cfg.autostart != autostart {
                    autostart = cfg.autostart;
                    set_autostart(autostart);
                }
                tray.sync(shared);
            }
            // SAFETY: forwarding the message we just received.
            _ => unsafe {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            },
        }

        while let Ok(event) = TrayIconEvent::receiver().try_recv() {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                open_ui();
            }
        }
        while let Ok(event) = MenuEvent::receiver().try_recv() {
            let id = event.id;
            if id == *tray.open.id() {
                open_ui();
            } else if id == *tray.power.id() {
                shared.update_config(|c| Config { enabled: !c.enabled, ..c.clone() });
            } else if id == *tray.quit.id() {
                shared.request_quit();
            } else if let Some((mode, _)) = tray.modes.iter().find(|(_, item)| id == *item.id()) {
                let mode = *mode;
                shared.update_config(|c| Config { mode, enabled: true, ..c.clone() });
            }
            tray.sync(shared);
        }
    }
}
