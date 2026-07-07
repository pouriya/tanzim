use tanzim_parse::{Parse, Source, closure::Closure};
use tanzim_source::SourceBuilder;
use tanzim_value::{LocatedValue, Location, Value};

#[test]
fn closure_parser_delegates_to_function() {
    let parser = Closure::new(
        "upper",
        "txt",
        Box::new(
            |source: &Source, bytes: &[u8], _other_source_list: &[Source]| {
                Ok(LocatedValue::new(
                    Value::String(String::from_utf8_lossy(bytes).to_uppercase()),
                    Location::in_source(source.clone(), None, None, None),
                ))
            },
        ),
    )
    .with_validator(Box::new(|bytes| Some(!bytes.is_empty())));
    let source = SourceBuilder::new()
        .with_source("file")
        .with_resource("test.txt")
        .build()
        .unwrap();
    let parsed = parser.parse(&source, b"hello", &[]).unwrap();
    assert_eq!(parsed.value().as_string().unwrap(), "HELLO");
    assert_eq!(parser.is_format_supported(b"x"), Some(true));
    assert_eq!(parser.is_format_supported(b""), Some(false));
}

#[test]
fn closure_parser_with_format_list() {
    let parser = Closure::new(
        "yaml",
        "yml",
        Box::new(
            |source: &Source, bytes: &[u8], _other_source_list: &[Source]| {
                Ok(LocatedValue::new(
                    Value::String(String::from_utf8_lossy(bytes).to_string()),
                    Location::at(source.source(), source.resource(), None, None, None),
                ))
            },
        ),
    )
    .with_format_list(&["yml", "yaml"]);
    assert_eq!(
        parser.supported_format_list(),
        vec!["yml".to_string(), "yaml".to_string()]
    );
}
