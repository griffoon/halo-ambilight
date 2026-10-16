#![windows_subsystem = "windows"]

mod art;
mod capture;
mod config;
mod daemon;
mod device;
mod effects;
mod engine;
mod ipc;
mod letterbox;
mod log;
mod openrgb;
mod proto;
mod ui;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let has = |flag: &str| args.iter().any(|a| a == flag);
    if has("--ui") {
        if let Err(e) = ui::run() {
            log::line(&format!("window failed: {e}"));
        }
    } else {
        daemon::run(!has("--background"));
    }
}
