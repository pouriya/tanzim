//! Basic CLI loader example.
//!
//! Accepts a source kind and optional resource as command-line arguments and loads
//! using the built-in loaders enabled by crate features.
//!
//! Run with (requires `env` and/or `file` features):
//!   cargo run -p tanzim-load --example load_config --features env -- env
//!   cargo run -p tanzim-load --example load_config --features file -- file /etc/myapp
//!   cargo run -p tanzim-load --example load_config --features env,file -- env '' file /etc/myapp

use tanzim_load::{Load, Source};
use tanzim_source::SourceBuilder;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("Usage: load_config <SOURCE> [<RESOURCE>] [<SOURCE> [<RESOURCE>]] ...");
        eprintln!("Examples:");
        eprintln!("  load_config env");
        eprintln!("  load_config file /etc/myapp");
        std::process::exit(1);
    }

    // Build the list of available loaders depending on enabled features.
    let mut loaders: Vec<Box<dyn Load>> = Vec::new();
    #[cfg(feature = "env")]
    loaders.extend([Box::new(tanzim_load::env::Env::new()) as Box<dyn Load>]);
    #[cfg(feature = "file")]
    loaders.extend([Box::new(tanzim_load::file::File::new()) as Box<dyn Load>]);

    if loaders.is_empty() {
        eprintln!("No loaders compiled in. Build with --features env,file");
        std::process::exit(1);
    }

    // Walk args in (source [resource]) pairs.
    let mut index = 0;
    while index < args.len() {
        let source_kind = &args[index];
        index += 1;
        let resource = if index < args.len()
            && !args[index].starts_with(|c: char| c.is_alphabetic() && args[index].len() < 10)
        {
            let r = args[index].clone();
            index += 1;
            r
        } else {
            String::new()
        };

        let mut found: Option<&dyn Load> = None;
        for loader in &loaders {
            let supported = loader.supported_source_list();
            let mut matches = false;
            for s in &supported {
                if s.as_str() == source_kind.as_str() {
                    matches = true;
                    break;
                }
            }
            if matches {
                found = Some(loader.as_ref());
                break;
            }
        }

        let loader = match found {
            Some(l) => l,
            None => {
                eprintln!("no loader for {source_kind:?} (check enabled features)");
                continue;
            }
        };

        let source: Source = match SourceBuilder::new()
            .with_source(source_kind.as_str())
            .with_resource(resource.as_str())
            .build()
        {
            Ok(s) => s,
            Err(e) => {
                eprintln!("invalid source {source_kind:?}: {e}");
                std::process::exit(1);
            }
        };

        let payloads = match loader.load(source) {
            Ok(o) => o,
            Err(e) => {
                eprintln!("source {source_kind:?} resource {resource:?}: load error: {e}");
                std::process::exit(1);
            }
        };

        println!(
            "source={source_kind:?} resource={resource:?}: {} payload(s)",
            payloads.len()
        );
        for (i, payload) in payloads.iter().enumerate() {
            let payload = payload.clone().normalize();
            println!(
                "  [{i}] source_resource={:?} name={:?} format={:?} bytes={}",
                payload.source.resource(),
                payload.name,
                payload.format,
                payload.content.len()
            );
        }
    }
}
