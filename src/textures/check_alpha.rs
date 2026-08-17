use super::image_format::ImageFormat;

/// Scans decoded pixels for transparency. BMP and JPG are recorded as opaque without decoding.
///
/// Most Minecraft block PNGs are palette, grayscale, or RGB. Those can be classified from the
/// IHDR/tRNS chunks without inflating IDAT. True RGBA / gray+alpha still decode.
pub(crate) fn inspect_alpha(bytes: &[u8], format: ImageFormat) -> bool {
    if !format.carries_alpha_channel() {
        return false;
    }
    match png_alpha_from_header(bytes) {
        Some(has_alpha) => has_alpha,
        None => scan_decoded_alpha(bytes),
    }
}

/// `Some` when IHDR color type plus optional tRNS is enough. `None` means decode the pixels.
fn png_alpha_from_header(bytes: &[u8]) -> Option<bool> {
    const SIGNATURE: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    if !bytes.starts_with(SIGNATURE) {
        return None;
    }

    let mut offset = 8;
    let mut color_type = None;
    let mut trns = None;

    while offset + 12 <= bytes.len() {
        let length = u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
        let chunk_type = &bytes[offset + 4..offset + 8];
        let data_start = offset + 8;
        let Some(data_end) = data_start.checked_add(length) else {
            break;
        };
        let Some(next) = data_end.checked_add(4) else {
            break;
        };
        if next > bytes.len() {
            break;
        }
        let data = &bytes[data_start..data_end];
        match chunk_type {
            b"IHDR" if data.len() >= 10 => color_type = Some(data[9]),
            b"tRNS" => trns = Some(data),
            b"IDAT" | b"IEND" => break,
            _ => {}
        }
        offset = next;
    }

    match color_type? {
        0 | 2 => Some(trns.is_some()),
        3 => Some(trns.is_some_and(|chunk| chunk.iter().any(|&alpha| alpha < 255))),
        4 | 6 => None,
        _ => None,
    }
}

fn scan_decoded_alpha(bytes: &[u8]) -> bool {
    match image::load_from_memory(bytes) {
        Ok(img) => img.to_rgba8().pixels().any(|pixel| pixel.0[3] < 255),
        Err(err) => {
            log::warn!("could not decode image to inspect alpha: {err}");
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};
    use std::io::Cursor;

    fn encode(img: &RgbaImage, format: image::ImageFormat) -> Vec<u8> {
        let mut bytes = Vec::new();
        img.write_to(&mut Cursor::new(&mut bytes), format)
            .expect("encode test image");
        bytes
    }

    fn png_chunks(color_type: u8, trns: Option<&[u8]>) -> Vec<u8> {
        let mut bytes = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        let mut write_chunk = |chunk_type: &[u8; 4], data: &[u8]| {
            bytes.extend_from_slice(&(data.len() as u32).to_be_bytes());
            bytes.extend_from_slice(chunk_type);
            bytes.extend_from_slice(data);
            bytes.extend_from_slice(&[0, 0, 0, 0]);
        };
        write_chunk(b"IHDR", &[0, 0, 0, 1, 0, 0, 0, 1, 8, color_type, 0, 0, 0]);
        if let Some(trns) = trns {
            write_chunk(b"tRNS", trns);
        }
        write_chunk(b"IDAT", &[]);
        bytes
    }

    #[test]
    fn inspect_alpha_skips_bmp_and_jpg() {
        assert!(!inspect_alpha(&[], ImageFormat::Bmp));
        assert!(!inspect_alpha(&[], ImageFormat::Jpg));
    }

    #[test]
    fn inspect_alpha_scans_png() {
        let opaque = encode(
            &RgbaImage::from_pixel(2, 2, Rgba([255, 0, 0, 255])),
            image::ImageFormat::Png,
        );
        let transparent = encode(
            &RgbaImage::from_pixel(2, 2, Rgba([255, 0, 0, 128])),
            image::ImageFormat::Png,
        );
        assert!(!inspect_alpha(&opaque, ImageFormat::Png));
        assert!(inspect_alpha(&transparent, ImageFormat::Png));
    }

    #[test]
    fn inspect_alpha_reads_png_header_without_decoding() {
        assert!(!inspect_alpha(&png_chunks(3, None), ImageFormat::Png));
        assert!(!inspect_alpha(
            &png_chunks(3, Some(&[255])),
            ImageFormat::Png
        ));
        assert!(inspect_alpha(
            &png_chunks(3, Some(&[128])),
            ImageFormat::Png
        ));
        assert!(!inspect_alpha(&png_chunks(2, None), ImageFormat::Png));
        assert!(inspect_alpha(
            &png_chunks(2, Some(&[0, 0, 0, 0, 0, 0])),
            ImageFormat::Png
        ));
        assert!(!inspect_alpha(&png_chunks(0, None), ImageFormat::Png));
        assert!(inspect_alpha(
            &png_chunks(0, Some(&[0, 0])),
            ImageFormat::Png
        ));
        assert_eq!(png_alpha_from_header(&png_chunks(6, None)), None);
    }
}
