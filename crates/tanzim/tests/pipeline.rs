use tanzim::{
    Config, ConfigBuilder, Sources,
    config::Error as SingleError,
    loader::closure::Closure as LoaderClosure,
    merger::{DeepMerge, LastWins},
    parser::closure::Closure as ParserClosure,
    pipeline::{Error as MultiError, Pipeline, PipelineBuilder},
    source::Source,
    validator::SchemaValue,
};
use tanzim_load::{Error as LoadError, Payload};
use tanzim_parse::LocatedValue;
use tanzim_value::{Location, Value};

/// Compile-time proof that the public builder and value types are `Send + Sync`, so a built
/// `Config`/`Pipeline` can be moved across threads and shared via `Arc` (e.g. as axum/actix handler
/// state). If any trait object stored inside were not thread-safe, this would fail to compile.
#[test]
fn public_types_are_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Config>();
    assert_send_sync::<Pipeline>();
    assert_send_sync::<ConfigBuilder<Sources>>();
    assert_send_sync::<ConfigBuilder<tanzim::Plan>>();
    assert_send_sync::<PipelineBuilder<Sources>>();
    assert_send_sync::<PipelineBuilder<tanzim::Plan>>();
    assert_send_sync::<tanzim::merger::plan::MergePlan>();
}

fn txt_parser() -> ParserClosure {
    ParserClosure::new(
        "mock",
        "txt",
        Box::new(|source, bytes, _other_source_list| {
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

fn build_single() -> ConfigBuilder<Sources> {
    Config::builder()
        .with_source("mock:one")
        .unwrap()
        .with_loader(mock_loader(b"hello", Some("app")))
        .with_parser(txt_parser())
        .with_merger(LastWins)
}

#[test]
fn single_reports_missing_loaders_and_parsers_at_run_time() {
    let no_loaders = Config::builder()
        .with_parser(txt_parser())
        .with_merger(LastWins);
    assert!(matches!(no_loaders.run(), Err(SingleError::NoLoaders)));

    let no_parsers = Config::builder()
        .with_source("mock:one")
        .unwrap()
        .with_loader(mock_loader(b"x", None))
        .with_merger(LastWins);
    assert!(matches!(no_parsers.run(), Err(SingleError::NoParsers)));

    // No explicit merger: the merge stage now defaults to `LastWins`, so the pipeline runs.
    let no_merger = Config::builder()
        .with_source("mock:one")
        .unwrap()
        .with_loader(mock_loader(b"x", None))
        .with_parser(txt_parser());
    assert!(no_merger.run().is_ok());
}

#[test]
fn single_default_includes_loaders_and_parsers_but_no_merger() {
    let pipeline = Config::builder()
        .with_default_loaders()
        .with_default_parsers();
    assert!(!pipeline.loaders().is_empty());
    assert!(!pipeline.parsers().is_empty());
    assert!(pipeline.merger().is_none());
    // With loaders and parsers but no merger and no sources, the merge stage defaults to `LastWins`
    // and the pipeline runs, yielding an empty unified entry.
    let entry = Config::builder()
        .with_default_loaders()
        .with_default_parsers()
        .run()
        .unwrap();
    assert!(entry.value().value().as_map().unwrap().is_empty());
}

#[test]
fn single_empty_registers_nothing() {
    let pipeline = Config::builder();
    assert!(pipeline.loaders().is_empty());
    assert!(pipeline.parsers().is_empty());
    assert!(pipeline.merger().is_none());
    assert!(matches!(pipeline.run(), Err(SingleError::NoLoaders)));
}

#[test]
fn source_parse_rejects_invalid_string() {
    assert!(Source::parse("env(prefix=)").is_err());
}

#[test]
fn single_load_errors_when_no_loader_matches() {
    let pipeline = Config::builder()
        .with_source("other:path")
        .unwrap()
        .with_loader(mock_loader(b"x", None))
        .with_parser(txt_parser())
        .with_merger(LastWins)
        .build();
    match pipeline.stages().load() {
        Ok(_) => panic!("expected load error"),
        Err(error) => assert!(matches!(error, SingleError::NoLoader { .. })),
    }
}

#[test]
fn single_load_skips_errors_when_source_ignores_them() {
    let pipeline = Config::builder()
        .with_source("mock(on_error=(load=skip)):bad")
        .unwrap()
        .with_loader(failing_loader())
        .with_parser(txt_parser())
        .with_merger(LastWins)
        .build();
    let loaded = pipeline.stages().load().unwrap();
    assert!(loaded.is_empty());
}

#[test]
fn single_parse_uses_explicit_format() {
    let pipeline = build_single().build();
    let stages = pipeline.stages();
    let loaded = stages.load().unwrap();
    let parsed = stages.parse(&loaded).unwrap();
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].value().value().as_string().unwrap(), "hello");
}

#[test]
fn single_parse_auto_detects_format() {
    let pipeline = Config::builder()
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
        .build();
    let stages = pipeline.stages();
    let loaded = stages.load().unwrap();
    let parsed = stages.parse(&loaded).unwrap();
    assert_eq!(parsed[0].value().value().as_string().unwrap(), "auto");
}

#[test]
fn single_parse_errors_when_no_parser_matches() {
    let pipeline = Config::builder()
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
        .build();
    let stages = pipeline.stages();
    let loaded = stages.load().unwrap();
    match stages.parse(&loaded) {
        Ok(_) => panic!("expected parse error"),
        Err(error) => assert!(matches!(error, SingleError::NoParser { .. })),
    }
}

#[test]
fn single_unify_empty_merge_returns_empty_map() {
    let pipeline = build_single().build();
    let stages = pipeline.stages();
    let merged = stages.merge(&[]).unwrap();
    let entry = stages.unify(&merged).unwrap();
    assert!(entry.payloads().is_empty());
    assert!(entry.value().value().as_map().unwrap().entries().is_empty());
}

#[test]
fn single_unify_collapses_named_groups_with_last_wins() {
    let pipeline = Config::builder()
        .with_source("mock:one")
        .unwrap()
        .with_loader(dual_loader())
        .with_parser(txt_parser())
        .with_merger(LastWins)
        .build();
    let stages = pipeline.stages();
    let loaded = stages.load().unwrap();
    let parsed = stages.parse(&loaded).unwrap();
    let merged = stages.merge(&parsed).unwrap();
    let entry = stages.unify(&merged).unwrap();
    assert_eq!(entry.value().value().as_string().unwrap(), "beta-value");
}

#[test]
fn single_run_executes_full_pipeline() {
    let entry = build_single().run().unwrap();
    assert_eq!(entry.value().value().as_string().unwrap(), "hello");
}

#[test]
fn single_validate_without_schema_is_noop() {
    let pipeline = build_single().build();
    let mut value = LocatedValue::new(
        Value::String("hello".into()),
        Location::at("mock", "one", None, None, None),
    );
    pipeline.stages().validate(&mut value).unwrap();
}

#[test]
fn single_validate_rejects_invalid_schema() {
    let pipeline = build_single()
        .with_schema(schema_from_json(r#"{"type": "nope"}"#))
        .build();
    let mut value = LocatedValue::new(
        Value::String("hello".into()),
        Location::at("mock", "one", None, None, None),
    );
    match pipeline.stages().validate(&mut value) {
        Ok(()) => panic!("expected schema error"),
        Err(error) => assert!(matches!(error, SingleError::Schema { .. })),
    }
}

#[test]
fn single_validate_rejects_bad_configuration() {
    let pipeline = build_single()
        .with_schema(schema_from_json(r#"{"type": "integer"}"#))
        .build();
    let mut value = LocatedValue::new(
        Value::String("hello".into()),
        Location::at("mock", "one", None, None, None),
    );
    match pipeline.stages().validate(&mut value) {
        Ok(()) => panic!("expected validation error"),
        Err(error) => assert!(matches!(error, SingleError::Validate { .. })),
    }
}

#[test]
fn single_builder_accessors_and_default_helpers() {
    let pipeline = Config::builder()
        .with_source("mock:one")
        .unwrap()
        .with_loader(mock_loader(b"x", None))
        .with_parser(txt_parser())
        .with_merger(LastWins)
        .with_default_loaders()
        .with_default_parsers();
    assert_eq!(pipeline.sources().count(), 1);
    assert!(!pipeline.loaders().is_empty());
    assert!(!pipeline.parsers().is_empty());

    let pipeline = pipeline
        .with_source("mock:two")
        .unwrap()
        .with_loader(mock_loader(b"y", None))
        .with_parser(txt_parser())
        .with_merger(DeepMerge::new())
        .set_default_loaders()
        .set_default_parsers();
    assert_eq!(pipeline.sources().count(), 2);
}

fn build_multi() -> PipelineBuilder<Sources> {
    Pipeline::builder()
        .with_source("mock:one")
        .unwrap()
        .with_loader(mock_loader(b"hello", Some("app")))
        .with_parser(txt_parser())
        .with_merger(DeepMerge::new())
}

#[test]
fn multi_reports_missing_components_at_run_time() {
    let no_loaders = Pipeline::builder()
        .with_parser(txt_parser())
        .with_merger(DeepMerge::new());
    assert!(matches!(no_loaders.run(), Err(MultiError::NoLoaders)));

    let no_parsers = Pipeline::builder()
        .with_source("mock:one")
        .unwrap()
        .with_loader(mock_loader(b"x", None))
        .with_merger(DeepMerge::new());
    assert!(matches!(no_parsers.run(), Err(MultiError::NoParsers)));

    // No explicit merger: the merge stage now defaults to `LastWins`, so the pipeline runs.
    let no_merger = Pipeline::builder()
        .with_source("mock:one")
        .unwrap()
        .with_loader(mock_loader(b"x", None))
        .with_parser(txt_parser());
    assert!(no_merger.run().is_ok());
}

#[test]
fn multi_default_and_empty() {
    // No sources and no merger: defaults to `LastWins`, runs and yields an empty entry map.
    assert!(
        Pipeline::builder()
            .with_default_loaders()
            .with_default_parsers()
            .run()
            .unwrap()
            .is_empty()
    );
    assert!(matches!(
        Pipeline::builder().run(),
        Err(MultiError::NoLoaders)
    ));
}

#[test]
fn multi_run_returns_named_entries() {
    let merged = build_multi().run().unwrap();
    assert!(merged.contains_key(&Some("app".into())));
}

#[test]
fn multi_validate_warns_when_schema_has_no_matching_entry() {
    let pipeline = build_multi()
        .with_schema(
            Some("missing".into()),
            schema_from_json(r#"{"type": "string"}"#),
        )
        .build();
    let stages = pipeline.stages();
    let loaded = stages.load().unwrap();
    let parsed = stages.parse(&loaded).unwrap();
    let mut merged = stages.merge(&parsed).unwrap();
    stages.validate(&mut merged).unwrap();
}

#[test]
fn multi_validate_rejects_bad_configuration() {
    let pipeline = build_multi()
        .with_schema(
            Some("app".into()),
            schema_from_json(r#"{"type": "integer"}"#),
        )
        .build();
    let stages = pipeline.stages();
    let loaded = stages.load().unwrap();
    let parsed = stages.parse(&loaded).unwrap();
    let mut merged = stages.merge(&parsed).unwrap();
    match stages.validate(&mut merged) {
        Ok(()) => panic!("expected validation error"),
        Err(error) => assert!(matches!(error, MultiError::Validate { .. })),
    }
}

#[test]
fn multi_with_schemas_registers_multiple_entries() {
    let mut schemas = tanzim::pipeline::Schemas::new();
    schemas.insert(
        Some("app".into()),
        schema_from_json(r#"{"type": "string"}"#),
    );
    let pipeline = build_multi().with_schemas(schemas);
    assert_eq!(pipeline.schemas().len(), 1);
}

struct FailMerge;

impl tanzim::merger::Merge for FailMerge {
    fn merge(
        &self,
        _parsed_list: &[(Payload, LocatedValue)],
    ) -> Result<tanzim_merge::Merged, tanzim::merger::Error> {
        Err(tanzim::merger::Error::Other(
            std::io::Error::other("merge failed").into(),
        ))
    }
}

fn failing_parser() -> ParserClosure {
    ParserClosure::new(
        "bad",
        "txt",
        Box::new(|source, _, _other_source_list| {
            Err(tanzim_value::Error::InvalidUtf8 {
                location: Box::new(Location::in_source(source.clone(), None, None, None)),
            })
        }),
    )
}

#[test]
fn single_load_propagates_loader_error() {
    let pipeline = Config::builder()
        .with_source("mock:bad")
        .unwrap()
        .with_loader(failing_loader())
        .with_parser(txt_parser())
        .with_merger(LastWins)
        .build();
    match pipeline.stages().load() {
        Ok(_) => panic!("expected load error"),
        Err(error) => assert!(matches!(error, SingleError::Load(_))),
    }
}

#[test]
fn single_parse_skips_errors_when_payload_source_ignores_them() {
    let pipeline = Config::builder()
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
        .build();
    let stages = pipeline.stages();
    let loaded = stages.load().unwrap();
    let parsed = stages.parse(&loaded).unwrap();
    assert!(parsed.is_empty());
}

#[test]
fn single_parse_propagates_parser_error() {
    let pipeline = Config::builder()
        .with_source("mock:one")
        .unwrap()
        .with_loader(mock_loader(b"x", Some("app")))
        .with_parser(failing_parser())
        .with_merger(LastWins)
        .build();
    let stages = pipeline.stages();
    let loaded = stages.load().unwrap();
    match stages.parse(&loaded) {
        Ok(_) => panic!("expected parse error"),
        Err(error) => assert!(matches!(error, SingleError::Parse(_))),
    }
}

#[test]
fn single_merge_propagates_merge_error() {
    let pipeline = Config::builder()
        .with_source("mock:one")
        .unwrap()
        .with_loader(mock_loader(b"x", None))
        .with_parser(txt_parser())
        .with_merger(FailMerge)
        .build();
    let stages = pipeline.stages();
    let loaded = stages.load().unwrap();
    let parsed = stages.parse(&loaded).unwrap();
    match stages.merge(&parsed) {
        Ok(_) => panic!("expected merge error"),
        Err(error) => assert!(matches!(error, SingleError::Merge(_))),
    }
}

#[test]
fn single_unify_with_deep_merge_combines_map_groups() {
    let pipeline = Config::builder()
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
        .with_merger(DeepMerge::new())
        .build();
    let stages = pipeline.stages();
    let loaded = stages.load().unwrap();
    let parsed = stages.parse(&loaded).unwrap();
    let merged = stages.merge(&parsed).unwrap();
    let entry = stages.unify(&merged).unwrap();
    let value = entry.value();
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
    let pipeline = Config::builder()
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
        .build();
    let stages = pipeline.stages();
    let loaded = stages.load().unwrap();
    let parsed = stages.parse(&loaded).unwrap();
    let merged = stages.merge(&parsed).unwrap();
    let entry = stages.unify(&merged).unwrap();
    assert_eq!(entry.value().value().as_string().unwrap(), "unnamed");
}

#[test]
fn single_run_with_valid_schema_coerces_configuration() {
    let pipeline = Config::builder()
        .with_source("mock:one")
        .unwrap()
        .with_loader(mock_loader(b"42", Some("app")))
        .with_parser(txt_parser())
        .with_merger(LastWins)
        .with_schema(schema_from_json(r#"{"type": "integer"}"#));
    let entry = pipeline.run().unwrap();
    assert_eq!(*entry.value().value(), Value::Int(42));
}

#[test]
fn single_schema_accessor_returns_registered_schema() {
    let schema = schema_from_json(r#"{"type": "string"}"#);
    let pipeline = build_single().with_schema(schema.clone());
    assert_eq!(pipeline.schema(), Some(&schema));
}

#[test]
fn multi_load_and_parse_error_paths() {
    let pipeline = Pipeline::builder()
        .with_source("mock:bad")
        .unwrap()
        .with_loader(failing_loader())
        .with_parser(txt_parser())
        .with_merger(DeepMerge::new())
        .build();
    match pipeline.stages().load() {
        Ok(_) => panic!("expected load error"),
        Err(error) => assert!(matches!(error, MultiError::Load(_))),
    }

    let pipeline = Pipeline::builder()
        .with_source("mock:one")
        .unwrap()
        .with_loader(mock_loader(b"x", Some("app")))
        .with_parser(failing_parser())
        .with_merger(DeepMerge::new())
        .build();
    let stages = pipeline.stages();
    let loaded = stages.load().unwrap();
    match stages.parse(&loaded) {
        Ok(_) => panic!("expected parse error"),
        Err(error) => assert!(matches!(error, MultiError::Parse(_))),
    }
}

#[test]
fn multi_validate_rejects_invalid_schema() {
    let pipeline = build_multi()
        .with_schema(Some("app".into()), schema_from_json(r#"{"type": "nope"}"#))
        .build();
    let stages = pipeline.stages();
    let loaded = stages.load().unwrap();
    let parsed = stages.parse(&loaded).unwrap();
    let mut merged = stages.merge(&parsed).unwrap();
    match stages.validate(&mut merged) {
        Ok(()) => panic!("expected schema error"),
        Err(error) => assert!(matches!(error, MultiError::Schema { .. })),
    }
}

#[test]
fn multi_validate_succeeds_for_matching_schema() {
    let pipeline = build_multi()
        .with_schema(
            Some("app".into()),
            schema_from_json(r#"{"type": "string"}"#),
        )
        .build();
    let mut merged = pipeline.run().unwrap();
    pipeline.stages().validate(&mut merged).unwrap();
}

#[test]
fn multi_pipeline_accessors_and_mutators() {
    let mut pipeline = build_multi();
    assert_eq!(pipeline.sources().count(), 1);
    assert!(!pipeline.loaders().is_empty());
    assert!(!pipeline.parsers().is_empty());
    pipeline.add_source("mock:two").unwrap();
    pipeline
        .loaders_mut()
        .push(Box::new(mock_loader(b"z", None)));
    pipeline.parsers_mut().push(Box::new(txt_parser()));
    assert_eq!(pipeline.sources().count(), 2);
    let _ = pipeline.merger();
    pipeline.schemas_mut().insert(
        Some("extra".into()),
        schema_from_json(r#"{"type": "string"}"#),
    );
    assert_eq!(pipeline.schemas().len(), 1);
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
    let entry = build_single().run().unwrap();
    assert_eq!(entry.value().value().as_string().unwrap(), "hello");
}

#[test]
fn multi_parse_skips_errors_when_payload_source_ignores_them() {
    init_logging();
    let pipeline = Pipeline::builder()
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
        .with_merger(DeepMerge::new())
        .build();
    let stages = pipeline.stages();
    let loaded = stages.load().unwrap();
    let parsed = stages.parse(&loaded).unwrap();
    assert!(parsed.is_empty());
}

#[test]
fn multi_load_skips_errors_when_source_ignores_them() {
    let pipeline = Pipeline::builder()
        .with_source("mock(on_error=(load=skip)):bad")
        .unwrap()
        .with_loader(failing_loader())
        .with_parser(txt_parser())
        .with_merger(DeepMerge::new())
        .build();
    let loaded = pipeline.stages().load().unwrap();
    assert!(loaded.is_empty());
}

#[test]
fn multi_merge_propagates_merge_error() {
    let pipeline = Pipeline::builder()
        .with_source("mock:one")
        .unwrap()
        .with_loader(mock_loader(b"x", None))
        .with_parser(txt_parser())
        .with_merger(FailMerge)
        .build();
    let stages = pipeline.stages();
    let loaded = stages.load().unwrap();
    let parsed = stages.parse(&loaded).unwrap();
    match stages.merge(&parsed) {
        Ok(_) => panic!("expected merge error"),
        Err(error) => assert!(matches!(error, MultiError::Merge(_))),
    }
}

#[test]
fn multi_from_plan_keeps_named_entries() {
    use tanzim::merger::plan::{deep, src};
    // The named + explicit-plan axis: a `Pipeline::from_plan` builder deep-merges two sources, and the
    // result stays keyed by entry name (unlike `Config::from_plan`, which would unify into one value).
    let pipeline = Pipeline::from_plan(deep(vec![
        src("mock:one").unwrap(),
        src("mock:two").unwrap(),
    ]))
    .with_loader(mock_loader(b"hello", Some("app")))
    .with_parser(txt_parser());
    assert_eq!(pipeline.sources().count(), 2);
    let merged = pipeline.run().unwrap();
    assert!(merged.contains_key(&Some("app".into())));
}

#[test]
fn single_schema_mut_and_add_source() {
    let mut pipeline = build_single();
    assert_eq!(pipeline.sources().count(), 1);
    pipeline.add_source("mock:two").unwrap();
    assert_eq!(pipeline.sources().count(), 2);
    pipeline
        .schema_mut()
        .replace(schema_from_json(r#"{"type": "string"}"#));
    assert!(pipeline.schema().is_some());
}

/// Parses comma-separated `k=v` pairs into a map, so distinct payloads carry distinct keys.
fn kv_parser() -> ParserClosure {
    ParserClosure::new(
        "mock",
        "txt",
        Box::new(|source, bytes, _other_source_list| {
            let location = || Location::at(source.source(), source.resource(), None, None, None);
            let text = String::from_utf8_lossy(bytes);
            let mut map = tanzim_value::Map::new();
            for pair in text.split(',') {
                if let Some((key, value)) = pair.split_once('=') {
                    map.insert(
                        key.trim().to_string(),
                        LocatedValue::new(Value::String(value.trim().to_string()), location()),
                    );
                }
            }
            Ok(LocatedValue::new(Value::Map(map), location()))
        }),
    )
}

/// A loader that returns two `app`-named payloads with the given contents.
fn two_payload_loader(first: &'static [u8], second: &'static [u8]) -> LoaderClosure {
    LoaderClosure::new(
        "mock",
        move |source| {
            Ok([first, second]
                .into_iter()
                .map(|content| Payload {
                    source: source.clone(),
                    maybe_name: Some("app".into()),
                    maybe_format: Some("txt".into()),
                    content: content.to_vec(),
                })
                .collect())
        },
        "mock",
    )
}

/// A loader that yields one `app`-named payload whose content is `<resource>=1`, so each configured
/// source contributes a distinct map key.
fn resource_kv_loader() -> LoaderClosure {
    LoaderClosure::new(
        "mock",
        |source| {
            Ok(vec![Payload {
                source: source.clone(),
                maybe_name: Some("app".into()),
                maybe_format: Some("txt".into()),
                content: format!("{}=1", source.resource()).into_bytes(),
            }])
        },
        "mock",
    )
}

#[test]
fn single_default_merger_is_last_wins() {
    // No merger configured: the source's two payloads fold with the default `LastWins`.
    let pipeline = Config::builder()
        .with_source("mock:a")
        .unwrap()
        .with_loader(two_payload_loader(b"x=1", b"y=2"))
        .with_parser(kv_parser());
    let entry = pipeline.run().unwrap();
    let map = entry.value().value().as_map().unwrap();
    assert!(map.get("x").is_none());
    assert_eq!(map.get("y").unwrap().value().as_string().unwrap(), "2");
}

#[test]
fn single_with_source_merged_pre_merges_before_global() {
    // The per-source `DeepMerge` combines the source's two payloads, keeping both keys — whereas the
    // default global `LastWins` alone would have dropped `x`.
    let pipeline = Config::builder()
        .with_source_merged("mock:a", DeepMerge::new())
        .unwrap()
        .with_loader(two_payload_loader(b"x=1", b"y=2"))
        .with_parser(kv_parser());
    let entry = pipeline.run().unwrap();
    let map = entry.value().value().as_map().unwrap();
    assert_eq!(map.get("x").unwrap().value().as_string().unwrap(), "1");
    assert_eq!(map.get("y").unwrap().value().as_string().unwrap(), "2");
}

#[test]
fn single_from_plan_advanced_fold() {
    use tanzim::merger::plan::{deep, last_wins, src};
    // The sources live entirely in the plan — a `from_plan` builder has no `with_source` (that would
    // not compile). last_wins(c, deep(a, b)): deep-merge a+b (keeps both keys), then last-wins with c
    // as the *earlier* child → the deep result wins. This differs from the default `LastWins` fold
    // (which, folding a, b, c in order, would keep only c) — proving the explicit plan is applied
    // and its nested `deep` node runs.
    let pipeline = Config::from_plan(last_wins(vec![
        src("mock:c").unwrap(),
        deep(vec![src("mock:a").unwrap(), src("mock:b").unwrap()]),
    ]))
    .with_loader(resource_kv_loader())
    .with_parser(kv_parser());
    // The plan's leaves are surfaced as the pipeline's sources for loading.
    assert_eq!(pipeline.sources().count(), 3);
    let entry = pipeline.run().unwrap();
    let map = entry.value().value().as_map().unwrap();
    assert_eq!(map.get("a").unwrap().value().as_string().unwrap(), "1");
    assert_eq!(map.get("b").unwrap().value().as_string().unwrap(), "1");
    assert!(map.get("c").is_none());
}

#[test]
fn with_source_rejects_invalid_source_string() {
    assert!(matches!(
        Config::builder().with_source("bad("),
        Err(SingleError::Source(_))
    ));
}

#[derive(serde::Deserialize, Debug, PartialEq)]
struct App {
    name: String,
    port: u16,
}

/// Parses the payload into a map `{ name: <content>, port: 8080 }`.
fn map_parser() -> ParserClosure {
    ParserClosure::new(
        "mock",
        "txt",
        Box::new(|source, bytes, _other_source_list| {
            let location = || Location::at(source.source(), source.resource(), None, None, None);
            let mut map = tanzim_value::Map::new();
            map.insert(
                "name".into(),
                LocatedValue::new(
                    Value::String(String::from_utf8_lossy(bytes).to_string()),
                    location(),
                ),
            );
            map.insert(
                "port".into(),
                LocatedValue::new(Value::Int(8080), location()),
            );
            Ok(LocatedValue::new(Value::Map(map), location()))
        }),
    )
}

/// Parses the payload into a map whose `port` is a (non-numeric) string, at line 3 column 5.
fn bad_port_parser() -> ParserClosure {
    ParserClosure::new(
        "mock",
        "txt",
        Box::new(|source, _bytes, _other_source_list| {
            let location =
                || Location::at(source.source(), source.resource(), Some(3), Some(5), None);
            let mut map = tanzim_value::Map::new();
            map.insert(
                "name".into(),
                LocatedValue::new(Value::String("x".into()), location()),
            );
            map.insert(
                "port".into(),
                LocatedValue::new(Value::String("not-a-number".into()), location()),
            );
            Ok(LocatedValue::new(Value::Map(map), location()))
        }),
    )
}

#[test]
fn single_try_deserialize_produces_typed_config() {
    let pipeline = Config::builder()
        .with_source("mock:one")
        .unwrap()
        .with_loader(mock_loader(b"hello", Some("app")))
        .with_parser(map_parser())
        .with_merger(LastWins);
    let app: App = pipeline.try_deserialize().unwrap();
    assert_eq!(
        app,
        App {
            name: "hello".into(),
            port: 8080
        }
    );
}

#[test]
fn single_try_deserialize_reports_located_error() {
    let pipeline = Config::builder()
        .with_source("mock:one")
        .unwrap()
        .with_loader(mock_loader(b"hello", Some("app")))
        .with_parser(bad_port_parser())
        .with_merger(LastWins);
    let error = pipeline.try_deserialize::<App>().unwrap_err();
    assert!(
        matches!(error, SingleError::Deserialize(_)),
        "expected a deserialize error, got {error:?}"
    );
    let message = error.to_string();
    assert!(
        message.contains("mock:one:3:5"),
        "error should point at the offending node: {message}"
    );
}

#[test]
fn single_try_deserialize_error_renders_caret() {
    use tanzim::parser::toml::Toml;

    #[derive(serde::Deserialize, Debug)]
    struct Cfg {
        #[allow(dead_code)]
        listen: Listen,
    }
    #[derive(serde::Deserialize, Debug)]
    struct Listen {
        #[allow(dead_code)]
        port: u16,
    }

    let toml = b"[listen]\nport = \"eighty\"\n";
    let pipeline = Config::builder()
        .with_source("file:app.toml")
        .unwrap()
        .with_loader(LoaderClosure::new(
            "file",
            move |source| {
                Ok(vec![Payload {
                    source: source.clone(),
                    maybe_name: None,
                    maybe_format: Some("toml".into()),
                    content: toml.to_vec(),
                }])
            },
            "file",
        ))
        .with_parser(Toml::new())
        .with_merger(LastWins);

    let error = pipeline.try_deserialize::<Cfg>().unwrap_err();
    assert!(matches!(error, SingleError::Deserialize(_)));

    // Default: one line, naming the expected type and the source location.
    let single_line = error.to_string();
    assert!(single_line.contains("expected u16"), "{single_line}");
    assert!(single_line.contains("file:app.toml:2:8"), "{single_line}");

    // Alternate `{:#}`: a source snippet with a caret under the offending value.
    let alternate = format!("{error:#}");
    assert!(alternate.contains("port = \"eighty\""), "{alternate}");
    assert!(alternate.contains("^^^^^^^^"), "{alternate}");
}

#[test]
fn multi_try_deserialize_returns_map_per_entry() {
    let pipeline = Pipeline::builder()
        .with_source("mock:one")
        .unwrap()
        .with_loader(mock_loader(b"hello", Some("app")))
        .with_parser(map_parser())
        .with_merger(LastWins);
    let deserialized: std::collections::HashMap<Option<String>, App> =
        pipeline.try_deserialize().unwrap();
    assert_eq!(
        deserialized.get(&Some("app".to_string())),
        Some(&App {
            name: "hello".into(),
            port: 8080
        })
    );
}
