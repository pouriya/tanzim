use std::collections::HashMap;
use std::time::Duration;
use tanzim_load::{
    Error, Load, Payload, Source,
    http::{Http, NAME, SOURCE, Url},
};
use tanzim_source::SourceBuilder;

#[test]
fn load_delegates_to_fetch_closure() {
    let loader = Http::new(Box::new(
        |source: Source,
         url: &Url,
         headers: &HashMap<String, String>,
         timeout: Duration,
         insecure: bool| {
            assert_eq!(url.as_str(), "https://example.com/config.json");
            assert_eq!(
                headers.get("Authorization").map(String::as_str),
                Some("TOKEN")
            );
            assert_eq!(timeout, Duration::from_secs(30));
            assert!(insecure);
            Ok(vec![Payload {
                source,
                maybe_name: Some("demo".into()),
                maybe_format: Some("json".into()),
                content: br#"{"hello":"world"}"#.to_vec(),
            }])
        },
    ));

    let source = SourceBuilder::new()
        .with_source("http")
        .with_resource("https://example.com/config.json")
        .with_option("headers", HashMap::from([("Authorization", "TOKEN")]))
        .with_option("timeout", 30_i64)
        .with_option("insecure", true)
        .build()
        .unwrap();
    let loaded = loader.load(source).unwrap();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].maybe_name, Some("demo".to_string()));
}

#[test]
fn load_rejects_invalid_url() {
    let loader = Http::new(Box::new(|_, _, _, _, _| Ok(Vec::new())));
    let source = SourceBuilder::new()
        .with_source("http")
        .with_resource("not a url")
        .build()
        .unwrap();
    let error = loader.load(source).unwrap_err();
    assert!(matches!(error, Error::InvalidResource { .. }));
}

#[test]
fn load_requires_resource() {
    let loader = Http::new(Box::new(|source: Source, _, _, _, _| {
        Ok(vec![Payload {
            source,
            maybe_name: None,
            maybe_format: None,
            content: Vec::new(),
        }])
    }));
    let source = SourceBuilder::new().with_source("http").build().unwrap();
    let error = loader.load(source).unwrap_err();
    assert!(matches!(error, Error::InvalidResource { .. }));
}

#[test]
fn name_and_supported_source_list() {
    let loader = Http::new(Box::new(|_, _, _, _, _| Ok(Vec::new())));
    assert_eq!(loader.name(), NAME);
    assert_eq!(loader.supported_source_list(), vec![SOURCE.to_string()]);
}

#[test]
fn load_ignores_unknown_option() {
    let loader = Http::new(Box::new(|_, _, _, _, _| Ok(Vec::new())));
    let source = SourceBuilder::new()
        .with_source("http")
        .with_resource("https://example.com")
        .with_option("bogus", true)
        .build()
        .unwrap();
    let loaded = loader.load(source).unwrap();
    assert!(loaded.is_empty());
}

#[test]
fn load_rejects_bad_headers_type() {
    let loader = Http::new(Box::new(|_, _, _, _, _| Ok(Vec::new())));
    let source = SourceBuilder::new()
        .with_source("http")
        .with_resource("https://example.com")
        .with_option("headers", "not-a-map")
        .build()
        .unwrap();
    let error = loader.load(source).unwrap_err();
    assert!(matches!(error, Error::InvalidOption { key, .. } if key == "headers"));
}

#[test]
fn load_rejects_non_string_header_value() {
    let loader = Http::new(Box::new(|_, _, _, _, _| Ok(Vec::new())));
    let source = SourceBuilder::new()
        .with_source("http")
        .with_resource("https://example.com")
        .with_option("headers", HashMap::from([("Authorization", 1_i64)]))
        .build()
        .unwrap();
    let error = loader.load(source).unwrap_err();
    assert!(matches!(error, Error::InvalidOption { key, .. } if key == "headers"));
}

#[test]
fn load_uses_default_timeout() {
    let loader = Http::new(Box::new(|_, _, _, timeout, _| {
        assert_eq!(timeout, Duration::from_secs(15));
        Ok(Vec::new())
    }));
    let source = SourceBuilder::new()
        .with_source("http")
        .with_resource("https://example.com")
        .build()
        .unwrap();
    loader.load(source).unwrap();
}

#[test]
fn load_rejects_non_positive_timeout() {
    let loader = Http::new(Box::new(|_, _, _, _, _| Ok(Vec::new())));
    let source = SourceBuilder::new()
        .with_source("http")
        .with_resource("https://example.com")
        .with_option("timeout", 0_i64)
        .build()
        .unwrap();
    let error = loader.load(source).unwrap_err();
    assert!(matches!(error, Error::InvalidOption { key, .. } if key == "timeout"));
}

#[test]
fn load_rejects_bad_insecure_type() {
    let loader = Http::new(Box::new(|_, _, _, _, _| Ok(Vec::new())));
    let source = SourceBuilder::new()
        .with_source("http")
        .with_resource("https://example.com")
        .with_option("insecure", "yes")
        .build()
        .unwrap();
    let error = loader.load(source).unwrap_err();
    assert!(matches!(error, Error::InvalidOption { key, .. } if key == "insecure"));
}

#[test]
fn load_wraps_fetch_error() {
    let loader = Http::new(Box::new(|_, _, _, _, _| Err("network down".into())));
    let source = SourceBuilder::new()
        .with_source("http")
        .with_resource("https://example.com")
        .build()
        .unwrap();
    let error = loader.load(source).unwrap_err();
    assert!(
        matches!(&error, Error::Load { description, .. } if description == "fetch configuration")
    );
    // Default display is the loader summary; alternate form appends the wrapped cause.
    assert_eq!(
        error.to_string(),
        "HTTP configuration loader could not fetch configuration `https://example.com`"
    );
    assert_eq!(
        format!("{error:#}"),
        "HTTP configuration loader could not fetch configuration `https://example.com`: network down"
    );
}

#[test]
fn load_normalizes_trimmed_empty_name_and_format() {
    let loader = Http::new(Box::new(|source: Source, _, _, _, _| {
        Ok(vec![Payload {
            source,
            maybe_name: Some("   ".into()),
            maybe_format: Some("\t".into()),
            content: Vec::new(),
        }])
    }));
    let source = SourceBuilder::new()
        .with_source("http")
        .with_resource("https://example.com")
        .build()
        .unwrap();
    let loaded = loader.load(source).unwrap();
    assert_eq!(loaded[0].maybe_name, None);
    assert_eq!(loaded[0].maybe_format, None);
}

#[test]
fn load_lowercases_name_and_format_by_default() {
    let loader = Http::new(Box::new(|source: Source, _, _, _, _| {
        Ok(vec![Payload {
            source,
            maybe_name: Some(" Demo ".into()),
            maybe_format: Some(" JSON ".into()),
            content: Vec::new(),
        }])
    }));
    let source = SourceBuilder::new()
        .with_source("http")
        .with_resource("https://example.com")
        .build()
        .unwrap();
    let loaded = loader.load(source).unwrap();
    assert_eq!(loaded[0].maybe_name.as_deref(), Some("demo"));
    assert_eq!(loaded[0].maybe_format.as_deref(), Some("json"));
}

#[test]
fn load_preserves_case_when_lowercase_disabled() {
    let loader = Http::new(Box::new(|source: Source, _, _, _, _| {
        Ok(vec![Payload {
            source,
            maybe_name: Some("Demo".into()),
            maybe_format: Some("JSON".into()),
            content: Vec::new(),
        }])
    }));
    let source = SourceBuilder::new()
        .with_source("http")
        .with_resource("https://example.com")
        .with_option("lowercase", false)
        .build()
        .unwrap();
    let loaded = loader.load(source).unwrap();
    assert_eq!(loaded[0].maybe_name.as_deref(), Some("Demo"));
    assert_eq!(loaded[0].maybe_format.as_deref(), Some("JSON"));
}

#[test]
fn load_clones_source_onto_payloads() {
    let loader = Http::new(Box::new(|source: Source, _, _, _, _| {
        Ok(vec![Payload {
            source,
            maybe_name: Some("app".into()),
            maybe_format: Some("json".into()),
            content: b"{}".to_vec(),
        }])
    }));
    let source = SourceBuilder::new()
        .with_source("http")
        .with_resource("https://example.com/x")
        .build()
        .unwrap();
    let loaded = loader.load(source.clone()).unwrap();
    assert_eq!(loaded[0].source.resource(), source.resource());
}
