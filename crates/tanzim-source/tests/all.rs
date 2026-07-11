use tanzim_source::{
    Error, OnError, OptionValue, OptionValueType, Options, ParseError, Source, SourceBuilder, Stage,
};

#[test]
fn builder_requires_source() {
    let error = SourceBuilder::new().build().unwrap_err();
    assert!(matches!(error, Error::MissingSource));

    let error = SourceBuilder::new().with_source("").build().unwrap_err();
    assert!(matches!(error, Error::MissingSource));
}

#[test]
fn builder_with_option_and_into_string() {
    let source = SourceBuilder::new()
        .with_source("env")
        .with_resource("")
        .with_option("prefix", "APP")
        .with_option("timeout", 30_i64)
        .with_option("retry", true)
        .build()
        .unwrap();

    assert_eq!(source.source(), "env");
    assert_eq!(source.resource(), "");
    assert_eq!(
        source.options().get("prefix"),
        Some(&OptionValue::String("APP".into()))
    );
    assert_eq!(
        source.options().get("timeout"),
        Some(&OptionValue::Integer(30))
    );
    assert_eq!(
        source.options().get("retry"),
        Some(&OptionValue::Bool(true))
    );
}

#[test]
fn options_last_key_wins() {
    let mut options = Options::new();
    options.insert("prefix", "OLD");
    options.insert("prefix", "NEW");
    assert_eq!(options.len(), 1);
    assert_eq!(
        options.get("prefix"),
        Some(&OptionValue::String("NEW".into()))
    );
}

#[test]
fn option_value_accessors_and_type_name() {
    let value = OptionValue::from(vec!["a", "b"]);
    assert!(value.is_list());
    assert_eq!(value.type_name(), OptionValueType::List);
    assert_eq!(value.as_list().unwrap().len(), 2);

    let mut map = OptionValue::new_map();
    map.map_mut()
        .unwrap()
        .insert("enabled", OptionValue::from(true));
    assert_eq!(map.type_name(), OptionValueType::Map);
}

#[test]
fn config_source_setters() {
    let mut source = SourceBuilder::new()
        .with_source("file")
        .with_resource("/etc/app")
        .build()
        .unwrap();

    source.set_source("http");
    source.set_resource("https://example.com/config.json");
    source.set_option("timeout", 5_u32);

    assert_eq!(source.source(), "http");
    assert_eq!(source.resource(), "https://example.com/config.json");
    assert_eq!(
        source.options().get("timeout"),
        Some(&OptionValue::Integer(5))
    );
}

#[test]
fn builder_with_options() {
    let mut options = Options::new();
    options.insert("prefix", "APP_");
    let source = SourceBuilder::new()
        .with_source("env")
        .with_options(options)
        .build()
        .unwrap();
    assert_eq!(
        source.options().get("prefix"),
        Some(&OptionValue::String("APP_".into()))
    );
}

#[test]
fn on_error_reads_reserved_option() {
    let fail = Source::parse("file:/etc/app").unwrap();
    assert_eq!(fail.on_error(Stage::Load), OnError::Fail);
    assert_eq!(fail.on_error(Stage::Validate), OnError::Fail);

    let source = Source::parse("file(on_error=(load=skip,validate=skip)):/etc/app").unwrap();
    assert_eq!(source.on_error(Stage::Load), OnError::Skip);
    assert_eq!(source.on_error(Stage::Parse), OnError::Fail);
    assert_eq!(source.on_error(Stage::Validate), OnError::Skip);
}

#[test]
fn named_builds_bare_source() {
    let source = Source::named("schema");
    assert_eq!(source.source(), "schema");
    assert_eq!(source.resource(), "");
    assert!(source.options().is_empty());
    assert_eq!(source.on_error(Stage::Validate), OnError::Fail);
}

#[test]
fn options_remove_and_option_value_mutators() {
    let mut options = Options::new();
    options.insert("keep", "yes");
    options.insert("drop", "no");
    options.remove("drop");
    assert!(!options.contains_key("drop"));
    assert!(options.contains_key("keep"));

    let mut value = OptionValue::Integer(1);
    assert_eq!(value.as_integer(), Some(1));
    if let Some(number) = value.integer_mut() {
        *number = 2;
    }
    assert_eq!(value.as_integer(), Some(2));
    assert_eq!(value.into_integer(), Some(2));
}

#[test]
fn options_display_iter_and_mutators() {
    let mut options = Options::new();
    options.insert("a", 1_i64);
    options.insert("b", "two");
    assert_eq!(options.len(), 2);
    assert!(!options.is_empty());

    let keys: Vec<&str> = options.keys().collect();
    assert_eq!(keys, vec!["a", "b"]);

    let mut values = 0;
    for (_, value) in options.iter() {
        if value.is_integer() || value.is_string() {
            values += 1;
        }
    }
    assert_eq!(values, 2);

    if let Some(value) = options.get_mut("a") {
        *value = OptionValue::Integer(9);
    }
    assert_eq!(options.get("a"), Some(&OptionValue::Integer(9)));

    let previous = options.insert("a", 3_i64);
    assert_eq!(previous, Some(OptionValue::Integer(9)));

    let display = options.to_string();
    assert!(display.contains("\"a\""));
    assert!(display.contains("two"));
}

#[test]
fn option_value_and_type_display() {
    assert_eq!(OptionValueType::Map.to_string(), "map");
    assert_eq!(OptionValueType::List.to_string(), "list");

    let list = OptionValue::from(vec![1_i64, 2_i64]);
    assert_eq!(list.to_string(), "[1, 2]");

    let mut map = Options::new();
    map.insert("enabled", true);
    let map_value = OptionValue::Map(map);
    assert!(map_value.to_string().contains("enabled"));
}

#[test]
fn source_display_and_builder_resource_colon() {
    let source = SourceBuilder::new()
        .with_source("env")
        .with_resource_colon(true)
        .build()
        .unwrap();
    assert!(source.resource_colon());
    assert_eq!(source.to_string(), "env:");

    let mut source = SourceBuilder::new()
        .with_source("file")
        .with_option("skip", vec!["not-found"])
        .with_resource("/tmp/x")
        .build()
        .unwrap();
    source.set_resource_colon(false);
    source.options_mut().insert("extra", "yes");
    assert_eq!(source.source(), "file");
    let text = source.to_string();
    assert!(text.contains("/tmp/x"));
    assert!(text.contains("extra=yes"));
}

#[test]
fn source_with_mutators_update_fields() {
    let source = SourceBuilder::new()
        .with_source("env")
        .build()
        .unwrap()
        .with_source("file")
        .with_resource("/etc/app")
        .with_option("lowercase", false);
    assert_eq!(source.source(), "file");
    assert_eq!(source.resource(), "/etc/app");
    assert_eq!(
        source.options().get("lowercase"),
        Some(&OptionValue::Bool(false))
    );
}

#[test]
fn error_wraps_parse_failure() {
    match SourceBuilder::try_from("env(prefix=)") {
        Ok(_) => panic!("expected parse error"),
        Err(error) => assert!(matches!(error, Error::Parse(ParseError::EmptyValue { .. }))),
    }
}

#[test]
fn option_value_coercions_and_type_names() {
    let float = OptionValue::from(1.5_f64);
    assert!(float.is_float());
    assert_eq!(float.type_name(), OptionValueType::Float);
    assert_eq!(float.as_float(), Some(1.5));

    let text = OptionValue::from("hello");
    assert!(text.into_string().is_some());

    let mut flag = OptionValue::Bool(false);
    if let Some(value) = flag.bool_mut() {
        *value = true;
    }
    assert_eq!(flag.as_bool(), Some(true));
}
