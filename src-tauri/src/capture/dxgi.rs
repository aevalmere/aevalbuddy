//! DXGI Desktop Duplication: the composed desktop of one output, as DWM
//! hands it out. Windows that set WDA_EXCLUDEFROMCAPTURE (all of ours) are
//! left out of it, and no capture border is drawn.
//!
//! A duplication is kept per output and reused: creating one costs tens of
//! milliseconds, and once it exists a frame that has not changed comes back
//! at once (the last copy is kept). Objects live on the capture thread only.

use std::collections::HashMap;
use std::sync::Arc;

use windows::core::Interface;
use windows::Win32::Foundation::HMODULE;
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_UNKNOWN;
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
    D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAPPED_SUBRESOURCE,
    D3D11_MAP_READ, D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_MODE_ROTATION_IDENTITY, DXGI_MODE_ROTATION_UNSPECIFIED,
    DXGI_SAMPLE_DESC,
};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, IDXGIAdapter, IDXGIFactory1, IDXGIOutput1, IDXGIOutputDuplication,
    IDXGIResource, DXGI_ERROR_WAIT_TIMEOUT, DXGI_OUTDUPL_FRAME_INFO,
};

use super::screen::wide;
use super::Frame;

struct Dup {
    ctx: ID3D11DeviceContext,
    device: ID3D11Device,
    dup: IDXGIOutputDuplication,
    staging: Option<(ID3D11Texture2D, u32, u32)>,
    last: Option<Arc<Frame>>,
}

#[derive(Default)]
pub struct Cache {
    dups: HashMap<String, Dup>,
}

fn open(device_name: &str) -> Result<Dup, String> {
    unsafe {
        let factory: IDXGIFactory1 = CreateDXGIFactory1().map_err(|e| format!("factory: {e}"))?;
        let mut a = 0;
        while let Ok(adapter) = factory.EnumAdapters1(a) {
            a += 1;
            let mut o = 0;
            loop {
                let output = match adapter.EnumOutputs(o) {
                    Ok(x) => x,
                    Err(_) => break,
                };
                o += 1;
                let desc = match output.GetDesc() {
                    Ok(d) => d,
                    Err(_) => continue,
                };
                if !wide(&desc.DeviceName).eq_ignore_ascii_case(device_name) {
                    continue;
                }
                // A rotated output arrives in panel orientation; GDI gives
                // the desktop as the user sees it, so leave those to GDI.
                if desc.Rotation != DXGI_MODE_ROTATION_IDENTITY
                    && desc.Rotation != DXGI_MODE_ROTATION_UNSPECIFIED
                {
                    return Err("rotated output".into());
                }
                let base: IDXGIAdapter = adapter.cast().map_err(|e| format!("adapter: {e}"))?;
                let mut device: Option<ID3D11Device> = None;
                let mut ctx: Option<ID3D11DeviceContext> = None;
                D3D11CreateDevice(
                    &base,
                    D3D_DRIVER_TYPE_UNKNOWN,
                    HMODULE::default(),
                    D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                    None,
                    D3D11_SDK_VERSION,
                    Some(&mut device),
                    None,
                    Some(&mut ctx),
                )
                .map_err(|e| format!("device: {e}"))?;
                let device = device.ok_or("no device")?;
                let ctx = ctx.ok_or("no context")?;
                let output1: IDXGIOutput1 = output.cast().map_err(|e| format!("output1: {e}"))?;
                let dup = output1
                    .DuplicateOutput(&device)
                    .map_err(|e| format!("duplicate: {e}"))?;
                return Ok(Dup {
                    ctx,
                    device,
                    dup,
                    staging: None,
                    last: None,
                });
            }
        }
        Err(format!("no DXGI output named {device_name}"))
    }
}

impl Dup {
    /// The newest frame, or the last one when nothing changed since.
    fn frame(&mut self) -> Result<Arc<Frame>, String> {
        // With a frame in hand a static screen answers immediately; a fresh
        // duplication gets a moment to produce its first image.
        let tries: &[u32] = if self.last.is_some() { &[0] } else { &[300, 300, 300] };
        for &timeout in tries {
            let mut info = DXGI_OUTDUPL_FRAME_INFO::default();
            let mut res: Option<IDXGIResource> = None;
            match unsafe { self.dup.AcquireNextFrame(timeout, &mut info, &mut res) } {
                Ok(()) => {}
                Err(e) if e.code() == DXGI_ERROR_WAIT_TIMEOUT => {
                    if let Some(last) = &self.last {
                        return Ok(last.clone());
                    }
                    continue;
                }
                Err(e) => return Err(format!("acquire: {e}")),
            }
            let copied = if info.LastPresentTime == 0 && self.last.is_some() {
                // Only the pointer moved; the image is the one we have.
                Ok(None)
            } else {
                self.copy(res).map(Some)
            };
            unsafe {
                let _ = self.dup.ReleaseFrame();
            }
            match copied {
                Ok(Some(f)) => {
                    let f = Arc::new(f);
                    self.last = Some(f.clone());
                    return Ok(f);
                }
                Ok(None) => {
                    if let Some(last) = &self.last {
                        return Ok(last.clone());
                    }
                }
                Err(e) => return Err(e),
            }
        }
        Err("no frame from DXGI".into())
    }

    fn copy(&mut self, res: Option<IDXGIResource>) -> Result<Frame, String> {
        unsafe {
            let tex: ID3D11Texture2D = res
                .ok_or("no resource")?
                .cast()
                .map_err(|e| format!("texture: {e}"))?;
            let mut desc = D3D11_TEXTURE2D_DESC::default();
            tex.GetDesc(&mut desc);
            if desc.Format != DXGI_FORMAT_B8G8R8A8_UNORM {
                return Err(format!("unexpected format {}", desc.Format.0));
            }
            let (w, h) = (desc.Width, desc.Height);
            let fresh = !matches!(&self.staging, Some((_, sw, sh)) if *sw == w && *sh == h);
            if fresh {
                let sdesc = D3D11_TEXTURE2D_DESC {
                    Width: w,
                    Height: h,
                    MipLevels: 1,
                    ArraySize: 1,
                    Format: desc.Format,
                    SampleDesc: DXGI_SAMPLE_DESC {
                        Count: 1,
                        Quality: 0,
                    },
                    Usage: D3D11_USAGE_STAGING,
                    BindFlags: 0,
                    CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
                    MiscFlags: 0,
                };
                let mut staging: Option<ID3D11Texture2D> = None;
                self.device
                    .CreateTexture2D(&sdesc, None, Some(&mut staging))
                    .map_err(|e| format!("staging: {e}"))?;
                self.staging = Some((staging.ok_or("no staging")?, w, h));
            }
            let staging = &self.staging.as_ref().ok_or("no staging")?.0;
            self.ctx.CopyResource(staging, &tex);
            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            self.ctx
                .Map(staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
                .map_err(|e| format!("map: {e}"))?;
            let pitch = mapped.RowPitch as usize;
            let row = w as usize * 4;
            let mut bgra = vec![0u8; row * h as usize];
            let src = mapped.pData as *const u8;
            for y in 0..h as usize {
                std::ptr::copy_nonoverlapping(src.add(y * pitch), bgra.as_mut_ptr().add(y * row), row);
            }
            self.ctx.Unmap(staging, 0);
            Ok(Frame {
                width: w,
                height: h,
                bgra,
            })
        }
    }
}

impl Cache {
    pub fn grab(&mut self, device_name: &str) -> Result<Arc<Frame>, String> {
        if !self.dups.contains_key(device_name) {
            let d = open(device_name)?;
            self.dups.insert(device_name.to_string(), d);
        }
        let result = self
            .dups
            .get_mut(device_name)
            .ok_or("duplication vanished")?
            .frame();
        if result.is_err() {
            // Access lost (mode change, secure desktop, UAC): start over next
            // time with a fresh duplication.
            self.dups.remove(device_name);
        }
        result
    }
}
