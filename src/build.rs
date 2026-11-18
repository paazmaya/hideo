//! The per-tier pipeline (plan 7.2): discover, parse, link, emit, images, diagrams, assemble,
//! compile. `check` runs the same steps without images decoding, diagram rendering and Typst.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use camino::{Utf8Path, Utf8PathBuf};
use rayon::prelude::*;
use typst::foundations::Datetime;

use crate::compile::{self, PdfSettings, SourceMaps};
use crate::config::{Config, DiagramBackendConfig};
use crate::diagrams::{self, DiagramCollector, DiagramRenderer, Theme, external::ExternalRenderer};
use crate::discover::{self, SourcePage, TierSources};
use crate::error::{DiagnosticCollector, ErrorCode, HideoError, Warning, WarningCode};
use crate::images::{self, ImageSettings, ProcessedImage};
use crate::linkmap::{LinkMap, UnresolvedLink};
use crate::markdown::emit::{AssetSink, emit_page};
use crate::page::{Page, assign_keys};
use crate::report::Stats;
use crate::template::{self, PageEntry};
use crate::world;

#[derive(Debug, Clone)]
pub struct BuildOptions {
    pub output: Utf8PathBuf,
    pub keep_typst: Option<Utf8PathBuf>,
    pub diagrams: DiagramBackendConfig,
    pub today: Datetime,
}

#[derive(Debug, Clone)]
pub enum Mode {
    /// Validate only: no PDF, no image decoding, no diagram rendering.
    Check,
    Build(BuildOptions),
}

#[derive(Debug, Clone)]
pub struct Options {
    pub source_dir: Utf8PathBuf,
    /// Selected tiers, in config order.
    pub tiers: Vec<String>,
    pub mode: Mode,
    pub verbose: bool,
}

#[derive(Debug, Clone)]
pub struct PageSummary {
    pub path: String,
    pub title: String,
}

#[derive(Debug, Clone)]
pub struct TierSummary {
    pub tier: String,
    pub pages: Vec<PageSummary>,
    pub pdf: Option<Utf8PathBuf>,
    pub pdf_pages: Option<usize>,
}

#[derive(Debug, Default)]
pub struct Outcome {
    pub diagnostics: DiagnosticCollector,
    pub unresolved: Vec<UnresolvedLink>,
    pub stats: Stats,
    pub tiers: Vec<TierSummary>,
}

/// Run the pipeline for the selected tiers.
pub fn run(config: &Config, options: &Options) -> Outcome {
    let mut outcome = Outcome::default();

    // All configured tiers are discovered so links into other tiers can be recognised.
    let (sources, discovery) = discover::discover(config, &config.tiers, &options.source_dir);
    outcome.diagnostics.extend(discovery);
    if outcome.diagnostics.has_errors() {
        // A page that cannot be read would leave a hole in the book: build nothing.
        outcome.diagnostics.sort();
        return outcome;
    }

    let selected: Vec<&TierSources> = sources
        .iter()
        .filter(|t| options.tiers.contains(&t.tier))
        .collect();
    outcome.stats.tiers = selected.len();

    let cache_dir = match &options.mode {
        Mode::Build(b) => Some(b.output.join(&config.build.cache_dir).into_std_path_buf()),
        Mode::Check => None,
    };

    for tier in selected {
        let others: Vec<String> = sources
            .iter()
            .filter(|t| t.tier != tier.tier)
            .flat_map(|t| t.pages.iter().map(|p| p.rel_path.clone()))
            .collect();
        let mut prepared = prepare_tier(config, tier, &others, options, cache_dir.as_deref());
        let summary_pages: Vec<PageSummary> = prepared
            .pages
            .iter()
            .map(|p| PageSummary {
                path: p.rel_path.clone(),
                title: p.title().to_string(),
            })
            .collect();
        outcome.stats.pages += prepared.pages.len();
        outcome.stats.images += prepared.image_count;
        outcome.stats.diagrams += prepared.diagram_count;
        outcome.unresolved.append(&mut prepared.unresolved);

        let mut summary = TierSummary {
            tier: tier.tier.clone(),
            pages: summary_pages,
            pdf: None,
            pdf_pages: None,
        };

        let warnings_block =
            config.diagnostics.warnings_as_errors && !prepared.diag.warnings.is_empty();
        if warnings_block {
            for w in &prepared.diag.warnings {
                prepared.diag.errors.push(
                    HideoError::new(
                        ErrorCode::E005,
                        format!("{} (warnings are errors)", w.message),
                    )
                    .with_page(w.page.clone().unwrap_or_default()),
                );
            }
        }

        if let (Mode::Build(build), false) = (&options.mode, prepared.diag.has_errors()) {
            match compile_tier(config, build, &tier.tier, &prepared) {
                Ok((path, pages, warnings)) => {
                    summary.pdf = Some(path);
                    summary.pdf_pages = Some(pages);
                    outcome.stats.pdfs += 1;
                    for w in warnings {
                        prepared.diag.add_warning(w);
                    }
                }
                Err(errors) => {
                    for e in errors {
                        prepared.diag.add_error(e);
                    }
                }
            }
        }
        outcome.diagnostics.extend(prepared.diag);
        outcome.tiers.push(summary);
    }

    outcome.diagnostics.sort();
    outcome.unresolved.sort();
    outcome.unresolved.dedup();
    outcome
}

/// The generated Typst of one tier without compiling it: for tests that render pages and for
/// debugging tools.
#[derive(Debug)]
pub struct Generated {
    pub files: BTreeMap<String, Vec<u8>>,
    pub maps: SourceMaps,
    pub diag: DiagnosticCollector,
}

/// Run the pipeline for one tier up to the virtual file tree. `options.mode` must be
/// [`Mode::Build`] for the files to be produced. Returns `None` if discovery failed or the tier
/// does not exist.
pub fn generate(config: &Config, options: &Options, tier_name: &str) -> Option<Generated> {
    let (sources, discovery) = discover::discover(config, &config.tiers, &options.source_dir);
    if discovery.has_errors() {
        return None;
    }
    let tier = sources.iter().find(|t| t.tier == tier_name)?;
    let others: Vec<String> = sources
        .iter()
        .filter(|t| t.tier != tier.tier)
        .flat_map(|t| t.pages.iter().map(|p| p.rel_path.clone()))
        .collect();
    let cache_dir = match &options.mode {
        Mode::Build(b) => Some(b.output.join(&config.build.cache_dir).into_std_path_buf()),
        Mode::Check => None,
    };
    let prepared = prepare_tier(config, tier, &others, options, cache_dir.as_deref());
    Some(Generated {
        files: prepared.files,
        maps: prepared.maps,
        diag: prepared.diag,
    })
}

struct Prepared {
    pages: Vec<Page>,
    diag: DiagnosticCollector,
    unresolved: Vec<UnresolvedLink>,
    /// Virtual files: template, main, pages, images, diagrams.
    files: BTreeMap<String, Vec<u8>>,
    maps: SourceMaps,
    image_count: usize,
    diagram_count: usize,
}

/// Image handling shared by heroes, covers and body images.
struct ImageStore {
    settings: ImageSettings,
    cache_dir: Option<PathBuf>,
    check_only: bool,
    source_dir: Utf8PathBuf,
    files: BTreeMap<String, Arc<Vec<u8>>>,
}

impl ImageStore {
    fn register(&mut self, image: &ProcessedImage) -> String {
        self.files
            .entry(image.vpath.clone())
            .or_insert_with(|| image.bytes.clone());
        image.vpath.clone()
    }

    /// Candidate locations: next to the page first, then relative to the source folder.
    fn resolve(&self, page_abs: &Utf8Path, rel: &str) -> Option<Utf8PathBuf> {
        let rel = rel.replace('\\', "/");
        let next_to_page = page_abs.parent().map(|d| d.join(&rel));
        let from_root = self.source_dir.join(&rel);
        next_to_page
            .into_iter()
            .chain([from_root])
            .find(|p| p.is_file())
    }
}

/// The emitter's view of the asset pipeline for one tier.
struct TierAssets<'a> {
    store: &'a mut ImageStore,
    diagrams: &'a mut DiagramCollector,
    diagrams_enabled: bool,
    abs_paths: &'a BTreeMap<String, Utf8PathBuf>,
    diag: DiagnosticCollector,
    body_images: usize,
}

impl AssetSink for TierAssets<'_> {
    fn body_image(&mut self, page: &Page, dest: &str, line: usize) -> Option<String> {
        let warn = |diag: &mut DiagnosticCollector, why: String| {
            diag.add_warning(
                Warning::new(WarningCode::W003, why).with_location(page.rel_path.clone(), line),
            );
        };
        if crate::linkmap::is_external(dest) {
            warn(
                &mut self.diag,
                format!("image `{dest}` is remote; only local images are supported"),
            );
            return None;
        }
        let abs = self.abs_paths.get(&page.rel_path)?;
        let Some(path) = self.store.resolve(abs, &percent_decode(dest)) else {
            warn(&mut self.diag, format!("image `{dest}` not found"));
            return None;
        };
        self.body_images += 1;
        if self.store.check_only {
            return Some(format!("images/{}", path.file_name().unwrap_or("image")));
        }
        match images::process_file(
            path.as_std_path(),
            &self.store.settings,
            self.store.cache_dir.as_deref(),
        ) {
            Ok(img) => Some(self.store.register(&img)),
            Err(e) => {
                warn(
                    &mut self.diag,
                    format!("image `{dest}` cannot be used: {e}"),
                );
                None
            }
        }
    }

    fn diagram(&mut self, page: &Page, source: &str, line: usize) -> Option<String> {
        if !self.diagrams_enabled {
            self.diag.add_warning(
                Warning::new(
                    WarningCode::W010,
                    "diagrams are off; the Mermaid source is shown as a code block",
                )
                .with_location(page.rel_path.clone(), line),
            );
            return None;
        }
        Some(self.diagrams.request(&page.rel_path, line, source))
    }
}

fn percent_decode(s: &str) -> String {
    percent_encoding::percent_decode_str(s)
        .decode_utf8_lossy()
        .into_owned()
}

fn prepare_tier(
    config: &Config,
    tier: &TierSources,
    other_tier_pages: &[String],
    options: &Options,
    cache_dir: Option<&Path>,
) -> Prepared {
    let build_mode = matches!(options.mode, Mode::Build(_));
    let mut diag = DiagnosticCollector::default();
    let mut files: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let mut maps = SourceMaps::new();

    // 1. Pages with unique keys and heading ids.
    let keys = assign_keys(&tier.pages);
    let mut pages: Vec<Page> = Vec::new();
    let mut abs_paths: BTreeMap<String, Utf8PathBuf> = BTreeMap::new();
    for (source, key) in tier.pages.iter().zip(keys) {
        match Page::build(SourcePage { ..source.clone() }, key) {
            Ok(p) => {
                abs_paths.insert(p.rel_path.clone(), source.abs_path.clone());
                pages.push(p);
            }
            Err(errors) => errors.into_iter().for_each(|e| diag.add_error(e)),
        }
    }

    let settings = ImageSettings {
        page_width_mm: config.page.width_mm,
        dpi: config.images.dpi,
        jpeg_quality: config.images.jpeg_quality,
    };
    let mut store = ImageStore {
        settings: settings.clone(),
        cache_dir: cache_dir.map(Path::to_path_buf),
        check_only: !build_mode,
        source_dir: options.source_dir.clone(),
        files: BTreeMap::new(),
    };

    for page in &pages {
        check_glyph_coverage(page, &mut diag);
    }

    // 2. Emit Markdown (links, tables, code, diagram requests).
    let mut links = LinkMap::new(
        &tier.tier,
        &pages,
        other_tier_pages.iter().cloned(),
        options.source_dir.as_std_path(),
    );
    let mut collector = DiagramCollector::default();
    let diagrams_enabled = match &options.mode {
        Mode::Build(b) => b.diagrams != DiagramBackendConfig::Off,
        Mode::Check => true,
    };
    let mut markups = Vec::with_capacity(pages.len());
    let mut body_images = 0;
    for page in &pages {
        let mut assets = TierAssets {
            store: &mut store,
            diagrams: &mut collector,
            diagrams_enabled,
            abs_paths: &abs_paths,
            diag: DiagnosticCollector::default(),
            body_images: 0,
        };
        let emitted = emit_page(page, &mut links, &mut assets, config.text_width_pt());
        diag.extend(assets.diag);
        body_images += assets.body_images;
        diag.extend(emitted.diag);
        markups.push(emitted.markup);
    }
    for w in std::mem::take(&mut links.warnings) {
        diag.add_warning(w);
    }

    // 3. Hero, cover and contents images.
    let heroes = resolve_heroes(&pages, &abs_paths, &store, &mut diag);
    let (hero_vpaths, cover_vpath, contents_vpath, hero_count) =
        prepare_images(config, &tier.tier, &heroes, &mut store, &mut diag);

    // 4. Diagrams.
    let mut diagram_count = 0;
    if let Mode::Build(b) = &options.mode {
        if diagrams_enabled && !collector.sources.is_empty() {
            match diagram_renderer(config, b.diagrams) {
                Ok(renderer) => {
                    let theme = match config.theme_path() {
                        Some(path) => Theme::from_file(path.as_std_path()),
                        None => Ok(Theme::embedded()),
                    };
                    match theme {
                        Ok(theme) => {
                            let rendered = diagrams::render_all(
                                &collector,
                                renderer.as_ref(),
                                &theme,
                                cache_dir,
                            );
                            diagram_count = rendered.rendered;
                            for (vpath, svg) in rendered.svgs {
                                files.insert(vpath, svg.into_bytes());
                            }
                            rendered.errors.into_iter().for_each(|e| diag.add_error(e));
                            rendered
                                .warnings
                                .into_iter()
                                .for_each(|w| diag.add_warning(w));
                        }
                        Err(message) => diag.add_error(HideoError::new(ErrorCode::E007, message)),
                    }
                }
                Err(message) => diag.add_error(HideoError::new(ErrorCode::E007, message)),
            }
        }
    } else {
        diagram_count = collector.sources.len();
    }

    // 5. Assemble the virtual file tree.
    let entries: Vec<PageEntry> = pages
        .iter()
        .enumerate()
        .map(|(i, page)| PageEntry {
            page,
            hero: hero_vpaths[i].clone(),
            number: i + 1,
        })
        .collect();
    if build_mode {
        files.insert(
            "template.typ".into(),
            template::template_typ(config).into_bytes(),
        );
        files.insert("arrow.svg".into(), template::ARROW_SVG.to_vec());
        files.insert(
            "main.typ".into(),
            template::main_typ(
                config,
                &tier.tier,
                &entries,
                &cover_vpath,
                contents_vpath.as_deref(),
            )
            .into_bytes(),
        );
        for (entry, markup) in entries.iter().zip(&markups) {
            let (text, map) = template::page_typ(config, entry, markup);
            let vpath = template::page_vpath(entry.number);
            files.insert(vpath.clone(), text.into_bytes());
            maps.insert(vpath, map);
        }
        for (vpath, bytes) in &store.files {
            files.insert(vpath.clone(), bytes.as_ref().clone());
        }
    }

    Prepared {
        pages,
        diag,
        unresolved: links.unresolved,
        files,
        maps,
        image_count: hero_count + body_images,
        diagram_count,
    }
}

/// W011: characters the embedded fonts cannot display. Without this, PDF/UA-1 validation turns
/// them into errors at the very end of a build (and without PDF/UA they print as boxes).
fn check_glyph_coverage(page: &Page, diag: &mut DiagnosticCollector) {
    let describe = |chars: &[char]| {
        let shown: String = chars.iter().take(8).collect();
        let more = if chars.len() > 8 { ", ..." } else { "" };
        format!("characters not covered by the embedded fonts: {shown}{more}")
    };
    let fm = &page.frontmatter;
    let head: String = [&fm.title, &fm.subtitle, &fm.author]
        .into_iter()
        .flatten()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(" ");
    let missing = world::uncovered_chars(&head);
    if !missing.is_empty() {
        diag.add_warning(
            Warning::new(WarningCode::W011, describe(&missing))
                .with_location(page.rel_path.clone(), 1),
        );
    }
    for (index, line) in page.body.lines().enumerate() {
        let missing = world::uncovered_chars(line);
        if !missing.is_empty() {
            diag.add_warning(
                Warning::new(WarningCode::W011, describe(&missing))
                    .with_location(page.rel_path.clone(), fm.body_line_offset + index + 1),
            );
        }
    }
}

fn diagram_renderer(
    config: &Config,
    backend: DiagramBackendConfig,
) -> Result<Box<dyn DiagramRenderer>, String> {
    match backend {
        DiagramBackendConfig::External => Ok(Box::new(ExternalRenderer::new(
            &config.diagrams.command,
            &config.diagrams.args,
            config.diagrams.timeout_s,
        ))),
        #[cfg(feature = "native-diagrams")]
        DiagramBackendConfig::Native => {
            let native: Box<dyn DiagramRenderer> =
                Box::new(crate::diagrams::native::NativeRenderer::new());
            if config.diagrams.native_only {
                return Ok(native);
            }
            Ok(Box::new(diagrams::RoutingRenderer {
                primary: native,
                fallback: Box::new(ExternalRenderer::new(
                    &config.diagrams.command,
                    &config.diagrams.args,
                    config.diagrams.timeout_s,
                )),
                kinds: diagrams::NATIVE_KINDS
                    .iter()
                    .map(|k| k.to_string())
                    .collect(),
            }))
        }
        #[cfg(not(feature = "native-diagrams"))]
        DiagramBackendConfig::Native => Err(
            "the native diagram backend needs hideo to be built with `--features native-diagrams`"
                .to_string(),
        ),
        DiagramBackendConfig::Off => Err("diagrams are off".to_string()),
    }
}

/// Where each page's hero image is, if anywhere.
fn resolve_heroes(
    pages: &[Page],
    abs_paths: &BTreeMap<String, Utf8PathBuf>,
    store: &ImageStore,
    diag: &mut DiagnosticCollector,
) -> Vec<Option<Utf8PathBuf>> {
    pages
        .iter()
        .map(|p| {
            let hero = p.frontmatter.hero.as_deref().unwrap_or("");
            let found = abs_paths
                .get(&p.rel_path)
                .and_then(|abs| store.resolve(abs, hero));
            if found.is_none() {
                diag.add_warning(
                    Warning::new(
                        WarningCode::W003,
                        format!("hero image `{hero}` not found; a placeholder is used"),
                    )
                    .with_page(p.rel_path.clone()),
                );
            }
            found
        })
        .collect()
}

/// Process heroes, the cover and the contents image. Returns per-page hero vpaths, the cover
/// vpath, the contents hero vpath and the number of distinct source images.
fn prepare_images(
    config: &Config,
    tier: &str,
    heroes: &[Option<Utf8PathBuf>],
    store: &mut ImageStore,
    diag: &mut DiagnosticCollector,
) -> (Vec<Option<String>>, String, Option<String>, usize) {
    // Unique source files in a stable order, processed in parallel; results are looked up by
    // path so the order of work never leaks into the output (F19).
    let mut unique: BTreeSet<Utf8PathBuf> = heroes.iter().flatten().cloned().collect();
    let cover_path = config.cover_path(tier);
    let cover_exists = cover_path.is_file();
    if cover_exists {
        unique.insert(cover_path.clone());
    }
    let contents_path = config.contents_hero_path();
    let contents_exists = contents_path.as_ref().is_some_and(|p| p.is_file());
    if let (Some(p), true) = (&contents_path, contents_exists) {
        unique.insert(p.clone());
    }

    let results: BTreeMap<Utf8PathBuf, Result<ProcessedImage, images::ImageError>> =
        if store.check_only {
            // Check mode: the file must exist and be readable; do not decode.
            unique
                .into_iter()
                .map(|p| {
                    let result = std::fs::File::open(&p)
                        .map(|_| ProcessedImage {
                            vpath: format!("images/{}", p.file_name().unwrap_or("image")),
                            bytes: Arc::new(Vec::new()),
                            width: 0,
                            height: 0,
                        })
                        .map_err(|source| images::ImageError::Read {
                            path: p.to_string(),
                            source,
                        });
                    (p, result)
                })
                .collect()
        } else {
            let list: Vec<Utf8PathBuf> = unique.into_iter().collect();
            let settings = store.settings.clone();
            let cache = store.cache_dir.clone();
            list.par_iter()
                .map(|p| {
                    (
                        p.clone(),
                        images::process_file(p.as_std_path(), &settings, cache.as_deref()),
                    )
                })
                .collect::<Vec<_>>()
                .into_iter()
                .collect()
        };
    let distinct = results.len();

    let placeholder = images::placeholder(&store.settings);
    let mut use_placeholder = false;
    let mut hero_vpaths = Vec::with_capacity(heroes.len());
    for hero in heroes {
        let vpath = match hero.as_ref().map(|p| (p, &results[p])) {
            Some((_, Ok(img))) => store.register(img),
            Some((path, Err(e))) => {
                diag.add_warning(
                    Warning::new(
                        WarningCode::W003,
                        format!("hero image `{path}` cannot be used ({e}); a placeholder is used"),
                    )
                    .with_page(tier.to_string()),
                );
                use_placeholder = true;
                placeholder.vpath.clone()
            }
            None => {
                use_placeholder = true;
                placeholder.vpath.clone()
            }
        };
        hero_vpaths.push(Some(vpath));
    }

    // Name images as the author wrote them, not by absolute path.
    let cover_name = config.images.cover_pattern.replace("{tier}", tier);
    let contents_name = config.images.contents_hero.clone().unwrap_or_default();
    let cover_vpath = match results.get(&cover_path) {
        Some(Ok(img)) => store.register(img),
        other => {
            let why = match other {
                Some(Err(e)) => format!("cover image `{cover_name}` cannot be used ({e})"),
                _ => format!("cover image `{cover_name}` not found"),
            };
            diag.add_warning(
                Warning::new(WarningCode::W003, format!("{why}; a placeholder is used"))
                    .with_page(tier.to_string()),
            );
            use_placeholder = true;
            placeholder.vpath.clone()
        }
    };

    let contents_vpath = match (&contents_path, contents_exists) {
        (None, _) => None,
        (Some(p), true) => {
            match &results[p] {
                Ok(img) => Some(store.register(img)),
                Err(e) => {
                    diag.add_warning(
                    Warning::new(
                        WarningCode::W003,
                        format!("contents image `{contents_name}` cannot be used ({e}); it is left out"),
                    )
                    .with_page(tier.to_string()),
                );
                    None
                }
            }
        }
        (Some(_), false) => {
            diag.add_warning(
                Warning::new(
                    WarningCode::W003,
                    format!("contents image `{contents_name}` not found; it is left out"),
                )
                .with_page(tier.to_string()),
            );
            None
        }
    };

    if use_placeholder {
        store.register(&placeholder);
    }
    (hero_vpaths, cover_vpath, contents_vpath, distinct)
}

fn compile_tier(
    config: &Config,
    build: &BuildOptions,
    tier: &str,
    prepared: &Prepared,
) -> Result<(Utf8PathBuf, usize, Vec<Warning>), Vec<HideoError>> {
    if let Some(dir) = &build.keep_typst {
        keep_typst(&dir.join(tier), &prepared.files).map_err(|e| {
            vec![HideoError::new(
                ErrorCode::E005,
                format!("cannot write generated Typst to {dir}: {e}"),
            )]
        })?;
    }
    let world = crate::world::HideoWorld::new(&prepared.files, build.today)
        .map_err(|e| vec![HideoError::new(ErrorCode::E005, e)])?;
    let settings = PdfSettings {
        tagged: config.accessibility.tagged,
        pdf_ua: config.accessibility.pdf_ua,
        timestamp: build.today,
        warnings_as_errors: config.diagnostics.warnings_as_errors,
    };
    let compiled = compile::compile_pdf(&world, &prepared.maps, &settings)?;
    let path = build.output.join(format!("{tier}.pdf"));
    compile::write_atomically(path.as_std_path(), &compiled.bytes).map_err(|e| {
        vec![HideoError::new(
            ErrorCode::E005,
            format!("cannot write {path}: {e}"),
        )]
    })?;
    Ok((path, compiled.pages, compiled.warnings))
}

/// `--keep-typst`: write the generated sources (text files only) for debugging.
fn keep_typst(dir: &Utf8Path, files: &BTreeMap<String, Vec<u8>>) -> std::io::Result<()> {
    for (vpath, bytes) in files {
        if vpath.ends_with(".typ") || vpath.ends_with(".svg") {
            let target = dir.join(vpath);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(target, bytes)?;
        }
    }
    Ok(())
}
