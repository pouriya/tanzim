use serde::Deserialize;
use tanzim_value::{LocatedValue, Location, Map, Value};

fn location() -> Location {
    Location::at("test", "cfg", Some(1), Some(1), None)
}

fn located(value: Value) -> LocatedValue {
    LocatedValue::new(value, location())
}

#[derive(Deserialize, Debug, PartialEq)]
struct Config {
    name: String,
    port: u16,
    ratio: f64,
    enabled: bool,
    tags: Vec<String>,
    nickname: Option<String>,
    level: Level,
}

#[derive(Deserialize, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
enum Level {
    Debug,
    Info,
    Warn,
}

fn sample_tree() -> LocatedValue {
    let mut map = Map::new();
    map.insert("name".into(), located(Value::String("app".into())));
    map.insert("port".into(), located(Value::Int(8080)));
    map.insert("ratio".into(), located(Value::Float(0.5)));
    map.insert("enabled".into(), located(Value::Bool(true)));
    map.insert(
        "tags".into(),
        located(Value::List(vec![
            located(Value::String("a".into())),
            located(Value::String("b".into())),
        ])),
    );
    map.insert("nickname".into(), located(Value::Null));
    map.insert("level".into(), located(Value::String("info".into())));
    located(Value::Map(map))
}

#[test]
fn deserializes_nested_struct() {
    let tree = sample_tree();
    let config: Config = tree.try_deserialize().unwrap();
    assert_eq!(
        config,
        Config {
            name: "app".into(),
            port: 8080,
            ratio: 0.5,
            enabled: true,
            tags: vec!["a".into(), "b".into()],
            nickname: None,
            level: Level::Info,
        }
    );
}

#[test]
fn type_mismatch_error_carries_location() {
    #[derive(Deserialize, Debug)]
    struct Port {
        #[allow(dead_code)]
        port: u16,
    }

    let mut map = Map::new();
    map.insert(
        "port".into(),
        LocatedValue::new(
            Value::String("nope".into()),
            Location::at("file", "config.toml", Some(4), Some(9), None),
        ),
    );
    let tree = located(Value::Map(map));

    let error = tree.try_deserialize::<Port>().unwrap_err();
    assert!(
        matches!(
            error,
            tanzim_value::Error::Deserialize {
                location: Some(_),
                ..
            }
        ),
        "expected a located deserialize error, got {error:?}"
    );
    let message = error.to_string();
    assert!(
        message.contains("file:config.toml:4:9"),
        "message should point at the offending node: {message}"
    );
}

#[test]
fn deserializes_borrowed_str_zero_copy() {
    #[derive(Deserialize)]
    struct Borrowed<'a> {
        name: &'a str,
    }

    let mut map = Map::new();
    map.insert("name".into(), located(Value::String("app".into())));
    let tree = located(Value::Map(map));

    let borrowed: Borrowed = tree.try_deserialize().unwrap();
    assert_eq!(borrowed.name, "app");
}

#[test]
fn deserializes_from_json_produced_tree_shape() {
    // A tree whose leaves came from an ordinary parser still deserializes.
    let mut inner = Map::new();
    inner.insert("port".into(), located(Value::Int(1)));
    let mut map = Map::new();
    map.insert("name".into(), located(Value::String("x".into())));
    map.insert("port".into(), located(Value::Int(2)));
    map.insert("ratio".into(), located(Value::Float(1.0)));
    map.insert("enabled".into(), located(Value::Bool(false)));
    map.insert("tags".into(), located(Value::List(vec![])));
    map.insert("nickname".into(), located(Value::String("n".into())));
    map.insert("level".into(), located(Value::String("warn".into())));
    let tree = located(Value::Map(map));
    let config: Config = tree.try_deserialize().unwrap();
    assert_eq!(config.nickname, Some("n".into()));
    assert_eq!(config.level, Level::Warn);
    let _ = inner;
}
