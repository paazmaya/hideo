//! Hero, cover and body images (plan 7.6, F11, F12).
//!
//! Pipeline: decode with limits, honour EXIF orientation, flatten alpha onto white, scale to
//! the page width at the configured DPI (never upscale), encode JPEG. Results are cached on disk
//! under a content hash so a warm rebuild skips all of it.

use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::metadata::Orientation;
use image::{DynamicImage, ImageDecoder, ImageReader, Limits, RgbImage};

/// Bump when the pipeline changes so old cache entries are not reused.
pub const PIPELINE_VERSION: &str = "hideo_image_pipeline_v1";
/// Decompression-bomb guard: refuse anything larger.
pub const MAX_DIMENSION: u32 = 20_000;
/// Placeholder colour, `#E7E3EF` (Appendix B).
pub const PLACEHOLDER_RGB: [u8; 3] = [0xE7, 0xE3, 0xEF];

#[derive(Debug, Clone)]
pub struct ImageSettings {
    pub page_width_mm: f64,
    pub dpi: u32,
    pub jpeg_quality: u8,
}

impl ImageSettings {
    /// `ceil(page_width_mm * dpi / 25.4)`.
    pub fn target_width_px(&self) -> u32 {
        (self.page_width_mm * self.dpi as f64 / 25.4).ceil() as u32
    }
}

/// An image ready to be registered as a virtual file.
#[derive(Debug, Clone)]
pub struct ProcessedImage {
    /// Virtual path, `images/<hash>.jpg`.
    pub vpath: String,
    pub bytes: Arc<Vec<u8>>,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, thiserror::Error)]
pub enum ImageError {
    #[error("cannot read {path}: {source}")]
    Read {
        path: String,
        source: std::io::Error,
    },
    #[error("cannot decode image: {0}")]
    Decode(String),
}

/// Cache key: `blake3(file bytes || settings || pipeline version)`.
pub fn cache_key(bytes: &[u8], settings: &ImageSettings) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(bytes);
    hasher.update(
        format!(
            "{}|{}|{}",
            settings.page_width_mm, settings.dpi, settings.jpeg_quality
        )
        .as_bytes(),
    );
    hasher.update(PIPELINE_VERSION.as_bytes());
    hasher.finalize().to_hex()[..32].to_string()
}

/// Process an image file, using `cache_dir` (if given) as `<cache_dir>/images/<hash>.jpg`.
pub fn process_file(
    path: &Path,
    settings: &ImageSettings,
    cache_dir: Option<&Path>,
) -> Result<ProcessedImage, ImageError> {
    let bytes = std::fs::read(path).map_err(|source| ImageError::Read {
        path: path.display().to_string(),
        source,
    })?;
    process_bytes(&bytes, settings, cache_dir)
}

pub fn process_bytes(
    bytes: &[u8],
    settings: &ImageSettings,
    cache_dir: Option<&Path>,
) -> Result<ProcessedImage, ImageError> {
    let key = cache_key(bytes, settings);
    let vpath = format!("images/{key}.jpg");
    let cache_path = cache_dir.map(|d| d.join("images").join(format!("{key}.jpg")));

    if let Some(cached) = cache_path.as_ref().and_then(|p| std::fs::read(p).ok())
        && let Ok((width, height)) = jpeg_dimensions(&cached)
    {
        return Ok(ProcessedImage {
            vpath,
            bytes: Arc::new(cached),
            width,
            height,
        });
    }

    let img = decode(bytes)?;
    let img = flatten_and_scale(img, settings);
    let encoded = encode_jpeg(&img, settings.jpeg_quality)?;

    if let Some(path) = &cache_path {
        // A failed cache write must never fail the build.
        let _ = write_cache(path, &encoded);
    }
    Ok(ProcessedImage {
        vpath,
        bytes: Arc::new(encoded),
        width: img.width(),
        height: img.height(),
    })
}

/// A 16:9 light placeholder, generated in memory (no disk cache).
pub fn placeholder(settings: &ImageSettings) -> ProcessedImage {
    let (width, height) = (1280u32, 720u32);
    let img = RgbImage::from_pixel(width, height, image::Rgb(PLACEHOLDER_RGB));
    let encoded = encode_jpeg(&img, settings.jpeg_quality).expect("encoding a flat image succeeds");
    ProcessedImage {
        vpath: "images/placeholder.jpg".to_string(),
        bytes: Arc::new(encoded),
        width,
        height,
    }
}

fn decode(bytes: &[u8]) -> Result<DynamicImage, ImageError> {
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| ImageError::Decode(e.to_string()))?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_DIMENSION);
    limits.max_image_height = Some(MAX_DIMENSION);
    limits.max_alloc = Some(1 << 30);
    reader.limits(limits);
    let mut decoder = reader
        .into_decoder()
        .map_err(|e| ImageError::Decode(e.to_string()))?;
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    let mut img =
        DynamicImage::from_decoder(decoder).map_err(|e| ImageError::Decode(e.to_string()))?;
    img.apply_orientation(orientation);
    Ok(img)
}

/// Composite alpha onto white, convert to RGB8, scale down to the target width.
fn flatten_and_scale(img: DynamicImage, settings: &ImageSettings) -> RgbImage {
    let rgba = img.to_rgba8();
    let mut rgb = RgbImage::new(rgba.width(), rgba.height());
    for (x, y, p) in rgba.enumerate_pixels() {
        let a = p[3] as u32;
        let blend = |c: u8| ((c as u32 * a + 255 * (255 - a) + 127) / 255) as u8;
        rgb.put_pixel(x, y, image::Rgb([blend(p[0]), blend(p[1]), blend(p[2])]));
    }
    let target = settings.target_width_px().max(1);
    if rgb.width() > target {
        let height =
            ((rgb.height() as f64 * target as f64 / rgb.width() as f64).round() as u32).max(1);
        image::imageops::resize(&rgb, target, height, FilterType::Lanczos3)
    } else {
        rgb
    }
}

fn encode_jpeg(img: &RgbImage, quality: u8) -> Result<Vec<u8>, ImageError> {
    let mut out = Vec::new();
    JpegEncoder::new_with_quality(&mut out, quality)
        .encode_image(img)
        .map_err(|e| ImageError::Decode(e.to_string()))?;
    Ok(out)
}

fn jpeg_dimensions(bytes: &[u8]) -> Result<(u32, u32), ImageError> {
    let reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| ImageError::Decode(e.to_string()))?;
    reader
        .into_dimensions()
        .map_err(|e| ImageError::Decode(e.to_string()))
}

/// Temp file + rename so parallel runs never read a partial file.
fn write_cache(path: &PathBuf, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let dir = path.parent().expect("cache path has a parent");
    std::fs::create_dir_all(dir)?;
    let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
    tmp.write_all(bytes)?;
    tmp.persist(path).map_err(|e| e.error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageFormat, Rgba, RgbaImage};

    fn settings() -> ImageSettings {
        // 100 mm at 100 dpi -> 394 px wide
        ImageSettings {
            page_width_mm: 100.0,
            dpi: 100,
            jpeg_quality: 70,
        }
    }

    fn png(width: u32, height: u32, pixel: Rgba<u8>) -> Vec<u8> {
        let img = RgbaImage::from_pixel(width, height, pixel);
        let mut out = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img)
            .write_to(&mut out, ImageFormat::Png)
            .unwrap();
        out.into_inner()
    }

    #[test]
    fn target_width_rounds_up() {
        assert_eq!(settings().target_width_px(), 394); // 393.7 -> 394
        let a4 = ImageSettings {
            page_width_mm: 210.0,
            dpi: 200,
            jpeg_quality: 70,
        };
        assert_eq!(a4.target_width_px(), 1654);
    }

    #[test]
    fn test_hero_scaled_not_upscaled() {
        let big =
            process_bytes(&png(1000, 500, Rgba([10, 20, 30, 255])), &settings(), None).unwrap();
        assert_eq!((big.width, big.height), (394, 197));
        let small =
            process_bytes(&png(100, 50, Rgba([10, 20, 30, 255])), &settings(), None).unwrap();
        assert_eq!((small.width, small.height), (100, 50), "never upscale");
        assert!(big.bytes.starts_with(&[0xFF, 0xD8]), "output is a JPEG");
        assert!(big.vpath.starts_with("images/") && big.vpath.ends_with(".jpg"));
    }

    #[test]
    fn alpha_is_composited_onto_white() {
        let img = process_bytes(&png(8, 8, Rgba([0, 0, 0, 0])), &settings(), None).unwrap();
        let decoded = image::load_from_memory(&img.bytes).unwrap().to_rgb8();
        let p = decoded.get_pixel(4, 4);
        assert!(
            p.0.iter().all(|c| *c >= 250),
            "transparent becomes white, got {p:?}"
        );
    }

    #[test]
    fn test_hero_exif_orientation() {
        // A JPEG with EXIF orientation 6 (rotate 90 deg clockwise): stored 40x20, shown 20x40.
        let mut jpeg = Vec::new();
        JpegEncoder::new_with_quality(&mut jpeg, 90)
            .encode_image(&RgbImage::from_pixel(40, 20, image::Rgb([200, 10, 10])))
            .unwrap();
        // Insert an APP1 EXIF segment right after SOI.
        let exif: Vec<u8> = {
            let mut e = b"Exif\0\0".to_vec();
            e.extend_from_slice(b"MM\0\x2A\0\0\0\x08"); // big endian TIFF header, IFD at 8
            e.extend_from_slice(&[0, 1]); // one entry
            e.extend_from_slice(&[0x01, 0x12, 0, 3, 0, 0, 0, 1, 0, 6, 0, 0]); // orientation = 6
            e.extend_from_slice(&[0, 0, 0, 0]); // next IFD
            e
        };
        let mut with_exif = vec![0xFF, 0xD8, 0xFF, 0xE1];
        with_exif.extend_from_slice(&((exif.len() + 2) as u16).to_be_bytes());
        with_exif.extend_from_slice(&exif);
        with_exif.extend_from_slice(&jpeg[2..]);

        let out = process_bytes(&with_exif, &settings(), None).unwrap();
        assert_eq!(
            (out.width, out.height),
            (20, 40),
            "rotated by EXIF orientation"
        );
    }

    #[test]
    fn test_hero_decompression_bomb_rejected() {
        // PNG header declaring 30000 x 30000 pixels; the limit check fires before allocation.
        let mut data = png(4, 4, Rgba([1, 2, 3, 255]));
        // IHDR width/height live at bytes 16..24.
        data[16..20].copy_from_slice(&30_000u32.to_be_bytes());
        data[20..24].copy_from_slice(&30_000u32.to_be_bytes());
        let err = process_bytes(&data, &settings(), None).unwrap_err();
        assert!(matches!(err, ImageError::Decode(_)), "{err}");
    }

    #[test]
    fn garbage_is_a_decode_error() {
        assert!(matches!(
            process_bytes(b"not an image", &settings(), None),
            Err(ImageError::Decode(_))
        ));
    }

    #[test]
    fn test_hero_missing_placeholder() {
        let p = placeholder(&settings());
        assert_eq!((p.width, p.height), (1280, 720));
        assert_eq!(p.vpath, "images/placeholder.jpg");
        let decoded = image::load_from_memory(&p.bytes).unwrap().to_rgb8();
        let px = decoded.get_pixel(10, 10);
        // JPEG is lossy: within a few levels of #E7E3EF.
        for (got, want) in px.0.iter().zip(PLACEHOLDER_RGB) {
            assert!((*got as i32 - want as i32).abs() <= 3, "{px:?}");
        }
        assert_eq!(placeholder(&settings()).bytes, p.bytes, "deterministic");
    }

    #[test]
    fn cache_hit_skips_decoding_and_survives_garbage_source() {
        let dir = tempfile::tempdir().unwrap();
        let source = png(500, 300, Rgba([5, 6, 7, 255]));
        let first = process_bytes(&source, &settings(), Some(dir.path())).unwrap();
        let cached = dir
            .path()
            .join("images")
            .join(first.vpath.trim_start_matches("images/"));
        assert!(cached.exists());
        // Replace the cache file's contents with a different valid JPEG: a hit must return it.
        let marker = encode_jpeg(&RgbImage::from_pixel(7, 9, image::Rgb([1, 2, 3])), 70).unwrap();
        std::fs::write(&cached, &marker).unwrap();
        let second = process_bytes(&source, &settings(), Some(dir.path())).unwrap();
        assert_eq!((second.width, second.height), (7, 9));
        assert_eq!(second.bytes.as_slice(), marker.as_slice());
        // A corrupt cache entry is ignored and rebuilt.
        std::fs::write(&cached, b"corrupt").unwrap();
        let third = process_bytes(&source, &settings(), Some(dir.path())).unwrap();
        assert_eq!(third.bytes, first.bytes);
    }

    #[test]
    fn cache_key_depends_on_content_and_settings() {
        let a = cache_key(b"abc", &settings());
        assert_eq!(a, cache_key(b"abc", &settings()));
        assert_ne!(a, cache_key(b"abd", &settings()));
        let mut other = settings();
        other.jpeg_quality = 71;
        assert_ne!(a, cache_key(b"abc", &other));
        other = settings();
        other.dpi = 101;
        assert_ne!(a, cache_key(b"abc", &other));
    }

    #[test]
    fn test_image_errors() {
        let err = process_file(Path::new("does/not/exist.png"), &settings(), None);
        assert!(matches!(err, Err(ImageError::Read { .. })));

        let bad_dims = jpeg_dimensions(b"corrupt image bytes");
        assert!(matches!(bad_dims, Err(ImageError::Decode(_))));
    }
}
