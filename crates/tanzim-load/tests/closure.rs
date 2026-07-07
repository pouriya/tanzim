use tanzim_load::{Load, Payload, Source, closure::Closure};
use tanzim_source::SourceBuilder;

#[test]
fn closure_loader_delegates_to_function() {
    let loader = Closure::new(
        "custom",
        |source: Source| {
            let resource = source.resource().to_string();
            Ok(vec![Payload {
                source,
                maybe_name: Some("demo".into()),
                maybe_format: Some("txt".into()),
                content: resource.into_bytes(),
            }])
        },
        "custom",
    );
    assert_eq!(loader.name(), "custom");
    assert_eq!(loader.supported_source_list(), vec!["custom".to_string()]);
    let source = SourceBuilder::new()
        .with_source("custom")
        .with_resource("hello")
        .build()
        .unwrap();
    let loaded = loader.load(source).unwrap();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].content, b"hello");
}

#[test]
fn closure_loader_with_name_and_supported_source_list() {
    let loader = Closure::new("old", |_source: Source| Ok(vec![]), "mock")
        .with_name("custom")
        .with_supported_source_list(vec!["mock", "other"]);
    assert_eq!(loader.name(), "custom");
    assert_eq!(
        loader.supported_source_list(),
        vec!["mock".to_string(), "other".to_string()]
    );
    let source = SourceBuilder::new().with_source("other").build().unwrap();
    assert!(loader.load(source).unwrap().is_empty());
}
