use clap::{Parser, Subcommand, ValueEnum};
use camino::Utf8PathBuf;

#[derive(Debug, Parser)]
#[command(name = "hideo", version, about = "Build PDF books from Markdown")]
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
    #[arg(short, long, default_value = ".")]
    pub input: Utf8PathBuf,

    /// Output folder
    #[arg(short, long, default_value = "pdf-output")]
    pub output: Utf8PathBuf,

    /// Config file
    #[arg(short, long)]
    pub config: Option<Utf8PathBuf>,

    /// Diagram backend
    #[arg(long, value_enum, default_value = "external")]
    pub diagrams: DiagramBackend,

    /// Write generated Typst source to directory
    #[arg(long)]
    pub keep_typst: Option<Utf8PathBuf>,

    /// Exit with error if any internal link is unresolved
    #[arg(long)]
    pub deny_unresolved: bool,

    /// Worker threads
    #[arg(long)]
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
    #[arg(short, long, default_value = ".")]
    pub input: Utf8PathBuf,

    /// Config file
    #[arg(short, long)]
    pub config: Option<Utf8PathBuf>,

    /// Report format
    #[arg(long, value_enum, default_value = "text")]
    pub format: ReportFormat,
}

#[derive(Debug, Clone, clap::Args)]
pub struct ListArgs {
    /// Tier folders to list (default: all from config)
    #[arg(value_name = "TIER")]
    pub tiers: Vec<String>,

    /// Source folder
    #[arg(short, long, default_value = ".")]
    pub input: Utf8PathBuf,

    /// Config file
    #[arg(short, long)]
    pub config: Option<Utf8PathBuf>,

    /// Output format
    #[arg(long, value_enum, default_value = "text")]
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