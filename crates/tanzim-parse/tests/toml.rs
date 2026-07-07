use std::path::PathBuf;
use tanzim_parse::{
    Parse, Source,
    toml::{Toml, unparse},
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
fn unparses_complex_toml_round_trip() {
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

    let text = unparse(&file_source("out.toml"), Value::Map(map)).unwrap();
    let reparsed = Toml::new()
        .parse(&file_source("out.toml"), text.as_bytes(), &[])
        .unwrap();
    let map = reparsed.value().as_map().unwrap();
    assert_eq!(
        map.get("name").unwrap().value().as_string().unwrap(),
        "tanzim"
    );
    assert_eq!(map.get("port").unwrap().value().as_int().unwrap(), 8080);
    assert_eq!(map.get("ratio").unwrap().value().as_float().unwrap(), 0.5);
    assert!(map.get("debug").unwrap().value().as_bool().unwrap());
    let tags = map.get("tags").unwrap().value().as_list().unwrap();
    assert_eq!(tags[0].value().as_string().unwrap(), "a");
    assert_eq!(tags[1].value().as_string().unwrap(), "b");
    let nested = map.get("nested").unwrap().value().as_map().unwrap();
    assert_eq!(
        nested.get("key").unwrap().value().as_string().unwrap(),
        "value"
    );
}

#[test]
fn unparse_non_map_root_is_error() {
    assert!(unparse(&file_source("out.toml"), Value::Int(1)).is_err());
}

#[test]
fn parses_toml_table() {
    let parsed = Toml::new()
        .parse(&file_source("config.toml"), b"hello = \"world\"\n", &[])
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
fn nested_table_key_has_line_number() {
    let parsed = Toml::new()
        .parse(
            &file_source("config.toml"),
            b"[https]\nfollow_redirects = false\ninsecure = true\nretries = 3\n",
            &[],
        )
        .unwrap();
    let https = parsed.value().as_map().unwrap().get("https").unwrap();
    let nested = https.value().as_map().unwrap();
    let retries = nested.get("retries").unwrap();
    assert_eq!(retries.location().line, std::num::NonZeroU32::new(4));
    assert_eq!(retries.location().column, std::num::NonZeroU32::new(11));
}

#[test]
fn parses_table_header_prefix_comment() {
    let parsed = Toml::new()
        .parse(
            &file_source("baz.toml"),
            b"# This is a comment\n[logging]\nlevel = \"debug\"\n",
            &[],
        )
        .unwrap();
    let root = parsed.value().as_map().unwrap();
    let logging = root.get("logging").unwrap();
    assert_eq!(logging.comment().before(), &["This is a comment"]);
    assert!(!root.contains_key("# This is a comment"));
    assert_eq!(
        logging
            .value()
            .as_map()
            .unwrap()
            .get("level")
            .unwrap()
            .value()
            .as_string()
            .unwrap(),
        "debug"
    );
}

#[test]
fn parses_inline_suffix_comments() {
    let text = b"# This is a comment\n[logging]\n# log level\nlevel = \"debug\" # debug, info, warn, error\n# output serialize format\noutput_serialize_format = \"json\" # json, yaml\n";
    let parsed = Toml::new()
        .parse(&file_source("baz.toml"), text, &[])
        .unwrap();
    let root = parsed.value().as_map().unwrap();
    let logging_lv = root.get("logging").unwrap();
    assert_eq!(logging_lv.comment().before(), &["This is a comment"]);
    let logging = logging_lv.value().as_map().unwrap();
    let level = logging.get("level").unwrap();
    assert_eq!(level.comment().before(), &["log level"]);
    assert_eq!(level.comment().after(), Some("debug, info, warn, error"));
    let osf = logging.get("output_serialize_format").unwrap();
    assert_eq!(osf.comment().before(), &["output serialize format"]);
    assert_eq!(osf.comment().after(), Some("json, yaml"));

    let reparsed = unparse(&file_source("out.toml"), parsed.into_value()).unwrap();
    assert!(reparsed.contains("# debug, info, warn, error"));
    assert!(reparsed.contains("# json, yaml"));
    assert!(reparsed.contains("# This is a comment\n[logging]"));
    assert!(!reparsed.contains("[# This is a comment"));
}

#[test]
fn parses_and_unparses_array_element_suffix_comments() {
    let text = b"buckets = [\n    0.001, # small\n    1, # big\n]\n";
    let parsed = Toml::new()
        .parse(&file_source("config.toml"), text, &[])
        .unwrap();
    let buckets = parsed
        .value()
        .as_map()
        .unwrap()
        .get("buckets")
        .unwrap()
        .value()
        .as_list()
        .unwrap();
    assert_eq!(buckets[0].comment().after(), Some("small"));
    assert_eq!(buckets[1].comment().after(), Some("big"));

    let reparsed = unparse(&file_source("out.toml"), parsed.into_value()).unwrap();
    let again = Toml::new()
        .parse(&file_source("out.toml"), reparsed.as_bytes(), &[])
        .unwrap();
    let again_buckets = again
        .value()
        .as_map()
        .unwrap()
        .get("buckets")
        .unwrap()
        .value()
        .as_list()
        .unwrap();
    assert_eq!(again_buckets[0].comment().after(), Some("small"));
    assert_eq!(again_buckets[1].comment().after(), Some("big"));
    assert!(!reparsed.contains("[0.001 # small"));
}

#[test]
fn parses_array_element_prefix_comments() {
    let text = b"\
buckets = [
    0.001, # small
    # before second 1
    # before second 2
    0.01, # big
]
";
    let parsed = Toml::new()
        .parse(&file_source("config.toml"), text, &[])
        .unwrap();
    let buckets = parsed
        .value()
        .as_map()
        .unwrap()
        .get("buckets")
        .unwrap()
        .value()
        .as_list()
        .unwrap();
    assert_eq!(buckets[0].comment().after(), Some("small"));
    assert_eq!(
        buckets[1].comment().before(),
        &["before second 1", "before second 2"]
    );
    assert_eq!(buckets[1].comment().after(), Some("big"));

    let reparsed = unparse(&file_source("out.toml"), parsed.into_value()).unwrap();
    let again = Toml::new()
        .parse(&file_source("config.toml"), reparsed.as_bytes(), &[])
        .unwrap();
    let again_buckets = again
        .value()
        .as_map()
        .unwrap()
        .get("buckets")
        .unwrap()
        .value()
        .as_list()
        .unwrap();
    assert_eq!(again_buckets[0].comment().after(), Some("small"));
    assert_eq!(
        again_buckets[1].comment().before(),
        &["before second 1", "before second 2"]
    );
    assert_eq!(again_buckets[1].comment().after(), Some("big"));
    assert!(reparsed.contains("# before second 1"));
    assert!(reparsed.contains("# before second 2"));
}

#[test]
fn unparses_baz_toml_comments() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/full/etc/baz.toml");
    let text = std::fs::read_to_string(&path).unwrap();
    let source = file_source("baz.toml");
    let parsed = Toml::new().parse(&source, text.as_bytes(), &[]).unwrap();

    let buckets = parsed
        .value()
        .as_map()
        .unwrap()
        .get("metrics")
        .unwrap()
        .value()
        .as_map()
        .unwrap()
        .get("histogram_buckets")
        .unwrap()
        .value()
        .as_list()
        .unwrap();
    assert_eq!(buckets[0].comment().after(), Some("0.001s"));
    assert_eq!(
        buckets[1].comment().before(),
        &["before second 1", "before second 2"]
    );
    assert_eq!(buckets[1].comment().after(), Some("0.01s"));

    let reparsed = unparse(&source, &parsed).unwrap();
    let again = Toml::new()
        .parse(&source, reparsed.as_bytes(), &[])
        .unwrap();
    let again_buckets = again
        .value()
        .as_map()
        .unwrap()
        .get("metrics")
        .unwrap()
        .value()
        .as_map()
        .unwrap()
        .get("histogram_buckets")
        .unwrap()
        .value()
        .as_list()
        .unwrap();
    assert_eq!(again_buckets[0].comment().after(), Some("0.001s"));
    assert_eq!(
        again_buckets[1].comment().before(),
        &["before second 1", "before second 2"]
    );
    assert_eq!(again_buckets[1].comment().after(), Some("0.01s"));
    assert!(reparsed.contains("# This is a comment\n[logging]"));
    assert!(reparsed.contains("# log level"));
    assert!(reparsed.contains("# debug, info, warn, error"));
    assert!(reparsed.contains("# before second 1"));
    assert!(reparsed.contains("# before second 2"));
    assert!(reparsed.contains("# 0.001s"));
    assert!(reparsed.contains("    1000 # 1000s\n]"));
    assert!(!reparsed.contains("[# This is a comment"));
    assert!(!reparsed.contains("histogram_buckets = [0.001"));
}

#[test]
fn parses_and_unparses_prefix_comments() {
    let parsed = Toml::new()
        .parse(
            &file_source("config.toml"),
            b"# top comment\nhello = \"world\"\n",
            &[],
        )
        .unwrap();
    let map = parsed.value().as_map().unwrap();
    let hello = map.get("hello").unwrap();
    assert_eq!(hello.comment().before(), &["top comment"]);
    assert!(!map.contains_key("# top comment"));
    assert_eq!(hello.value().as_string().unwrap(), "world");

    let text = unparse(&file_source("out.toml"), parsed.into_value()).unwrap();
    assert_eq!(text, "# top comment\nhello = \"world\"\n");
}

#[test]
fn syntax_error_has_location() {
    let error = Toml::new()
        .parse(&file_source("config.toml"), b"hello = \n", &[])
        .unwrap_err();
    if let Error::Parse { location, .. } = &error {
        assert!(location.is_some());
        assert_eq!(
            location.as_ref().unwrap().line,
            std::num::NonZeroU32::new(1)
        );
    } else {
        panic!("expected parse error");
    }
    let message = format!("{error:#}");
    assert!(message.contains('^'));
}
