use tanzim::{
    loader::closure::Closure as LoaderClosure,
    merger::{DeepMerge, LastWins},
    parser::closure::Closure as ParserClosure,
    pipeline::multi::{Error as MultiError, Multi},
    pipeline::single::{Error as SingleError, Single},
    source::Source,
    validator::SchemaValue,
};
use tanzim_load::{Error as LoadError, Payload};
use tanzim_parse::LocatedValue;
use tanzim_value::{Location, Value};

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

fn build_single() -> Single {
    Single::empty()
        .with_source("mock:one")
        .unwrap()
        .with_loader(mock_loader(b"hello", Some("app")))
        .with_parser(txt_parser())
        .with_merger(LastWins)
        .unwrap()
}

#[test]
fn single_reports_missing_loaders_and_parsers_at_run_time() {
    let no_loaders = Single::empty()
        .with_parser(txt_parser())
        .with_merger(LastWins)
        .unwrap();
    assert!(matches!(no_loaders.run(), Err(SingleError::NoLoaders)));

    let no_parsers = Single::empty()
        .with_source("mock:one")
        .unwrap()
        .with_loader(mock_loader(b"x", None))
        .with_merger(LastWins)
        .unwrap();
    assert!(matches!(no_parsers.run(), Err(SingleError::NoParsers)));

    // No explicit merger: the merge stage now defaults to `LastWins`, so the pipeline runs.
    let no_merger = Single::empty()
        .with_source("mock:one")
        .unwrap()
        .with_loader(mock_loader(b"x", None))
        .with_parser(txt_parser());
    assert!(no_merger.run().is_ok());
}

#[test]
fn single_default_includes_loaders_and_parsers_but_no_merger() {
    let pipeline = Single::default();
    assert!(!pipeline.loaders().is_empty());
    assert!(!pipeline.parsers().is_empty());
    assert!(pipeline.merger().is_none());
    // With loaders and parsers but no merger and no sources, the merge stage defaults to `LastWins`
    // and the pipeline runs, yielding an empty unified entry.
    let entry = Single::default().run().unwrap();
    assert!(entry.value().value().as_map().unwrap().is_empty());
}

#[test]
fn single_empty_registers_nothing() {
    let pipeline = Single::empty();
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
    let pipeline = Single::empty()
        .with_source("other:path")
        .unwrap()
        .with_loader(mock_loader(b"x", None))
        .with_parser(txt_parser())
        .with_merger(LastWins)
        .unwrap();
    match pipeline.load() {
        Ok(_) => panic!("expected load error"),
        Err(error) => assert!(matches!(error, SingleError::NoLoader { .. })),
    }
}

#[test]
fn single_load_skips_errors_when_source_ignores_them() {
    let pipeline = Single::empty()
        .with_source("mock(on_error=(load=skip)):bad")
        .unwrap()
        .with_loader(failing_loader())
        .with_parser(txt_parser())
        .with_merger(LastWins)
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
    assert_eq!(parsed[0].value().value().as_string().unwrap(), "hello");
}

#[test]
fn single_parse_auto_detects_format() {
    let pipeline = Single::empty()
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
        .unwrap();
    let loaded = pipeline.load().unwrap();
    let parsed = pipeline.parse(&loaded).unwrap();
    assert_eq!(parsed[0].value().value().as_string().unwrap(), "auto");
}

#[test]
fn single_parse_errors_when_no_parser_matches() {
    let pipeline = Single::empty()
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
    let entry = pipeline.unify(&merged).unwrap();
    assert!(entry.payloads().is_empty());
    assert!(entry.value().value().as_map().unwrap().entries().is_empty());
}

#[test]
fn single_unify_collapses_named_groups_with_last_wins() {
    let pipeline = Single::empty()
        .with_source("mock:one")
        .unwrap()
        .with_loader(dual_loader())
        .with_parser(txt_parser())
        .with_merger(LastWins)
        .unwrap();
    let loaded = pipeline.load().unwrap();
    let parsed = pipeline.parse(&loaded).unwrap();
    let merged = pipeline.merge(&parsed).unwrap();
    let entry = pipeline.unify(&merged).unwrap();
    assert_eq!(entry.value().value().as_string().unwrap(), "beta-value");
}

#[test]
fn single_run_executes_full_pipeline() {
    let pipeline = build_single();
    let entry = pipeline.run().unwrap();
    assert_eq!(entry.value().value().as_string().unwrap(), "hello");
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
    let pipeline = build_single().with_schema(schema_from_json(r#"{"type": "nope"}"#));
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
    let pipeline = build_single().with_schema(schema_from_json(r#"{"type": "integer"}"#));
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
    let pipeline = Single::empty()
        .with_source("mock:one")
        .unwrap()
        .with_loader(mock_loader(b"x", None))
        .with_parser(txt_parser())
        .with_merger(LastWins)
        .unwrap()
        .with_included_loaders()
        .with_included_parsers();
    assert_eq!(pipeline.sources().count(), 1);
    assert!(!pipeline.loaders().is_empty());
    assert!(!pipeline.parsers().is_empty());

    let pipeline = pipeline
        .with_source("mock:two")
        .unwrap()
        .with_loader(mock_loader(b"y", None))
        .with_parser(txt_parser())
        .with_merger(DeepMerge::new())
        .unwrap()
        .set_included_loaders()
        .set_included_parsers();
    assert_eq!(pipeline.sources().count(), 2);
}

fn build_multi() -> Multi {
    Multi::empty()
        .with_source("mock:one")
        .unwrap()
        .with_loader(mock_loader(b"hello", Some("app")))
        .with_parser(txt_parser())
        .with_merger(DeepMerge::new())
        .unwrap()
}

#[test]
fn multi_reports_missing_components_at_run_time() {
    let no_loaders = Multi::empty()
        .with_parser(txt_parser())
        .with_merger(DeepMerge::new())
        .unwrap();
    assert!(matches!(no_loaders.run(), Err(MultiError::NoLoaders)));

    let no_parsers = Multi::empty()
        .with_source("mock:one")
        .unwrap()
        .with_loader(mock_loader(b"x", None))
        .with_merger(DeepMerge::new())
        .unwrap();
    assert!(matches!(no_parsers.run(), Err(MultiError::NoParsers)));

    // No explicit merger: the merge stage now defaults to `LastWins`, so the pipeline runs.
    let no_merger = Multi::empty()
        .with_source("mock:one")
        .unwrap()
        .with_loader(mock_loader(b"x", None))
        .with_parser(txt_parser());
    assert!(no_merger.run().is_ok());
}

#[test]
fn multi_default_and_empty() {
    // No sources and no merger: defaults to `LastWins`, runs and yields an empty entry map.
    assert!(Multi::default().run().unwrap().is_empty());
    assert!(matches!(Multi::empty().run(), Err(MultiError::NoLoaders)));
}

#[test]
fn multi_run_returns_named_entries() {
    let pipeline = build_multi();
    let merged = pipeline.run().unwrap();
    assert!(merged.contains_key(&Some("app".into())));
}

#[test]
fn multi_validate_warns_when_schema_has_no_matching_entry() {
    let pipeline = build_multi().with_schema(
        Some("missing".into()),
        schema_from_json(r#"{"type": "string"}"#),
    );
    let loaded = pipeline.load().unwrap();
    let parsed = pipeline.parse(&loaded).unwrap();
    let mut merged = pipeline.merge(&parsed).unwrap();
    pipeline.validate(&mut merged).unwrap();
}

#[test]
fn multi_validate_rejects_bad_configuration() {
    let pipeline = build_multi().with_schema(
        Some("app".into()),
        schema_from_json(r#"{"type": "integer"}"#),
    );
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
    let mut schemas = tanzim::pipeline::multi::Schemas::new();
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
    ) -> Result<tanzim::merger::Merged, tanzim::merger::Error> {
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
    let pipeline = Single::empty()
        .with_source("mock:bad")
        .unwrap()
        .with_loader(failing_loader())
        .with_parser(txt_parser())
        .with_merger(LastWins)
        .unwrap();
    match pipeline.load() {
        Ok(_) => panic!("expected load error"),
        Err(error) => assert!(matches!(error, SingleError::Load(_))),
    }
}

#[test]
fn single_parse_skips_errors_when_payload_source_ignores_them() {
    let pipeline = Single::empty()
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
        .unwrap();
    let loaded = pipeline.load().unwrap();
    let parsed = pipeline.parse(&loaded).unwrap();
    assert!(parsed.is_empty());
}

#[test]
fn single_parse_propagates_parser_error() {
    let pipeline = Single::empty()
        .with_source("mock:one")
        .unwrap()
        .with_loader(mock_loader(b"x", Some("app")))
        .with_parser(failing_parser())
        .with_merger(LastWins)
        .unwrap();
    let loaded = pipeline.load().unwrap();
    match pipeline.parse(&loaded) {
        Ok(_) => panic!("expected parse error"),
        Err(error) => assert!(matches!(error, SingleError::Parse(_))),
    }
}

#[test]
fn single_merge_propagates_merge_error() {
    let pipeline = Single::empty()
        .with_source("mock:one")
        .unwrap()
        .with_loader(mock_loader(b"x", None))
        .with_parser(txt_parser())
        .with_merger(FailMerge)
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
    let pipeline = Single::empty()
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
        .unwrap();
    let loaded = pipeline.load().unwrap();
    let parsed = pipeline.parse(&loaded).unwrap();
    let merged = pipeline.merge(&parsed).unwrap();
    let entry = pipeline.unify(&merged).unwrap();
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
    let pipeline = Single::empty()
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
        .unwrap();
    let loaded = pipeline.load().unwrap();
    let parsed = pipeline.parse(&loaded).unwrap();
    let merged = pipeline.merge(&parsed).unwrap();
    let entry = pipeline.unify(&merged).unwrap();
    assert_eq!(entry.value().value().as_string().unwrap(), "unnamed");
}

#[test]
fn single_run_with_valid_schema_coerces_configuration() {
    let pipeline = Single::empty()
        .with_source("mock:one")
        .unwrap()
        .with_loader(mock_loader(b"42", Some("app")))
        .with_parser(txt_parser())
        .with_merger(LastWins)
        .unwrap()
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
    let pipeline = Multi::empty()
        .with_source("mock:bad")
        .unwrap()
        .with_loader(failing_loader())
        .with_parser(txt_parser())
        .with_merger(DeepMerge::new())
        .unwrap();
    match pipeline.load() {
        Ok(_) => panic!("expected load error"),
        Err(error) => assert!(matches!(error, MultiError::Load(_))),
    }

    let pipeline = Multi::empty()
        .with_source("mock:one")
        .unwrap()
        .with_loader(mock_loader(b"x", Some("app")))
        .with_parser(failing_parser())
        .with_merger(DeepMerge::new())
        .unwrap();
    let loaded = pipeline.load().unwrap();
    match pipeline.parse(&loaded) {
        Ok(_) => panic!("expected parse error"),
        Err(error) => assert!(matches!(error, MultiError::Parse(_))),
    }
}

#[test]
fn multi_validate_rejects_invalid_schema() {
    let pipeline =
        build_multi().with_schema(Some("app".into()), schema_from_json(r#"{"type": "nope"}"#));
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
    let pipeline = build_multi().with_schema(
        Some("app".into()),
        schema_from_json(r#"{"type": "string"}"#),
    );
    let mut merged = pipeline.run().unwrap();
    pipeline.validate(&mut merged).unwrap();
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
    let pipeline = build_single();
    let entry = pipeline.run().unwrap();
    assert_eq!(entry.value().value().as_string().unwrap(), "hello");
}

#[test]
fn multi_parse_skips_errors_when_payload_source_ignores_them() {
    init_logging();
    let pipeline = Multi::empty()
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
        .unwrap();
    let loaded = pipeline.load().unwrap();
    let parsed = pipeline.parse(&loaded).unwrap();
    assert!(parsed.is_empty());
}

#[test]
fn multi_load_skips_errors_when_source_ignores_them() {
    let pipeline = Multi::empty()
        .with_source("mock(on_error=(load=skip)):bad")
        .unwrap()
        .with_loader(failing_loader())
        .with_parser(txt_parser())
        .with_merger(DeepMerge::new())
        .unwrap();
    let loaded = pipeline.load().unwrap();
    assert!(loaded.is_empty());
}

#[test]
fn multi_merge_propagates_merge_error() {
    let pipeline = Multi::empty()
        .with_source("mock:one")
        .unwrap()
        .with_loader(mock_loader(b"x", None))
        .with_parser(txt_parser())
        .with_merger(FailMerge)
        .unwrap();
    let loaded = pipeline.load().unwrap();
    let parsed = pipeline.parse(&loaded).unwrap();
    match pipeline.merge(&parsed) {
        Ok(_) => panic!("expected merge error"),
        Err(error) => assert!(matches!(error, MultiError::Merge(_))),
    }
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
    let pipeline = Single::empty()
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
    let pipeline = Single::empty()
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
fn single_with_merge_plan_advanced_fold() {
    use tanzim::pipeline::single::{deep, last_wins, src};
    // The sources live entirely in the plan — no simple `with_source` calls (which would conflict).
    // last_wins(c, deep(a, b)): deep-merge a+b (keeps both keys), then last-wins with c as the
    // *earlier* child → the deep result wins. This differs from the default `LastWins` fold
    // (which, folding a, b, c in order, would keep only c) — proving the explicit plan is applied
    // and its nested `deep` node runs.
    let pipeline = Single::empty()
        .with_loader(resource_kv_loader())
        .with_parser(kv_parser())
        .with_merge_plan(last_wins(vec![
            src("mock:c").unwrap(),
            deep(vec![src("mock:a").unwrap(), src("mock:b").unwrap()]),
        ]))
        .unwrap();
    // The plan's leaves are surfaced as the pipeline's sources for loading.
    assert_eq!(pipeline.sources().count(), 3);
    let entry = pipeline.run().unwrap();
    let map = entry.value().value().as_map().unwrap();
    assert_eq!(map.get("a").unwrap().value().as_string().unwrap(), "1");
    assert_eq!(map.get("b").unwrap().value().as_string().unwrap(), "1");
    assert!(map.get("c").is_none());
}

#[test]
fn merge_plan_and_simple_builders_are_mutually_exclusive() {
    use tanzim::pipeline::single::{last_wins, src};
    // A source configured first, then an explicit plan → conflict.
    let after_source = Single::empty()
        .with_source("mock:a")
        .unwrap()
        .with_merge_plan(last_wins(vec![src("mock:a").unwrap()]));
    assert!(matches!(after_source, Err(SingleError::PlanConflict)));

    // A merger configured first, then an explicit plan → conflict.
    let after_merger = Single::empty()
        .with_merger(LastWins)
        .unwrap()
        .with_merge_plan(last_wins(vec![src("mock:a").unwrap()]));
    assert!(matches!(after_merger, Err(SingleError::PlanConflict)));

    // The reverse: an explicit plan first, then a simple builder → conflict.
    let mut pipeline = Single::empty();
    pipeline
        .add_merge_plan(last_wins(vec![src("mock:a").unwrap()]))
        .unwrap();
    assert!(matches!(
        pipeline.add_source("mock:b"),
        Err(SingleError::PlanConflict)
    ));
    assert!(matches!(
        pipeline.add_merger(LastWins),
        Err(SingleError::PlanConflict)
    ));
}

#[test]
fn with_source_rejects_invalid_source_string() {
    assert!(matches!(
        Single::empty().with_source("bad("),
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
    let pipeline = Single::empty()
        .with_source("mock:one")
        .unwrap()
        .with_loader(mock_loader(b"hello", Some("app")))
        .with_parser(map_parser())
        .with_merger(LastWins)
        .unwrap();
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
    let pipeline = Single::empty()
        .with_source("mock:one")
        .unwrap()
        .with_loader(mock_loader(b"hello", Some("app")))
        .with_parser(bad_port_parser())
        .with_merger(LastWins)
        .unwrap();
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
    let pipeline = Single::empty()
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
        .with_merger(LastWins)
        .unwrap();

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
    let pipeline = Multi::empty()
        .with_source("mock:one")
        .unwrap()
        .with_loader(mock_loader(b"hello", Some("app")))
        .with_parser(map_parser())
        .with_merger(LastWins)
        .unwrap();
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
