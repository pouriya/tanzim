use tanzim::{merger::DeepMerge, pipeline::multi::Multi, source::Source};

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

    // `Multi::default()` pre-registers all feature-enabled loaders and parsers.
    let config = Multi::default()
        .with_merger(DeepMerge)
        .with_source(Source::parse("env(prefix=APP_NAME,separator=__)")?)
        .with_source(Source::parse(&format!("file:{}", etc.display()))?);

    let merged = config.run()?;

    assert!(
        merged.contains_key(&Some("foo".to_string())),
        "expected 'foo' entry in merged config"
    );

    if let Some(entry) = merged.get(&Some("foo".to_string())) {
        assert!(!entry.payloads().is_empty());
        assert!(
            entry.value().value().as_map().is_some(),
            "'foo' value should be a map"
        );
    }

    for (name, entry) in merged.iter() {
        let display = match name {
            None => "(unnamed)",
            Some(n) => n.as_str(),
        };
        println!(
            "{display} (from {} source(s)): {}",
            entry.payloads().len(),
            entry.value().value()
        );
    }

    Ok(())
}
