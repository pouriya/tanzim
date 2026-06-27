use std::path::PathBuf;
use tanzim::{
    ConfigBuilder,
    loader::{env::Env, file::File},
    merge::DeepMerge,
    parser::{Env as EnvParser, Json, Toml, Yaml},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (trace, source_args) = parse_args()?;

    cfg_if::cfg_if! {
        if #[cfg(feature = "tracing")] {
            tracing_subscriber::fmt()
                .pretty()
                .with_max_level(if trace { tracing::Level::TRACE } else { tracing::Level::INFO })
                .with_line_number(false)
                .with_file(false)
                .without_time()
                .init();
        } else if #[cfg(feature = "logging")] {
            let _ = trace;
            env_logger::builder()
                .filter_level(if trace { log::LevelFilter::Trace } else { log::LevelFilter::Info })
                .format_timestamp(None)
                .init();
        } else {
            let _ = trace;
        }
    }

    let mut builder = ConfigBuilder::new()
        .with_loader(Env::new())
        .with_loader(File::new())
        .with_parser(EnvParser::new())
        .with_parser(Json::new())
        .with_parser(Yaml::new())
        .with_parser(Toml::new())
        .with_merger(DeepMerge);

    for source_str in &source_args {
        builder = builder.with_source(source_str.as_str())?;
    }

    let merged = builder.build().run()?;

    println!("Merged configuration ({} entries):", merged.len());
    let mut keys: Vec<&String> = merged.keys().collect();
    keys.sort();
    for key in keys {
        let (sources, value) = &merged[key];
        let display = if key.is_empty() {
            "(unnamed)"
        } else {
            key.as_str()
        };
        let source_list = {
            let mut s = String::new();
            let mut first = true;
            for payload in sources {
                if !first {
                    s.push_str(", ");
                }
                s.push_str(&payload.source.to_string());
                first = false;
            }
            s
        };
        println!("  {display}  [{source_list}]");
        println!("    {:#}", value.value);
    }

    Ok(())
}

fn parse_args() -> Result<(bool, Vec<String>), Box<dyn std::error::Error>> {
    let mut trace = false;
    let mut sources = Vec::new();

    for arg in std::env::args().skip(1) {
        if arg == "--trace" {
            trace = true;
        } else {
            sources.push(arg);
        }
    }

    if sources.is_empty() {
        sources.push("env(prefix=APP_NAME,separator=.)".to_string());
        let etc = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("examples")
            .join("basic")
            .join("etc");
        sources.push(format!("file:{}", etc.display()));
    }

    Ok((trace, sources))
}
