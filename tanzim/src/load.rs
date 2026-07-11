//! The `load` subcommand: fetch raw configuration payloads from source(s).

use std::collections::HashMap;
use std::process::ExitCode;
use std::time::Duration;

use clap::Args;

use tanzim::loader::Load;
use tanzim::loader::env::Env;
use tanzim::loader::file::File;
use tanzim::loader::http::{Http, HttpFetchFn, Url};
use tanzim::loader::Payload;
use tanzim::source::Source;

fn http_load() -> HttpFetchFn {
    Box::new(
        |source: Source, url: &Url, headers: &HashMap<String, String>, timeout: Duration, insecure: bool| {
            let agent: ureq::Agent = ureq::Agent::config_builder()
                .timeout_global(Some(timeout))
                .tls_config(
                    ureq::tls::TlsConfig::builder()
                        .disable_verification(insecure)
                        .build(),
                )
                .build()
                .into();

            let mut request = agent.get(url.as_str());
            for (key, value) in headers {
                request = request.header(key, value);
            }

            let mut response = match request.call() {
                Ok(response) => response,
                Err(e) => return Err(format!("Failed to fetch HTTP: {e}")),
            };

            let status = response.status();
            if !status.is_success() {
                return Err(format!("HTTP {}", status.as_u16()));
            }

            let maybe_format = if let Some(content_type) = response.headers().get("content-type") {
                match content_type.to_str() {
                    Ok(content_type) => {
                        let mime = match content_type.split(';').next() {
                            Some(mime) => mime.trim(),
                            None => "",
                        };
                        if let Some(format) = mime.strip_prefix("application/") {
                            let format = format.trim();
                            if format.is_empty() {
                                None
                            } else {
                                Some(format.to_lowercase())
                            }
                        } else {
                            None
                        }
                    }
                    Err(_) => None,
                }
            } else {
                None
            };

            let maybe_name = match url.path_segments().and_then(|mut segments| segments.next_back()) {
                Some(segment) if segment.contains('.') => match segment.rsplit_once('.') {
                    Some((stem, _)) if !stem.is_empty() => Some(stem.to_lowercase()),
                    _ => None,
                },
                _ => None,
            };

            let content = match response.body_mut().read_to_vec() {
                Ok(content) => content,
                Err(e) => return Err(format!("HTTP response body read failed: {e}")),
            };

            Ok(vec![Payload {
                source,
                maybe_name,
                maybe_format,
                content,
            }])
        },
    )
}

fn loaders() -> Vec<Box<dyn Load>> {
    vec![
        Box::new(Env::new()),
        Box::new(File::new()),
        Box::new(Http::new(http_load())),
    ]
}

/// Settings for the `load` subcommand.
#[derive(Args)]
pub struct LoadArgs {
    /// One or more sources in tanzim-source format.
    /// Examples:
    ///  env(prefix=APP_) |
    ///  file(on_error=(load=skip)):/etc/app | 
    ///  http(timeout=10s):https://raw.githubusercontent.com/pouriya/tanzim/refs/heads/master/crates/tanzim/tests/etc/baz.toml
    #[arg(required = true)]
    sources: Vec<String>,
}

/// Load every source and print each payload. Returns [`ExitCode::FAILURE`] on any error.
pub fn run(args: LoadArgs) -> ExitCode {
    let loader_list = loaders();
    let mut failed = false;

    for input in &args.sources {
        let source = match Source::try_from(input.as_str()) {
            Ok(source) => source,
            Err(e) => {
                eprintln!("error: cannot parse source {input:?}:\n{e:#}");
                failed = true;
                continue;
            }
        };

        let mut loaded = false;
        for loader in &loader_list {
            if !loader
                .supported_source_list()
                .contains(&source.source().to_string())
            {
                continue;
            }

            let payload_list = match loader.load(source.clone()) {
                Ok(payload_list) => payload_list,
                Err(e) => {
                    eprintln!("error: {e:#}");
                    failed = true;
                    loaded = true;
                    break;
                }
            };

            for payload in payload_list {
                println!("{}:", payload.source);
                println!("\tName: {}", payload.maybe_name.as_deref().unwrap_or("<none>"));
                println!("\tFormat: {}", payload.maybe_format.as_deref().unwrap_or("<none>"));
                println!("\tLength: {}", payload.content.len());
                println!("\tContent:");
                for (mut i, line) in String::from_utf8_lossy(&payload.content)
                    .lines()
                    .enumerate()
                {
                    i += 1;
                    println!("\t\t{i:>3}| {line}");
                }
                println!();
            }

            loaded = true;
            break;
        }

        if !loaded {
            eprintln!("error: no loader for {source}");
            failed = true;
        }
    }

    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
