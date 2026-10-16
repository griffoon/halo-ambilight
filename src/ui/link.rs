//! Window side of the loopback channel. Reconnects on its own if the service restarts.

use crate::proto::{endpoint_path, Endpoint, ToDaemon, ToUi};
use std::io::{self, BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

const RETRY: Duration = Duration::from_millis(800);
const CONNECT_TIMEOUT: Duration = Duration::from_millis(500);

#[derive(Default)]
struct Inner {
    writer: Mutex<Option<TcpStream>>,
    inbox: Mutex<Vec<ToUi>>,
    online: AtomicBool,
}

pub struct Link {
    inner: Arc<Inner>,
}

fn write_line(stream: &mut TcpStream, msg: &ToDaemon) -> io::Result<()> {
    let mut line = serde_json::to_string(msg)?;
    line.push('\n');
    stream.write_all(line.as_bytes())
}

fn connect() -> io::Result<TcpStream> {
    let endpoint: Endpoint = serde_json::from_str(&std::fs::read_to_string(endpoint_path())?)?;
    let mut stream = TcpStream::connect_timeout(&SocketAddr::from(([127, 0, 0, 1], endpoint.port)), CONNECT_TIMEOUT)?;
    stream.set_nodelay(true)?;
    write_line(&mut stream, &ToDaemon::Hello { token: endpoint.token })?;
    Ok(stream)
}

impl Link {
    pub fn start(ctx: egui::Context) -> Self {
        let inner = Arc::new(Inner::default());
        let worker = inner.clone();
        thread::spawn(move || loop {
            if let Ok(stream) = connect() {
                if let Ok(clone) = stream.try_clone() {
                    *worker.writer.lock().unwrap_or_else(|e| e.into_inner()) = Some(clone);
                    worker.online.store(true, Ordering::SeqCst);
                    ctx.request_repaint();
                    for line in BufReader::new(stream).lines() {
                        let Ok(line) = line else { break };
                        if let Ok(msg) = serde_json::from_str::<ToUi>(&line) {
                            worker.inbox.lock().unwrap_or_else(|e| e.into_inner()).push(msg);
                            ctx.request_repaint();
                        }
                    }
                }
                worker.online.store(false, Ordering::SeqCst);
                *worker.writer.lock().unwrap_or_else(|e| e.into_inner()) = None;
                ctx.request_repaint();
            }
            thread::sleep(RETRY);
        });
        Self { inner }
    }

    pub fn send(&self, msg: &ToDaemon) {
        let mut writer = self.inner.writer.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(stream) = writer.as_mut() {
            if write_line(stream, msg).is_err() {
                *writer = None;
            }
        }
    }

    pub fn drain(&self) -> Vec<ToUi> {
        std::mem::take(&mut *self.inner.inbox.lock().unwrap_or_else(|e| e.into_inner()))
    }

    pub fn online(&self) -> bool {
        self.inner.online.load(Ordering::SeqCst)
    }
}
