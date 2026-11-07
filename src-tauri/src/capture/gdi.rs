//! GDI fallback: BitBlt from the screen DC. Used when DXGI duplication is
//! unavailable (remote sessions, some virtual adapters, rotated outputs).
//! Display affinity applies here too, so WDA_EXCLUDEFROMCAPTURE windows are
//! left out, not painted black. CAPTUREBLT includes other apps' layered
//! windows (tooltips, menus), as Chromium's GDI capturer does.

use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetDIBits,
    ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, CAPTUREBLT, DIB_RGB_COLORS,
    HGDIOBJ, SRCCOPY,
};

use super::screen::PxRect;
use super::Frame;

pub fn grab(r: PxRect) -> Result<Frame, String> {
    if r.width <= 0 || r.height <= 0 {
        return Err("empty rect".into());
    }
    unsafe {
        let screen = GetDC(None);
        if screen.is_invalid() {
            return Err("no screen DC".into());
        }
        let mem = CreateCompatibleDC(Some(screen));
        let bmp = CreateCompatibleBitmap(screen, r.width, r.height);
        let old = SelectObject(mem, HGDIOBJ::from(bmp));
        let blit = BitBlt(
            mem,
            0,
            0,
            r.width,
            r.height,
            Some(screen),
            r.x,
            r.y,
            SRCCOPY | CAPTUREBLT,
        );
        SelectObject(mem, old);
        let mut bi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: r.width,
                biHeight: -r.height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bgra = vec![0u8; r.width as usize * r.height as usize * 4];
        let lines = GetDIBits(
            mem,
            bmp,
            0,
            r.height as u32,
            Some(bgra.as_mut_ptr().cast()),
            &mut bi,
            DIB_RGB_COLORS,
        );
        let _ = DeleteObject(HGDIOBJ::from(bmp));
        let _ = DeleteDC(mem);
        ReleaseDC(None, screen);
        blit.map_err(|e| format!("BitBlt: {e}"))?;
        if lines != r.height {
            return Err(format!("GetDIBits returned {lines} lines"));
        }
        Ok(Frame {
            width: r.width as u32,
            height: r.height as u32,
            bgra,
        })
    }
}
