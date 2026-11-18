use camino::Utf8PathBuf;
use clap::{Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(
    name = "hideo",
    version,
    about = "Build PDF books from Markdown",
    long_about = "Build one accessible PDF book per tier folder from Markdown pages and images.

The Typst compiler is embedded; the only optional external tool is a Mermaid renderer for diagrams.",
    after_help = "EXIT CODES:
  0  success
  1  content or build error (the message names the page and line)
  2  usage or configuration error

ENVIRONMENT:
  SOURCE_DATE_EPOCH  seconds since the Unix epoch (UTC) used as the PDF timestamp, for
                     reproducible builds; default 946684800 (2000-01-01T00:00:00Z)"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Build PDF books from Markdown source
    Build(BuildArgs),
    /// Validate source without writing PDF
    Check(CheckArgs),
    /// List pages in PDF order
    List(ListArgs),
}

#[derive(Debug, Clone, clap::Args)]
pub struct BuildArgs {
    /// Tier folders to build (default: all from config)
    #[arg(value_name = "TIER")]
    pub tiers: Vec<String>,

    /// Source folder
    #[arg(short, long, value_name = "DIR", default_value = ".")]
    pub input: Utf8PathBuf,

    /// Output folder
    #[arg(short, long, value_name = "DIR", default_value = "pdf-output")]
    pub output: Utf8PathBuf,

    /// Config file (default: hideo.toml in the source folder)
    #[arg(short, long, value_name = "FILE")]
    pub config: Option<Utf8PathBuf>,

    /// Diagram backend (overrides diagrams.backend from the config)
    #[arg(long, value_name = "BACKEND", value_enum)]
    pub diagrams: Option<DiagramBackend>,

    /// Write the generated Typst source per tier to this directory (for debugging)
    #[arg(long, value_name = "DIR")]
    pub keep_typst: Option<Utf8PathBuf>,

    /// Exit with error if any internal link is unresolved
    #[arg(long)]
    pub deny_unresolved: bool,

    /// Worker threads (default: logical cores)
    #[arg(long, value_name = "N")]
    pub jobs: Option<usize>,

    /// Suppress warnings
    #[arg(short, long)]
    pub quiet: bool,

    /// Verbose logging
    #[arg(short, long)]
    pub verbose: bool,
}

#[derive(Debug, Clone, clap::Args)]
pub struct CheckArgs {
    /// Tier folders to check (default: all from config)
    #[arg(value_name = "TIER")]
    pub tiers: Vec<String>,

    /// Source folder
    #[arg(short, long, value_name = "DIR", default_value = ".")]
    pub input: Utf8PathBuf,

    /// Config file (default: hideo.toml in the source folder)
    #[arg(short, long, value_name = "FILE")]
    pub config: Option<Utf8PathBuf>,

    /// Report format
    #[arg(long, value_name = "FORMAT", value_enum, default_value = "text")]
    pub format: ReportFormat,
}

#[derive(Debug, Clone, clap::Args)]
pub struct ListArgs {
    /// Tier folders to list (default: all from config)
    #[arg(value_name = "TIER")]
    pub tiers: Vec<String>,

    /// Source folder
    #[arg(short, long, value_name = "DIR", default_value = ".")]
    pub input: Utf8PathBuf,

    /// Config file (default: hideo.toml in the source folder)
    #[arg(short, long, value_name = "FILE")]
    pub config: Option<Utf8PathBuf>,

    /// Output format
    #[arg(long, value_name = "FORMAT", value_enum, default_value = "text")]
    pub format: ReportFormat,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum DiagramBackend {
    Native,
    External,
    Off,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ReportFormat {
    Text,
    Json,
}
