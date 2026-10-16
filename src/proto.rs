//! Types shared between the background service and the settings window.

use crate::config::{data_dir, Config};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub model: String,
    pub serial: String,
    pub port: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MonitorInfo {
    pub index: u32,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub primary: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Status {
    pub device: Option<DeviceInfo>,
    /// Human-readable note about the device search, empty when all is well.
    pub note: String,
    pub monitors: Vec<MonitorInfo>,
    pub capture_error: Option<String>,
    /// Device names reported by OpenRGB, when sync is on and the server answers.
    pub openrgb: Vec<String>,
    pub openrgb_note: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum ToDaemon {
    Hello { token: String },
    Set { config: Config },
    Rescan,
    Quit,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum ToUi {
    State {
        config: Config,
        status: Status,
        /// Service build; older builds omit it.
        #[serde(default)]
        version: String,
    },
    Leds { c: Vec<[u8; 3]> },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Endpoint {
    pub port: u16,
    pub token: String,
}

pub fn endpoint_path() -> PathBuf {
    data_dir().join("ipc.json")
}
