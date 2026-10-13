//! A tiny, dependency-free PNG writer plus the PWA icon renderer.
//!
//! Icons are generated at build time so a freshly scaffolded Selarust app is an
//! installable PWA without shipping binary assets in the template. Only the
//! "stored" (uncompressed) deflate mode is used, which keeps the encoder short
//! and obviously correct; generated icons are a few kilobytes each.

use crate::{Error, Result};

/// An 8-bit RGBA image.
#[derive(Debug, Clone)]
pub struct RgbaImage {
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
    /// Row-major RGBA bytes, `width * height * 4` long.
    pub data: Vec<u8>,
}

impl RgbaImage {
    /// Allocates a fully transparent image.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            data: vec![0; width as usize * height as usize * 4],
        }
    }

    /// Reads a pixel as RGBA.
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let index = ((y * self.width + x) * 4) as usize;
        [
            self.data[index],
            self.data[index + 1],
            self.data[index + 2],
            self.data[index + 3],
        ]
    }

    /// Encodes the image as a PNG byte stream.
    pub fn encode_png(&self) -> Vec<u8> {
        let stride = (self.width * 4) as usize;
        let mut raw = Vec::with_capacity(self.height as usize * (stride + 1));
        for row in self.data.chunks(stride) {
            raw.push(0); // filter type: None
            raw.extend_from_slice(row);
        }

        let mut ihdr = Vec::with_capacity(13);
        ihdr.extend_from_slice(&self.width.to_be_bytes());
        ihdr.extend_from_slice(&self.height.to_be_bytes());
        // 8 bit depth, color type 6 (RGBA), deflate, adaptive filtering, no interlace.
        ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);

        let mut png = Vec::with_capacity(raw.len() / 8 + 128);
        png.extend_from_slice(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]);
        write_chunk(&mut png, b"IHDR", &ihdr);
        write_chunk(&mut png, b"IDAT", &zlib_deflate(&raw));
        write_chunk(&mut png, b"IEND", &[]);
        png
    }
}

/// Parses `#rrggbb` or `#rgb` (with or without the leading `#`) into RGB bytes.
pub fn parse_hex_color(value: &str) -> Result<[u8; 3]> {
    let trimmed = value.trim();
    let hex = trimmed.strip_prefix('#').unwrap_or(trimmed);
    let invalid = || Error::new(format!("invalid color `{value}`: expected #rgb or #rrggbb"));

    if !hex.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err(invalid());
    }

    let nibble = |ch: char| -> u8 { ch.to_digit(16).map(|value| value as u8).unwrap_or(0) };
    let bytes = hex.as_bytes();

    match bytes.len() {
        3 => {
            let digits: Vec<u8> = hex.chars().map(|ch| nibble(ch) * 17).collect();
            Ok([digits[0], digits[1], digits[2]])
        }
        6 => {
            let mut channels = [0u8; 3];
            for (index, channel) in channels.iter_mut().enumerate() {
                let high = nibble(bytes[index * 2] as char);
                let low = nibble(bytes[index * 2 + 1] as char);
                *channel = high * 16 + low;
            }
            Ok(channels)
        }
        _ => Err(invalid()),
    }
}

/// Renders a Selarust app icon.
///
/// Maskable icons are drawn full-bleed (no transparent corners) with the glyph
/// kept inside the 80% safe zone that maskable icons require.
pub fn render_icon(size: u32, theme: [u8; 3], maskable: bool) -> RgbaImage {
    const SUPERSAMPLE: u32 = 3;

    let size = size.clamp(16, 2048);
    let canvas = (size * SUPERSAMPLE) as f32;
    let samples = (SUPERSAMPLE * SUPERSAMPLE) as f32;

    let corner_radius = if maskable { 0.0 } else { canvas * 0.22 };
    let glyph_extent = if maskable { 0.16 } else { 0.20 };
    let top = to_unit(theme);
    let bottom = to_unit(darken(theme, 0.45));

    let mut accum = vec![0f32; (size * size * 4) as usize];

    for y in 0..size * SUPERSAMPLE {
        for x in 0..size * SUPERSAMPLE {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;

            let mut color = [0f32; 3];
            let mut alpha = 0f32;

            if corner_radius <= 0.0 || inside_rounded_rect(px, py, canvas, corner_radius) {
                let gradient = lerp3(top, bottom, py / canvas);
                let outer = inside_diamond(px, py, canvas, glyph_extent);
                let inner = inside_diamond(px, py, canvas, glyph_extent * 0.42);
                color = if outer && !inner { [1.0, 1.0, 1.0] } else { gradient };
                alpha = 1.0;
            }

            let index = (((y / SUPERSAMPLE) * size + (x / SUPERSAMPLE)) * 4) as usize;
            accum[index] += color[0] * alpha;
            accum[index + 1] += color[1] * alpha;
            accum[index + 2] += color[2] * alpha;
            accum[index + 3] += alpha;
        }
    }

    let mut image = RgbaImage::new(size, size);
    for (index, pixel) in image.data.chunks_exact_mut(4).enumerate() {
        let offset = index * 4;
        let alpha_sum = accum[offset + 3];
        if alpha_sum > 0.0 {
            let inverse = 1.0 / alpha_sum;
            pixel[0] = to_u8(accum[offset] * inverse);
            pixel[1] = to_u8(accum[offset + 1] * inverse);
            pixel[2] = to_u8(accum[offset + 2] * inverse);
        }
        pixel[3] = to_u8(alpha_sum / samples);
    }

    image
}

/// Renders a Selarust app icon as PNG bytes.
pub fn icon_png(size: u32, theme: [u8; 3], maskable: bool) -> Vec<u8> {
    render_icon(size, theme, maskable).encode_png()
}

fn to_unit(color: [u8; 3]) -> [f32; 3] {
    [
        color[0] as f32 / 255.0,
        color[1] as f32 / 255.0,
        color[2] as f32 / 255.0,
    ]
}

fn to_u8(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn darken(color: [u8; 3], factor: f32) -> [u8; 3] {
    [
        (color[0] as f32 * factor).round() as u8,
        (color[1] as f32 * factor).round() as u8,
        (color[2] as f32 * factor).round() as u8,
    ]
}

fn lerp3(from: [f32; 3], to: [f32; 3], t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0);
    [
        from[0] + (to[0] - from[0]) * t,
        from[1] + (to[1] - from[1]) * t,
        from[2] + (to[2] - from[2]) * t,
    ]
}

fn inside_rounded_rect(x: f32, y: f32, size: f32, radius: f32) -> bool {
    let clamped_x = x.clamp(radius, size - radius);
    let clamped_y = y.clamp(radius, size - radius);
    let dx = x - clamped_x;
    let dy = y - clamped_y;
    dx * dx + dy * dy <= radius * radius
}

fn inside_diamond(x: f32, y: f32, size: f32, extent: f32) -> bool {
    let center = size / 2.0;
    let radius = size * extent;
    (x - center).abs() / radius + (y - center).abs() / radius <= 1.0
}

fn write_chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let crc = crc32_update(crc32_update(0xFFFF_FFFF, kind), data) ^ 0xFFFF_FFFF;
    out.extend_from_slice(&crc.to_be_bytes());
}

/// Wraps `data` in a zlib stream (deflate + Adler-32) via `flate2`.
///
/// Icons are generated once per build, so the highest compression level is worth
/// it: a 512×512 icon would otherwise be ~1 MB of uncompressed RGBA.
fn zlib_deflate(data: &[u8]) -> Vec<u8> {
    use std::io::Write;

    let mut encoder = flate2::write::ZlibEncoder::new(
        Vec::with_capacity(data.len() / 8 + 64),
        flate2::Compression::best(),
    );
    encoder
        .write_all(data)
        .expect("writing into a Vec never fails");
    encoder.finish().expect("finishing a Vec encoder never fails")
}

fn crc32_update(previous: u32, data: &[u8]) -> u32 {
    let mut crc = previous;
    for byte in data {
        crc ^= *byte as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32_matches_known_value() {
        assert_eq!(crc32_update(0xFFFF_FFFF, b"123456789") ^ 0xFFFF_FFFF, 0xCBF4_3926);
    }

    #[test]
    fn parses_short_and_long_hex_colors() {
        assert_eq!(parse_hex_color("#abc").unwrap(), [170, 187, 204]);
        assert_eq!(parse_hex_color("#0f172a").unwrap(), [15, 23, 42]);
        assert!(parse_hex_color("nope").is_err());
    }

    #[test]
    fn encodes_a_readable_png_container() {
        let png = RgbaImage::new(2, 2).encode_png();
        assert_eq!(&png[..8], &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]);
        assert_eq!(&png[12..16], b"IHDR");
        assert_eq!(u32::from_be_bytes([png[16], png[17], png[18], png[19]]), 2);
        assert_eq!(u32::from_be_bytes([png[20], png[21], png[22], png[23]]), 2);
        assert!(png.ends_with(&[0xAE, 0x42, 0x60, 0x82]));
    }

    #[test]
    fn renders_both_icon_flavours() {
        assert!(icon_png(64, [15, 23, 42], true).len() > 200);
        assert!(icon_png(64, [15, 23, 42], false).len() > 200);
    }

    #[test]
    fn maskable_icons_bleed_to_the_edges() {
        let theme = [15, 23, 42];
        let regular = render_icon(64, theme, false);
        let maskable = render_icon(64, theme, true);

        // Rounded corners stay transparent on the regular icon, opaque when maskable.
        assert_eq!(regular.pixel(0, 0)[3], 0);
        assert_eq!(maskable.pixel(0, 0)[3], 255);

        // The centre of the glyph is the background; the ring around it is white.
        assert!(maskable.pixel(32, 32)[0] < 128, "centre should be background");
        assert!(maskable.pixel(32, 25)[0] > 200, "ring should be white");
    }

    #[test]
    fn compresses_icon_payloads() {
        let raw_size = 512 * (1 + 512 * 4);
        let png = icon_png(512, [15, 23, 42], true);
        assert!(
            png.len() < raw_size / 8,
            "icon should compress well below raw RGBA, got {} bytes",
            png.len()
        );
    }
}
