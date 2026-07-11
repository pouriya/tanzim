//! `tanzim` command-line interface.
//!
//! A thin binary over the [`tanzim`] facade crate. Each subcommand lives in its own module
//! with its own settings struct, referenced from the top-level [`Cli`] setting here.

mod source;
mod load;

use clap::{Args, Parser, Subcommand};
use tracing::level_filters::LevelFilter;

/// Top-level command-line settings.
#[derive(Parser)]
#[command(name = "tanzim", version, about)]
struct Cli {
    #[command(flatten)]
    verbosity: Verbosity,
    #[command(subcommand)]
    command: Command,
}

/// Mutually exclusive global logging flags. At most one may be set; the resolved level
/// controls the tracing subscriber installed in [`main`].
#[derive(Args)]
#[group(multiple = false)]
struct Verbosity {
    /// Silence all logging.
    #[arg(long, global = true)]
    quiet: bool,
    /// Enable debug-level logging.
    #[arg(long, global = true)]
    debug: bool,
    /// Enable trace-level logging.
    #[arg(long, global = true)]
    trace: bool,
}

impl Verbosity {
    fn level(&self) -> LevelFilter {
        if self.quiet {
            LevelFilter::OFF
        } else if self.trace {
            LevelFilter::TRACE
        } else if self.debug {
            LevelFilter::DEBUG
        } else {
            LevelFilter::INFO
        }
    }
}

/// The available subcommands.
#[derive(Subcommand)]
enum Command {
    /// Parse one or more tanzim-source strings.
    Source(source::SourceArgs),
    /// Load one or more raw configuration payloads from source(s).
    Load(load::LoadArgs),
}

fn main() -> std::process::ExitCode {
    let cli = Cli::parse();

    tracing_subscriber::fmt()
        .with_max_level(cli.verbosity.level())
        .with_target(false)
        .without_time()
        .init();

    match cli.command {
        Command::Source(args) => source::run(args),
        Command::Load(args) => load::run(args),
    }
}
