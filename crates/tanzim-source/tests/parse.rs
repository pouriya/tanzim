use tanzim_source::{OnError, OptionValue, ParseError, Source, Stage, parse};

fn parsed(input: &str) -> Source {
    parse(input).unwrap_or_else(|error| panic!("{error}"))
}

#[test]
fn parses_documented_examples() {
    let env = parsed("env");
    assert_eq!(env.source(), "env");
    assert!(env.options().is_empty());
    assert_eq!(env.resource(), "");
    assert_eq!(env.on_error(Stage::Load), OnError::Fail);
    assert!(!env.resource_colon());

    let env_opts = parsed("env(prefix=APP_)");
    assert_eq!(
        env_opts.options().get("prefix"),
        Some(&OptionValue::String("APP_".into()))
    );

    let file = parsed("file:/x/y/z");
    assert_eq!(file.resource(), "/x/y/z");
    assert_eq!(file.on_error(Stage::Load), OnError::Fail);

    let file_skip = parsed("file(on_error=(load=skip)):.env");
    assert_eq!(file_skip.on_error(Stage::Load), OnError::Skip);
    assert_eq!(file_skip.resource(), ".env");

    let http = parsed(
        r#"http(headers=(Authorization="TOKEN"),timeout=3s,on_error=(load=skip)):https://domain.tld/my/config.yml"#,
    );
    assert_eq!(http.source(), "http");
    assert_eq!(http.on_error(Stage::Load), OnError::Skip);
    assert_eq!(http.resource(), "https://domain.tld/my/config.yml");
    assert_eq!(
        http.options().get("timeout"),
        Some(&OptionValue::String("3s".into()))
    );
}

#[test]
fn round_trips_examples() {
    for input in [
        "env",
        "env(prefix=APP_)",
        "file:/x/y/z",
        "file(on_error=(load=skip)):.env",
        "env:",
    ] {
        let source = parsed(input);
        assert_eq!(source.to_string(), input, "round-trip failed for `{input}`");
    }

    let http = parsed(
        r#"http(headers=(Authorization="TOKEN"),timeout=3s,on_error=(load=skip,validate=skip)):https://domain.tld/my/config.yml"#,
    );
    assert_eq!(parsed(&http.to_string()), http);
}

#[test]
fn parses_bool_case_insensitive() {
    let source = parsed("env(on=TRUE,off=false)");
    assert_eq!(source.options().get("on"), Some(&OptionValue::Bool(true)));
    assert_eq!(source.options().get("off"), Some(&OptionValue::Bool(false)));
}

#[test]
fn old_skip_marker_now_errors() {
    // The legacy `?` ignore-errors marker is gone; it is now trailing input.
    assert!(matches!(
        parse("file?:.env"),
        Err(ParseError::TrailingInput { .. })
    ));
    assert!(matches!(
        parse("env?(kv=salam):oops"),
        Err(ParseError::TrailingInput { .. })
    ));
}

#[test]
fn rejects_malformed_on_error() {
    assert!(matches!(
        parse("file(on_error=skip):.env"),
        Err(ParseError::InvalidOnError { .. })
    ));
    assert!(matches!(
        parse("file(on_error=(bogus=skip)):.env"),
        Err(ParseError::InvalidOnError { .. })
    ));
    assert!(matches!(
        parse("file(on_error=(load=maybe)):.env"),
        Err(ParseError::InvalidOnError { .. })
    ));
}

#[test]
fn parses_complex_options_with_on_error() {
    let source = parsed(r#"env(kv=salam,h=(o=b,z=[1,2,3.14,""]),on_error=(parse=skip)):oops"#);
    assert_eq!(source.on_error(Stage::Parse), OnError::Skip);
    assert_eq!(source.resource(), "oops");
    assert_eq!(
        source.options().get("kv"),
        Some(&OptionValue::String("salam".into()))
    );
}

#[test]
fn rejects_invalid_forms() {
    assert!(matches!(parse(""), Err(ParseError::MissingSource { .. })));
    assert!(matches!(
        parse("env(a=)"),
        Err(ParseError::EmptyValue { .. })
    ));
    assert!(matches!(
        parse("env(a=1,)"),
        Err(ParseError::TrailingComma { .. })
    ));
    assert!(matches!(
        parse("env(a=.5)"),
        Err(ParseError::InvalidNumber { .. })
    ));
    assert!(matches!(
        parse("env(a=+5)"),
        Err(ParseError::UnexpectedChar { .. })
    ));
    assert!(matches!(
        parse("env()oops"),
        Err(ParseError::TrailingInput { .. })
    ));
}

#[test]
fn parse_error_alternate_includes_caret() {
    let error = parse("env(prefix=)").unwrap_err();
    let message = format!("{error:#}");
    assert!(message.contains("column"));
    assert!(message.contains('^'));
    assert!(message.contains('\n'));
}

#[test]
fn parse_error_default_is_single_line() {
    let error = parse("env(prefix=)").unwrap_err();
    let message = error.to_string();
    assert!(!message.contains('^'));
    assert!(!message.contains('\n'));
}

#[test]
fn rejects_more_invalid_forms() {
    assert!(matches!(parse("env(=1)"), Err(ParseError::EmptyKey { .. })));
    assert!(matches!(
        parse("env(@a=1)"),
        Err(ParseError::UnexpectedChar { .. })
    ));
    assert!(matches!(
        parse(r#"env(x="unclosed)"#),
        Err(ParseError::UnclosedString { .. })
    ));
    assert!(matches!(
        parse(r#"env(x="\q")"#),
        Err(ParseError::InvalidEscape { .. })
    ));
}

#[test]
fn parses_resource_colon_without_path() {
    let source = parsed("env:");
    assert!(source.resource_colon());
    assert_eq!(source.resource(), "");
    assert_eq!(source.to_string(), "env:");
}

#[test]
fn rejects_unclosed_list_and_map_forms() {
    assert!(matches!(
        parse("env(x=[1"),
        Err(ParseError::UnclosedList { .. })
    ));
    assert!(matches!(
        parse("env(a=1"),
        Err(ParseError::UnclosedMap { .. })
    ));
    assert!(matches!(
        parse("env(x=(a=1"),
        Err(ParseError::UnclosedMap { .. })
    ));
    assert!(matches!(
        parse("env("),
        Err(ParseError::InvalidIdentifier { .. })
    ));
    assert!(matches!(
        parse("env(a"),
        Err(ParseError::UnexpectedEnd { .. })
    ));
}

#[test]
fn parses_empty_options_list_and_map_values() {
    let source = parsed("env()");
    assert!(source.options().is_empty());

    let source = parsed("env(items=[],nested=())");
    assert_eq!(
        source.options().get("items"),
        Some(&OptionValue::List(Vec::new()))
    );
    assert!(source.options().get("nested").unwrap().is_map());
}

#[test]
fn parses_numeric_and_escaped_string_values() {
    let source = parsed(r#"env(n=-7,pi=2.5,token="a\"b",nl="x\ny")"#);
    assert_eq!(source.options().get("n"), Some(&OptionValue::Integer(-7)));
    assert_eq!(source.options().get("pi"), Some(&OptionValue::Float(2.5)));
    assert_eq!(
        source.options().get("token"),
        Some(&OptionValue::String("a\"b".into()))
    );
    assert_eq!(
        source.options().get("nl"),
        Some(&OptionValue::String("x\ny".into()))
    );
}

#[test]
fn display_quotes_ambiguous_strings_and_formats_collections() {
    let source = parsed(r#"env(empty="",name="007",items=[a,b],nested=(k=v))"#);
    let text = source.to_string();
    assert!(text.contains(r#"empty="""#));
    assert!(text.contains(r#"name="007""#));
    assert!(text.contains("items=[a,b]"));
    assert!(text.contains("nested=(k=v)"));
    assert_eq!(parsed(&text), source);
}

#[test]
fn display_renders_whole_number_floats_with_one_decimal_place() {
    let source = parsed("env(n=2.0)");
    assert_eq!(source.to_string(), "env(n=2.0)");
}

#[test]
fn list_and_map_reject_trailing_commas_and_bad_separators() {
    assert!(matches!(
        parse("env(x=[1,])"),
        Err(ParseError::TrailingComma { .. })
    ));
    assert!(matches!(
        parse("env(x=[1 2])"),
        Err(ParseError::UnexpectedChar { .. })
    ));
    assert!(matches!(
        parse("env(x=(a=1,))"),
        Err(ParseError::TrailingComma { .. })
    ));
}

#[test]
fn parse_error_variants_include_context_in_display() {
    let error = parse("env()oops").unwrap_err();
    let message = error.to_string();
    assert!(message.contains("unexpected trailing input"));
    assert!(message.contains("column"));

    let error = parse("env(a=.5)").unwrap_err();
    assert!(error.to_string().contains("invalid number"));
}
