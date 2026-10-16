//! Screen capture through DXGI Desktop Duplication.
//!
//! The full frame never leaves the GPU: it is copied into a mip-mapped texture,
//! the GPU builds the mip chain, and only a ~120 px wide level is read back.

use crate::proto::MonitorInfo;
use windows::core::{Interface, Result};
use windows::Win32::Foundation::HMODULE;
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_UNKNOWN;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::DXGI_SAMPLE_DESC;
use windows::Win32::Graphics::Dxgi::*;

/// Mip levels are halved until the width drops to this or below.
const TARGET_WIDTH: u32 = 160;

pub fn list_monitors() -> Vec<MonitorInfo> {
    let mut out = Vec::new();
    for_each_output(|index, _, output| {
        // SAFETY: `output` is a live IDXGIOutput returned by EnumOutputs.
        if let Ok(desc) = unsafe { output.GetDesc() } {
            let r = desc.DesktopCoordinates;
            let name_len = desc.DeviceName.iter().position(|&c| c == 0).unwrap_or(desc.DeviceName.len());
            out.push(MonitorInfo {
                index,
                name: String::from_utf16_lossy(&desc.DeviceName[..name_len]).trim_start_matches("\\\\.\\").to_string(),
                width: (r.right - r.left).unsigned_abs(),
                height: (r.bottom - r.top).unsigned_abs(),
                primary: r.left == 0 && r.top == 0,
            });
        }
        false
    });
    out
}

/// Walks every output of every adapter; the callback returns `true` to stop.
fn for_each_output(mut f: impl FnMut(u32, &IDXGIAdapter1, &IDXGIOutput) -> bool) {
    // SAFETY: plain COM factory creation and enumeration; all interfaces are reference counted.
    unsafe {
        let Ok(factory) = CreateDXGIFactory1::<IDXGIFactory1>() else { return };
        let mut index = 0;
        let mut a = 0;
        while let Ok(adapter) = factory.EnumAdapters1(a) {
            let mut o = 0;
            while let Ok(output) = adapter.EnumOutputs(o) {
                if f(index, &adapter, &output) {
                    return;
                }
                index += 1;
                o += 1;
            }
            a += 1;
        }
    }
}

struct Targets {
    mips: ID3D11Texture2D,
    view: ID3D11ShaderResourceView,
    staging: ID3D11Texture2D,
    src_w: u32,
    src_h: u32,
    level: u32,
    w: u32,
    h: u32,
}

pub struct Capturer {
    device: ID3D11Device,
    ctx: ID3D11DeviceContext,
    dupl: IDXGIOutputDuplication,
    targets: Option<Targets>,
    pixels: Vec<u8>,
    has_frame: bool,
}

impl Capturer {
    pub fn new(monitor: u32) -> std::result::Result<Self, String> {
        let mut found: Option<(IDXGIAdapter1, IDXGIOutput)> = None;
        for_each_output(|index, adapter, output| {
            if index == monitor {
                found = Some((adapter.clone(), output.clone()));
                return true;
            }
            false
        });
        let (adapter, output) = found.ok_or_else(|| format!("Монитор {} не найден", monitor + 1))?;
        Self::open(&adapter, &output).map_err(|e| format!("Захват экрана недоступен: {}", e.message()))
    }

    fn open(adapter: &IDXGIAdapter1, output: &IDXGIOutput) -> Result<Self> {
        let mut device = None;
        let mut ctx = None;
        // SAFETY: out-pointers are valid locals; the adapter outlives the call.
        unsafe {
            D3D11CreateDevice(
                adapter,
                D3D_DRIVER_TYPE_UNKNOWN,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut ctx),
            )?;
        }
        let device = device.ok_or_else(windows::core::Error::empty)?;
        let ctx = ctx.ok_or_else(windows::core::Error::empty)?;
        let output1: IDXGIOutput1 = output.cast()?;
        // SAFETY: device and output belong to the same adapter.
        let dupl = unsafe { output1.DuplicateOutput(&device)? };
        Ok(Self { device, ctx, dupl, targets: None, pixels: Vec::new(), has_frame: false })
    }

    /// Pulls the newest desktop frame if one arrived since the last call. Never blocks.
    pub fn grab(&mut self) -> Result<()> {
        let mut info = DXGI_OUTDUPL_FRAME_INFO::default();
        let mut resource: Option<IDXGIResource> = None;
        // SAFETY: out-pointers are valid locals.
        match unsafe { self.dupl.AcquireNextFrame(0, &mut info, &mut resource) } {
            Ok(()) => {}
            Err(e) if e.code() == DXGI_ERROR_WAIT_TIMEOUT => return Ok(()),
            Err(e) => return Err(e),
        }
        let result = match resource {
            // A frame with LastPresentTime == 0 only carries a cursor update.
            Some(res) if info.LastPresentTime != 0 || !self.has_frame => res.cast::<ID3D11Texture2D>().and_then(|t| self.downsample(&t)),
            _ => Ok(()),
        };
        // SAFETY: a frame was acquired above and must be released exactly once.
        unsafe {
            let _ = self.dupl.ReleaseFrame();
        }
        result
    }

    /// Latest downsampled frame as tightly packed BGRA.
    pub fn frame(&self) -> Option<(&[u8], u32, u32)> {
        let t = self.targets.as_ref()?;
        self.has_frame.then_some((self.pixels.as_slice(), t.w, t.h))
    }

    fn downsample(&mut self, src: &ID3D11Texture2D) -> Result<()> {
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        // SAFETY: valid texture and out-pointer.
        unsafe { src.GetDesc(&mut desc) };
        let stale = self.targets.as_ref().map_or(true, |t| t.src_w != desc.Width || t.src_h != desc.Height);
        if stale {
            self.targets = Some(self.build_targets(&desc)?);
        }
        let Some(t) = self.targets.as_ref() else { return Ok(()) };

        // SAFETY: all resources were created on `self.device`; the staging texture is CPU-readable
        // and is unmapped before returning.
        unsafe {
            self.ctx.CopySubresourceRegion(&t.mips, 0, 0, 0, 0, src, 0, None);
            self.ctx.GenerateMips(&t.view);
            self.ctx.CopySubresourceRegion(&t.staging, 0, 0, 0, 0, &t.mips, t.level, None);

            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            self.ctx.Map(&t.staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))?;
            let row = (t.w * 4) as usize;
            self.pixels.resize(row * t.h as usize, 0);
            let base = mapped.pData as *const u8;
            for y in 0..t.h as usize {
                let line = std::slice::from_raw_parts(base.add(y * mapped.RowPitch as usize), row);
                self.pixels[y * row..(y + 1) * row].copy_from_slice(line);
            }
            self.ctx.Unmap(&t.staging, 0);
        }
        self.has_frame = true;
        Ok(())
    }

    fn build_targets(&self, src: &D3D11_TEXTURE2D_DESC) -> Result<Targets> {
        let mut level = 0;
        while (src.Width >> level) > TARGET_WIDTH && level < 12 {
            level += 1;
        }
        let w = (src.Width >> level).max(1);
        let h = (src.Height >> level).max(1);

        let mips_desc = D3D11_TEXTURE2D_DESC {
            Width: src.Width,
            Height: src.Height,
            MipLevels: level + 1,
            ArraySize: 1,
            Format: src.Format,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: (D3D11_BIND_RENDER_TARGET.0 | D3D11_BIND_SHADER_RESOURCE.0) as u32,
            CPUAccessFlags: 0,
            MiscFlags: D3D11_RESOURCE_MISC_GENERATE_MIPS.0 as u32,
        };
        let staging_desc = D3D11_TEXTURE2D_DESC {
            Width: w,
            Height: h,
            MipLevels: 1,
            Usage: D3D11_USAGE_STAGING,
            BindFlags: 0,
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
            MiscFlags: 0,
            ..mips_desc
        };

        let mut mips = None;
        let mut staging = None;
        let mut view = None;
        // SAFETY: descriptors are fully initialised and out-pointers are valid locals.
        unsafe {
            self.device.CreateTexture2D(&mips_desc, None, Some(&mut mips))?;
            self.device.CreateTexture2D(&staging_desc, None, Some(&mut staging))?;
            let mips_ref = mips.as_ref().ok_or_else(windows::core::Error::empty)?;
            self.device.CreateShaderResourceView(mips_ref, None, Some(&mut view))?;
        }
        Ok(Targets {
            mips: mips.ok_or_else(windows::core::Error::empty)?,
            view: view.ok_or_else(windows::core::Error::empty)?,
            staging: staging.ok_or_else(windows::core::Error::empty)?,
            src_w: src.Width,
            src_h: src.Height,
            level,
            w,
            h,
        })
    }
}
