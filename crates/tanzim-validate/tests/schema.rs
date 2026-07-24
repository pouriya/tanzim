use tanzim_validate::{
    Bool, Registry, SchemaError, SchemaErrorKind, SchemaValue, Segment, build, build_value,
};
use tanzim_value::{LocatedValue, Location, Value};

fn schema_location() -> Location {
    Location::at("schema", "", None, None, None)
}

fn parse(json: &str) -> Value {
    let schema: SchemaValue = serde_json::from_str(json).unwrap();
    schema.into_value()
}

fn build_err(json: &str) -> SchemaError {
    match build_value(&parse(json)) {
        Ok(_) => panic!("expected a schema error"),
        Err(error) => error,
    }
}

#[test]
fn builds_nested_schema_and_validates() {
    let schema = parse(
        r#"{
            "type": "static_map",
            "fields": {
                "host": {"required": true,  "validator": {"type": "host"}},
                "port": {"required": false, "validator": {"type": "port"}},
                "tags": {"required": false, "validator": {
                    "type": "list", "unique": true,
                    "items": {"type": "string", "min_chars": 1}
                }},
                "mode": {"required": true, "validator": {
                    "type": "either",
                    "first":  {"type": "enum", "values": ["auto", "manual"]},
                    "second": {"type": "integer", "min": 0}
                }}
            }
        }"#,
    );
    let validator = build_value(&schema).unwrap();

    let mut config = LocatedValue::new(
        parse(r#"{"host": "localhost", "port": "8080", "tags": ["a", "b"], "mode": "auto"}"#),
        schema_location(),
    );
    tanzim_validate::validate(validator.as_ref(), &mut config).unwrap();

    let port = config.value().as_map().unwrap().get("port").unwrap();
    assert_eq!(*port.value(), Value::Int(8080));
}

#[test]
fn unknown_type_is_reported() {
    let error = build_err(r#"{"type": "nope"}"#);
    assert!(matches!(error.kind, SchemaErrorKind::UnknownType { .. }));
}

#[test]
fn wrong_option_type_is_reported() {
    let error = build_err(r#"{"type": "integer", "min": "x"}"#);
    assert!(matches!(error.kind, SchemaErrorKind::WrongType { .. }));
}

#[test]
fn missing_type_is_reported() {
    let error = build_err(r#"{"min": 1}"#);
    assert!(matches!(error.kind, SchemaErrorKind::MissingField { .. }));
}

#[test]
fn nested_error_carries_path() {
    let error = build_err(r#"{"type": "list", "items": {"type": "integer", "min": "x"}}"#);
    assert_eq!(error.path, vec![Segment::Key("items".to_string())]);
}

#[test]
fn custom_validator_can_be_registered() {
    let mut registry = Registry::with_builtins();
    registry.register("yes", |_node| Ok(Box::new(Bool::new())));
    let validator = registry.build_value(&parse(r#"{"type": "yes"}"#)).unwrap();
    assert!(validator.validate(&mut Value::Bool(true)).is_ok());
}

#[test]
fn empty_registry_knows_nothing() {
    let error = match Registry::empty().build_value(&parse(r#"{"type": "bool"}"#)) {
        Ok(_) => panic!("expected a schema error"),
        Err(error) => error,
    };
    assert!(matches!(error.kind, SchemaErrorKind::UnknownType { .. }));
}

#[test]
fn feature_gated_tag_round_trips() {
    let validator = build_value(&parse(r#"{"type": "uuid"}"#)).unwrap();
    assert!(
        validator
            .validate(&mut Value::String(
                "67e55044-10b1-426f-9247-bb680e5fe0c8".into()
            ))
            .is_ok()
    );
}

#[test]
fn integer_schema_honors_min_and_max() {
    let validator = build_value(&parse(r#"{"type": "integer", "min": 1, "max": 10}"#)).unwrap();
    let mut ok = Value::Int(5);
    validator.validate(&mut ok).unwrap();
    let mut low = Value::Int(0);
    assert!(validator.validate(&mut low).is_err());
}

#[test]
fn string_schema_honors_min_chars() {
    let validator = build_value(&parse(r#"{"type": "string", "min_chars": 3}"#)).unwrap();
    let mut short = Value::String("ab".into());
    assert!(validator.validate(&mut short).is_err());
}

#[test]
fn list_schema_honors_min_len() {
    let validator = build_value(&parse(r#"{"type": "list", "min_len": 2}"#)).unwrap();
    let mut short = Value::List(Vec::new());
    assert!(validator.validate(&mut short).is_err());
}

#[test]
fn enum_schema_supports_case_insensitive() {
    let validator = build_value(&parse(
        r#"{"type": "enum", "values": ["Auto"], "case_insensitive": true}"#,
    ))
    .unwrap();
    let mut value = Value::String("auto".into());
    validator.validate(&mut value).unwrap();
}

#[test]
fn build_rejects_non_map_root() {
    let located = LocatedValue::new(Value::String("nope".into()), schema_location());
    let error = match build(&located) {
        Ok(_) => panic!("expected a schema error"),
        Err(error) => error,
    };
    assert!(matches!(error.kind, SchemaErrorKind::NotMap));
}

#[test]
fn schema_error_display_includes_path_and_location() {
    let error = build_err(r#"{"type": "list", "min_len": -1}"#);
    let message = error.to_string();
    assert!(message.contains("min_len"));
    assert!(message.contains("must be non-negative"));
}

#[test]
fn negative_opt_usize_is_invalid_value() {
    let error = build_err(r#"{"type": "list", "max_len": -1}"#);
    assert!(matches!(error.kind, SchemaErrorKind::InvalidValue { .. }));
}

#[test]
fn schema_value_deserializes_null() {
    let value: SchemaValue = serde_json::from_str("null").unwrap();
    assert!(value.into_value().is_null());
}

#[test]
fn schema_value_deserializes_scalar_and_collection_forms() {
    let float: SchemaValue = serde_json::from_str("1.5").unwrap();
    assert!(matches!(float.into_value(), Value::Float(_)));
    let list: SchemaValue = serde_json::from_str("[1, 2]").unwrap();
    assert!(list.into_value().as_list().is_some());
    let map: SchemaValue = serde_json::from_str(r#"{"type":"bool"}"#).unwrap();
    assert!(map.into_value().as_map().is_some());
}

#[test]
fn integer_schema_honors_sign_flags() {
    let validator = build_value(&parse(r#"{"type": "integer", "positive": true}"#)).unwrap();
    let mut ok = Value::Int(3);
    validator.validate(&mut ok).unwrap();
    let mut zero = Value::Int(0);
    assert!(validator.validate(&mut zero).is_err());
}

#[test]
fn float_and_number_schema_honor_bounds() {
    let float = build_value(&parse(r#"{"type": "float", "min": 0.5, "max": 2.0}"#)).unwrap();
    let mut ok = Value::Float(1.0);
    float.validate(&mut ok).unwrap();

    let number = build_value(&parse(
        r#"{"type": "number", "non_negative": true, "non_positive": false}"#,
    ))
    .unwrap();
    let mut ok = Value::Int(0);
    number.validate(&mut ok).unwrap();
}

#[test]
fn string_schema_honors_max_chars_and_invalid_regex() {
    let validator = build_value(&parse(r#"{"type": "string", "max_chars": 2}"#)).unwrap();
    let mut long = Value::String("abc".into());
    assert!(validator.validate(&mut long).is_err());

    let error = build_err(r#"{"type": "string", "regex": "[invalid"}"#);
    assert!(matches!(error.kind, SchemaErrorKind::InvalidValue { .. }));
}

#[test]
fn list_schema_honors_unique_items_and_max_len() {
    let validator = build_value(&parse(
        r#"{"type": "list", "unique": true, "max_len": 1, "items": {"type": "string"}}"#,
    ))
    .unwrap();
    let mut dup = Value::List(vec![
        LocatedValue::new(Value::String("a".into()), schema_location()),
        LocatedValue::new(Value::String("a".into()), schema_location()),
    ]);
    assert!(validator.validate(&mut dup).is_err());
}

#[test]
fn dynamic_map_schema_honors_values_validator() {
    let validator = build_value(&parse(
        r#"{"type": "dynamic_map", "values": {"type": "integer"}}"#,
    ))
    .unwrap();
    let mut ok = parse(r#"{"count": "7"}"#);
    validator.validate(&mut ok).unwrap();
    assert_eq!(
        *ok.as_map().unwrap().get("count").unwrap().value(),
        Value::Int(7)
    );
}

#[test]
fn static_map_schema_supports_optional_and_required_fields() {
    let validator = build_value(&parse(
        r#"{
            "type": "static_map",
            "allow_unknown": true,
            "fields": {
                "name": {"required": true, "validator": {"type": "string"}},
                "tag": {"required": false},
                "mode": {"required": false, "validator": {"type": "enum", "values": ["a"]}}
            }
        }"#,
    ))
    .unwrap();
    let mut ok = parse(r#"{"name": "demo", "tag": "x", "extra": 1, "mode": "a"}"#);
    validator.validate(&mut ok).unwrap();
}

#[test]
fn static_map_fields_must_be_a_map() {
    let error = build_err(r#"{"type": "static_map", "fields": "nope"}"#);
    assert!(matches!(error.kind, SchemaErrorKind::WrongType { .. }));
}

#[test]
fn net_schema_constructors_accept_options() {
    let domain = build_value(&parse(r#"{"type": "domain", "require_dot": true}"#)).unwrap();
    let mut ok = Value::String("example.com".into());
    domain.validate(&mut ok).unwrap();

    let port = build_value(&parse(
        r#"{"type": "port", "allow_zero": true, "privileged_ok": false}"#,
    ))
    .unwrap();
    let mut zero = Value::Int(0);
    port.validate(&mut zero).unwrap();

    let ip = build_value(&parse(r#"{"type": "ip_addr", "v4_only": true}"#)).unwrap();
    let mut v4 = Value::String("127.0.0.1".into());
    ip.validate(&mut v4).unwrap();

    build_value(&parse(r#"{"type": "host"}"#)).unwrap();
    build_value(&parse(r#"{"type": "email"}"#)).unwrap();
    build_value(&parse(r#"{"type": "socket_addr"}"#)).unwrap();
}

#[test]
fn path_schema_rejects_unknown_kind() {
    let error = build_err(r#"{"type": "path", "kind": "pipe"}"#);
    assert!(matches!(error.kind, SchemaErrorKind::InvalidValue { .. }));
}

#[test]
fn path_schema_accepts_extensions_and_flags() {
    let validator = build_value(&parse(
        r#"{"type": "path", "relative": true, "extensions": ["toml", "json"]}"#,
    ))
    .unwrap();
    let mut ok = Value::String("config.toml".into());
    validator.validate(&mut ok).unwrap();
}

#[test]
fn feature_gated_schema_tags_build_successfully() {
    build_value(&parse(r#"{"type": "bool"}"#)).unwrap();
    build_value(&parse(r#"{"type": "non_empty"}"#)).unwrap();
    build_value(&parse(r#"{"type": "percentage"}"#)).unwrap();
    build_value(&parse(
        r#"{"type": "either", "first": {"type": "string"}, "second": {"type": "integer"}}"#,
    ))
    .unwrap();
    build_value(&parse(r#"{"type": "regex_pattern"}"#)).unwrap();
    build_value(&parse(
        r#"{"type": "url", "schemes": ["https"], "require_host": true}"#,
    ))
    .unwrap();
    build_value(&parse(r#"{"type": "cidr"}"#)).unwrap();
    build_value(&parse(r#"{"type": "semver"}"#)).unwrap();
    build_value(&parse(r#"{"type": "base64"}"#)).unwrap();
    build_value(&parse(r#"{"type": "hex"}"#)).unwrap();
    build_value(&parse(r#"{"type": "duration"}"#)).unwrap();
    build_value(&parse(r#"{"type": "duration", "convert": "int"}"#)).unwrap();
    build_value(&parse(r#"{"type": "duration", "convert": "string"}"#)).unwrap();
    build_value(&parse(r#"{"type": "bytesize"}"#)).unwrap();
    build_value(&parse(r#"{"type": "datetime"}"#)).unwrap();
    build_value(&parse(r#"{"type": "date"}"#)).unwrap();
}
