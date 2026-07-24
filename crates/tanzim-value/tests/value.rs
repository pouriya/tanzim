use tanzim_source::Source;
use tanzim_value::{Comment, LocatedValue, Location, Map, Value, ValueType};

fn located_string(text: &str) -> LocatedValue {
    LocatedValue::new(
        Value::String(text.to_string()),
        Location::at("file", "test", None, None, None),
    )
}

#[test]
fn as_ref_value_accepts_all_forms() {
    fn take<V: AsRef<Value>>(value: V) -> Value {
        value.as_ref().clone()
    }
    let value = Value::Int(7);
    let located = LocatedValue::new(
        Value::Int(7),
        Location::at("file", "test", None, None, None),
    );
    assert_eq!(take(value.clone()), value);
    assert_eq!(take(&value), value);
    assert_eq!(take(located.clone()), value);
    assert_eq!(take(&located), value);
}

#[test]
fn last_key_wins() {
    let mut map = Map::new();
    map.insert("foo".to_string(), located_string("first"));
    map.insert("foo".to_string(), located_string("second"));
    assert_eq!(
        map.get("foo").unwrap().value().as_string().unwrap(),
        "second"
    );
}

#[test]
fn get_path_walks_nested_maps() {
    let location = Location::at("file", "cfg.toml", Some(2), Some(5), None);
    let mut server = Map::new();
    server.insert(
        "port".to_string(),
        LocatedValue::new(Value::Int(8080), location.clone()),
    );
    let mut root_map = Map::new();
    root_map.insert(
        "server".to_string(),
        LocatedValue::new(Value::Map(server), location.clone()),
    );
    root_map.insert("host".to_string(), located_string("localhost"));

    assert_eq!(
        root_map.get_path("server.port").unwrap().value().as_int(),
        Some(8080)
    );
    assert_eq!(
        root_map
            .get_path("host")
            .unwrap()
            .value()
            .as_string()
            .unwrap(),
        "localhost"
    );
    assert!(root_map.get_path("server.host").is_none());
    assert!(root_map.get_path("missing").is_none());
    // Non-map mid-path.
    assert!(root_map.get_path("host.port").is_none());

    let root = LocatedValue::new(Value::Map(root_map), location);
    assert_eq!(
        root.get_path("server.port").unwrap().location().to_string(),
        "file:cfg.toml:2:5"
    );
    assert!(root.get_path("").unwrap().value().is_map());
    assert!(
        LocatedValue::new(Value::Int(1), Location::at("x", "", None, None, None))
            .get_path("a")
            .is_none()
    );
}

#[test]
fn default_display_is_compact() {
    let value = LocatedValue::new(
        Value::String("hello".to_string()),
        Location::at("file", "config.yaml", Some(2), Some(5), None),
    );
    let message = value.to_string();
    assert!(!message.contains('\n'));
    assert!(!message.starts_with('@'));
    assert_eq!(message, "\"hello\"");
}

#[test]
fn alternate_display_shows_location_and_multiline() {
    let value = LocatedValue::new(
        Value::String("hello".to_string()),
        Location::at("file", "config.yaml", Some(2), Some(5), None),
    );
    let message = format!("{value:#}");
    assert_eq!(
        message,
        "{\n    \"value\": \"hello\",\n    \"location\": \"file:config.yaml:2:5\",\n}"
    );
    assert!(!message.contains('@'));
}

#[test]
fn value_accessors_and_constructors() {
    let mut value = Value::Bool(true);
    assert!(value.is_bool());
    assert_eq!(value.as_bool(), Some(true));
    assert_eq!(value.type_name(), ValueType::Bool);
    if let Some(flag) = value.bool_mut() {
        *flag = false;
    }
    assert_eq!(value.into_bool(), Some(false));

    let list = Value::new_list();
    assert!(list.is_list());
    let map = Value::new_map();
    assert!(map.is_map());
    let text = Value::new_string();
    assert!(text.is_string());
}

#[test]
fn map_remove_get_mut_and_display() {
    let mut map = Map::new();
    map.insert("a".to_string(), located_string("one"));
    map.insert("b".to_string(), located_string("two"));
    assert_eq!(map.len(), 2);
    assert!(map.contains_key("a"));
    assert!(map.get_mut("b").is_some());
    let removed = map.remove("a");
    assert!(removed.is_some());
    assert!(!map.contains_key("a"));

    let compact = format!("{map}");
    assert!(compact.contains("b"));
    let detailed = format!("{map:#}");
    assert!(detailed.contains("location"));
}

#[test]
fn location_display_and_with_length() {
    let location = Location::at("file", "", Some(1), Some(2), None).with_length(3);
    assert_eq!(location.to_string(), "file:1:2");
    let resourceful = Location::at("file", "cfg.yml", Some(4), None, None);
    assert_eq!(resourceful.to_string(), "file:cfg.yml:4");
}

#[test]
fn in_text_renders_gutter_and_caret_window() {
    let text = "a\nb\nc\ntarget\ne\nf\ng\n";
    // Offending token on line 4, column 1, three characters wide.
    let location = Location::in_source(Source::named("file"), None, None, None);
    let with_snippet = Location::in_text(location.source.clone(), text, Some(4), Some(1), Some(3));
    let snippet = &with_snippet.snippet;
    // Three lines of context on each side are included (lines 1..=7).
    for number in 1..=7 {
        assert!(
            snippet.contains(&format!("{number} | ")),
            "expected gutter for line {number} in:\n{snippet}"
        );
    }
    // The caret line underlines the offending span with `length` carets.
    assert!(snippet.contains("^^^"), "expected caret in:\n{snippet}");
    let target_line = snippet
        .lines()
        .find(|line| line.contains("target"))
        .expect("target line");
    let caret_line = snippet
        .lines()
        .find(|line| line.contains('^'))
        .expect("caret line");
    assert_eq!(
        target_line.find('|'),
        caret_line.find('|'),
        "gutter pipes should align:\n{snippet}"
    );
}

#[test]
fn in_text_clamps_window_near_bounds() {
    let text = "only\nline\nhere\n";
    let location = Location::in_text(Source::named("file"), text, Some(1), Some(1), None);
    // The window cannot extend before the first line; single caret when no length.
    assert!(location.snippet.contains("1 | only"));
    assert!(location.snippet.contains('^'));
    assert!(!location.snippet.contains("0 | "));
}

#[test]
fn in_text_without_line_leaves_snippet_empty() {
    let location = Location::in_text(Source::named("file"), "a\nb\n", None, None, None);
    assert!(location.snippet.is_empty());
}

#[test]
fn in_source_and_at_leave_snippet_empty() {
    assert!(
        Location::in_source(Source::named("file"), Some(2), Some(1), None)
            .snippet
            .is_empty()
    );
    assert!(
        Location::at("file", "cfg", Some(2), Some(1), None)
            .snippet
            .is_empty()
    );
}

#[test]
fn comment_attached_to_located_value() {
    let lv = LocatedValue::new(
        Value::String("debug".into()),
        Location::at("file", "baz.toml", Some(4), Some(9), None),
    )
    .with_comment(
        Comment::new()
            .with_before(["# log level: debug, info, warn, error"])
            .with_after(Some("# inline")),
    );
    assert_eq!(
        lv.comment().before(),
        &["# log level: debug, info, warn, error"]
    );
    assert_eq!(lv.comment().after(), Some("# inline"));
    assert_eq!(lv.value().as_string().unwrap(), "debug");
}

#[test]
fn comment_alternate_display_shows_comment_fields() {
    let lv = LocatedValue::new(
        Value::String("debug".into()),
        Location::at("file", "baz.toml", Some(4), Some(9), None),
    )
    .with_comment(Comment::new().with_before(["# level comment"]));
    let text = format!("{lv:#}");
    assert!(text.contains("\"comment_before\""));
    assert!(text.contains("level comment"));
}

#[test]
fn value_list_and_map_display_modes() {
    let list = Value::List(vec![located_string("a"), located_string("b")]);
    assert!(format!("{list}").contains("a"));
    assert!(format!("{list:#}").contains("location"));

    let mut map = Map::new();
    map.insert("k".to_string(), located_string("v"));
    let map_value = Value::Map(map);
    assert!(format!("{map_value}").contains("k"));
}
