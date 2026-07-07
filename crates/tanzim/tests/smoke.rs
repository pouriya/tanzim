use tanzim::{merger::DeepMerge, pipeline};

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

    use std::path::PathBuf;
    use tanzim_testing::environment::run;

    // A sandbox sets the environment variables the pipeline reads and restores the process
    // environment afterwards, so the test is self-contained and safe under parallel test threads.
    run(|env| {
        env.set_env("APP_NAME__FOO__SERVER__ADDRESS", "127.0.0.1")?;
        env.set_env("APP_NAME__BAR__SQLITE__FILE", "/path/to/app.db")?;
        env.set_env("APP_NAME__BAZ__LOGGING__LEVEL", "debug")?;
        env.set_env("APP_NAME__QUX__HTTPS__INSECURE", "false")?;

        let etc = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("etc");

        // `pipeline::default()` pre-registers all feature-enabled loaders and parsers.
        let config = pipeline::default()
            .with_merger(DeepMerge::new())
            .expect("register merger")
            .with_source("env(prefix=APP_NAME,separator=__)")
            .expect("register env source")
            .with_source(format!("file:{}", etc.display()))
            .expect("register file source");

        let merged = config.run().expect("run pipeline");

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
    })?;

    Ok(())
}
