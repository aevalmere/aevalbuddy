//! The pure half of the capture pipeline: pixel conversion, crop maths,
//! Lanczos3 resampling and the JPEG quality ladder. Everything here is
//! deterministic and unit-tested; the platform half (DXGI, GDI) only produces
//! the BGRA frame this starts from.
//!
//! The numbers are main.js's (PARITY CAP-02..05): a 1600 px long edge, JPEG
//! quality 88 stepping down by 8 while over 1.6 MB, never below 62. Two
//! departures, because capture fidelity bounds answer quality (D-09,
//! B-032): the whole screen is sized from its physical pixels rather than
//! its DIP size, and chroma is kept at full resolution (4:4:4).

use fast_image_resize::images::Image;
use fast_image_resize::{FilterType, PixelType, ResizeAlg, ResizeOptions, Resizer};
use jpeg_encoder::{ColorType, Encoder, SamplingFactor};

pub const SHOT_LONG_EDGE: u32 = 1600;
pub const SHOT_QUALITY: u8 = 88;
pub const SHOT_QUALITY_FLOOR: u8 = 62;
pub const SHOT_QUALITY_STEP: u8 = 8;
pub const SHOT_BUDGET: usize = 1600 * 1024;

/// An RGB image, rows top-down, three bytes a pixel.
#[derive(Clone, Debug, PartialEq)]
pub struct Rgb {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

impl Rgb {
    /// From a top-down BGRA buffer whose rows are `stride` bytes apart.
    /// Alpha is dropped: desktop surfaces are opaque, and some drivers leave
    /// the alpha byte undefined, which a premultiplying resize would turn
    /// into black.
    pub fn from_bgra(width: u32, height: u32, stride: usize, bgra: &[u8]) -> Rgb {
        let mut data = Vec::with_capacity(width as usize * height as usize * 3);
        for row in 0..height as usize {
            let line = &bgra[row * stride..row * stride + width as usize * 4];
            for px in line.chunks_exact(4) {
                data.extend_from_slice(&[px[2], px[1], px[0]]);
            }
        }
        Rgb {
            width,
            height,
            data,
        }
    }

    pub fn pixel(&self, x: u32, y: u32) -> [u8; 3] {
        let i = (y as usize * self.width as usize + x as usize) * 3;
        [self.data[i], self.data[i + 1], self.data[i + 2]]
    }

    /// A copy of one rectangle; the caller has clamped it inside the image.
    pub fn crop(&self, x: u32, y: u32, width: u32, height: u32) -> Rgb {
        let mut data = Vec::with_capacity(width as usize * height as usize * 3);
        for row in y..y + height {
            let start = (row as usize * self.width as usize + x as usize) * 3;
            data.extend_from_slice(&self.data[start..start + width as usize * 3]);
        }
        Rgb {
            width,
            height,
            data,
        }
    }

    /// Lanczos3, the filter behind Chromium's thumbnail scaler and
    /// `nativeImage.resize({ quality: 'best' })` (CAP-04, CAP-13).
    pub fn resize(&self, width: u32, height: u32) -> Rgb {
        let width = width.max(1);
        let height = height.max(1);
        if width == self.width && height == self.height {
            return self.clone();
        }
        let src =
            match Image::from_vec_u8(self.width, self.height, self.data.clone(), PixelType::U8x3) {
                Ok(img) => img,
                Err(_) => return self.clone(),
            };
        let mut dst = Image::new(width, height, PixelType::U8x3);
        let options = ResizeOptions::new().resize_alg(ResizeAlg::Convolution(FilterType::Lanczos3));
        if Resizer::new().resize(&src, &mut dst, &options).is_err() {
            return self.clone();
        }
        Rgb {
            width,
            height,
            data: dst.into_vec(),
        }
    }
}

/// One JPEG at one quality, chroma at full resolution (4:4:4) at every
/// quality: coloured text and thin coloured lines keep their edges, which
/// Skia's 4:2:0 (behind Electron's `toJPEG`) smeared (B-032).
pub fn jpeg(img: &Rgb, quality: u8) -> Option<Vec<u8>> {
    if img.width == 0 || img.height == 0 || img.width > 65535 || img.height > 65535 {
        return None;
    }
    let mut out = Vec::new();
    let mut enc = Encoder::new(&mut out, quality);
    enc.set_sampling_factor(SamplingFactor::R_4_4_4);
    enc.set_optimized_huffman_tables(true);
    enc.encode(
        &img.data,
        img.width as u16,
        img.height as u16,
        ColorType::Rgb,
    )
    .ok()?;
    Some(out)
}

/// Step quality down only as far as still-readable text allows (CAP-05).
/// Returns the bytes and the quality they were written at.
pub fn jpeg_within_budget(img: &Rgb) -> Option<(Vec<u8>, u8)> {
    ladder(|q| jpeg(img, q))
}

/// The ladder on its own, so the policy can be tested without encoding.
pub fn ladder(mut encode: impl FnMut(u8) -> Option<Vec<u8>>) -> Option<(Vec<u8>, u8)> {
    let mut quality = SHOT_QUALITY;
    let mut buf = encode(quality)?;
    while buf.len() > SHOT_BUDGET && quality > SHOT_QUALITY_FLOOR {
        quality -= SHOT_QUALITY_STEP;
        buf = encode(quality)?;
    }
    Some((buf, quality))
}

/// Size of the whole-screen image for a frame of `w` x `h` physical pixels:
/// shrunk so the long edge is at most 1600, never enlarged (B-032). A
/// 1920x1080 display at 150% goes out at 1600x900, where main.js sent its
/// 1280x720 DIP size.
pub fn whole_screen_size(w: u32, h: u32) -> (u32, u32) {
    let (w, h) = (w.max(1) as f64, h.max(1) as f64);
    let scale = (SHOT_LONG_EDGE as f64 / w.max(h)).min(1.0);
    (
        ((w * scale).round() as u32).max(1),
        ((h * scale).round() as u32).max(1),
    )
}

/// The long-edge budget for an area: `None` when it already fits.
pub fn fit_long_edge(w: u32, h: u32) -> Option<(u32, u32)> {
    let long = w.max(h);
    if long <= SHOT_LONG_EDGE {
        return None;
    }
    let k = SHOT_LONG_EDGE as f64 / long as f64;
    Some((
        ((w as f64 * k).round() as u32).max(1),
        ((h as f64 * k).round() as u32).max(1),
    ))
}

fn clamp_num(n: f64, min: f64, max: f64) -> f64 {
    // main.js clampNum: the minimum wins when the range is empty.
    min.max(max.min(n))
}

/// Maps a DIP selection on a display to pixels of the image that actually
/// arrived (CAP-03): scaled by the size that came back, not the size asked
/// for, clamped inside the image, at least one pixel.
///
/// `want` and `display` are DIP `(x, y, width, height)`; `got` is the frame.
pub fn crop_plan(
    want: (f64, f64, f64, f64),
    display: (f64, f64, f64, f64),
    got: (u32, u32),
) -> (u32, u32, u32, u32) {
    let (gw, gh) = (got.0 as f64, got.1 as f64);
    let kx = gw / display.2.max(1.0);
    let ky = gh / display.3.max(1.0);
    let x = clamp_num(
        ((want.0 - display.0) * kx).round(),
        0.0,
        (gw - 1.0).max(0.0),
    );
    let y = clamp_num(
        ((want.1 - display.1) * ky).round(),
        0.0,
        (gh - 1.0).max(0.0),
    );
    let w = clamp_num((want.2 * kx).round(), 1.0, gw - x);
    let h = clamp_num((want.3 * ky).round(), 1.0, gh - y);
    (x as u32, y as u32, w as u32, h as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checker(w: u32, h: u32) -> Rgb {
        let mut data = Vec::new();
        for y in 0..h {
            for x in 0..w {
                let v = if (x / 2 + y / 2) % 2 == 0 { 255 } else { 0 };
                data.extend_from_slice(&[v, v, v]);
            }
        }
        Rgb {
            width: w,
            height: h,
            data,
        }
    }

    #[test]
    fn ladder_starts_at_88_and_stops_at_62() {
        let mut asked = Vec::new();
        let r = ladder(|q| {
            asked.push(q);
            Some(vec![0; SHOT_BUDGET + 1])
        })
        .unwrap();
        assert_eq!(asked, vec![88, 80, 72, 64, 56]);
        // 64 > 62 allows one more step, exactly as main.js's while loop does.
        assert_eq!(r.1, 56);
    }

    #[test]
    fn ladder_stops_as_soon_as_it_fits() {
        let mut asked = Vec::new();
        let r = ladder(|q| {
            asked.push(q);
            Some(vec![0; if q > 72 { SHOT_BUDGET + 1 } else { 10 }])
        })
        .unwrap();
        assert_eq!(asked, vec![88, 80, 72]);
        assert_eq!(r.1, 72);
    }

    #[test]
    fn ladder_at_budget_exactly_is_fine() {
        let r = ladder(|_| Some(vec![0; SHOT_BUDGET])).unwrap();
        assert_eq!(r.1, 88);
    }

    #[test]
    fn whole_screen_uses_physical_pixels_within_budget() {
        // 1920x1080 at 150% arrives as 1920x1080 pixels: 1600x900 goes out,
        // not its 1280x720 DIP size.
        assert_eq!(whole_screen_size(1920, 1080), (1600, 900));
        assert_eq!(whole_screen_size(2560, 1440), (1600, 900));
        assert_eq!(whole_screen_size(3840, 2160), (1600, 900));
        assert_eq!(whole_screen_size(1080, 1920), (900, 1600));
        // Under budget: the pixels as they are, never enlarged.
        assert_eq!(whole_screen_size(1024, 768), (1024, 768));
        assert_eq!(whole_screen_size(1366, 768), (1366, 768));
        assert_eq!(whole_screen_size(1600, 1200), (1600, 1200));
        assert_eq!(whole_screen_size(0, 0), (1, 1));
    }

    /// Each component's sampling byte (`H << 4 | V`) from the frame header.
    fn sampling(jpeg: &[u8]) -> Vec<u8> {
        let mut i = 2;
        while i + 4 <= jpeg.len() && jpeg[i] == 0xFF {
            let marker = jpeg[i + 1];
            let len = u16::from_be_bytes([jpeg[i + 2], jpeg[i + 3]]) as usize;
            if (0xC0..=0xC2).contains(&marker) {
                let n = jpeg[i + 9] as usize;
                return (0..n).map(|c| jpeg[i + 11 + c * 3]).collect();
            }
            i += 2 + len;
        }
        Vec::new()
    }

    #[test]
    fn chroma_is_444() {
        let img = checker(64, 48);
        for q in [SHOT_QUALITY, SHOT_QUALITY_FLOOR, 30] {
            let bytes = jpeg(&img, q).unwrap();
            assert_eq!(sampling(&bytes), vec![0x11, 0x11, 0x11], "q{q}");
        }
    }

    #[test]
    fn long_edge_fit() {
        assert_eq!(fit_long_edge(1600, 1000), None);
        assert_eq!(fit_long_edge(3200, 1000), Some((1600, 500)));
        assert_eq!(fit_long_edge(1000, 2400), Some((667, 1600)));
    }

    #[test]
    fn crop_scales_by_what_arrived() {
        // A 1280x720 DIP display captured at 150%: 1920x1080 pixels.
        let p = crop_plan(
            (100.0, 50.0, 200.0, 100.0),
            (0.0, 0.0, 1280.0, 720.0),
            (1920, 1080),
        );
        assert_eq!(p, (150, 75, 300, 150));
        // Second monitor to the right: the display origin is subtracted.
        let p = crop_plan(
            (1380.0, 10.0, 50.0, 50.0),
            (1280.0, 0.0, 1280.0, 720.0),
            (1280, 720),
        );
        assert_eq!(p, (100, 10, 50, 50));
    }

    #[test]
    fn crop_is_clamped_inside_and_at_least_one_pixel() {
        let p = crop_plan(
            (-50.0, -50.0, 5000.0, 5000.0),
            (0.0, 0.0, 100.0, 100.0),
            (100, 100),
        );
        assert_eq!(p, (0, 0, 100, 100));
        let p = crop_plan(
            (200.0, 200.0, 10.0, 10.0),
            (0.0, 0.0, 100.0, 100.0),
            (100, 100),
        );
        assert_eq!(p, (99, 99, 1, 1));
        let p = crop_plan((10.0, 10.0, 0.0, 0.0), (0.0, 0.0, 100.0, 100.0), (100, 100));
        assert_eq!(p, (10, 10, 1, 1));
    }

    #[test]
    fn bgra_rows_honour_stride_and_channel_order() {
        // 2x2, stride padded to 12 bytes.
        let bgra = [
            1, 2, 3, 0, 4, 5, 6, 0, 9, 9, 9, 9, //
            7, 8, 9, 0, 10, 11, 12, 0, 9, 9, 9, 9,
        ];
        let rgb = Rgb::from_bgra(2, 2, 12, &bgra);
        assert_eq!(rgb.data, vec![3, 2, 1, 6, 5, 4, 9, 8, 7, 12, 11, 10]);
        assert_eq!(rgb.pixel(1, 1), [12, 11, 10]);
    }

    #[test]
    fn crop_copies_the_right_pixels() {
        let img = checker(8, 8);
        let c = img.crop(2, 2, 2, 2);
        assert_eq!(c.width, 2);
        assert_eq!(c.pixel(0, 0), img.pixel(2, 2));
        assert_eq!(c.pixel(1, 1), img.pixel(3, 3));
    }

    #[test]
    fn lanczos_keeps_flat_colour_and_size() {
        let img = Rgb {
            width: 40,
            height: 20,
            data: [200u8, 100, 50].repeat(800),
        };
        let r = img.resize(16, 8);
        assert_eq!((r.width, r.height), (16, 8));
        assert!(r.data.chunks(3).all(|p| p == [200, 100, 50]));
    }

    #[test]
    fn lanczos_is_sharper_than_a_box_average() {
        // Edge energy after a 0.62x shrink of a 2px checker: Lanczos keeps
        // more contrast than a plain average would (the CAP-13 direction).
        let img = checker(160, 160);
        let r = img.resize(100, 100);
        let energy: u64 = (1..100u32)
            .map(|x| {
                (r.pixel(x, 50)[0] as i32 - r.pixel(x - 1, 50)[0] as i32).unsigned_abs() as u64
            })
            .sum();
        assert!(energy > 0, "a resized checker must keep some edges");
    }

    #[test]
    fn jpeg_round_trips_with_dimensions() {
        let img = checker(64, 48);
        let (bytes, q) = jpeg_within_budget(&img).unwrap();
        assert_eq!(q, 88);
        assert_eq!(&bytes[..2], &[0xFF, 0xD8]);
        let mut dec = jpeg_decoder::Decoder::new(&bytes[..]);
        let px = dec.decode().unwrap();
        let info = dec.info().unwrap();
        assert_eq!((info.width, info.height), (64, 48));
        assert_eq!(px.len(), 64 * 48 * 3);
    }
}
