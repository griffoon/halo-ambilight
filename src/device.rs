//! Skydimo serial protocol, recovered from SkyDimo 1.2.1:
//!
//! * 115200 8N1 over a CH340 bridge.
//! * Identify: host sends `Moni-A`, the strip answers `SK0124,<7 raw serial bytes>\r\n`.
//! * Frame: `A d a 0x00 <count hi> <count lo>` followed by `count` RGB triplets.

use crate::proto::DeviceInfo;
use serialport::{ClearBuffer, SerialPort, SerialPortType};
use std::io::{self, Read, Write};
use std::time::{Duration, Instant};

const BAUD: u32 = 115_200;
const CH340_VID: u16 = 0x1A86;
const HANDSHAKE: &[u8] = b"Moni-A";
const PROBE_WINDOW: Duration = Duration::from_millis(400);

pub struct Device {
    port: Box<dyn SerialPort>,
    pub info: DeviceInfo,
    frame: Vec<u8>,
}

impl Device {
    pub fn send(&mut self, colors: &[[u8; 3]]) -> io::Result<()> {
        let n = colors.len();
        self.frame.clear();
        self.frame.extend_from_slice(&[b'A', b'd', b'a', 0, (n >> 8) as u8, (n & 0xFF) as u8]);
        colors.iter().for_each(|c| self.frame.extend_from_slice(c));
        self.port.write_all(&self.frame)?;
        self.port.flush()
    }
}

enum ProbeError {
    Busy,
    Silent,
}

/// Looks for a strip, trying the last known port first, then CH340 bridges, then other USB serial ports.
pub fn discover(hint: Option<&str>) -> Result<Device, String> {
    let ports = serialport::available_ports().map_err(|e| format!("Не удалось получить список портов: {e}"))?;

    let mut candidates: Vec<(u8, String)> = ports
        .into_iter()
        .filter_map(|p| {
            let rank = match &p.port_type {
                _ if Some(p.port_name.as_str()) == hint => 0,
                SerialPortType::UsbPort(usb) if usb.vid == CH340_VID => 1,
                SerialPortType::UsbPort(_) => 2,
                _ => return None,
            };
            Some((rank, p.port_name))
        })
        .collect();
    candidates.sort();

    let mut busy: Option<String> = None;
    for (_, name) in candidates {
        match probe(&name) {
            Ok(device) => return Ok(device),
            Err(ProbeError::Busy) => busy = busy.or(Some(name)),
            Err(ProbeError::Silent) => {}
        }
    }
    Err(match busy {
        Some(port) => format!("{port} занят другой программой. Закрой SkyDimo"),
        None => "Лента не найдена. Проверь USB".to_string(),
    })
}

fn probe(name: &str) -> Result<Device, ProbeError> {
    let mut port = serialport::new(name, BAUD).timeout(Duration::from_millis(50)).open().map_err(|e| {
        let text = e.to_string().to_lowercase();
        let denied = matches!(e.kind(), serialport::ErrorKind::Io(io::ErrorKind::PermissionDenied))
            || text.contains("denied")
            || text.contains("отказано");
        if denied {
            ProbeError::Busy
        } else {
            ProbeError::Silent
        }
    })?;

    let _ = port.clear(ClearBuffer::All);
    port.write_all(HANDSHAKE).map_err(|_| ProbeError::Silent)?;

    let deadline = Instant::now() + PROBE_WINDOW;
    let mut reply = Vec::with_capacity(32);
    let mut chunk = [0u8; 64];
    while Instant::now() < deadline {
        match port.read(&mut chunk) {
            Ok(n) if n > 0 => {
                reply.extend_from_slice(&chunk[..n]);
                if reply.len() > 8 && reply.ends_with(b"\r\n") {
                    break;
                }
            }
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::TimedOut => {}
            Err(_) => return Err(ProbeError::Silent),
        }
    }

    let (model, serial) = parse_identity(&reply).ok_or(ProbeError::Silent)?;
    let _ = port.set_timeout(Duration::from_millis(200));
    Ok(Device {
        port,
        info: DeviceInfo { model, serial, port: name.to_string() },
        frame: Vec::with_capacity(6 + 3 * 300),
    })
}

fn parse_identity(reply: &[u8]) -> Option<(String, String)> {
    let start = reply.windows(2).position(|w| w == b"SK")?;
    let rest = &reply[start..];
    let comma = rest.iter().position(|&b| b == b',')?;
    let model = String::from_utf8_lossy(&rest[..comma]).trim().to_string();
    let tail = rest[comma + 1..].strip_suffix(b"\r\n").unwrap_or(&rest[comma + 1..]);
    let serial = tail.iter().map(|b| format!("{b:02X}")).collect();
    Some((model, serial))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_identity_reply_from_sk0124() {
        let reply = b"SK0124,<\x12\"\x03\x0c\x15\x02\r\n";
        let (model, serial) = parse_identity(reply).unwrap();
        assert_eq!(model, "SK0124");
        assert_eq!(serial, "3C1222030C1502");
    }

    #[test]
    fn rejects_noise() {
        assert!(parse_identity(b"\x00\x01garbage").is_none());
    }
}
