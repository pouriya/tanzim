use tanzim_load::{Error, Load, Source, file::File};
use tanzim_source::SourceBuilder;
use tanzim_testing::environment::run;

fn make_source(resource: &str) -> Source {
    SourceBuilder::new()
        .with_source("file")
        .with_resource(resource)
        .build()
        .unwrap()
}

#[test]
fn load_resolves_name_and_format_from_path() {
    run(|env| {
        env.write_file("foo.JSON", b"{}")?;
        env.write_file("README", b"x")?;
        env.write_file(".env", b"x")?;
        let loaded = File::new().load(make_source(".")).unwrap();

        let mut foo = None;
        let mut readme = None;
        let mut dotenv = None;
        for payload in &loaded {
            if payload.maybe_name == Some("foo".to_string()) {
                foo = Some(payload);
            } else if payload.maybe_name == Some("readme".to_string()) {
                readme = Some(payload);
            } else if payload.maybe_name == Some(".env".to_string()) {
                dotenv = Some(payload);
            }
        }

        assert_eq!(foo.expect("foo").maybe_format, Some("json".to_string()));
        assert!(readme.expect("readme").maybe_format.is_none());
        assert!(dotenv.expect(".env").maybe_format.is_none());
        Ok(())
    })
    .unwrap();
}

#[test]
fn load_reads_files_with_and_without_extension() {
    run(|env| {
        env.write_file("foo.json", br#"{"hello":"world"}"#)?;
        env.write_file("README", b"no extension")?;
        env.write_file(".env", b"KEY=value")?;
        let loaded = File::new().load(make_source(".")).unwrap();
        assert_eq!(loaded.len(), 3);

        let mut foo = None;
        let mut readme = None;
        let mut dotenv = None;
        for payload in &loaded {
            if payload.maybe_name == Some("foo".to_string()) {
                foo = Some(payload);
            } else if payload.maybe_name == Some("readme".to_string()) {
                readme = Some(payload);
            } else if payload.maybe_name == Some(".env".to_string()) {
                dotenv = Some(payload);
            }
        }

        let foo = foo.expect("foo payload");
        assert_eq!(foo.maybe_format, Some("json".to_string()));

        let readme = readme.expect("readme payload");
        assert!(readme.maybe_format.is_none());

        let dotenv = dotenv.expect(".env payload");
        assert!(dotenv.maybe_format.is_none());
        Ok(())
    })
    .unwrap();
}

#[test]
fn load_reads_files_from_directory() {
    run(|env| {
        env.write_file("foo.json", br#"{"hello":"world"}"#)?;
        let loaded = File::new().load(make_source(".")).unwrap();
        assert_eq!(loaded.len(), 1);
        let payload = &loaded[0];
        assert_eq!(payload.maybe_name, Some("foo".to_string()));
        assert_eq!(payload.maybe_format, Some("json".to_string()));
        // Source resource updated to full file path
        assert!(payload.source.resource().ends_with("foo.json"));
        Ok(())
    })
    .unwrap();
}

#[test]
fn load_skips_not_found_when_configured() {
    let source = SourceBuilder::new()
        .with_source("file")
        .with_resource("/no/such/path")
        .with_option("skip", vec!["not-found"])
        .build()
        .unwrap();
    let loaded = File::new().load(source).unwrap();
    assert!(loaded.is_empty());
}

#[test]
fn load_requires_resource() {
    let source = SourceBuilder::new().with_source("file").build().unwrap();
    let error = File::new().load(source).unwrap_err();
    assert!(matches!(error, Error::InvalidResource { .. }));
}

#[test]
fn load_single_file_path() {
    run(|env| {
        env.write_file("solo.json", br#"{"ok":true}"#)?;
        let loaded = File::new().load(make_source("solo.json")).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].maybe_name.as_deref(), Some("solo"));
        assert_eq!(loaded[0].source.resource(), "solo.json");
        Ok(())
    })
    .unwrap();
}

#[test]
fn load_ignores_unknown_option() {
    run(|env| {
        env.write_file("foo.json", b"{}")?;
        let source = SourceBuilder::new()
            .with_source("file")
            .with_resource(".")
            .with_option("bogus", true)
            .build()
            .unwrap();
        let loaded = File::new().load(source).unwrap();
        assert_eq!(loaded.len(), 1);
        Ok(())
    })
    .unwrap();
}

#[test]
fn load_rejects_invalid_skip_list() {
    let source = SourceBuilder::new()
        .with_source("file")
        .with_resource("/tmp")
        .with_option("skip", "not-a-list")
        .build()
        .unwrap();
    let error = File::new().load(source).unwrap_err();
    assert!(matches!(error, Error::InvalidOption { key, .. } if key == "skip"));
}

#[test]
fn load_rejects_unknown_skip_value() {
    let source = SourceBuilder::new()
        .with_source("file")
        .with_resource("/tmp")
        .with_option("skip", vec!["bogus"])
        .build()
        .unwrap();
    let error = File::new().load(source).unwrap_err();
    assert!(matches!(error, Error::InvalidOption { key, .. } if key == "skip"));
}

#[test]
fn load_preserves_case_when_lowercase_disabled() {
    run(|env| {
        env.write_file("Demo.JSON", b"{}")?;
        let source = SourceBuilder::new()
            .with_source("file")
            .with_resource(".")
            .with_option("lowercase", false)
            .build()
            .unwrap();
        let loaded = File::new().load(source).unwrap();
        assert_eq!(loaded[0].maybe_name.as_deref(), Some("Demo"));
        assert_eq!(loaded[0].maybe_format.as_deref(), Some("JSON"));
        Ok(())
    })
    .unwrap();
}

#[test]
fn load_reports_not_found_for_missing_path() {
    let source = SourceBuilder::new()
        .with_source("file")
        .with_resource("/no/such/tanzim-file-path")
        .build()
        .unwrap();
    let error = File::new().load(source).unwrap_err();
    assert!(matches!(error, Error::NotFound { .. }));
}
