use tanzim::{
    loader::closure::Closure as LoaderClosure,
    merge::{DeepMerge, LastWins},
    multi::{Error as MultiError, PipelineMulti, PipelineMultiBuilder},
    parser::closure::Closure as ParserClosure,
    single::{Error as SingleError, PipelineSingle, PipelineSingleBuilder},
    validate::SchemaValue,
};
use tanzim_load::{Error as LoadError, Payload};
use tanzim_parse::LocatedValue;
use tanzim_source::SourceBuilder;
use tanzim_value::{Location, Value};

fn txt_parser() -> ParserClosure {
    ParserClosure::new(
        "mock",
        "txt",
        Box::new(|source, bytes| {
            Ok(LocatedValue::new(
                Value::String(String::from_utf8_lossy(bytes).to_string()),
                Location::at(source.source(), source.resource(), None, None, None),
            ))
        }),
    )
}

fn auto_txt_parser() -> ParserClosure {
    txt_parser().with_validator(Box::new(|bytes| Some(!bytes.is_empty())))
}

fn mock_loader(content: &'static [u8], name: Option<&str>) -> LoaderClosure {
    let maybe_name = name.map(str::to_string);
    LoaderClosure::new(
        "mock",
        move |source| {
            Ok(vec![Payload {
                source: source.clone(),
                maybe_name: maybe_name.clone(),
                maybe_format: Some("txt".into()),
                content: content.to_vec(),
            }])
        },
        "mock",
    )
}

fn dual_loader() -> LoaderClosure {
    LoaderClosure::new(
        "mock",
        |source| {
            Ok(vec![
                Payload {
                    source: source.clone(),
                    maybe_name: Some("alpha".into()),
                    maybe_format: Some("txt".into()),
                    content: b"alpha-value".to_vec(),
                },
                Payload {
                    source: source.clone(),
                    maybe_name: Some("beta".into()),
                    maybe_format: Some("txt".into()),
                    content: b"beta-value".to_vec(),
                },
            ])
        },
        "mock",
    )
}

fn failing_loader() -> LoaderClosure {
    LoaderClosure::new(
        "mock",
        |_| {
            Err(LoadError::InvalidResource {
                loader: "mock".into(),
                resource: "bad".into(),
                reason: "boom".into(),
            })
        },
        "mock",
    )
}

fn schema_from_json(json: &str) -> Value {
    let schema: SchemaValue = serde_json::from_str(json).unwrap();
    schema.into_value()
}

fn build_single() -> PipelineSingle {
    PipelineSingleBuilder::new()
        .with_source("mock:one")
        .unwrap()
        .with_loader(mock_loader(b"hello", Some("app")))
        .with_parser(txt_parser())
        .with_merger(LastWins)
        .build()
        .unwrap()
}

fn expect_single_build_error(builder: PipelineSingleBuilder, expected: SingleError) {
    match builder.build() {
        Ok(_) => panic!("expected build error"),
        Err(error) => assert!(
            matches!(error, ref e if std::mem::discriminant(e) == std::mem::discriminant(&expected))
        ),
    }
}

fn expect_multi_build_error(builder: PipelineMultiBuilder, expected: MultiError) {
    match builder.build() {
        Ok(_) => panic!("expected build error"),
        Err(error) => assert!(
            matches!(error, ref e if std::mem::discriminant(e) == std::mem::discriminant(&expected))
        ),
    }
}

#[test]
fn single_build_requires_loaders_parsers_and_merger() {
    let parser = txt_parser();
    expect_single_build_error(
        PipelineSingleBuilder::new()
            .with_parser(parser)
            .with_merger(LastWins),
        SingleError::NoLoaders,
    );
    expect_single_build_error(
        PipelineSingleBuilder::new()
            .with_loader(mock_loader(b"x", None))
            .with_merger(LastWins),
        SingleError::NoParsers,
    );
    expect_single_build_error(
        PipelineSingleBuilder::new()
            .with_loader(mock_loader(b"x", None))
            .with_parser(txt_parser()),
        SingleError::NoMerger,
    );
}

#[test]
fn single_rejects_invalid_source_string() {
    match PipelineSingleBuilder::new().with_source("env(prefix=)") {
        Ok(_) => panic!("expected source parse error"),
        Err(error) => assert!(matches!(error, SingleError::Source(_))),
    }
}

#[test]
fn single_load_errors_when_no_loader_matches() {
    let pipeline = PipelineSingleBuilder::new()
        .with_source("other:path")
        .unwrap()
        .with_loader(mock_loader(b"x", None))
        .with_parser(txt_parser())
        .with_merger(LastWins)
        .build()
        .unwrap();
    match pipeline.load() {
        Ok(_) => panic!("expected load error"),
        Err(error) => assert!(matches!(error, SingleError::NoLoader { .. })),
    }
}

#[test]
fn single_load_skips_errors_when_source_ignores_them() {
    let pipeline = PipelineSingleBuilder::new()
        .with_source("mock(on_error=(load=skip)):bad")
        .unwrap()
        .with_loader(failing_loader())
        .with_parser(txt_parser())
        .with_merger(LastWins)
        .build()
        .unwrap();
    let loaded = pipeline.load().unwrap();
    assert!(loaded.is_empty());
}

#[test]
fn single_parse_uses_explicit_format() {
    let pipeline = build_single();
    let loaded = pipeline.load().unwrap();
    let parsed = pipeline.parse(&loaded).unwrap();
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].1.value().as_string().unwrap(), "hello");
}

#[test]
fn single_parse_auto_detects_format() {
    let pipeline = PipelineSingleBuilder::new()
        .with_source("mock:one")
        .unwrap()
        .with_loader(LoaderClosure::new(
            "mock",
            |source| {
                Ok(vec![Payload {
                    source: source.clone(),
                    maybe_name: Some("app".into()),
                    maybe_format: None,
                    content: b"auto".to_vec(),
                }])
            },
            "mock",
        ))
        .with_parser(auto_txt_parser())
        .with_merger(LastWins)
        .build()
        .unwrap();
    let loaded = pipeline.load().unwrap();
    let parsed = pipeline.parse(&loaded).unwrap();
    assert_eq!(parsed[0].1.value().as_string().unwrap(), "auto");
}

#[test]
fn single_parse_errors_when_no_parser_matches() {
    let pipeline = PipelineSingleBuilder::new()
        .with_source("mock:one")
        .unwrap()
        .with_loader(LoaderClosure::new(
            "mock",
            |source| {
                Ok(vec![Payload {
                    source: source.clone(),
                    maybe_name: Some("app".into()),
                    maybe_format: Some("missing".into()),
                    content: b"x".to_vec(),
                }])
            },
            "mock",
        ))
        .with_parser(txt_parser())
        .with_merger(LastWins)
        .build()
        .unwrap();
    let loaded = pipeline.load().unwrap();
    match pipeline.parse(&loaded) {
        Ok(_) => panic!("expected parse error"),
        Err(error) => assert!(matches!(error, SingleError::NoParser { .. })),
    }
}

#[test]
fn single_unify_empty_merge_returns_empty_map() {
    let pipeline = build_single();
    let merged = pipeline.merge(&[]).unwrap();
    let (payloads, value) = pipeline.unify(&merged).unwrap();
    assert!(payloads.is_empty());
    assert!(value.value().as_map().unwrap().entries().is_empty());
}

#[test]
fn single_unify_collapses_named_groups_with_last_wins() {
    let pipeline = PipelineSingleBuilder::new()
        .with_source("mock:one")
        .unwrap()
        .with_loader(dual_loader())
        .with_parser(txt_parser())
        .with_merger(LastWins)
        .build()
        .unwrap();
    let loaded = pipeline.load().unwrap();
    let parsed = pipeline.parse(&loaded).unwrap();
    let merged = pipeline.merge(&parsed).unwrap();
    let (_, value) = pipeline.unify(&merged).unwrap();
    assert_eq!(value.value().as_string().unwrap(), "beta-value");
}

#[test]
fn single_run_executes_full_pipeline() {
    let pipeline = build_single();
    let (_, value) = pipeline.run().unwrap();
    assert_eq!(value.value().as_string().unwrap(), "hello");
}

#[test]
fn single_validate_without_schema_is_noop() {
    let pipeline = build_single();
    let mut value = LocatedValue::new(
        Value::String("hello".into()),
        Location::at("mock", "one", None, None, None),
    );
    pipeline.validate(&mut value).unwrap();
}

#[test]
fn single_validate_rejects_invalid_schema() {
    let pipeline = PipelineSingleBuilder::new()
        .with_source("mock:one")
        .unwrap()
        .with_loader(mock_loader(b"hello", Some("app")))
        .with_parser(txt_parser())
        .with_merger(LastWins)
        .with_schema(schema_from_json(r#"{"type": "nope"}"#))
        .build()
        .unwrap();
    let mut value = LocatedValue::new(
        Value::String("hello".into()),
        Location::at("mock", "one", None, None, None),
    );
    match pipeline.validate(&mut value) {
        Ok(()) => panic!("expected schema error"),
        Err(error) => assert!(matches!(error, SingleError::Schema { .. })),
    }
}

#[test]
fn single_validate_rejects_bad_configuration() {
    let pipeline = PipelineSingleBuilder::new()
        .with_source("mock:one")
        .unwrap()
        .with_loader(mock_loader(b"hello", Some("app")))
        .with_parser(txt_parser())
        .with_merger(LastWins)
        .with_schema(schema_from_json(r#"{"type": "integer"}"#))
        .build()
        .unwrap();
    let mut value = LocatedValue::new(
        Value::String("hello".into()),
        Location::at("mock", "one", None, None, None),
    );
    match pipeline.validate(&mut value) {
        Ok(()) => panic!("expected validation error"),
        Err(error) => assert!(matches!(error, SingleError::Validate { .. })),
    }
}

#[test]
fn single_pipeline_accessors_and_included_helpers() {
    let pipeline = PipelineSingleBuilder::new()
        .with_source("mock:one")
        .unwrap()
        .with_loader(mock_loader(b"x", None))
        .with_parser(txt_parser())
        .with_merger(LastWins)
        .with_included_loaders()
        .with_included_parsers()
        .build()
        .unwrap();
    assert_eq!(pipeline.sources().len(), 1);
    assert!(!pipeline.loaders().is_empty());
    assert!(!pipeline.parsers().is_empty());

    let pipeline = pipeline
        .with_source("mock:two")
        .unwrap()
        .with_loader(mock_loader(b"y", None))
        .with_parser(txt_parser())
        .with_merger(DeepMerge)
        .set_included_loaders()
        .set_included_parsers();
    assert_eq!(pipeline.sources().len(), 2);
}

fn build_multi() -> PipelineMulti {
    PipelineMultiBuilder::new()
        .with_source("mock:one")
        .unwrap()
        .with_loader(mock_loader(b"hello", Some("app")))
        .with_parser(txt_parser())
        .with_merger(DeepMerge)
        .build()
        .unwrap()
}

#[test]
fn multi_build_requires_components() {
    expect_multi_build_error(
        PipelineMultiBuilder::new()
            .with_parser(txt_parser())
            .with_merger(DeepMerge),
        MultiError::NoLoaders,
    );
}

#[test]
fn multi_run_returns_named_entries() {
    let pipeline = build_multi();
    let merged = pipeline.run().unwrap();
    assert!(merged.contains_key(&Some("app".into())));
}

#[test]
fn multi_validate_warns_when_schema_has_no_matching_entry() {
    let pipeline = PipelineMultiBuilder::new()
        .with_source("mock:one")
        .unwrap()
        .with_loader(mock_loader(b"hello", Some("app")))
        .with_parser(txt_parser())
        .with_merger(DeepMerge)
        .with_schema(
            Some("missing".into()),
            schema_from_json(r#"{"type": "string"}"#),
        )
        .build()
        .unwrap();
    let loaded = pipeline.load().unwrap();
    let parsed = pipeline.parse(&loaded).unwrap();
    let mut merged = pipeline.merge(&parsed).unwrap();
    pipeline.validate(&mut merged).unwrap();
}

#[test]
fn multi_validate_rejects_bad_configuration() {
    let pipeline = PipelineMultiBuilder::new()
        .with_source("mock:one")
        .unwrap()
        .with_loader(mock_loader(b"hello", Some("app")))
        .with_parser(txt_parser())
        .with_merger(DeepMerge)
        .with_schema(
            Some("app".into()),
            schema_from_json(r#"{"type": "integer"}"#),
        )
        .build()
        .unwrap();
    let loaded = pipeline.load().unwrap();
    let parsed = pipeline.parse(&loaded).unwrap();
    let mut merged = pipeline.merge(&parsed).unwrap();
    match pipeline.validate(&mut merged) {
        Ok(()) => panic!("expected validation error"),
        Err(error) => assert!(matches!(error, MultiError::Validate { .. })),
    }
}

#[test]
fn multi_with_schemas_registers_multiple_entries() {
    let mut schemas = tanzim::multi::Schemas::new();
    schemas.insert(
        Some("app".into()),
        schema_from_json(r#"{"type": "string"}"#),
    );
    let pipeline = PipelineMultiBuilder::new()
        .with_source("mock:one")
        .unwrap()
        .with_loader(mock_loader(b"hello", Some("app")))
        .with_parser(txt_parser())
        .with_merger(DeepMerge)
        .with_schemas(schemas)
        .build()
        .unwrap();
    assert_eq!(pipeline.schemas().len(), 1);
}

struct FailMerge;

impl tanzim::merge::Merge for FailMerge {
    fn merge(
        &self,
        _parsed_list: &[(Payload, LocatedValue)],
    ) -> Result<tanzim::merge::Merged, tanzim::merge::Error> {
        Err(tanzim::merge::Error::Other(
            std::io::Error::other("merge failed").into(),
        ))
    }
}

fn failing_parser() -> ParserClosure {
    ParserClosure::new(
        "bad",
        "txt",
        Box::new(|source, _| {
            Err(tanzim_value::Error::InvalidUtf8 {
                location: Box::new(Location::in_source(source.clone(), None, None, None)),
            })
        }),
    )
}

#[test]
fn single_load_propagates_loader_error() {
    let pipeline = PipelineSingleBuilder::new()
        .with_source("mock:bad")
        .unwrap()
        .with_loader(failing_loader())
        .with_parser(txt_parser())
        .with_merger(LastWins)
        .build()
        .unwrap();
    match pipeline.load() {
        Ok(_) => panic!("expected load error"),
        Err(error) => assert!(matches!(error, SingleError::Load(_))),
    }
}

#[test]
fn single_parse_skips_errors_when_payload_source_ignores_them() {
    let pipeline = PipelineSingleBuilder::new()
        .with_source("mock(on_error=(parse=skip)):one")
        .unwrap()
        .with_loader(LoaderClosure::new(
            "mock",
            |source| {
                Ok(vec![Payload {
                    source: source.clone(),
                    maybe_name: Some("app".into()),
                    maybe_format: Some("txt".into()),
                    content: b"x".to_vec(),
                }])
            },
            "mock",
        ))
        .with_parser(failing_parser())
        .with_merger(LastWins)
        .build()
        .unwrap();
    let loaded = pipeline.load().unwrap();
    let parsed = pipeline.parse(&loaded).unwrap();
    assert!(parsed.is_empty());
}

#[test]
fn single_parse_propagates_parser_error() {
    let pipeline = PipelineSingleBuilder::new()
        .with_source("mock:one")
        .unwrap()
        .with_loader(mock_loader(b"x", Some("app")))
        .with_parser(failing_parser())
        .with_merger(LastWins)
        .build()
        .unwrap();
    let loaded = pipeline.load().unwrap();
    match pipeline.parse(&loaded) {
        Ok(_) => panic!("expected parse error"),
        Err(error) => assert!(matches!(error, SingleError::Parse(_))),
    }
}

#[test]
fn single_merge_propagates_merge_error() {
    let pipeline = PipelineSingleBuilder::new()
        .with_source("mock:one")
        .unwrap()
        .with_loader(mock_loader(b"x", None))
        .with_parser(txt_parser())
        .with_merger(FailMerge)
        .build()
        .unwrap();
    let loaded = pipeline.load().unwrap();
    let parsed = pipeline.parse(&loaded).unwrap();
    match pipeline.merge(&parsed) {
        Ok(_) => panic!("expected merge error"),
        Err(error) => assert!(matches!(error, SingleError::Merge(_))),
    }
}

#[test]
fn single_unify_with_deep_merge_combines_map_groups() {
    let pipeline = PipelineSingleBuilder::new()
        .with_source("mock:one")
        .unwrap()
        .with_loader(LoaderClosure::new(
            "mock",
            |source| {
                Ok(vec![
                    Payload {
                        source: source.clone(),
                        maybe_name: Some("alpha".into()),
                        maybe_format: Some("json".into()),
                        content: br#"{"alpha":"alpha-value"}"#.to_vec(),
                    },
                    Payload {
                        source: source.clone(),
                        maybe_name: Some("beta".into()),
                        maybe_format: Some("json".into()),
                        content: br#"{"beta":"beta-value"}"#.to_vec(),
                    },
                ])
            },
            "mock",
        ))
        .with_parser(tanzim::parser::json::Json::new())
        .with_merger(DeepMerge)
        .build()
        .unwrap();
    let loaded = pipeline.load().unwrap();
    let parsed = pipeline.parse(&loaded).unwrap();
    let merged = pipeline.merge(&parsed).unwrap();
    let (_, value) = pipeline.unify(&merged).unwrap();
    let map = value.value().as_map().unwrap();
    assert_eq!(
        map.get("alpha").unwrap().value().as_string().unwrap(),
        "alpha-value"
    );
    assert_eq!(
        map.get("beta").unwrap().value().as_string().unwrap(),
        "beta-value"
    );
}

#[test]
fn single_unify_last_wins_prefers_unnamed_bucket() {
    let pipeline = PipelineSingleBuilder::new()
        .with_source("mock:one")
        .unwrap()
        .with_loader(LoaderClosure::new(
            "mock",
            |source| {
                Ok(vec![
                    Payload {
                        source: source.clone(),
                        maybe_name: Some("alpha".into()),
                        maybe_format: Some("txt".into()),
                        content: b"named".to_vec(),
                    },
                    Payload {
                        source: source.clone(),
                        maybe_name: None,
                        maybe_format: Some("txt".into()),
                        content: b"unnamed".to_vec(),
                    },
                ])
            },
            "mock",
        ))
        .with_parser(txt_parser())
        .with_merger(LastWins)
        .build()
        .unwrap();
    let loaded = pipeline.load().unwrap();
    let parsed = pipeline.parse(&loaded).unwrap();
    let merged = pipeline.merge(&parsed).unwrap();
    let (_, value) = pipeline.unify(&merged).unwrap();
    assert_eq!(value.value().as_string().unwrap(), "unnamed");
}

#[test]
fn single_run_with_valid_schema_coerces_configuration() {
    let pipeline = PipelineSingleBuilder::new()
        .with_source("mock:one")
        .unwrap()
        .with_loader(mock_loader(b"42", Some("app")))
        .with_parser(txt_parser())
        .with_merger(LastWins)
        .with_schema(schema_from_json(r#"{"type": "integer"}"#))
        .build()
        .unwrap();
    let (_, value) = pipeline.run().unwrap();
    assert_eq!(*value.value(), Value::Int(42));
}

#[test]
fn single_builder_default_matches_new() {
    let from_default = PipelineSingleBuilder::default()
        .with_loader(mock_loader(b"x", None))
        .with_parser(txt_parser())
        .with_merger(LastWins)
        .build();
    let from_new = PipelineSingleBuilder::new()
        .with_loader(mock_loader(b"x", None))
        .with_parser(txt_parser())
        .with_merger(LastWins)
        .build();
    assert!(from_default.is_ok());
    assert!(from_new.is_ok());
}

#[test]
fn single_schema_accessor_returns_registered_schema() {
    let schema = schema_from_json(r#"{"type": "string"}"#);
    let pipeline = PipelineSingleBuilder::new()
        .with_source("mock:one")
        .unwrap()
        .with_loader(mock_loader(b"x", None))
        .with_parser(txt_parser())
        .with_merger(LastWins)
        .with_schema(schema.clone())
        .build()
        .unwrap();
    assert_eq!(pipeline.schema(), Some(&schema));
}

#[test]
fn multi_build_requires_parsers_and_merger() {
    expect_multi_build_error(
        PipelineMultiBuilder::new()
            .with_loader(mock_loader(b"x", None))
            .with_merger(DeepMerge),
        MultiError::NoParsers,
    );
    expect_multi_build_error(
        PipelineMultiBuilder::new()
            .with_loader(mock_loader(b"x", None))
            .with_parser(txt_parser()),
        MultiError::NoMerger,
    );
}

#[test]
fn multi_load_and_parse_error_paths() {
    let pipeline = PipelineMultiBuilder::new()
        .with_source("mock:bad")
        .unwrap()
        .with_loader(failing_loader())
        .with_parser(txt_parser())
        .with_merger(DeepMerge)
        .build()
        .unwrap();
    match pipeline.load() {
        Ok(_) => panic!("expected load error"),
        Err(error) => assert!(matches!(error, MultiError::Load(_))),
    }

    let pipeline = PipelineMultiBuilder::new()
        .with_source("mock:one")
        .unwrap()
        .with_loader(mock_loader(b"x", Some("app")))
        .with_parser(failing_parser())
        .with_merger(DeepMerge)
        .build()
        .unwrap();
    let loaded = pipeline.load().unwrap();
    match pipeline.parse(&loaded) {
        Ok(_) => panic!("expected parse error"),
        Err(error) => assert!(matches!(error, MultiError::Parse(_))),
    }
}

#[test]
fn multi_validate_rejects_invalid_schema() {
    let pipeline = PipelineMultiBuilder::new()
        .with_source("mock:one")
        .unwrap()
        .with_loader(mock_loader(b"hello", Some("app")))
        .with_parser(txt_parser())
        .with_merger(DeepMerge)
        .with_schema(Some("app".into()), schema_from_json(r#"{"type": "nope"}"#))
        .build()
        .unwrap();
    let loaded = pipeline.load().unwrap();
    let parsed = pipeline.parse(&loaded).unwrap();
    let mut merged = pipeline.merge(&parsed).unwrap();
    match pipeline.validate(&mut merged) {
        Ok(()) => panic!("expected schema error"),
        Err(error) => assert!(matches!(error, MultiError::Schema { .. })),
    }
}

#[test]
fn multi_validate_succeeds_for_matching_schema() {
    let pipeline = PipelineMultiBuilder::new()
        .with_source("mock:one")
        .unwrap()
        .with_loader(mock_loader(b"hello", Some("app")))
        .with_parser(txt_parser())
        .with_merger(DeepMerge)
        .with_schema(
            Some("app".into()),
            schema_from_json(r#"{"type": "string"}"#),
        )
        .build()
        .unwrap();
    let mut merged = pipeline.run().unwrap();
    pipeline.validate(&mut merged).unwrap();
}

#[test]
fn multi_pipeline_accessors_and_mutators() {
    let mut pipeline = build_multi();
    assert_eq!(pipeline.sources().len(), 1);
    assert!(!pipeline.loaders().is_empty());
    assert!(!pipeline.parsers().is_empty());
    pipeline.sources_mut().push(
        SourceBuilder::new()
            .with_source("mock")
            .with_resource("two")
            .build()
            .unwrap(),
    );
    pipeline
        .loaders_mut()
        .push(Box::new(mock_loader(b"z", None)));
    pipeline.parsers_mut().push(Box::new(txt_parser()));
    assert_eq!(pipeline.sources().len(), 2);
    let _ = pipeline.merger();
    let _ = pipeline.merger_mut();
    pipeline.schemas_mut().insert(
        Some("extra".into()),
        schema_from_json(r#"{"type": "string"}"#),
    );
    assert_eq!(pipeline.schemas().len(), 1);
}

#[test]
fn multi_builder_default_matches_new() {
    let from_default = PipelineMultiBuilder::default()
        .with_loader(mock_loader(b"x", None))
        .with_parser(txt_parser())
        .with_merger(DeepMerge)
        .build();
    let from_new = PipelineMultiBuilder::new()
        .with_loader(mock_loader(b"x", None))
        .with_parser(txt_parser())
        .with_merger(DeepMerge)
        .build();
    assert!(from_default.is_ok());
    assert!(from_new.is_ok());
}

fn init_logging() {
    cfg_if::cfg_if! {
        if #[cfg(feature = "tracing")] {
            let _ = tracing_subscriber::fmt()
                .with_max_level(tracing::Level::TRACE)
                .without_time()
                .try_init();
        } else if #[cfg(feature = "logging")] {
            let _ = env_logger::builder()
                .filter_level(log::LevelFilter::max())
                .format_timestamp(None)
                .is_test(true)
                .try_init();
        }
    }
}

#[test]
fn single_run_with_logging_enabled_exercises_pipeline_stages() {
    init_logging();
    let pipeline = build_single();
    let (_, value) = pipeline.run().unwrap();
    assert_eq!(value.value().as_string().unwrap(), "hello");
}

#[test]
fn multi_parse_skips_errors_when_payload_source_ignores_them() {
    init_logging();
    let pipeline = PipelineMultiBuilder::new()
        .with_source("mock(on_error=(parse=skip)):one")
        .unwrap()
        .with_loader(LoaderClosure::new(
            "mock",
            |source| {
                Ok(vec![Payload {
                    source: source.clone(),
                    maybe_name: Some("app".into()),
                    maybe_format: Some("txt".into()),
                    content: b"x".to_vec(),
                }])
            },
            "mock",
        ))
        .with_parser(failing_parser())
        .with_merger(DeepMerge)
        .build()
        .unwrap();
    let loaded = pipeline.load().unwrap();
    let parsed = pipeline.parse(&loaded).unwrap();
    assert!(parsed.is_empty());
}

#[test]
fn multi_load_skips_errors_when_source_ignores_them() {
    let pipeline = PipelineMultiBuilder::new()
        .with_source("mock(on_error=(load=skip)):bad")
        .unwrap()
        .with_loader(failing_loader())
        .with_parser(txt_parser())
        .with_merger(DeepMerge)
        .build()
        .unwrap();
    let loaded = pipeline.load().unwrap();
    assert!(loaded.is_empty());
}

#[test]
fn multi_merge_propagates_merge_error() {
    let pipeline = PipelineMultiBuilder::new()
        .with_source("mock:one")
        .unwrap()
        .with_loader(mock_loader(b"x", None))
        .with_parser(txt_parser())
        .with_merger(FailMerge)
        .build()
        .unwrap();
    let loaded = pipeline.load().unwrap();
    let parsed = pipeline.parse(&loaded).unwrap();
    match pipeline.merge(&parsed) {
        Ok(_) => panic!("expected merge error"),
        Err(error) => assert!(matches!(error, MultiError::Merge(_))),
    }
}

#[test]
fn single_schema_mut_and_sources_mut() {
    let mut pipeline = build_single();
    pipeline.sources_mut().clear();
    assert!(pipeline.sources().is_empty());
    pipeline
        .schema_mut()
        .replace(schema_from_json(r#"{"type": "string"}"#));
    assert!(pipeline.schema().is_some());
}
