use serde::{Deserialize, Serialize};
use tanzim_source::{OptionValue, Source};

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct App {
    source: Source,
}

#[test]
fn deserializes_config_source_from_string() {
    let app: App = serde_json::from_str(r#"{ "source": "env(prefix=APP_)" }"#).unwrap();
    assert_eq!(app.source.source(), "env");
    assert_eq!(
        app.source.options().get("prefix"),
        Some(&OptionValue::String("APP_".into()))
    );
}

#[test]
fn serializes_config_source_as_string() {
    let app = App {
        source: Source::parse("file(on_error=(load=skip)):.env").unwrap(),
    };
    let json = serde_json::to_string(&app).unwrap();
    assert_eq!(json, r#"{"source":"file(on_error=(load=skip)):.env"}"#);
}

#[test]
fn deserializes_invalid_config_source_with_parse_error() {
    let error = serde_json::from_str::<App>(r#"{ "source": "env(prefix=)" }"#).unwrap_err();
    let message = error.to_string();
    assert!(message.contains("configuration source option value cannot be empty"));
    assert!(!message.contains('\n'));
}
