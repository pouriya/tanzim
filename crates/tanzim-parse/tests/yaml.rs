use tanzim_parse::{Parse, Source, yaml::Yaml};
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
fn parses_yaml_map() {
    let parsed = Yaml::new()
        .parse(&file_source("config.yaml"), b"hello: world\n", &[])
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
fn parses_yaml_map_with_lines() {
    let root = Yaml::new()
        .parse(&file_source("config.yaml"), b"foo: bar\nbaz: qux\n", &[])
        .unwrap();
    let map = root.value().as_map().unwrap();
    let foo = map.get("foo").unwrap();
    assert_eq!(foo.value().as_string().unwrap(), "bar");
    assert_eq!(foo.location().line, std::num::NonZeroU32::new(1));
    let baz = map.get("baz").unwrap();
    assert_eq!(baz.location().line, std::num::NonZeroU32::new(2));
}

#[test]
fn parses_yaml_null_at_correct_column() {
    let text = "foo: bar\n\nbaz:\n\n  qux: ~\n";
    let root = Yaml::new()
        .parse(&file_source("config.yaml"), text.as_bytes(), &[])
        .unwrap();
    let map = root.value().as_map().unwrap();
    let baz = map.get("baz").unwrap();
    let nested = baz.value().as_map().unwrap();
    let qux = nested.get("qux").unwrap();
    assert!(qux.value().is_null());
    assert_eq!(qux.location().line, std::num::NonZeroU32::new(5));
    assert_eq!(qux.location().column, std::num::NonZeroU32::new(8));
    assert_eq!(qux.location().length, std::num::NonZeroU32::new(1));
}

#[test]
fn syntax_error_has_location() {
    let error = Yaml::new()
        .parse(&file_source("config.yaml"), b"foo: [\n", &[])
        .unwrap_err();
    if let Error::Parse { location, .. } = &error {
        let location = location.as_ref().expect("syntax error location");
        assert!(location.line.is_some());
        assert!(location.column.is_some());
    } else {
        panic!("expected parse error");
    }
    let message = format!("{error:#}");
    assert!(message.contains('^'));
}
