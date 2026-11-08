//! DXGI Desktop Duplication: the composed desktop of one output, as DWM
//! hands it out. Windows that set WDA_EXCLUDEFROMCAPTURE (all of ours) are
//! left out of it, and no capture border is drawn.
//!
//! A duplication is kept per output and reused: creating one costs tens of
//! milliseconds, and once it exists a frame that has not changed comes back
//! at once (the last copy is kept). Objects live on the capture thread only.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use windows::core::Interface;
use windows::Win32::Foundation::HMODULE;
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_UNKNOWN;
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D, D3D11_CPU_ACCESS_READ,
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAPPED_SUBRESOURCE, D3D11_MAP_READ, D3D11_SDK_VERSION,
    D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_COLOR_SPACE_RGB_FULL_G2084_NONE_P2020, DXGI_FORMAT_B8G8R8A8_UNORM,
    DXGI_FORMAT_R16G16B16A16_FLOAT, DXGI_MODE_ROTATION_IDENTITY, DXGI_MODE_ROTATION_UNSPECIFIED,
    DXGI_SAMPLE_DESC,
};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, IDXGIAdapter, IDXGIFactory1, IDXGIOutput, IDXGIOutput1, IDXGIOutput5,
    IDXGIOutput6, IDXGIOutputDuplication, IDXGIResource, DXGI_ERROR_WAIT_TIMEOUT,
    DXGI_OUTDUPL_FRAME_INFO,
};

use super::screen::{self, wide};
use super::Frame;

struct Dup {
    ctx: ID3D11DeviceContext,
    device: ID3D11Device,
    dup: IDXGIOutputDuplication,
    staging: Option<(ID3D11Texture2D, u32, u32)>,
    last: Option<Arc<Frame>>,
    /// A retired frame's buffer, for the next readback (B-030).
    spare: Option<Vec<u8>>,
    /// An HDR output's half-float channels to sRGB bytes (B-109).
    lut: Option<Vec<u8>>,
}

type Device = (ID3D11Device, ID3D11DeviceContext);

#[derive(Default)]
pub struct Cache {
    dups: HashMap<String, Dup>,
    /// One device per adapter LUID, shared by its outputs (B-108).
    devices: HashMap<(u32, i32), Device>,
    backoff: Backoff,
    /// The display layout the backoff entries belong to.
    layout: String,
}

/// How long a failed open keeps an output on GDI.
const OPEN_BACKOFF: Duration = Duration::from_secs(10);

/// Failed opens, per output. Opening is the expensive part: a machine where
/// it keeps failing must not pay for it on every tick (B-108).
#[derive(Default)]
struct Backoff {
    until: HashMap<String, Instant>,
}

impl Backoff {
    /// `open`, unless the last one for `name` failed within the backoff.
    fn attempt<T>(
        &mut self,
        name: &str,
        now: Instant,
        open: impl FnOnce() -> Result<T, String>,
    ) -> Result<T, String> {
        if self.until.get(name).is_some_and(|t| now < *t) {
            return Err(format!("{name}: backing off after a failed open"));
        }
        let got = open();
        if got.is_ok() {
            self.until.remove(name);
        } else {
            self.until.insert(name.to_string(), now + OPEN_BACKOFF);
        }
        got
    }
}

static OPENS: AtomicU64 = AtomicU64::new(0);
static FORCE_OPEN_FAILURE: AtomicBool = AtomicBool::new(false);
static HDR_SEEN: AtomicBool = AtomicBool::new(false);

/// How many opens have been tried (the capture check counts them).
pub fn dxgi_opens() -> u64 {
    OPENS.load(Ordering::Relaxed)
}

/// Makes every open fail, for the capture check's backoff test.
pub fn force_dxgi_open_failure(on: bool) {
    FORCE_OPEN_FAILURE.store(on, Ordering::Relaxed);
}

/// Whether any output opened so far was in HDR.
pub fn dxgi_hdr_seen() -> bool {
    HDR_SEEN.load(Ordering::Relaxed)
}

fn new_device(adapter: &IDXGIAdapter) -> Result<Device, String> {
    let mut device: Option<ID3D11Device> = None;
    let mut ctx: Option<ID3D11DeviceContext> = None;
    // SAFETY: a live adapter and out pointers to live locals.
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
        )
    }
    .map_err(|e| format!("device: {e}"))?;
    Ok((device.ok_or("no device")?, ctx.ok_or("no context")?))
}

/// Whether the output is showing HDR (a PQ/BT.2020 colour space).
fn is_hdr(output: &IDXGIOutput) -> bool {
    let Ok(output6) = output.cast::<IDXGIOutput6>() else {
        return false;
    };
    // SAFETY: a live output.
    let desc = unsafe { output6.GetDesc1() };
    desc.is_ok_and(|d| d.ColorSpace == DXGI_COLOR_SPACE_RGB_FULL_G2084_NONE_P2020)
}

/// How bright Windows draws SDR content on an HDR output, in scRGB units
/// (1.0 is 80 nits): the "SDR content brightness" setting.
fn sdr_white(device_name: &str) -> f32 {
    use windows_sys::Win32::Devices::Display::{
        DisplayConfigGetDeviceInfo, GetDisplayConfigBufferSizes, QueryDisplayConfig,
        DISPLAYCONFIG_DEVICE_INFO_GET_SDR_WHITE_LEVEL, DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
        DISPLAYCONFIG_MODE_INFO, DISPLAYCONFIG_PATH_INFO, DISPLAYCONFIG_SDR_WHITE_LEVEL,
        DISPLAYCONFIG_SOURCE_DEVICE_NAME,
    };
    const QDC_ONLY_ACTIVE_PATHS: u32 = 2;
    // 200 nits, when the level cannot be read.
    const FALLBACK: f32 = 2.5;
    // SAFETY: the arrays are the sizes the first call reported, and each
    // request is a zeroed struct whose header describes it.
    unsafe {
        let (mut np, mut nm) = (0u32, 0u32);
        GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut np, &mut nm);
        let mut paths = vec![std::mem::zeroed::<DISPLAYCONFIG_PATH_INFO>(); np as usize];
        let mut modes = vec![std::mem::zeroed::<DISPLAYCONFIG_MODE_INFO>(); nm as usize];
        QueryDisplayConfig(
            QDC_ONLY_ACTIVE_PATHS,
            &mut np,
            paths.as_mut_ptr(),
            &mut nm,
            modes.as_mut_ptr(),
            std::ptr::null_mut(),
        );
        for path in paths.iter().take(np as usize) {
            let mut name: DISPLAYCONFIG_SOURCE_DEVICE_NAME = std::mem::zeroed();
            name.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME;
            name.header.size = std::mem::size_of_val(&name) as u32;
            name.header.adapterId = path.sourceInfo.adapterId;
            name.header.id = path.sourceInfo.id;
            DisplayConfigGetDeviceInfo(&mut name.header);
            if !wide(&name.viewGdiDeviceName).eq_ignore_ascii_case(device_name) {
                continue;
            }
            let mut level: DISPLAYCONFIG_SDR_WHITE_LEVEL = std::mem::zeroed();
            level.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_SDR_WHITE_LEVEL;
            level.header.size = std::mem::size_of_val(&level) as u32;
            level.header.adapterId = path.targetInfo.adapterId;
            level.header.id = path.targetInfo.id;
            DisplayConfigGetDeviceInfo(&mut level.header);
            if level.SDRWhiteLevel > 0 {
                return level.SDRWhiteLevel as f32 / 1000.0;
            }
        }
    }
    FALLBACK
}

/// One linear scRGB channel as an sRGB byte, with `white` as full scale.
fn scrgb_to_srgb(v: f32, white: f32) -> u8 {
    let x = (v / white).clamp(0.0, 1.0);
    let e = if x <= 0.003_130_8 {
        12.92 * x
    } else {
        1.055 * x.powf(1.0 / 2.4) - 0.055
    };
    (e * 255.0 + 0.5) as u8
}

fn half_to_f32(h: u16) -> f32 {
    let sign = if h & 0x8000 != 0 { -1.0f32 } else { 1.0 };
    let exp = i32::from((h >> 10) & 0x1f);
    let frac = f32::from(h & 0x3ff) / 1024.0;
    match exp {
        0 => sign * frac * 2f32.powi(-14),
        31 => sign * f32::INFINITY,
        e => sign * (1.0 + frac) * 2f32.powi(e - 15),
    }
}

/// Every half-float channel value's sRGB byte, so a frame is one lookup per
/// channel.
fn hdr_lut(white: f32) -> Vec<u8> {
    (0..=u16::MAX)
        .map(|h| scrgb_to_srgb(half_to_f32(h), white))
        .collect()
}

/// One row of RGBA half floats as BGRA bytes.
fn hdr_row(src: &[u8], dst: &mut [u8], lut: &[u8]) {
    for (s, d) in src.chunks_exact(8).zip(dst.chunks_exact_mut(4)) {
        let channel = |i: usize| lut[usize::from(u16::from_le_bytes([s[i], s[i + 1]]))];
        d[0] = channel(4);
        d[1] = channel(2);
        d[2] = channel(0);
        d[3] = 255;
    }
}

fn open(device_name: &str, devices: &mut HashMap<(u32, i32), Device>) -> Result<Dup, String> {
    OPENS.fetch_add(1, Ordering::Relaxed);
    if FORCE_OPEN_FAILURE.load(Ordering::Relaxed) {
        return Err("forced to fail".into());
    }
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
                let luid = adapter
                    .GetDesc1()
                    .map_err(|e| format!("adapter desc: {e}"))?
                    .AdapterLuid;
                let key = (luid.LowPart, luid.HighPart);
                let (device, ctx) = match devices.get(&key) {
                    Some(shared) => shared.clone(),
                    None => {
                        let made = new_device(&base)?;
                        devices.insert(key, made.clone());
                        made
                    }
                };
                let output1: IDXGIOutput1 = output.cast().map_err(|e| format!("output1: {e}"))?;
                let (dup, lut) = if is_hdr(&output) {
                    HDR_SEEN.store(true, Ordering::Relaxed);
                    let output5: IDXGIOutput5 = output.cast().map_err(|e| format!("hdr: {e}"))?;
                    let formats = [DXGI_FORMAT_R16G16B16A16_FLOAT, DXGI_FORMAT_B8G8R8A8_UNORM];
                    let dup = output5
                        .DuplicateOutput1(&device, 0, &formats)
                        .map_err(|e| format!("hdr duplicate: {e}"))?;
                    (dup, Some(hdr_lut(sdr_white(device_name))))
                } else {
                    let dup = output1
                        .DuplicateOutput(&device)
                        .map_err(|e| format!("duplicate: {e}"))?;
                    (dup, None)
                };
                return Ok(Dup {
                    ctx,
                    device,
                    dup,
                    staging: None,
                    last: None,
                    spare: None,
                    lut,
                });
            }
        }
        Err(format!("no DXGI output named {device_name}"))
    }
}

/// How long a new duplication may take to produce its first image.
const FIRST_FRAME: Duration = Duration::from_millis(900);

/// One `AcquireNextFrame` and what follows it, behind a trait so the
/// first-frame rule can be tested without a GPU.
trait Source {
    /// `Ok(None)` on a timeout. `Ok(Some(presented))` with a frame held;
    /// `presented` when the desktop image changed, not only the pointer.
    fn acquire(&mut self, timeout_ms: u32) -> Result<Option<bool>, String>;
    /// Copies the held frame out.
    fn copy(&mut self) -> Result<Frame, String>;
    fn release(&mut self);
}

/// B-029: the first frame of a new duplication. Only a frame that carries a
/// desktop image counts: DXGI can hand out a pointer-only update first,
/// whose surface is empty, and caching it would serve a black screen until
/// the desktop next changed. An image of all zero bytes (alpha included) is
/// refused too. With nothing inside `budget` this fails, so the capture
/// falls back to GDI instead of caching an empty frame.
fn first_frame(src: &mut impl Source, budget: Duration) -> Result<Frame, String> {
    let start = Instant::now();
    while start.elapsed() < budget {
        let left = budget.saturating_sub(start.elapsed());
        let wait = left.as_millis().clamp(1, 300) as u32;
        let Some(presented) = src.acquire(wait)? else {
            continue;
        };
        let frame = presented.then(|| src.copy());
        src.release();
        match frame {
            Some(Ok(f)) if !all_zero(&f.bgra) => return Ok(f),
            Some(Err(e)) => return Err(e),
            _ => {}
        }
    }
    Err("no desktop image from DXGI".into())
}

fn all_zero(bytes: &[u8]) -> bool {
    bytes.iter().all(|&b| b == 0)
}

/// A duplication as a `Source`, holding the acquired resource between
/// `acquire` and `copy`.
struct Live<'a> {
    dup: &'a mut Dup,
    res: Option<IDXGIResource>,
}

impl Source for Live<'_> {
    fn acquire(&mut self, timeout_ms: u32) -> Result<Option<bool>, String> {
        let mut info = DXGI_OUTDUPL_FRAME_INFO::default();
        let mut res: Option<IDXGIResource> = None;
        let dup = &self.dup.dup;
        match unsafe { dup.AcquireNextFrame(timeout_ms, &mut info, &mut res) } {
            Ok(()) => {
                self.res = res;
                let presented = info.LastPresentTime != 0 || info.AccumulatedFrames > 0;
                Ok(Some(presented))
            }
            Err(e) if e.code() == DXGI_ERROR_WAIT_TIMEOUT => Ok(None),
            Err(e) => Err(format!("acquire: {e}")),
        }
    }

    fn copy(&mut self) -> Result<Frame, String> {
        let res = self.res.take();
        self.dup.copy(res)
    }

    fn release(&mut self) {
        self.res = None;
        unsafe {
            let _ = self.dup.dup.ReleaseFrame();
        }
    }
}

impl Dup {
    fn live(&mut self) -> Live<'_> {
        Live {
            dup: self,
            res: None,
        }
    }

    /// The newest frame, or the last one when nothing changed since.
    fn frame(&mut self) -> Result<Arc<Frame>, String> {
        let Some(last) = self.last.clone() else {
            let f = Arc::new(first_frame(&mut self.live(), FIRST_FRAME)?);
            self.last = Some(f.clone());
            return Ok(f);
        };
        // With a frame in hand a static screen answers at once.
        let mut src = self.live();
        let Some(presented) = src.acquire(0)? else {
            return Ok(last);
        };
        let copied = if presented {
            src.copy().map(Some)
        } else {
            // Only the pointer moved; the image is the one we have.
            Ok(None)
        };
        src.release();
        match copied? {
            Some(f) => {
                let f = Arc::new(f);
                let old = self.last.replace(f.clone());
                drop(last);
                // Nobody else holds the frame this one replaces, so its
                // buffer takes the next readback.
                if let Some(Ok(frame)) = old.map(Arc::try_unwrap) {
                    self.spare = Some(frame.bgra);
                }
                Ok(f)
            }
            None => Ok(last),
        }
    }

    fn copy(&mut self, res: Option<IDXGIResource>) -> Result<Frame, String> {
        unsafe {
            let tex: ID3D11Texture2D = res
                .ok_or("no resource")?
                .cast()
                .map_err(|e| format!("texture: {e}"))?;
            let mut desc = D3D11_TEXTURE2D_DESC::default();
            tex.GetDesc(&mut desc);
            let hdr = desc.Format == DXGI_FORMAT_R16G16B16A16_FLOAT && self.lut.is_some();
            if !hdr && desc.Format != DXGI_FORMAT_B8G8R8A8_UNORM {
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
            let len = row * h as usize;
            let mut bgra = match self.spare.take() {
                Some(b) if b.len() == len => b,
                _ => vec![0u8; len],
            };
            let src = mapped.pData as *const u8;
            for y in 0..h as usize {
                let line = src.add(y * pitch);
                match (&self.lut, hdr) {
                    (Some(lut), true) => {
                        let halves = std::slice::from_raw_parts(line, w as usize * 8);
                        hdr_row(halves, &mut bgra[y * row..(y + 1) * row], lut);
                    }
                    _ => std::ptr::copy_nonoverlapping(line, bgra.as_mut_ptr().add(y * row), row),
                }
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
    /// Drops the duplication of one output, so the next grab opens a new one
    /// (the capture check's first-frame test).
    pub fn forget(&mut self, device_name: &str) {
        self.dups.remove(device_name);
        self.backoff.until.remove(device_name);
    }

    pub fn grab(&mut self, device_name: &str) -> Result<Arc<Frame>, String> {
        if !self.dups.contains_key(device_name) {
            // A changed display layout may be what the failure was about
            // (WM_DISPLAYCHANGE, seen from here).
            let layout = layout_key();
            if self.layout != layout {
                self.layout = layout;
                self.backoff.until.clear();
            }
            let devices = &mut self.devices;
            let opened = self
                .backoff
                .attempt(device_name, Instant::now(), || open(device_name, devices));
            self.dups.insert(device_name.to_string(), opened?);
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

fn layout_key() -> String {
    let list = screen::all_displays();
    let each: Vec<String> = list
        .iter()
        .map(|d| format!("{}:{:?}", d.device, d.phys))
        .collect();
    each.join(",")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Hands out a scripted run of acquisitions: `None` is a timeout,
    /// `Some((presented, fill))` a frame whose bytes are all `fill`.
    struct Fake {
        script: Vec<Option<(bool, u8)>>,
        held: Option<u8>,
        copies: usize,
    }

    impl Source for Fake {
        fn acquire(&mut self, _timeout_ms: u32) -> Result<Option<bool>, String> {
            if self.script.is_empty() {
                std::thread::sleep(Duration::from_millis(5));
                return Ok(None);
            }
            let next = self.script.remove(0);
            if let Some((_, fill)) = next {
                self.held = Some(fill);
            }
            Ok(next.map(|(presented, _)| presented))
        }

        fn copy(&mut self) -> Result<Frame, String> {
            self.copies += 1;
            let fill = self.held.ok_or("nothing held")?;
            Ok(Frame {
                width: 2,
                height: 1,
                bgra: vec![fill; 8],
            })
        }

        fn release(&mut self) {
            self.held = None;
        }
    }

    fn fake(script: Vec<Option<(bool, u8)>>) -> Fake {
        Fake {
            script,
            held: None,
            copies: 0,
        }
    }

    #[test]
    fn pointer_only_first_frame_is_skipped() {
        // A timeout, a pointer-only update, then the desktop.
        let mut src = fake(vec![None, Some((false, 0)), Some((true, 200))]);
        let f = first_frame(&mut src, FIRST_FRAME).expect("a desktop frame");
        assert_eq!(f.bgra, vec![200; 8]);
        // The pointer-only update was never copied out.
        assert_eq!(src.copies, 1);
    }

    #[test]
    fn empty_first_frame_falls_back() {
        // An all-zero image is refused too; with nothing better inside the
        // budget the open fails, so the capture goes to GDI instead of
        // caching black.
        let mut src = fake(vec![Some((true, 0)), Some((false, 0))]);
        let budget = Duration::from_millis(50);
        assert!(first_frame(&mut src, budget).is_err());
        assert_eq!(src.copies, 1);
    }

    #[test]
    fn open_failure_backs_off() {
        let t0 = Instant::now();
        let mut b = Backoff::default();
        let mut opens = 0;
        let mut fail = |name: &str, at: Duration| {
            let got = b.attempt(name, t0 + at, || {
                opens += 1;
                Err::<(), String>("no".into())
            });
            assert!(got.is_err());
        };
        fail("A", Duration::ZERO);
        fail("A", Duration::from_secs(5));
        fail("A", Duration::from_millis(9999));
        // Another output has its own clock, and the backoff runs out.
        fail("B", Duration::from_secs(1));
        fail("A", OPEN_BACKOFF);
        assert_eq!(opens, 3);
        // A success clears it.
        let later = t0 + OPEN_BACKOFF * 2;
        assert_eq!(b.attempt("A", later, || Ok(7)), Ok(7));
        assert!(!b.until.contains_key("A"));
    }

    #[test]
    fn scrgb_to_srgb_at_sdr_white() {
        let white = 2.5;
        assert_eq!(scrgb_to_srgb(white, white), 255);
        assert_eq!(scrgb_to_srgb(white * 4.0, white), 255);
        assert_eq!(scrgb_to_srgb(0.0, white), 0);
        assert_eq!(scrgb_to_srgb(-1.0, white), 0);
        // Half of SDR white is 50% linear, which sRGB writes as 188.
        assert_eq!(scrgb_to_srgb(white / 2.0, white), 188);
        // 1.0 in half float is 0x3C00: at a white of 1.0, full scale.
        assert_eq!(half_to_f32(0x3C00), 1.0);
        assert_eq!(half_to_f32(0xC000), -2.0);
        assert_eq!(half_to_f32(0), 0.0);
        let lut = hdr_lut(1.0);
        let mut out = [0u8; 4];
        let px = [0x00, 0x3C, 0x00, 0x00, 0x00, 0x00, 0x00, 0x3C];
        hdr_row(&px, &mut out, &lut);
        // R full, G and B none; written as BGRA, alpha opaque.
        assert_eq!(out, [0, 0, 255, 255]);
    }
}
