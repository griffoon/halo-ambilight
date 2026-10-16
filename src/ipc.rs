//! Loopback channel between the service and the settings window: newline-delimited JSON over TCP.
//! The port and a per-run token live in %APPDATA%\Halo\ipc.json.

use crate::engine::Shared;
use crate::log;
use crate::proto::{endpoint_path, Endpoint, ToDaemon, ToUi};
use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::io::{self, BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

const PUSH_INTERVAL: Duration = Duration::from_millis(33);
const HELLO_TIMEOUT: Duration = Duration::from_secs(2);

pub fn serve(shared: Arc<Shared>) -> io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let endpoint = Endpoint { port: listener.local_addr()?.port(), token: random_token() };
    std::fs::write(endpoint_path(), serde_json::to_string(&endpoint)?)?;
    let token = endpoint.token;
    thread::Builder::new().name("halo-ipc".into()).spawn(move || {
        for stream in listener.incoming().flatten() {
            let shared = shared.clone();
            let token = token.clone();
            let _ = thread::Builder::new().name("halo-ipc-client".into()).spawn(move || {
                if let Err(e) = client(&shared, stream, &token) {
                    log::line(&format!("ipc client ended: {e}"));
                }
            });
        }
    })?;
    Ok(())
}

fn random_token() -> String {
    (0..4u64)
        .map(|i| {
            let mut h = RandomState::new().build_hasher();
            h.write_u64(i);
            h.write_u32(std::process::id());
            format!("{:016x}", h.finish())
        })
        .collect()
}

fn send(stream: &mut TcpStream, msg: &ToUi) -> io::Result<()> {
    let mut line = serde_json::to_string(msg)?;
    line.push('\n');
    stream.write_all(line.as_bytes())
}

fn client(shared: &Arc<Shared>, mut stream: TcpStream, token: &str) -> io::Result<()> {
    stream.set_nodelay(true)?;
    stream.set_read_timeout(Some(HELLO_TIMEOUT))?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut hello = String::new();
    reader.read_line(&mut hello)?;
    match serde_json::from_str::<ToDaemon>(&hello) {
        Ok(ToDaemon::Hello { token: t }) if t == token => {}
        _ => return Ok(()),
    }
    stream.set_read_timeout(None)?;

    let alive = Arc::new(AtomicBool::new(true));
    {
        let shared = shared.clone();
        let alive = alive.clone();
        thread::spawn(move || {
            for line in reader.lines() {
                let Ok(line) = line else { break };
                match serde_json::from_str::<ToDaemon>(&line) {
                    Ok(ToDaemon::Set { config }) => shared.update_config(|_| config),
                    Ok(ToDaemon::Rescan) => shared.rescan.store(true, Ordering::SeqCst),
                    Ok(ToDaemon::Quit) => shared.request_quit(),
                    Ok(ToDaemon::Hello { .. }) | Err(_) => {}
                }
            }
            alive.store(false, Ordering::SeqCst);
        });
    }

    let (mut seen_cfg, mut seen_status, mut seen_leds) = (0, 0, 0);
    while alive.load(Ordering::SeqCst) && !shared.quit.load(Ordering::SeqCst) {
        let (c, s) = (shared.config_rev.load(Ordering::SeqCst), shared.status_rev.load(Ordering::SeqCst));
        if c != seen_cfg || s != seen_status {
            send(&mut stream, &ToUi::State { config: shared.config(), status: shared.status() })?;
            (seen_cfg, seen_status) = (c, s);
        }
        let l = shared.leds_rev.load(Ordering::SeqCst);
        if l != seen_leds {
            send(&mut stream, &ToUi::Leds { c: shared.leds() })?;
            seen_leds = l;
        }
        thread::sleep(PUSH_INTERVAL);
    }
    Ok(())
}
