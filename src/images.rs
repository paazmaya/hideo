use std::collections::HashMap;
use std::path::Path;
use camino::Utf8PathBuf;
use blake3;
use image::{ImageReader, DynamicImage, ImageFormat, imageops::FilterType};
use image::codecs::png::PngDecoder;
use std::io::Cursor;
use tempfile::NamedTempFile;
use crate::config::Config;
use crate::error::{DiagnosticCollector, Warning, WarningCode};

#[derive(Debug, Clone)]
pub struct ProcessedImage {
    pub virtual_path: String,
    pub hash: String,
}

pub struct ImageProcessor {
    cache_dir: Utf8PathBuf,
    source_dir: Utf8PathBuf,
    config: Config,
    cache: HashMap<String, ProcessedImage>,
}

impl ImageProcessor {
    pub fn new(cache_dir: Utf8PathBuf, source_dir: Utf8PathBuf, config: Config) -> Self {
        Self {
            cache_dir,
            source_dir,
            config,
            cache: HashMap::new(),
        }
    }

    pub fn process_hero(&mut self, page_path: &Utf8PathBuf, hero_path: &str, page_key: &str, diagnostics: &mut DiagnosticCollector) -> ProcessedImage {
        let hero_path = self.resolve_hero_path(page_path, hero_path);
        let cache_key = self.compute_cache_key(&hero_path, "hero");

        if let Some(cached) = self.cache.get(&cache_key) {
            return cached.clone();
        }

        let result = self.process_image(&hero_path, &cache_key, diagnostics, page_key);

        match result {
            Ok(processed) => {
                self.cache.insert(cache_key.clone(), processed.clone());
                processed
            }
            Err(_) => {
                diagnostics.add_warning(
                    Warning::new(WarningCode::W003, "missing hero image, using placeholder")
                        .with_page(page_key.to_string()),
                );
                self.create_placeholder(&cache_key, "hero")
            }
        }
    }

    pub fn process_cover(&mut self, tier: &str, diagnostics: &mut DiagnosticCollector) -> ProcessedImage {
        let pattern = &self.config.images.cover_pattern;
        let cover_path = pattern.replace("{tier}", tier);
        let cover_path = self.source_dir.join(&cover_path);
        let cache_key = self.compute_cache_key(&cover_path, "cover");

        if let Some(cached) = self.cache.get(&cache_key) {
            return cached.clone();
        }

        let result = self.process_image(&cover_path, &cache_key, diagnostics, tier);

        match result {
            Ok(processed) => {
                self.cache.insert(cache_key.clone(), processed.clone());
                processed
            }
            Err(_) => {
                self.create_placeholder(&cache_key, "cover")
            }
        }
    }

    pub fn process_contents_hero(&mut self, diagnostics: &mut DiagnosticCollector) -> Option<ProcessedImage> {
        if let Some(hero_path) = &self.config.images.contents_hero {
            let hero_path = self.source_dir.join(hero_path);
            let cache_key = self.compute_cache_key(&hero_path, "contents");

            if let Some(cached) = self.cache.get(&cache_key) {
                return Some(cached.clone());
            }

            let result = self.process_image(&hero_path, &cache_key, diagnostics, "contents");

            match result {
                Ok(processed) => {
                    self.cache.insert(cache_key.clone(), processed.clone());
                    Some(processed)
                }
                Err(_) => None,
            }
        } else {
            None
        }
    }

    fn resolve_hero_path(&self, page_path: &Utf8PathBuf, hero_rel: &str) -> Utf8PathBuf {
        let page_dir = page_path.parent().unwrap_or(&Utf8PathBuf::from(""));
        let candidate = page_dir.join(hero_rel);
        if candidate.exists() {
            return candidate;
        }
        self.source_dir.join(hero_rel)
    }

    fn compute_cache_key(&self, path: &Utf8PathBuf, kind: &str) -> String {
        let mut hasher = blake3::Hasher::new();
        if let Ok(bytes) = std::fs::read(path) {
            hasher.update(&bytes);
        }
        let settings = serde_json::json!({
            "dpi": self.config.images.dpi,
            "jpeg_quality": self.config.images.jpeg_quality,
            "page_width_mm": self.config.page.width_mm,
            "margin_x_mm": self.config.page.margin_x_mm,
        });
        hasher.update(settings.to_string().as_bytes());
        hasher.update(b"hideo_image_pipeline_v1");
        let hash = hasher.finalize();
        format!("{}_{}.jpg", kind, hash.to_hex().as_str()[..16].to_string())
    }

    fn process_image(
        &self,
        path: &Utf8PathBuf,
        cache_key: &str,
        diagnostics: &mut DiagnosticCollector,
        page_key: &str,
    ) -> Result<ProcessedImage, anyhow::Error> {
        let bytes = std::fs::read(path)?;

        let mut reader = ImageReader::new(std::io::Cursor::new(&bytes));
        reader = reader.with_guessed_format()?;
        reader.no_limits();

        let mut img = reader.decode()?;

        if img.width() > 20000 || img.height() > 20000 {
            anyhow::bail!("image dimensions exceed limit");
        }

        // Apply EXIF orientation if present
        if let Some(orientation) = reader.orientation() {
            img.apply_orientation(orientation);
        }

        if img.color().has_alpha() {
            let mut rgb = DynamicImage::new_rgb8(img.width(), img.height());
            image::imageops::overlay(&mut rgb, &img.to_rgba8(), 0, 0);
            img = rgb;
        } else {
            img = img.to_rgb8().into();
        }

        let target_width_px = ((self.config.page.width_mm * self.config.images.dpi as f32) / 25.4).ceil() as u32;
        if img.width() > target_width_px {
            img = img.resize(target_width_px, 
                (img.height() as f32 * target_width_px as f32 / img.width() as f32) as u32,
                image::imageops::FilterType::Lanczos3);
        }

        let cache_path = self.cache_dir.join(cache_key);
        if let Some(parent) = cache_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let temp_file = NamedTempFile::new_in(&self.cache_dir)?;
        img.write_to(&mut std::io::BufWriter::new(temp_file.as_file()), ImageFormat::Jpeg)?;
        temp_file.persist(&cache_path)?;

        Ok(ProcessedImage {
            virtual_path: format!("images/{}", cache_key),
            hash: cache_key.to_string(),
        })
    }

    fn create_placeholder(&self, cache_key: &str, kind: &str) -> ProcessedImage {
        let (width, height, color) = match kind {
            "hero" => (1280, 720, [0xE7, 0xE3, 0xEF]),
            "cover" => (1280, 720, [0xE7, 0xE3, 0xEF]),
            _ => (1280, 720, [0xE7, 0xE3, 0xEF]),
        };

        let mut img = DynamicImage::new_rgb8(width, height);
        for pixel in img.as_mut_rgb8().unwrap().pixels_mut() {
            pixel[0] = color[0];
            pixel[1] = color[1];
            pixel[2] = color[2];
        }

        let cache_path = self.cache_dir.join(cache_key);
        if let Some(parent) = cache_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let _ = img.save_with_format(&cache_path, ImageFormat::Jpeg);

        ProcessedImage {
            virtual_path: format!("images/{}", cache_key),
            hash: cache_key.to_string(),
        }
    }
}