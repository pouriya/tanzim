use tanzim_load::Payload;
use tanzim_merge::plan::{
    MergePlan, SourceGroup, deep, evaluate, last_wins, named_value, src, value,
};
use tanzim_source::{Source, SourceBuilder};
use tanzim_value::{LocatedValue, Location, Map, Value};

fn make_source(resource: &str) -> Source {
    SourceBuilder::new()
        .with_source("mock")
        .with_resource(resource)
        .build()
        .unwrap()
}

fn payload(source: &Source) -> Payload {
    Payload {
        source: source.clone(),
        maybe_name: None,
        maybe_format: Some("txt".into()),
        content: Vec::new(),
    }
}

fn string_value(text: &str) -> LocatedValue {
    LocatedValue::new(
        Value::String(text.to_string()),
        Location::at("mock", "test", None, None, None),
    )
}

fn map_value(entries: &[(&str, &str)]) -> LocatedValue {
    let mut map = Map::new();
    for (key, value) in entries {
        map.insert(key.to_string(), string_value(value));
    }
    LocatedValue::new(
        Value::Map(map),
        Location::at("mock", "test", None, None, None),
    )
}

/// A single-payload source group under the unnamed bucket.
fn group(source: &Source, value: LocatedValue) -> SourceGroup {
    (source.clone(), vec![(payload(source), value)])
}

#[test]
fn source_leaf_resolves_only_its_source() {
    let a = make_source("a");
    let b = make_source("b");
    let groups = vec![
        group(&a, string_value("from-a")),
        group(&b, string_value("from-b")),
    ];
    let merged = evaluate(&MergePlan::Source(a.clone()), &groups).unwrap();
    assert_eq!(merged.len(), 1);
    let (_, value) = merged.get(&None).unwrap();
    assert_eq!(value.value().as_string().unwrap(), "from-a");
}

#[test]
fn desugared_last_wins_over_two_sources() {
    // Root Merge(LastWins, [Source(a), Source(b)]) — the flat/backward-compat shape.
    let a = make_source("a");
    let b = make_source("b");
    let groups = vec![
        group(&a, string_value("first")),
        group(&b, string_value("second")),
    ];
    let plan = last_wins(vec![
        MergePlan::Source(a.clone()),
        MergePlan::Source(b.clone()),
    ]);
    let merged = evaluate(&plan, &groups).unwrap();
    let (_, value) = merged.get(&None).unwrap();
    assert_eq!(value.value().as_string().unwrap(), "second");
}

#[test]
fn deep_child_merges_its_sources() {
    let a = make_source("a");
    let b = make_source("b");
    let groups = vec![
        group(&a, map_value(&[("x", "A"), ("port", "1")])),
        group(&b, map_value(&[("y", "B"), ("port", "2")])),
    ];
    let plan = deep(vec![
        MergePlan::Source(a.clone()),
        MergePlan::Source(b.clone()),
    ]);
    let merged = evaluate(&plan, &groups).unwrap();
    let map = merged.get(&None).unwrap().1.value().as_map().unwrap();
    assert_eq!(map.get("x").unwrap().value().as_string().unwrap(), "A");
    assert_eq!(map.get("y").unwrap().value().as_string().unwrap(), "B");
    // Overlay (b) wins the shared scalar key.
    assert_eq!(map.get("port").unwrap().value().as_string().unwrap(), "2");
}

#[test]
fn last_wins_of_deep_and_source() {
    // last_wins(deep(A, B), C): deep-merge A+B, then last-wins the result with C → C wins.
    let a = make_source("a");
    let b = make_source("b");
    let c = make_source("c");
    let groups = vec![
        group(&a, map_value(&[("x", "A")])),
        group(&b, map_value(&[("y", "B")])),
        group(&c, map_value(&[("z", "C")])),
    ];
    let plan = last_wins(vec![
        deep(vec![
            MergePlan::Source(a.clone()),
            MergePlan::Source(b.clone()),
        ]),
        MergePlan::Source(c.clone()),
    ]);
    let merged = evaluate(&plan, &groups).unwrap();
    let map = merged.get(&None).unwrap().1.value().as_map().unwrap();
    // last-wins replaces: only C's key survives.
    assert!(map.get("x").is_none());
    assert!(map.get("y").is_none());
    assert_eq!(map.get("z").unwrap().value().as_string().unwrap(), "C");
}

#[test]
fn deep_of_lastwins_and_source_composes() {
    // deep(last_wins(A, B), C): last_wins collapses A,B → B, then deep-merge with C.
    let a = make_source("a");
    let b = make_source("b");
    let c = make_source("c");
    let groups = vec![
        group(&a, map_value(&[("x", "A")])),
        group(&b, map_value(&[("y", "B")])),
        group(&c, map_value(&[("z", "C")])),
    ];
    let plan = deep(vec![
        last_wins(vec![
            MergePlan::Source(a.clone()),
            MergePlan::Source(b.clone()),
        ]),
        MergePlan::Source(c.clone()),
    ]);
    let merged = evaluate(&plan, &groups).unwrap();
    let map = merged.get(&None).unwrap().1.value().as_map().unwrap();
    assert!(map.get("x").is_none()); // dropped by inner last-wins (B wins over A)
    assert_eq!(map.get("y").unwrap().value().as_string().unwrap(), "B");
    assert_eq!(map.get("z").unwrap().value().as_string().unwrap(), "C");
}

#[test]
fn src_helper_rejects_invalid_source() {
    assert!(src("bad(").is_err());
}

#[test]
fn value_leaf_skips_source_groups() {
    let a = make_source("a");
    let groups = vec![group(&a, string_value("from-a"))];
    let plan = value(string_value("built-in"));
    let merged = evaluate(&plan, &groups).unwrap();
    let (_, value) = merged.get(&None).unwrap();
    assert_eq!(value.value().as_string().unwrap(), "built-in");
}

#[test]
fn value_leaf_loses_to_later_source_under_last_wins() {
    let a = make_source("a");
    let groups = vec![group(&a, string_value("from-file"))];
    let plan = last_wins(vec![
        value(string_value("built-in")),
        MergePlan::Source(a.clone()),
    ]);
    let merged = evaluate(&plan, &groups).unwrap();
    let (_, value) = merged.get(&None).unwrap();
    assert_eq!(value.value().as_string().unwrap(), "from-file");
}

#[test]
fn named_value_leaf_lands_in_named_bucket() {
    let plan = named_value("app", string_value("defaults"));
    let merged = evaluate(&plan, &[]).unwrap();
    assert!(!merged.contains_key(&None));
    let (_, value) = merged.get(&Some("app".into())).unwrap();
    assert_eq!(value.value().as_string().unwrap(), "defaults");
}
