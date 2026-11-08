//! The little audio maths voice input needs: reading a WAV file (the CI
//! fixture, which stands in for a microphone), writing one (Gemini takes
//! audio as a file), resampling to Whisper's 16 kHz, and loudness.

/// What Whisper wants: 16 kHz, mono, f32 in -1..1.
pub const RATE: u32 = 16_000;

/// Reads a RIFF WAVE file: 8, 16, 24 or 32-bit PCM, or 32-bit float, any
/// rate and channel count. Returns mono samples (channels averaged) and the
/// rate.
pub fn read(bytes: &[u8]) -> Result<(Vec<f32>, u32), String> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("not a WAV file".into());
    }
    let u16_at = |i: usize| u16::from_le_bytes([bytes[i], bytes[i + 1]]);
    let u32_at = |i: usize| u32::from_le_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]);
    let mut fmt: Option<(u16, usize, u32, usize)> = None;
    let mut at = 12;
    while at + 8 <= bytes.len() {
        let id = &bytes[at..at + 4];
        let size = u32_at(at + 4) as usize;
        let body = at + 8;
        let end = (body + size).min(bytes.len());
        if id == b"fmt " && size >= 16 && body + 16 <= bytes.len() {
            let mut tag = u16_at(body);
            let channels = u16_at(body + 2).max(1) as usize;
            let rate = u32_at(body + 4);
            let bits = u16_at(body + 14) as usize;
            // WAVE_FORMAT_EXTENSIBLE: the real tag opens the sub-format GUID.
            if tag == 0xFFFE && size >= 26 && body + 26 <= bytes.len() {
                tag = u16_at(body + 24);
            }
            fmt = Some((tag, channels, rate, bits));
        } else if id == b"data" {
            let (tag, channels, rate, bits) = fmt.ok_or("WAV data before its format")?;
            let data = &bytes[body..end];
            let frame: Vec<f32> = match (tag, bits) {
                (1, 8) => data.iter().map(|b| (*b as f32 - 128.0) / 128.0).collect(),
                (1, 16) => data
                    .chunks_exact(2)
                    .map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0)
                    .collect(),
                (1, 24) => data
                    .chunks_exact(3)
                    .map(|c| (i32::from_le_bytes([0, c[0], c[1], c[2]]) >> 8) as f32 / 8_388_608.0)
                    .collect(),
                (1, 32) => data
                    .chunks_exact(4)
                    .map(|c| i32::from_le_bytes([c[0], c[1], c[2], c[3]]) as f32 / 2_147_483_648.0)
                    .collect(),
                (3, 32) => data
                    .chunks_exact(4)
                    .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                    .collect(),
                _ => return Err(format!("WAV format {tag} at {bits} bits is not supported")),
            };
            return Ok((downmix(&frame, channels), rate));
        }
        // Chunks are padded to an even length.
        at = body + size + (size & 1);
    }
    Err("WAV file without audio".into())
}

/// Interleaved frames to mono by averaging the channels.
pub fn downmix(interleaved: &[f32], channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return interleaved.to_vec();
    }
    interleaved
        .chunks_exact(channels)
        .map(|f| f.iter().sum::<f32>() / channels as f32)
        .collect()
}

/// A 16-bit PCM mono WAV file.
pub fn encode(samples: &[f32], rate: u32) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        let v = (s.clamp(-1.0, 1.0) * 32767.0).round() as i16;
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

/// `input` at `from` Hz to `to` Hz. Going down, each output sample is the
/// mean of the input it covers (a box filter, enough to keep speech clear of
/// aliasing); going up, linear interpolation.
pub fn resample(input: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to || input.is_empty() || from == 0 || to == 0 {
        return input.to_vec();
    }
    let ratio = from as f64 / to as f64;
    let n = ((input.len() as f64) / ratio).floor() as usize;
    let mut out = Vec::with_capacity(n);
    if ratio > 1.0 {
        for i in 0..n {
            let a = (i as f64 * ratio) as usize;
            let b = (((i + 1) as f64 * ratio) as usize).clamp(a + 1, input.len());
            let span = &input[a..b];
            out.push(span.iter().sum::<f32>() / span.len() as f32);
        }
    } else {
        for i in 0..n {
            let x = i as f64 * ratio;
            let a = x.floor() as usize;
            let b = (a + 1).min(input.len() - 1);
            let t = (x - a as f64) as f32;
            out.push(input[a] * (1.0 - t) + input[b] * t);
        }
    }
    out
}

/// Root mean square loudness.
pub fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
}

/// Loudness as the meters draw it: 0 at -55 dBFS (a quiet room), 1 at
/// -10 dBFS (speech close to the microphone).
pub fn level(rms: f32) -> f32 {
    if rms <= 0.0 {
        return 0.0;
    }
    ((20.0 * rms.log10() + 55.0) / 45.0).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_then_read_gives_the_samples_back() {
        let tone: Vec<f32> = (0..1600).map(|i| (i as f32 * 0.05).sin() * 0.5).collect();
        let (back, rate) = read(&encode(&tone, RATE)).unwrap();
        assert_eq!(rate, RATE);
        assert_eq!(back.len(), tone.len());
        assert!(back.iter().zip(&tone).all(|(a, b)| (a - b).abs() < 1e-3));
    }

    #[test]
    fn the_fixture_reads_as_speech_at_16k() {
        let (s, rate) = read(include_bytes!("fixtures/dictation.wav")).unwrap();
        assert_eq!(rate, RATE);
        let secs = s.len() as f32 / rate as f32;
        assert!((5.0..12.0).contains(&secs), "{secs} s");
        assert!(level(rms(&s)) > 0.3, "the fixture is too quiet");
    }

    #[test]
    fn stereo_float_at_48k_becomes_mono_at_16k() {
        // A float WAV, two channels, by hand.
        let frames = 4800;
        let mut wav = encode(&[], 48_000);
        wav[20..22].copy_from_slice(&3u16.to_le_bytes());
        wav[22..24].copy_from_slice(&2u16.to_le_bytes());
        wav[34..36].copy_from_slice(&32u16.to_le_bytes());
        let len = (frames * 2 * 4) as u32;
        wav[40..44].copy_from_slice(&len.to_le_bytes());
        for _ in 0..frames {
            wav.extend_from_slice(&0.5f32.to_le_bytes());
            wav.extend_from_slice(&0.0f32.to_le_bytes());
        }
        let (mono, rate) = read(&wav).unwrap();
        assert_eq!((mono.len(), rate), (frames, 48_000));
        assert!((mono[10] - 0.25).abs() < 1e-6);
        let down = resample(&mono, 48_000, RATE);
        assert_eq!(down.len(), frames / 3);
        assert!((down[5] - 0.25).abs() < 1e-6);
        assert_eq!(resample(&down, RATE, 32_000).len(), down.len() * 2);
    }

    #[test]
    fn levels_span_quiet_to_loud() {
        assert_eq!(level(0.0), 0.0);
        assert!(level(0.001) < 0.05);
        assert!(level(0.3) >= 0.99);
        assert!(read(b"not audio").is_err());
    }
}
