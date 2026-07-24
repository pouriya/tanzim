use tanzim_parse::{Parse, Source, toml::Toml};
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
}

#[test]
fn parses_array_element_suffix_comments() {
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
}

#[test]
fn parses_prefix_comments() {
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
