//! The `source` subcommand: parse tanzim-source format strings and report the result.

use clap::Args;
use std::process::ExitCode;
use tanzim::source::Source;

/// Settings for the `source` subcommand.
#[derive(Args)]
pub struct SourceArgs {
    /// One or more sources in tanzim-source format, e.g. `env(prefix=APP_)` or
    /// `file(on_error=(load=skip)):/etc/app`.
    #[arg(required = true)]
    sources: Vec<String>,
}

/// Parse every source string, printing the structured breakdown for each. Returns
/// [`ExitCode::FAILURE`] if any source failed to parse.
pub fn run(args: SourceArgs) -> ExitCode {
    let mut failed = false;
    for input in &args.sources {
        tracing::debug!(msg = "Parsing source", source = input.as_str());
        match Source::try_from(input.as_str()) {
            Ok(source) => {
                tracing::info!(
                    msg = "Parsed source",
                    name = source.source(),
                    options = ?source.options(),
                    resource = source.resource(),
                );
            }
            Err(error) => {
                eprintln!("error: cannot parse source {input:?}:\n{error:#}");
                failed = true;
            }
        }
    }

    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
