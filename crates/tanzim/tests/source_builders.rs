//! Tests for the typed source builders in `tanzim::source` (`env` / `file` / `http`).

#[cfg(feature = "load-env")]
#[test]
fn env_builder_emits_only_set_options() {
    use tanzim::source::{Source, env};

    let source = Source::from(env().with_prefix("APP_").with_separator("__"));
    assert_eq!(source.source(), "env");
    assert_eq!(source.resource(), "");
    assert_eq!(source.to_string(), "env(prefix=APP_,separator=__)");

    // An unconfigured builder carries no options at all.
    let bare = Source::from(env());
    assert_eq!(bare.to_string(), "env");
}

#[cfg(feature = "load-env")]
#[test]
fn env_builder_sets_strip_prefix_and_lowercase() {
    use tanzim::source::{Source, env};

    let source = Source::from(env().strip_prefix(false).lowercase(false));
    assert_eq!(
        source.options().get("strip_prefix").unwrap().as_bool(),
        Some(false)
    );
    assert_eq!(
        source.options().get("lowercase").unwrap().as_bool(),
        Some(false)
    );
}

#[cfg(feature = "load-file")]
#[test]
fn file_builder_sets_resource_skip_and_on_error() {
    use tanzim::source::{OnError, Source, Stage, file};

    let source = Source::from(
        file("app.toml")
            .skip_not_found()
            .skip_no_access()
            .skip_load_error(),
    );
    assert_eq!(source.source(), "file");
    assert_eq!(source.resource(), "app.toml");

    // The loader `skip` option is a list of the requested kinds.
    let skip = source.options().get("skip").unwrap().as_list().unwrap();
    let kinds: Vec<&str> = skip
        .iter()
        .map(|v| v.as_string().unwrap().as_str())
        .collect();
    assert_eq!(kinds, ["not-found", "no-access"]);

    // The reserved `on_error` option drives the stage policy.
    assert_eq!(source.on_error(Stage::Load), OnError::Skip);
    assert_eq!(source.on_error(Stage::Parse), OnError::Fail);
}

#[cfg(feature = "load-file")]
#[test]
fn file_builder_deduplicates_skip_kinds() {
    use tanzim::source::{Source, file};

    let source = Source::from(file("app.toml").skip_not_found().skip_not_found());
    let skip = source.options().get("skip").unwrap().as_list().unwrap();
    assert_eq!(skip.len(), 1);
}

#[cfg(feature = "load-http-closure")]
#[test]
fn http_builder_sets_headers_timeout_and_insecure() {
    use std::time::Duration;
    use tanzim::source::{Source, Url, http};

    let source = Source::from(
        http(Url::parse("https://example.com/c.json").unwrap())
            .with_header("Authorization", "Bearer token")
            .with_timeout(Duration::from_secs(30))
            .insecure(true),
    );
    assert_eq!(source.source(), "http");
    assert_eq!(source.resource(), "https://example.com/c.json");
    assert_eq!(
        source.options().get("timeout").unwrap().as_integer(),
        Some(30)
    );
    assert_eq!(
        source.options().get("insecure").unwrap().as_bool(),
        Some(true)
    );

    let headers = source.options().get("headers").unwrap().as_map().unwrap();
    assert_eq!(
        headers.get("Authorization").unwrap().as_string().unwrap(),
        "Bearer token"
    );
}

/// End-to-end: build a pipeline entirely from the typed builders and run it, mirroring the string
/// form used in `smoke.rs`.
#[cfg(all(feature = "load-env", feature = "load-file"))]
#[test]
fn builders_drive_pipeline_end_to_end() -> Result<(), Box<dyn std::error::Error>> {
    use std::path::PathBuf;
    use tanzim::{
        merger::DeepMerge,
        pipeline::Pipeline,
        source::{env, file},
    };
    use tanzim_testing::environment::run;

    run(|sandbox| {
        sandbox.set_env("APP_NAME__FOO__SERVER__ADDRESS", "127.0.0.1")?;

        // Load a single env-format file so only the default `parse-env` parser is needed.
        let foo_env = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("etc")
            .join("foo.env");

        let merged = Pipeline::builder()
            .with_default_loaders()
            .with_default_parsers()
            .with_merger(DeepMerge::new())
            .with_source(env().with_prefix("APP_NAME").with_separator("__"))
            .with_source(file(foo_env.display().to_string()))
            .run()
            .expect("run pipeline");

        assert!(
            merged.contains_key(&Some("foo".to_string())),
            "expected 'foo' entry in merged config"
        );
        Ok(())
    })?;

    Ok(())
}
