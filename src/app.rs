//! Command line behaviour: `build`, `check`, `list` and exit codes (plan 7.3).
//!
//! Exit codes: `0` success; `1` build or content error; `2` usage or configuration error.

use std::io::Write;
use std::process::ExitCode;

use camino::Utf8PathBuf;
use serde::Serialize;

use crate::build::{self, BuildOptions, Mode, Options};
use crate::cli::{BuildArgs, CheckArgs, Cli, Command, DiagramBackend, ListArgs, ReportFormat};
use crate::compile;
use crate::config::{Config, DiagramBackendConfig};
use crate::discover;
use crate::error::ConfigError;
use crate::report::{self, Report, Stats};

pub const EXIT_OK: u8 = 0;
pub const EXIT_CONTENT: u8 = 1;
pub const EXIT_USAGE: u8 = 2;

pub fn run(cli: Cli) -> ExitCode {
    let result = match cli.command {
        Command::Build(args) => build_command(args),
        Command::Check(args) => check_command(args),
        Command::List(args) => list_command(args),
    };
    match result {
        Ok(code) => ExitCode::from(code),
        Err(e) => {
            eprintln!("error: {}", e.0);
            ExitCode::from(EXIT_USAGE)
        }
    }
}

fn load_config(input: &Utf8PathBuf, explicit: &Option<Utf8PathBuf>) -> Result<Config, ConfigError> {
    let path = explicit.clone().unwrap_or_else(|| input.join("hideo.toml"));
    Config::load(&path)
}

fn select_tiers(config: &Config, requested: &[String]) -> Result<Vec<String>, ConfigError> {
    if requested.is_empty() {
        return Ok(config.tiers.clone());
    }
    for t in requested {
        if !config.tiers.contains(t) {
            return Err(ConfigError(format!(
                "unknown tier `{t}`; the configuration defines: {}",
                config.tiers.join(", ")
            )));
        }
    }
    // Keep config order, drop duplicates.
    Ok(config
        .tiers
        .iter()
        .filter(|t| requested.contains(t))
        .cloned()
        .collect())
}

fn thread_pool(jobs: Option<usize>) -> Result<rayon::ThreadPool, ConfigError> {
    let mut builder = rayon::ThreadPoolBuilder::new();
    if let Some(n) = jobs {
        if n == 0 {
            return Err(ConfigError("--jobs must be at least 1".to_string()));
        }
        builder = builder.num_threads(n);
    }
    builder
        .build()
        .map_err(|e| ConfigError(format!("cannot start worker threads: {e}")))
}

fn backend_from_cli(config: &Config, flag: Option<DiagramBackend>) -> DiagramBackendConfig {
    match flag {
        Some(DiagramBackend::Native) => DiagramBackendConfig::Native,
        Some(DiagramBackend::External) => DiagramBackendConfig::External,
        Some(DiagramBackend::Off) => DiagramBackendConfig::Off,
        None => config.diagrams.backend,
    }
}

fn build_command(args: BuildArgs) -> Result<u8, ConfigError> {
    let config = load_config(&args.input, &args.config)?;
    let tiers = select_tiers(&config, &args.tiers)?;
    let today = compile::source_date(std::env::var("SOURCE_DATE_EPOCH").ok().as_deref())?;
    let backend = backend_from_cli(&config, args.diagrams);
    if backend == DiagramBackendConfig::Native && !cfg!(feature = "native-diagrams") {
        return Err(ConfigError(
            "the native diagram backend needs hideo to be built with `--features native-diagrams`"
                .to_string(),
        ));
    }
    if !config.accessibility.pdf_ua && !args.quiet {
        eprintln!(
            "warning: accessibility.pdf_ua is off; the PDF is not validated against PDF/UA-1"
        );
    }
    let pool = thread_pool(args.jobs)?;

    let options = Options {
        source_dir: args.input.clone(),
        tiers,
        verbose: args.verbose,
        mode: Mode::Build(BuildOptions {
            output: args.output.clone(),
            keep_typst: args.keep_typst.clone(),
            diagrams: backend,
            today,
        }),
    };
    let started = std::time::Instant::now();
    let outcome = pool.install(|| build::run(&config, &options));
    let report = Report::new(
        &outcome.diagnostics,
        &outcome.unresolved,
        outcome.stats.clone(),
    );
    let _ = report::write_text(
        &mut std::io::stderr(),
        &report,
        &outcome.unresolved,
        args.quiet,
    );

    for tier in &outcome.tiers {
        if let (Some(pdf), Some(pages)) = (&tier.pdf, tier.pdf_pages) {
            println!("{pdf} ({pages} pages)");
        }
    }
    if args.verbose {
        eprintln!(
            "built {} PDFs from {} pages in {:.1}s ({} images, {} diagrams)",
            outcome.stats.pdfs,
            outcome.stats.pages,
            started.elapsed().as_secs_f64(),
            outcome.stats.images,
            outcome.stats.diagrams
        );
    }

    let denied = args.deny_unresolved && !outcome.unresolved.is_empty();
    if denied && !args.quiet {
        eprintln!(
            "error: {} unresolved links and --deny-unresolved is set",
            outcome.unresolved.len()
        );
    }
    Ok(if outcome.diagnostics.has_errors() || denied {
        EXIT_CONTENT
    } else {
        EXIT_OK
    })
}

fn check_command(args: CheckArgs) -> Result<u8, ConfigError> {
    let config = load_config(&args.input, &args.config)?;
    let tiers = select_tiers(&config, &args.tiers)?;
    let options = Options {
        source_dir: args.input.clone(),
        tiers,
        mode: Mode::Check,
        verbose: false,
    };
    let outcome = build::run(&config, &options);
    let report = Report::new(
        &outcome.diagnostics,
        &outcome.unresolved,
        outcome.stats.clone(),
    );
    match args.format {
        ReportFormat::Json => println!("{}", report.to_json()),
        ReportFormat::Text => {
            let mut stdout = std::io::stdout();
            let _ = report::write_text(&mut stdout, &report, &outcome.unresolved, false);
            let _ = writeln!(
                stdout,
                "checked {} tiers, {} pages: {} errors, {} warnings",
                report.stats.tiers,
                report.stats.pages,
                report.errors.len(),
                report.warnings.len()
            );
        }
    }
    Ok(if outcome.diagnostics.has_errors() {
        EXIT_CONTENT
    } else {
        EXIT_OK
    })
}

#[derive(Serialize)]
struct ListItem<'a> {
    tier: &'a str,
    path: &'a str,
    title: &'a str,
}

fn list_command(args: ListArgs) -> Result<u8, ConfigError> {
    let config = load_config(&args.input, &args.config)?;
    let tiers = select_tiers(&config, &args.tiers)?;
    let (sources, diagnostics) = discover::discover(&config, &tiers, &args.input);
    let report = Report::new(&diagnostics, &[], Stats::default());
    if diagnostics.has_errors() {
        let _ = report::write_text(&mut std::io::stderr(), &report, &[], true);
        return Ok(EXIT_CONTENT);
    }
    let items: Vec<ListItem> = sources
        .iter()
        .flat_map(|t| {
            t.pages.iter().map(move |p| ListItem {
                tier: &t.tier,
                path: &p.rel_path,
                title: p.frontmatter.title.as_deref().unwrap_or(""),
            })
        })
        .collect();
    match args.format {
        ReportFormat::Json => println!(
            "{}",
            serde_json::to_string_pretty(&items).expect("list serialises")
        ),
        ReportFormat::Text => {
            for i in &items {
                println!("{}\t{}\t{}", i.tier, i.path, i.title);
            }
        }
    }
    let _ = report::write_text(&mut std::io::stderr(), &report, &[], false);
    Ok(EXIT_OK)
}
