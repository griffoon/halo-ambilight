//! Optional sync of other RGB gear (keyboard, motherboard, ...) through a running OpenRGB SDK server.
//!
//! Off by default, and only devices the user ticks are touched, so nothing gets hijacked.
//! Each selected device is switched to its direct ("custom") mode and painted with the
//! strip's average colour.

use crate::engine::Shared;
use crate::log;
use crate::proto::Status;
use std::collections::HashSet;
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

const MAGIC: &[u8; 4] = b"ORGB";
const REQUEST_CONTROLLER_COUNT: u32 = 0;
const REQUEST_CONTROLLER_DATA: u32 = 1;
const SET_CLIENT_NAME: u32 = 50;
const UPDATE_LEDS: u32 = 1050;
const SET_CUSTOM_MODE: u32 = 1100;

const TICK: Duration = Duration::from_millis(33);
const RETRY: Duration = Duration::from_secs(4);
const KEEPALIVE: Duration = Duration::from_secs(2);

struct Controller {
    index: u32,
    name: String,
    leds: u16,
}

struct Client {
    stream: TcpStream,
}

impl Client {
    fn connect(port: u16) -> io::Result<Self> {
        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        let stream = TcpStream::connect_timeout(&addr, Duration::from_millis(300))?;
        stream.set_read_timeout(Some(Duration::from_secs(2)))?;
        stream.set_nodelay(true)?;
        let mut client = Self { stream };
        client.send(0, SET_CLIENT_NAME, b"Halo\0")?;
        Ok(client)
    }

    fn send(&mut self, device: u32, id: u32, data: &[u8]) -> io::Result<()> {
        let mut packet = Vec::with_capacity(16 + data.len());
        packet.extend_from_slice(MAGIC);
        packet.extend_from_slice(&device.to_le_bytes());
        packet.extend_from_slice(&id.to_le_bytes());
        packet.extend_from_slice(&(data.len() as u32).to_le_bytes());
        packet.extend_from_slice(data);
        self.stream.write_all(&packet)
    }

    /// Sends a request and waits for the reply with the same packet id, skipping notifications.
    fn request(&mut self, device: u32, id: u32, data: &[u8]) -> io::Result<Vec<u8>> {
        self.send(device, id, data)?;
        loop {
            let mut header = [0u8; 16];
            self.stream.read_exact(&mut header)?;
            if &header[..4] != MAGIC {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "not an OpenRGB server"));
            }
            let reply_id = u32::from_le_bytes([header[8], header[9], header[10], header[11]]);
            let size = u32::from_le_bytes([header[12], header[13], header[14], header[15]]) as usize;
            let mut body = vec![0u8; size];
            self.stream.read_exact(&mut body)?;
            if reply_id == id {
                return Ok(body);
            }
        }
    }

    fn controllers(&mut self) -> io::Result<Vec<Controller>> {
        let count = self.request(0, REQUEST_CONTROLLER_COUNT, &[])?;
        let count = u32::from_le_bytes(count.get(..4).and_then(|b| b.try_into().ok()).unwrap_or([0; 4]));
        let mut out = Vec::new();
        for index in 0..count.min(64) {
            let data = self.request(index, REQUEST_CONTROLLER_DATA, &[])?;
            if let Some((name, leds)) = parse_controller(&data) {
                out.push(Controller { index, name, leds });
            }
        }
        Ok(out)
    }

    fn paint(&mut self, c: &Controller, color: [u8; 3]) -> io::Result<()> {
        let n = c.leds as usize;
        let mut data = Vec::with_capacity(6 + 4 * n);
        data.extend_from_slice(&((6 + 4 * n) as u32).to_le_bytes());
        data.extend_from_slice(&c.leds.to_le_bytes());
        for _ in 0..n {
            data.extend_from_slice(&[color[0], color[1], color[2], 0]);
        }
        self.send(c.index, UPDATE_LEDS, &data)
    }
}

struct Reader<'a> {
    b: &'a [u8],
    p: usize,
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Option<&[u8]> {
        let s = self.b.get(self.p..self.p + n)?;
        self.p += n;
        Some(s)
    }
    fn u16(&mut self) -> Option<u16> {
        self.take(2).map(|s| u16::from_le_bytes([s[0], s[1]]))
    }
    fn u32(&mut self) -> Option<u32> {
        self.take(4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }
    fn string(&mut self) -> Option<String> {
        let len = self.u16()? as usize;
        let raw = self.take(len)?;
        Some(String::from_utf8_lossy(raw).trim_end_matches('\0').to_string())
    }
}

/// Reads name and LED count from a protocol-0 controller blob.
fn parse_controller(data: &[u8]) -> Option<(String, u16)> {
    let mut r = Reader { b: data, p: 0 };
    r.u32()?; // blob size
    r.u32()?; // device type
    let name = r.string()?;
    for _ in 0..4 {
        r.string()?; // description, version, serial, location
    }
    let modes = r.u16()?;
    r.u32()?; // active mode
    for _ in 0..modes {
        r.string()?;
        r.take(4 * 9)?; // value, flags, speed min/max, colours min/max, speed, direction, colour mode
        let colors = r.u16()? as usize;
        r.take(4 * colors)?;
    }
    let zones = r.u16()?;
    for _ in 0..zones {
        r.string()?;
        r.take(4 * 4)?; // type, leds min/max/count
        let matrix = r.u16()? as usize;
        r.take(matrix)?;
    }
    let leds = r.u16()?;
    Some((name, leds))
}

pub fn spawn(shared: Arc<Shared>) {
    let _ = thread::Builder::new().name("halo-openrgb".into()).spawn(move || run(&shared));
}

struct Session {
    client: Client,
    controllers: Vec<Controller>,
    prepared: HashSet<u32>,
    last: Option<[u8; 3]>,
    last_at: Instant,
}

fn run(shared: &Shared) {
    let mut session: Option<Session> = None;
    let mut retry_at = Instant::now();
    while !shared.quit.load(Ordering::SeqCst) {
        let cfg = shared.config();
        if !cfg.openrgb.enabled {
            session = None;
            thread::sleep(Duration::from_millis(400));
            continue;
        }
        if session.is_none() && Instant::now() >= retry_at {
            match open(cfg.openrgb.port) {
                Ok(s) => {
                    let names: Vec<String> = s.controllers.iter().map(|c| c.name.clone()).collect();
                    shared.update_status(|st| Status { openrgb: names, openrgb_note: String::new(), ..st.clone() });
                    session = Some(s);
                }
                Err(e) => {
                    let note = format!("OpenRGB недоступен ({e}). Запусти его с включённым SDK-сервером");
                    shared.update_status(|st| Status { openrgb: Vec::new(), openrgb_note: note, ..st.clone() });
                    retry_at = Instant::now() + RETRY;
                }
            }
        }
        if let Some(s) = session.as_mut() {
            if let Err(e) = tick(s, &cfg.openrgb.devices, shared.ambient()) {
                log::line(&format!("openrgb: {e}"));
                session = None;
                retry_at = Instant::now() + RETRY;
            }
        }
        thread::sleep(TICK);
    }
}

fn open(port: u16) -> io::Result<Session> {
    let mut client = Client::connect(port)?;
    let controllers = client.controllers()?;
    Ok(Session { client, controllers, prepared: HashSet::new(), last: None, last_at: Instant::now() })
}

fn tick(s: &mut Session, selected: &[String], color: [u8; 3]) -> io::Result<()> {
    let due = s.last != Some(color) || s.last_at.elapsed() >= KEEPALIVE;
    if !due {
        return Ok(());
    }
    for c in s.controllers.iter().filter(|c| selected.contains(&c.name)) {
        if s.prepared.insert(c.index) {
            s.client.send(c.index, SET_CUSTOM_MODE, &[])?;
        }
        s.client.paint(c, color)?;
    }
    s.prepared.retain(|i| s.controllers.iter().any(|c| c.index == *i && selected.contains(&c.name)));
    s.last = Some(color);
    s.last_at = Instant::now();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn string(out: &mut Vec<u8>, s: &str) {
        out.extend_from_slice(&((s.len() + 1) as u16).to_le_bytes());
        out.extend_from_slice(s.as_bytes());
        out.push(0);
    }

    #[test]
    fn parses_name_and_led_count() {
        let mut d = Vec::new();
        d.extend_from_slice(&0u32.to_le_bytes());
        d.extend_from_slice(&5u32.to_le_bytes());
        string(&mut d, "Keychron K2");
        for s in ["desc", "1.0", "SN", "HID"] {
            string(&mut d, s);
        }
        d.extend_from_slice(&1u16.to_le_bytes()); // one mode
        d.extend_from_slice(&0u32.to_le_bytes());
        string(&mut d, "Direct");
        d.extend_from_slice(&[0u8; 36]);
        d.extend_from_slice(&0u16.to_le_bytes());
        d.extend_from_slice(&1u16.to_le_bytes()); // one zone
        string(&mut d, "Keys");
        d.extend_from_slice(&[0u8; 16]);
        d.extend_from_slice(&0u16.to_le_bytes());
        d.extend_from_slice(&84u16.to_le_bytes());
        assert_eq!(parse_controller(&d), Some(("Keychron K2".to_string(), 84)));
    }
}
