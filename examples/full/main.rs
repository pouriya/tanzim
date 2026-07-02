use std::collections::HashMap;
use std::path::PathBuf;
use tanzim::{
    loader::{env::Env, file::File},
    merge::DeepMerge,
    multi::{PipelineMultiBuilder, Schemas},
    parser::{env::Env as EnvParser, json::Json, toml::Toml, yaml::Yaml},
    validate::SchemaValue,
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

    let mut builder = PipelineMultiBuilder::new()
        .with_loader(Env::new())
        .with_loader(File::new())
        .with_parser(EnvParser::new())
        .with_parser(Json::new())
        .with_parser(Yaml::new())
        .with_parser(Toml::new())
        .with_merger(DeepMerge);

    let schemas = load_schemas()?;
    println!(
        "Loaded {} validation schema(s) from schema.yml",
        schemas.len()
    );
    builder = builder.with_schemas(schemas);

    for source_str in &source_args {
        builder = builder.with_source(source_str.as_str())?;
    }

    let merged = match builder.build()?.run() {
        Ok(merged) => merged,
        Err(error) => {
            eprintln!("Error: {error:#}");
            std::process::exit(1);
        }
    };

    println!("Merged configuration ({} entries):", merged.len());
    let mut keys: Vec<&Option<String>> = merged.keys().collect();
    keys.sort_by(|a, b| match (a, b) {
        (None, None) => std::cmp::Ordering::Equal,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (Some(_), None) => std::cmp::Ordering::Less,
        (Some(a), Some(b)) => a.cmp(b),
    });
    for key in keys {
        let (sources, value) = &merged[key];
        let display = match key {
            None => "(unnamed)",
            Some(name) => name.as_str(),
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
        println!("{display} [{source_list}]:\n{value:#}\n");
    }

    Ok(())
}

/// Load `schema.yml` from this example's directory with `serde_yaml`, convert it into the
/// facade's [`Schemas`] map, and hand it to the pipeline.
fn load_schemas() -> Result<Schemas, Box<dyn std::error::Error>> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("examples")
        .join("full")
        .join("schema.yml");
    let text = std::fs::read_to_string(&path)?;
    let documents: HashMap<String, SchemaValue> = serde_yaml::from_str(&text)?;

    let mut schemas = Schemas::new();
    for (name, document) in documents {
        schemas.insert(Some(name), document.into_value());
    }
    Ok(schemas)
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
            .join("full")
            .join("etc");
        sources.push(format!("file:{}", etc.display()));
    }

    Ok((trace, sources))
}
