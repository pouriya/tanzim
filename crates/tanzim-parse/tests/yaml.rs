use tanzim_parse::{
    Parse, Source,
    yaml::{Yaml, unparse},
};
use tanzim_source::SourceBuilder;
use tanzim_value::{Error, LocatedValue, Location, Map, Value};

fn file_source(resource: &str) -> Source {
    SourceBuilder::new()
        .with_source("file")
        .with_resource(resource)
        .build()
        .unwrap()
}

fn loc(value: Value) -> LocatedValue {
    LocatedValue::new(value, Location::at("file", "test", None, None, None))
}

#[test]
fn unparses_complex_yaml() {
    let mut nested = Map::new();
    nested.insert("key".into(), loc(Value::String("value".into())));
    let mut map = Map::new();
    map.insert("name".into(), loc(Value::String("tanzim".into())));
    map.insert("port".into(), loc(Value::Int(8080)));
    map.insert("ratio".into(), loc(Value::Float(0.5)));
    map.insert("debug".into(), loc(Value::Bool(true)));
    map.insert(
        "tags".into(),
        loc(Value::List(vec![
            loc(Value::String("a".into())),
            loc(Value::String("b".into())),
        ])),
    );
    map.insert("nested".into(), loc(Value::Map(nested)));

    let text = unparse(&file_source("out.yaml"), Value::Map(map)).unwrap();
    assert_eq!(
        text,
        "name: tanzim\nport: 8080\nratio: 0.5\ndebug: true\ntags:\n  - a\n  - b\nnested:\n  key: value\n"
    );
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
