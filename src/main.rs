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

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let has = |flag: &str| args.iter().any(|a| a == flag);
    if has("--ui") {
        daemon::open_ui();
    } else {
        daemon::run(!has("--background"));
    }
}
