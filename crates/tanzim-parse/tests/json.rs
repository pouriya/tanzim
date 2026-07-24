use tanzim_parse::{Parse, Source, json::Json};
use tanzim_source::SourceBuilder;
use tanzim_value::Error;

fn file_source(resource: &str) -> Source {
    SourceBuilder::new()
        .with_source("file")
        .with_resource(resource)
        .build()
        .unwrap()
}

#[test]
fn parses_json_object() {
    let parsed = Json::new()
        .parse(&file_source("config.json"), br#"{"hello":"world"}"#, &[])
        .unwrap();
    assert_eq!(
        parsed
            .value()
            .as_map()
            .unwrap()
            .get("hello")
            .unwrap()
            .value()
            .as_string()
            .unwrap(),
        "world"
    );
}

#[test]
fn detects_json_format() {
    let parser = Json::new();
    assert_eq!(parser.is_format_supported(br#"{"a":1}"#), Some(true));
    assert_eq!(parser.is_format_supported(b"not json"), Some(false));
}

#[test]
fn single_line_json_omits_position() {
    let root = Json::new()
        .parse(&file_source("a.json"), br#"{"a":1}"#, &[])
        .unwrap();
    let map = root.value().as_map().unwrap();
    let entry = map.get("a").unwrap();
    assert_eq!(entry.location().line, None);
    assert_eq!(entry.location().column, None);
}

#[test]
fn parses_null() {
    let root = Json::new()
        .parse(&file_source("a.json"), b"{\n  \"a\": null\n}", &[])
        .unwrap();
    let map = root.value().as_map().unwrap();
    let entry = map.get("a").unwrap();
    assert!(entry.value().is_null());
    assert_eq!(entry.location().line, std::num::NonZeroU32::new(2));
}

#[test]
fn syntax_error_has_location() {
    let error = Json::new()
        .parse(&file_source("a.json"), b"{\n  \"a\":\n}\n", &[])
        .unwrap_err();
    if let Error::Parse { ref location, .. } = error {
        let location = location.as_ref().expect("syntax error location");
        assert!(location.line.is_some());
        assert!(location.column.is_some());
    } else {
        panic!("expected parse error");
    }
    let message = format!("{error:#}");
    assert!(message.contains('^'));
}
