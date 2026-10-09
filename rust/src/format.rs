//! DWXG binary format and RGB565 / grayscale conversion helpers.
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use std::io::{Read, Write};

pub const HEADER_SIZE: usize = 24;
pub const MAX_PIXELS: u64 = 100_000_000;

#[derive(Debug, Clone)]
pub struct DecodedImage {
    pub width: u32,
    pub height: u32,
    /// Interleaved RGB bytes, always three bytes per pixel for display.
    pub rgb: Vec<u8>,
    pub bits_per_pixel: u32,
    pub compressed: bool,
}

fn u32_at(data: &[u8], offset: usize) -> Result<u32, String> {
    let slice = data.get(offset..offset + 4).ok_or("DWXG header is truncated")?;
    Ok(u32::from_be_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn dimensions_ok(width: u32, height: u32) -> Result<usize, String> {
    if width == 0 || height == 0 {
        return Err("DWXG width and height must be greater than zero".into());
    }
    let count = u64::from(width) * u64::from(height);
    if count > MAX_PIXELS {
        return Err(format!("Image is too large ({count} pixels; limit is {MAX_PIXELS})"));
    }
    usize::try_from(count).map_err(|_| "Image dimensions exceed this system's limits".into())
}

fn inflate(data: &[u8], expected_max: usize) -> Result<Vec<u8>, String> {
    // Do not let a compressed file expand without bound. One extra byte is enough
    // to detect a payload that exceeds the maximum size implied by its dimensions.
    let limit = u64::try_from(expected_max).map_err(|_| "Expected payload is too large")? + 1;
    let mut decoder = ZlibDecoder::new(data).take(limit);
    let mut out = Vec::new();
    decoder.read_to_end(&mut out).map_err(|e| format!("Unable to decompress DWXG pixels: {e}"))?;
    if out.len() > expected_max {
        return Err("Decompressed DWXG pixel data exceeds the expected image size".into());
    }
    Ok(out)
}

fn expand_rgb565(data: &[u8], pixel_count: usize) -> Result<Vec<u8>, String> {
    if data.len() != pixel_count.checked_mul(2).ok_or("Image size overflow")? {
        return Err("RGB565 data length does not match the declared image dimensions".into());
    }
    let mut rgb = Vec::with_capacity(pixel_count * 3);
    for pair in data.chunks_exact(2) {
        let word = u16::from_be_bytes([pair[0], pair[1]]);
        let r5 = ((word >> 11) & 0x1f) as u8;
        let g6 = ((word >> 5) & 0x3f) as u8;
        let b5 = (word & 0x1f) as u8;
        rgb.push((r5 << 3) | (r5 >> 2));
        rgb.push((g6 << 2) | (g6 >> 4));
        rgb.push((b5 << 3) | (b5 >> 2));
    }
    Ok(rgb)
}

fn expand_gray(data: &[u8], pixel_count: usize) -> Result<Vec<u8>, String> {
    if data.len() != pixel_count {
        return Err("Grayscale data length does not match the declared image dimensions".into());
    }
    let mut rgb = Vec::with_capacity(pixel_count * 3);
    for &gray in data {
        rgb.extend_from_slice(&[gray, gray, gray]);
    }
    Ok(rgb)
}

fn expand_mono(data: &[u8], width: u32, height: u32, pixel_count: usize) -> Result<Vec<u8>, String> {
    let row_bytes = (width as usize + 7) / 8;
    let expected = row_bytes.checked_mul(height as usize).ok_or("Image size overflow")?;
    if data.len() != expected {
        return Err("Monochrome data length does not match the declared image dimensions".into());
    }
    let mut rgb = Vec::with_capacity(pixel_count * 3);
    for y in 0..height as usize {
        for x in 0..width as usize {
            let byte = data[y * row_bytes + x / 8];
            let is_white = ((byte >> (7 - (x % 8))) & 1) != 0;
            let value = if is_white { 255 } else { 0 };
            rgb.extend_from_slice(&[value, value, value]);
        }
    }
    Ok(rgb)
}

/// Decode current 24-byte DWXG files and identifiable earlier RGB565 / 1-bit files.
pub fn decode_dwxg(data: &[u8]) -> Result<DecodedImage, String> {
    if data.len() < 16 || data.get(0..4) != Some(&b"DWXG"[..]) {
        return Err("This file does not have a valid DWXG signature".into());
    }
    let width = u32_at(data, 4)?;
    let height = u32_at(data, 8)?;
    let count = dimensions_ok(width, height)?;

    // Current format: magic, width, height, compression, depth, payload length, payload.
    if data.len() >= HEADER_SIZE {
        let flag = u32_at(data, 12)?;
        let depth = u32_at(data, 16)?;
        let length = u32_at(data, 20)? as usize;
        if flag <= 1 && (depth == 8 || depth == 16) && length == data.len() - HEADER_SIZE {
            let payload = &data[HEADER_SIZE..];
            let decoded = if flag == 1 { inflate(payload, count * if depth == 16 { 2 } else { 1 })? } else { payload.to_vec() };
            let (rgb, expected_depth) = if depth == 16 {
                (expand_rgb565(&decoded, count)?, 16)
            } else {
                (expand_gray(&decoded, count)?, 8)
            };
            return Ok(DecodedImage { width, height, rgb, bits_per_pixel: expected_depth, compressed: flag == 1 });
        }
    }

    // Earlier compressed-capable format: magic, width, height, compression, payload length, RGB565.
    if data.len() >= 20 {
        let flag = u32_at(data, 12)?;
        let length = u32_at(data, 16)? as usize;
        if flag <= 1 && length == data.len() - 20 {
            let payload = &data[20..];
            let decoded = if flag == 1 { inflate(payload, count * 2)? } else { payload.to_vec() };
            let rgb = expand_rgb565(&decoded, count)?;
            return Ok(DecodedImage { width, height, rgb, bits_per_pixel: 16, compressed: flag == 1 });
        }
    }

    // Earlier raw format: magic, width, height, reserved zero, raw pixel bytes.
    let reserved = u32_at(data, 12)?;
    if reserved == 0 {
        let payload = &data[16..];
        if payload.len() == count.saturating_mul(2) {
            return Ok(DecodedImage {
                width, height, rgb: expand_rgb565(payload, count)?, bits_per_pixel: 16, compressed: false,
            });
        }
        let mono_len = ((width as usize + 7) / 8).checked_mul(height as usize).ok_or("Image size overflow")?;
        if payload.len() == mono_len {
            return Ok(DecodedImage {
                width, height, rgb: expand_mono(payload, width, height, count)?, bits_per_pixel: 1, compressed: false,
            });
        }
    }

    Err("Unrecognised DWXG layout or corrupt pixel data".into())
}

/// Convert interleaved 8-bit RGB samples into DWXG's RGB565 or grayscale payload.
pub fn encode_pixels(rgb: &[u8], width: u32, height: u32, depth: u32, dither: bool) -> Result<Vec<u8>, String> {
    let count = dimensions_ok(width, height)?;
    if rgb.len() != count.checked_mul(3).ok_or("Image size overflow")? {
        return Err("Input RGB buffer length does not match dimensions".into());
    }
    match depth {
        8 => {
            let mut out = Vec::with_capacity(count);
            for p in rgb.chunks_exact(3) {
                let gray = (299u32 * p[0] as u32 + 587u32 * p[1] as u32 + 114u32 * p[2] as u32 + 500) / 1000;
                out.push(gray as u8);
            }
            Ok(out)
        }
        16 if !dither => {
            let mut out = Vec::with_capacity(count * 2);
            for p in rgb.chunks_exact(3) {
                let word = (((p[0] as u16 >> 3) & 0x1f) << 11)
                    | (((p[1] as u16 >> 2) & 0x3f) << 5)
                    | ((p[2] as u16 >> 3) & 0x1f);
                out.extend_from_slice(&word.to_be_bytes());
            }
            Ok(out)
        }
        16 => {
            // Floyd-Steinberg error diffusion into the RGB565 palette.
            let mut out = Vec::with_capacity(count * 2);
            let w = width as usize;
            let mut current = vec![[0.0f32; 3]; w + 2];
            let mut next = vec![[0.0f32; 3]; w + 2];
            for y in 0..height as usize {
                for x in 0..w {
                    let i = (y * w + x) * 3;
                    let mut adjusted = [rgb[i] as f32, rgb[i + 1] as f32, rgb[i + 2] as f32];
                    for c in 0..3 { adjusted[c] = (adjusted[c] + current[x + 1][c]).clamp(0.0, 255.0); }
                    let qr = (adjusted[0] * 31.0 / 255.0).round() as u16;
                    let qg = (adjusted[1] * 63.0 / 255.0).round() as u16;
                    let qb = (adjusted[2] * 31.0 / 255.0).round() as u16;
                    let reconstructed = [qr as f32 * 255.0 / 31.0, qg as f32 * 255.0 / 63.0, qb as f32 * 255.0 / 31.0];
                    let word = (qr << 11) | (qg << 5) | qb;
                    out.extend_from_slice(&word.to_be_bytes());
                    for c in 0..3 {
                        let error = adjusted[c] - reconstructed[c];
                        current[x + 2][c] += error * (7.0 / 16.0);
                        next[x][c] += error * (3.0 / 16.0);
                        next[x + 1][c] += error * (5.0 / 16.0);
                        next[x + 2][c] += error * (1.0 / 16.0);
                    }
                }
                current.fill([0.0; 3]);
                std::mem::swap(&mut current, &mut next);
                next.fill([0.0; 3]);
            }
            Ok(out)
        }
        _ => Err("Supported DWXG colour depths are 16-bit RGB565 and 8-bit grayscale".into()),
    }
}

pub fn encode_dwxg(width: u32, height: u32, rgb: &[u8], depth: u32, compress: bool, dither: bool) -> Result<Vec<u8>, String> {
    let raw = encode_pixels(rgb, width, height, depth, dither)?;
    let payload = if compress {
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&raw).map_err(|e| format!("Compression failed: {e}"))?;
        encoder.finish().map_err(|e| format!("Compression failed: {e}"))?
    } else {
        raw
    };
    let payload_len = u32::try_from(payload.len()).map_err(|_| "DWXG payload is too large")?;
    let mut output = Vec::with_capacity(HEADER_SIZE + payload.len());
    output.extend_from_slice(b"DWXG");
    output.extend_from_slice(&width.to_be_bytes());
    output.extend_from_slice(&height.to_be_bytes());
    output.extend_from_slice(&(if compress { 1u32 } else { 0u32 }).to_be_bytes());
    output.extend_from_slice(&depth.to_be_bytes());
    output.extend_from_slice(&payload_len.to_be_bytes());
    output.extend_from_slice(&payload);
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_rgb565_raw_and_compressed() {
        let rgb = [255, 0, 0, 0, 255, 0, 0, 0, 255, 250, 250, 250];
        for compressed in [false, true] {
            let bytes = encode_dwxg(2, 2, &rgb, 16, compressed, false).unwrap();
            let decoded = decode_dwxg(&bytes).unwrap();
            assert_eq!((decoded.width, decoded.height, decoded.bits_per_pixel), (2, 2, 16));
            assert_eq!(decoded.rgb.len(), 12);
            assert_eq!(&decoded.rgb[0..3], &[255, 0, 0]);
        }
    }

    #[test]
    fn round_trip_grayscale() {
        let rgb = [255, 0, 0, 0, 255, 0];
        let bytes = encode_dwxg(2, 1, &rgb, 8, false, false).unwrap();
        let decoded = decode_dwxg(&bytes).unwrap();
        assert_eq!(decoded.bits_per_pixel, 8);
        assert_eq!(decoded.rgb.len(), 6);
        assert_eq!(decoded.rgb[0], decoded.rgb[1]);
    }
}
