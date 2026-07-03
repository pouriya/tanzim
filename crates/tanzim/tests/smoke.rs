use tanzim::{
    loader::{env::Env as EnvLoader, file::File as FileLoader},
    merge::DeepMerge,
    multi::PipelineMultiBuilder,
    parser::{env::Env as EnvParser, json::Json, toml::Toml, yaml::Yaml},
};

#[test]
fn smoke() -> Result<(), Box<dyn std::error::Error>> {
    cfg_if::cfg_if! {
        if #[cfg(feature = "tracing")] {
            let _ = tracing_subscriber::fmt()
                .json()
                .with_max_level(tracing::Level::TRACE)
                .without_time()
                .try_init();
        } else if #[cfg(feature = "logging")] {
            let _ = env_logger::builder()
                .filter_level(log::LevelFilter::max())
                .format_timestamp(None)
                .is_test(true)
                .try_init();
        }
    }

    use std::{env, path::PathBuf};

    // SAFETY: test-only; single-threaded test env vars.
    unsafe {
        env::set_var("APP_NAME__FOO__SERVER__ADDRESS", "127.0.0.1");
        env::set_var("APP_NAME__BAR__SQLITE__FILE", "/path/to/app.db");
        env::set_var("APP_NAME__BAZ__LOGGING__LEVEL", "debug");
        env::set_var("APP_NAME__QUX__HTTPS__INSECURE", "false");
    }

    let etc = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("etc");

    let config = PipelineMultiBuilder::new()
        .with_source("env(prefix=APP_NAME,separator=__)")?
        .with_source(format!("file:{}", etc.display()))?
        .with_loader(EnvLoader::new())
        .with_loader(FileLoader::new())
        .with_parser(EnvParser::new())
        .with_parser(Json::new())
        .with_parser(Yaml::new())
        .with_parser(Toml::new())
        .with_merger(DeepMerge)
        .build()?;

    let merged = config.run()?;

    assert!(
        merged.contains_key(&Some("foo".to_string())),
        "expected 'foo' entry in merged config"
    );

    if let Some((sources, value)) = merged.get(&Some("foo".to_string())) {
        assert!(!sources.is_empty());
        assert!(
            value.value().as_map().is_some(),
            "'foo' value should be a map"
        );
    }

    for (name, (sources, value)) in &merged {
        let display = match name {
            None => "(unnamed)",
            Some(n) => n.as_str(),
        };
        println!(
            "{display} (from {} source(s)): {}",
            sources.len(),
            value.value()
        );
    }

    Ok(())
}
