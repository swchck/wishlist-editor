//! Turns whatever picture the user drops in into a small JPEG fit for a public repository.
//!
//! The input is untrusted bytes, so decoding runs under explicit size limits. Re-encoding
//! is also what strips metadata: phone photos carry GPS coordinates in EXIF, and those
//! must never reach a public repo.

use std::io::Cursor;

use image::codecs::jpeg::JpegEncoder;
use image::{DynamicImage, ImageDecoder, ImageReader, Limits};

use crate::error::{AppError, AppResult};

/// Largest file accepted before decoding, in bytes.
pub const MAX_INPUT_BYTES: usize = 40 * 1024 * 1024;
/// Longest side of a stored photo, in pixels; the site never shows cards bigger than this.
const MAX_SIDE: u32 = 1600;
const JPEG_QUALITY: u8 = 82;

/// File extension of every photo `compress` produces.
pub const EXTENSION: &str = "jpg";

/// Decodes a JPEG, PNG, WebP or GIF (first frame), applies its EXIF orientation,
/// fits it into 1600×1600 and re-encodes it as a metadata-free JPEG.
///
/// # Errors
///
/// `BadImage` when the bytes are too large, not a supported format, or corrupt.
pub fn compress(bytes: &[u8]) -> AppResult<Vec<u8>> {
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(AppError::BadImage("файл больше 40 МБ".into()));
    }
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| AppError::BadImage(e.to_string()))?;
    if reader.format().is_none() {
        return Err(AppError::BadImage(
            "формат не поддерживается — нужен JPEG, PNG, WebP или GIF".into(),
        ));
    }
    // a 40 MB PNG can claim 60000×60000 px; refuse before allocating 14 GB for it
    let mut limits = Limits::default();
    limits.max_image_width = Some(12_000);
    limits.max_image_height = Some(12_000);
    limits.max_alloc = Some(512 * 1024 * 1024);
    reader.limits(limits);

    let bad = |e: image::ImageError| AppError::BadImage(e.to_string());
    let mut decoder = reader.into_decoder().map_err(bad)?;
    let orientation = decoder.orientation().map_err(bad)?;
    let mut img = DynamicImage::from_decoder(decoder).map_err(bad)?;
    img.apply_orientation(orientation);

    if img.width() > MAX_SIDE || img.height() > MAX_SIDE {
        img = img.resize(MAX_SIDE, MAX_SIDE, image::imageops::FilterType::Lanczos3);
    }
    // JPEG has no alpha: transparent PNG areas would turn black, white matches the site's cards
    let rgb = flatten_on_white(img);

    let mut out = Vec::new();
    JpegEncoder::new_with_quality(&mut out, JPEG_QUALITY)
        .encode_image(&rgb)
        .map_err(bad)?;
    Ok(out)
}

fn flatten_on_white(img: DynamicImage) -> image::RgbImage {
    if !img.color().has_alpha() {
        return img.into_rgb8();
    }
    let rgba = img.into_rgba8();
    image::RgbImage::from_fn(rgba.width(), rgba.height(), |x, y| {
        let [r, g, b, a] = rgba.get_pixel(x, y).0;
        let blend =
            |c: u8| ((u16::from(c) * u16::from(a) + 255 * (255 - u16::from(a))) / 255) as u8;
        image::Rgb([blend(r), blend(g), blend(b)])
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageFormat, RgbaImage};

    fn encode(img: DynamicImage, format: ImageFormat) -> Vec<u8> {
        let mut out = Cursor::new(Vec::new());
        img.write_to(&mut out, format).unwrap();
        out.into_inner()
    }

    #[test]
    fn shrinks_big_pictures_and_keeps_aspect() {
        let big = DynamicImage::new_rgb8(4000, 2000);
        let out = compress(&encode(big, ImageFormat::Png)).unwrap();
        let back = image::load_from_memory(&out).unwrap();
        assert_eq!((back.width(), back.height()), (1600, 800));
        assert_eq!(
            image::guess_format(&out).unwrap(),
            ImageFormat::Jpeg,
            "always stored as jpeg"
        );
    }

    #[test]
    fn small_pictures_keep_their_size() {
        let out = compress(&encode(DynamicImage::new_rgb8(300, 200), ImageFormat::WebP)).unwrap();
        let back = image::load_from_memory(&out).unwrap();
        assert_eq!((back.width(), back.height()), (300, 200));
    }

    #[test]
    fn transparency_becomes_white() {
        let clear = DynamicImage::ImageRgba8(RgbaImage::new(8, 8));
        let out = compress(&encode(clear, ImageFormat::Png)).unwrap();
        let px = image::load_from_memory(&out).unwrap().into_rgb8();
        assert!(px.get_pixel(4, 4).0.iter().all(|&c| c > 245));
    }

    #[test]
    fn rejects_junk() {
        for junk in [&b"not an image"[..], &b"\x89PNG\r\n\x1a\n broken"[..], &[]] {
            assert!(matches!(compress(junk), Err(AppError::BadImage(_))));
        }
    }
}
