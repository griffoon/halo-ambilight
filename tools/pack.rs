//! Packaging helper: renders the icon to `assets/halo.ico` and stamps it into a built exe.
//!
//! The GNU toolchain has no `windres`, so the icon resources are written with UpdateResourceW.
//!
//! Usage: `halo-pack <path-to-halo.exe>`

#[path = "../src/art.rs"]
mod art;

use std::fs;
use std::path::PathBuf;
use windows::core::{HSTRING, PCWSTR};
use windows::Win32::System::LibraryLoader::{BeginUpdateResourceW, EndUpdateResourceW, UpdateResourceW};

const SIZES: [u32; 8] = [16, 20, 24, 32, 40, 48, 64, 256];
const RT_ICON: u16 = 3;
const RT_GROUP_ICON: u16 = 14;
const RT_MANIFEST: u16 = 24;
const LANG_NEUTRAL: u16 = 0;

/// Common Controls v6 is required: the tray menu imports SetWindowSubclass by name,
/// which the default v5 comctl32 only exports by ordinal.
const MANIFEST: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <assemblyIdentity type="win32" name="Halo" version="0.9.0.0"/>
  <dependency>
    <dependentAssembly>
      <assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0"
        processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*"/>
    </dependentAssembly>
  </dependency>
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
      <supportedOS Id="{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}"/>
    </application>
  </compatibility>
  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>
      <dpiAware xmlns="http://schemas.microsoft.com/SMI/2005/WindowsSettings">true/pm</dpiAware>
      <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">PerMonitorV2</dpiAwareness>
      <activeCodePage xmlns="http://schemas.microsoft.com/SMI/2019/WindowsSettings">UTF-8</activeCodePage>
    </windowsSettings>
  </application>
</assembly>
"#;

/// 32-bit BGRA DIB as stored inside .ico files: bottom-up rows, doubled height, empty AND mask.
fn dib(size: u32, rgba: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let push32 = |v: &mut Vec<u8>, x: u32| v.extend_from_slice(&x.to_le_bytes());
    push32(&mut out, 40);
    push32(&mut out, size);
    push32(&mut out, size * 2);
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&32u16.to_le_bytes());
    for _ in 0..6 {
        push32(&mut out, 0);
    }
    for y in (0..size).rev() {
        for x in 0..size {
            let i = ((y * size + x) * 4) as usize;
            out.extend_from_slice(&[rgba[i + 2], rgba[i + 1], rgba[i], rgba[i + 3]]);
        }
    }
    let mask_row = size.div_ceil(32) * 4;
    out.resize(out.len() + (mask_row * size) as usize, 0);
    out
}

fn dir_entry(size: u32, bytes: usize) -> [u8; 12] {
    let dim = if size >= 256 { 0 } else { size as u8 };
    let mut e = [0u8; 12];
    e[0] = dim;
    e[1] = dim;
    e[4..6].copy_from_slice(&1u16.to_le_bytes());
    e[6..8].copy_from_slice(&32u16.to_le_bytes());
    e[8..12].copy_from_slice(&(bytes as u32).to_le_bytes());
    e
}

fn main() {
    let images: Vec<(u32, Vec<u8>)> = SIZES.iter().map(|&s| (s, dib(s, &art::render(s)))).collect();

    let header = |count: usize| {
        let mut h = Vec::new();
        h.extend_from_slice(&0u16.to_le_bytes());
        h.extend_from_slice(&1u16.to_le_bytes());
        h.extend_from_slice(&(count as u16).to_le_bytes());
        h
    };

    // .ico file
    let mut ico = header(images.len());
    let mut offset = 6 + 16 * images.len();
    for (size, data) in &images {
        ico.extend_from_slice(&dir_entry(*size, data.len()));
        ico.extend_from_slice(&(offset as u32).to_le_bytes());
        offset += data.len();
    }
    images.iter().for_each(|(_, d)| ico.extend_from_slice(d));
    let assets = PathBuf::from("assets");
    let _ = fs::create_dir_all(&assets);
    fs::write(assets.join("halo.ico"), &ico).expect("write assets/halo.ico");
    println!("wrote assets/halo.ico");

    let Some(exe) = std::env::args().nth(1) else { return };
    let mut group = header(images.len());
    for (i, (size, data)) in images.iter().enumerate() {
        group.extend_from_slice(&dir_entry(*size, data.len()));
        group.extend_from_slice(&((i + 1) as u16).to_le_bytes());
    }

    let int_res = |id: u16| PCWSTR(id as usize as *const u16);
    // SAFETY: resource ids are MAKEINTRESOURCE values, data buffers outlive each call,
    // and the update handle is closed with EndUpdateResourceW.
    unsafe {
        let handle = BeginUpdateResourceW(&HSTRING::from(exe.as_str()), false).expect("open exe for resources");
        for (i, (_, data)) in images.iter().enumerate() {
            UpdateResourceW(handle, int_res(RT_ICON), int_res((i + 1) as u16), LANG_NEUTRAL, Some(data.as_ptr().cast()), data.len() as u32)
                .expect("write icon image");
        }
        UpdateResourceW(handle, int_res(RT_GROUP_ICON), int_res(1), LANG_NEUTRAL, Some(group.as_ptr().cast()), group.len() as u32)
            .expect("write icon group");
        UpdateResourceW(handle, int_res(RT_MANIFEST), int_res(1), LANG_NEUTRAL, Some(MANIFEST.as_ptr().cast()), MANIFEST.len() as u32)
            .expect("write manifest");
        EndUpdateResourceW(handle, false).expect("commit resources");
    }
    println!("stamped icon and manifest into {exe}");
}
