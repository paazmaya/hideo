//! Visual regression (plan 8.5): render pages with `typst-render` and compare them with the
//! approved PNGs in `tests/visual/`.
//!
//! Review a change by looking at the new PNGs, then refresh the approved ones with
//! `HIDEO_UPDATE_VISUALS=1 cargo test --test integration visual`. A run passes when at most
//! 0.1 % of the pixels differ by more than a small per-channel threshold.

use std::path::PathBuf;

use camino::Utf8PathBuf;
use hideo::build::{BuildOptions, Mode, Options, generate};
use hideo::compile::{compile_document, source_date};
use hideo::config::{Config, DiagramBackendConfig};
use hideo::world::HideoWorld;

use crate::common::fixture;

const SCALE: f32 = 0.6;
const MAX_DIFFERENT_FRACTION: f64 = 0.001;
const CHANNEL_THRESHOLD: i32 = 24;

fn snapshot_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/visual")
}

/// Render the 1-based `pages` of `tier` of a fixture to PNG bytes.
fn render(fixture_name: &str, tier: &str, pages: &[usize]) -> Vec<(usize, Vec<u8>)> {
    let root = Utf8PathBuf::from_path_buf(fixture(fixture_name)).unwrap();
    let config = Config::load(&root.join("hideo.toml")).unwrap();
    let out = tempfile::tempdir().unwrap();
    let today = source_date(Some("1700000000")).unwrap();
    let options = Options {
        source_dir: root,
        tiers: vec![tier.to_string()],
        verbose: false,
        mode: Mode::Build(BuildOptions {
            output: Utf8PathBuf::from_path_buf(out.path().to_path_buf()).unwrap(),
            keep_typst: None,
            diagrams: DiagramBackendConfig::Off,
            today,
        }),
    };
    let generated = generate(&config, &options, tier).expect("tier generates");
    assert!(!generated.diag.has_errors(), "{:?}", generated.diag.errors);
    let world = HideoWorld::new(&generated.files, today).unwrap();
    let document = compile_document(&world)
        .output
        .unwrap_or_else(|e| panic!("{e:?}"));
    pages
        .iter()
        .map(|&n| {
            let page = &document.pages()[n - 1];
            let options = typst_render::RenderOptions {
                pixel_per_pt: typst::utils::Scalar::new(SCALE as f64),
                render_bleed: false,
            };
            let pixmap = typst_render::render(page, &options);
            (n, pixmap.encode_png().expect("PNG encoding"))
        })
        .collect()
}

/// Fraction of pixels that differ by more than the threshold in any channel.
fn different_fraction(a: &[u8], b: &[u8]) -> f64 {
    let a = image::load_from_memory(a).unwrap().to_rgba8();
    let b = image::load_from_memory(b).unwrap().to_rgba8();
    if a.dimensions() != b.dimensions() {
        return 1.0;
    }
    let differing = a
        .pixels()
        .zip(b.pixels())
        .filter(|(p, q)| {
            p.0.iter()
                .zip(q.0.iter())
                .any(|(x, y)| (*x as i32 - *y as i32).abs() > CHANNEL_THRESHOLD)
        })
        .count();
    differing as f64 / (a.width() as f64 * a.height() as f64)
}

fn check(fixture_name: &str, tier: &str, pages: &[usize]) {
    let update = std::env::var_os("HIDEO_UPDATE_VISUALS").is_some();
    for (n, png) in render(fixture_name, tier, pages) {
        let path = snapshot_dir().join(format!("{fixture_name}-{tier}-p{n}.png"));
        if update {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, &png).unwrap();
            continue;
        }
        let approved = std::fs::read(&path).unwrap_or_else(|_| {
            panic!(
                "no approved snapshot {}; run with HIDEO_UPDATE_VISUALS=1 and review it",
                path.display()
            )
        });
        let fraction = different_fraction(&approved, &png);
        if fraction > MAX_DIFFERENT_FRACTION {
            // Leave the new rendering next to the approved one for review.
            let new = path.with_extension("new.png");
            std::fs::write(&new, &png).unwrap();
            panic!(
                "{} differs in {:.3} % of its pixels (limit {:.3} %); see {}",
                path.display(),
                fraction * 100.0,
                MAX_DIFFERENT_FRACTION * 100.0,
                new.display()
            );
        }
    }
}

#[test]
fn visual_minimal_book() {
    // cover, contents, first page, second page
    check("minimal", "required", &[1, 2, 3, 4]);
}

#[test]
fn visual_full_book_content_pages() {
    // first content page (hero, lead-in, lists), headings/quote page, code page, table page
    check("full", "required", &[3, 5, 6, 7]);
}

#[test]
fn the_comparison_notices_a_change() {
    let png = render("minimal", "required", &[3]).remove(0).1;
    assert_eq!(different_fraction(&png, &png), 0.0);
    let other = render("minimal", "required", &[4]).remove(0).1;
    assert!(
        different_fraction(&png, &other) > MAX_DIFFERENT_FRACTION,
        "different pages must not compare equal"
    );
}
