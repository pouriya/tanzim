use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;
use tanzim::{
    merger::DeepMerge,
    pipeline::{self, Schemas},
    validator::SchemaValue,
};

/// A named configuration entry, deserialized from the merged tree. Each source file under `etc/`
/// contributes a differently-shaped entry, so every section is optional and unknown keys are
/// ignored; `try_deserialize::<Entry>()` yields one `Entry` per entry name.
#[derive(Default, Deserialize)]
#[serde(default)]
struct Entry {
    sqlite: Option<Sqlite>,
    logging: Option<Logging>,
    https: Option<Https>,
}

#[derive(Deserialize)]
struct Sqlite {
    file: String,
    #[serde(default)]
    recreate: bool,
}

#[derive(Deserialize)]
struct Logging {
    level: String,
    #[serde(default)]
    output_serialize_format: Option<String>,
}

#[derive(Deserialize)]
struct Https {
    insecure: bool,
    #[serde(default)]
    follow_redirects: bool,
    #[serde(default)]
    retries: Option<i64>,
}

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

    // `pipeline::default()` pre-registers all feature-enabled loaders and parsers; pick a merger.
    let mut pipeline = pipeline::default().with_merger(DeepMerge::new())?;

    let schemas = load_schemas()?;
    println!(
        "Loaded {} validation schema(s) from schema.yml",
        schemas.len()
    );
    pipeline = pipeline.with_schemas(schemas);

    for source_str in &source_args {
        pipeline = pipeline.with_source(source_str)?;
    }

    // Run the whole pipeline and deserialize each named entry straight into `Entry`. On failure
    // the error points at the exact source `file:line:column`.
    let configs: HashMap<Option<String>, Entry> = match pipeline.try_deserialize() {
        Ok(configs) => configs,
        Err(error) => {
            eprintln!("Error: {error:#}");
            std::process::exit(1);
        }
    };

    println!("Deserialized configuration ({} entries):", configs.len());
    let mut keys: Vec<&Option<String>> = configs.keys().collect();
    keys.sort_by(|a, b| match (a, b) {
        (None, None) => std::cmp::Ordering::Equal,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (Some(_), None) => std::cmp::Ordering::Less,
        (Some(a), Some(b)) => a.cmp(b),
    });
    for key in keys {
        let display = match key {
            None => "(unnamed)",
            Some(name) => name.as_str(),
        };
        let entry = &configs[key];
        println!("{display}:");
        if let Some(sqlite) = &entry.sqlite {
            println!(
                "  sqlite: file={} recreate={}",
                sqlite.file, sqlite.recreate
            );
        }
        if let Some(logging) = &entry.logging {
            println!(
                "  logging: level={} output_serialize_format={:?}",
                logging.level, logging.output_serialize_format
            );
        }
        if let Some(https) = &entry.https {
            println!(
                "  https: insecure={} follow_redirects={} retries={:?}",
                https.insecure, https.follow_redirects, https.retries
            );
        }
        println!();
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
