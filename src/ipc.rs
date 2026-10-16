//! Loopback channel between the service and the settings window: newline-delimited JSON over TCP.
//! The port and a per-run token live in %APPDATA%\Halo\ipc.json.

use crate::engine::Shared;
use crate::daemon::VERSION;
use crate::log;
use crate::proto::{endpoint_path, Endpoint, ToDaemon, ToUi};
use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::io::{self, BufRead, BufReader, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

const PUSH_INTERVAL: Duration = Duration::from_millis(33);
const HELLO_TIMEOUT: Duration = Duration::from_secs(2);
const CONNECT_TIMEOUT: Duration = Duration::from_millis(500);

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

fn write_line(stream: &mut TcpStream, msg: &ToDaemon) -> io::Result<()> {
    let mut line = serde_json::to_string(msg)?;
    line.push('\n');
    stream.write_all(line.as_bytes())
}

/// Client side: connects to the running service and says hello.
pub fn connect() -> io::Result<TcpStream> {
    let endpoint: Endpoint = serde_json::from_str(&std::fs::read_to_string(endpoint_path())?)?;
    let addr = SocketAddr::from(([127, 0, 0, 1], endpoint.port));
    let mut stream = TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT)?;
    stream.set_nodelay(true)?;
    write_line(&mut stream, &ToDaemon::Hello { token: endpoint.token })?;
    Ok(stream)
}

/// Version of the service that is running now; empty for builds that predate the field.
pub fn running_version() -> Option<String> {
    let stream = connect().ok()?;
    stream.set_read_timeout(Some(HELLO_TIMEOUT)).ok()?;
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line).ok()?;
    match serde_json::from_str::<ToUi>(&line).ok()? {
        ToUi::State { version, .. } => Some(version),
        ToUi::Leds { .. } => None,
    }
}

pub fn request_quit() {
    let Ok(mut stream) = connect() else { return };
    if write_line(&mut stream, &ToDaemon::Quit).is_err() {
        return;
    }
    // Closing while the service's greeting is still unread makes Windows reset the connection,
    // and the service can lose the Quit before reading it. Half-close and drain instead.
    let _ = stream.shutdown(Shutdown::Write);
    let _ = stream.set_read_timeout(Some(HELLO_TIMEOUT));
    let _ = io::copy(&mut stream, &mut io::sink());
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
    // The timeout goes on the very handle that reads: on Windows a cloned socket keeps its own
    // SO_RCVTIMEO, and a stale 2 s timeout here used to drop every idle window after 2 seconds.
    let mut reader = BufReader::new(stream.try_clone()?);
    reader.get_ref().set_read_timeout(Some(HELLO_TIMEOUT))?;
    let mut hello = String::new();
    reader.read_line(&mut hello)?;
    match serde_json::from_str::<ToDaemon>(&hello) {
        Ok(ToDaemon::Hello { token: t }) if t == token => {}
        _ => return Ok(()),
    }
    reader.get_ref().set_read_timeout(None)?;

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
            send(&mut stream, &ToUi::State { config: shared.config(), status: shared.status(), version: VERSION.into() })?;
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
