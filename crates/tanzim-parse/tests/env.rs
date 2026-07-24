use tanzim_parse::{Parse, Source, env::Env};
use tanzim_source::{OptionValue, SourceBuilder};
use tanzim_value::Error;

fn file_source(resource: &str) -> Source {
    SourceBuilder::new()
        .with_source("file")
        .with_resource(resource)
        .build()
        .unwrap()
}

#[test]
fn parses_dotenv_contents() {
    let source = file_source(".env");
    let parsed = Env::new()
        .parse(&source, b"FOO=bar\nBAZ=qux\n", &[])
        .unwrap();
    let map = parsed.value().as_map().unwrap();
    assert_eq!(map.get("foo").unwrap().value().as_string().unwrap(), "bar");
    assert_eq!(map.get("baz").unwrap().value().as_string().unwrap(), "qux");
}

#[test]
fn parses_env_with_line_numbers() {
    let source = file_source(".env");
    let root = Env::new()
        .parse(&source, b"FOO=bar\nBAZ=qux\n", &[])
        .unwrap();
    let map = root.value().as_map().unwrap();
    let foo = map.get("foo").unwrap();
    assert_eq!(foo.value().as_string().unwrap(), "bar");
    assert_eq!(foo.location().line, std::num::NonZeroU32::new(1));
    let baz = map.get("baz").unwrap();
    assert_eq!(baz.location().line, std::num::NonZeroU32::new(2));
}

#[test]
fn parses_nested_keys_with_separator() {
    let source = SourceBuilder::new()
        .with_source("env")
        .with_option("separator", OptionValue::String("__".into()))
        .build()
        .unwrap();
    let parsed = Env::new().parse(&source, b"BAR__BAZ=val\n", &[]).unwrap();
    let map = parsed.value().as_map().unwrap();
    let bar = map.get("bar").unwrap();
    let nested = bar.value().as_map().unwrap();
    assert_eq!(
        nested.get("baz").unwrap().value().as_string().unwrap(),
        "val"
    );
}

#[test]
fn parses_prefix_and_suffix_comments() {
    let text = b"# top comment\n# second line\nPORT=8080 # listen port\n";
    let parsed = Env::new().parse(&file_source(".env"), text, &[]).unwrap();
    let port = parsed.value().as_map().unwrap().get("port").unwrap();
    assert_eq!(port.comment().before(), &["top comment", "second line"]);
    assert_eq!(port.comment().after(), Some("listen port"));
    assert_eq!(port.value().as_string().unwrap(), "8080");
}

#[test]
fn parses_quoted_value_with_suffix_comment() {
    let source = SourceBuilder::new()
        .with_source("env")
        .with_option("separator", OptionValue::String("__".into()))
        .build()
        .unwrap();
    let parsed = Env::new()
        .parse(&source, b"SERVER__PORT=\"8080\" # listen port\n", &[])
        .unwrap();
    let server = parsed.value().as_map().unwrap().get("server").unwrap();
    let port = server.value().as_map().unwrap().get("port").unwrap();
    assert_eq!(port.value().as_string().unwrap(), "8080");
    assert_eq!(port.comment().after(), Some("listen port"));
}

#[test]
fn parses_file_env_inheriting_separator() {
    let env_source = SourceBuilder::new()
        .with_source("env")
        .with_option("separator", OptionValue::String(".".into()))
        .build()
        .unwrap();
    let file_source = file_source("foo.env");
    let other_sources = [env_source];
    let parsed = Env::new()
        .parse(
            &file_source,
            b"SERVER.PORT=8080\n",
            other_sources.as_slice(),
        )
        .unwrap();
    let server = parsed.value().as_map().unwrap().get("server").unwrap();
    let port = server.value().as_map().unwrap().get("port").unwrap();
    assert_eq!(port.value().as_string().unwrap(), "8080");
}

#[test]
fn rejects_conflicting_env_separators() {
    let env_dot = SourceBuilder::new()
        .with_source("env")
        .with_option("separator", OptionValue::String(".".into()))
        .build()
        .unwrap();
    let env_underscore = SourceBuilder::new()
        .with_source("env")
        .with_option("separator", OptionValue::String("__".into()))
        .build()
        .unwrap();
    let other_sources = [env_dot, env_underscore];
    let error = Env::new()
        .parse(
            &file_source("foo.env"),
            b"SERVER.PORT=8080\n",
            other_sources.as_slice(),
        )
        .unwrap_err();
    assert!(matches!(error, Error::Parse { .. }));
    assert!(error.to_string().contains("separator"));
}
