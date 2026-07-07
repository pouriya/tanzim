use tanzim_load::{Error, Load, env::Env};
use tanzim_source::{Options, Source, SourceBuilder};
use tanzim_testing::environment::run;

fn make_source_with_options(options: Options) -> Source {
    let mut builder = SourceBuilder::new().with_source("env");
    builder = builder.with_options(options);
    builder.build().unwrap()
}

#[test]
fn load_groups_environment_variables_by_name() {
    run(|env| {
        env.set_env("TANZIM_TEST__FOO__BAR", "baz")?;
        env.set_env("TANZIM_TEST__QUX__ABC", "123")?;

        let mut options = Options::new();
        options.insert("prefix", "TANZIM_TEST__");
        options.insert("separator", "__");
        let loaded = Env::new().load(make_source_with_options(options)).unwrap();

        let mut foo = None;
        let mut qux = None;
        for payload in &loaded {
            if payload.maybe_name == Some("foo".to_string()) {
                foo = Some(payload);
            } else if payload.maybe_name == Some("qux".to_string()) {
                qux = Some(payload);
            }
        }

        let foo = foo.expect("foo payload");
        assert_eq!(foo.maybe_format, Some("env".to_string()));
        assert!(String::from_utf8_lossy(&foo.content).contains("BAR=\"baz\""));

        let qux = qux.expect("qux payload");
        assert!(String::from_utf8_lossy(&qux.content).contains("ABC=\"123\""));
        Ok(())
    })
    .unwrap();
}

#[test]
fn load_without_separator_puts_all_keys_in_one_payload() {
    run(|env| {
        env.set_env("TANZIM_FLAT__FOO", "1")?;
        env.set_env("TANZIM_FLAT__BAR", "2")?;

        let mut options = Options::new();
        options.insert("prefix", "TANZIM_FLAT__");
        let loaded = Env::new().load(make_source_with_options(options)).unwrap();

        assert_eq!(loaded.len(), 1);
        let payload = &loaded[0];
        assert!(payload.maybe_name.is_none());
        let content = String::from_utf8_lossy(&payload.content);
        assert!(content.contains("FOO=\"1\""));
        assert!(content.contains("BAR=\"2\""));
        Ok(())
    })
    .unwrap();
}

#[test]
fn load_rejects_non_empty_resource() {
    let source = SourceBuilder::new()
        .with_source("env")
        .with_resource("oops")
        .build()
        .unwrap();
    let error = Env::new().load(source).unwrap_err();
    assert!(matches!(error, Error::InvalidResource { .. }));
}

#[test]
fn load_honors_strip_prefix_and_lowercase_options() {
    run(|env| {
        env.set_env("TANZIM_CASE__Foo__BAR", "1")?;
        let mut options = Options::new();
        options.insert("prefix", "TANZIM_CASE__");
        options.insert("separator", "__");
        options.insert("lowercase", false);
        let loaded = Env::new().load(make_source_with_options(options)).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].maybe_name.as_deref(), Some("Foo"));
        Ok(())
    })
    .unwrap();
}

#[test]
fn load_ignores_unknown_option() {
    let mut options = Options::new();
    options.insert("bogus", true);
    Env::new()
        .load(make_source_with_options(options))
        .expect("unknown options are ignored");
}

#[test]
fn load_rejects_bad_separator_type() {
    let mut options = Options::new();
    options.insert("separator", 1_i64);
    let error = Env::new()
        .load(make_source_with_options(options))
        .unwrap_err();
    assert!(matches!(error, Error::InvalidOption { key, .. } if key == "separator"));
}

#[test]
fn with_prefix_override_skips_source_option() {
    run(|env| {
        env.set_env("PINNED__X", "yes")?;
        let source = SourceBuilder::new()
            .with_source("env")
            .with_option("prefix", "OTHER__")
            .build()
            .unwrap();
        let loaded = Env::new().with_prefix("PINNED__").load(source).unwrap();
        let content = String::from_utf8_lossy(&loaded[0].content);
        assert!(content.contains(r#"X="yes""#));
        Ok(())
    })
    .unwrap();
}
