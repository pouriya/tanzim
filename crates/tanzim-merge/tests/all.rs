use tanzim_load::Payload;
use tanzim_merge::{ArrayStrategy, DeepMerge, LastWins, Merge};
use tanzim_source::SourceBuilder;
use tanzim_value::{LocatedValue, Location, Map, Value};

fn source() -> tanzim_source::Source {
    SourceBuilder::new()
        .with_source("mock")
        .with_resource("test")
        .build()
        .unwrap()
}

fn payload(name: Option<&str>) -> Payload {
    Payload {
        source: source(),
        maybe_name: name.map(str::to_string),
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

fn list_value(items: Vec<LocatedValue>) -> LocatedValue {
    LocatedValue::new(
        Value::List(items),
        Location::at("mock", "test", None, None, None),
    )
}

fn null_value() -> LocatedValue {
    LocatedValue::new(Value::Null, Location::at("mock", "test", None, None, None))
}

fn located_map(entries: Vec<(&str, LocatedValue)>) -> LocatedValue {
    let mut map = Map::new();
    for (key, value) in entries {
        map.insert(key.to_string(), value);
    }
    LocatedValue::new(
        Value::Map(map),
        Location::at("mock", "test", None, None, None),
    )
}

/// Deep-merge two named-`app` list values under `strategy` and return the merged list's items.
fn merge_lists_via(
    base: LocatedValue,
    overlay: LocatedValue,
    strategy: ArrayStrategy,
) -> Vec<LocatedValue> {
    let parsed = vec![
        (payload(Some("app")), base),
        (payload(Some("app")), overlay),
    ];
    let merged = DeepMerge::new()
        .with_array_strategy(strategy)
        .merge(&parsed)
        .unwrap();
    merged
        .get(&Some("app".into()))
        .unwrap()
        .1
        .value()
        .as_list()
        .unwrap()
        .clone()
}

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

fn list_strings(items: &[LocatedValue]) -> Vec<String> {
    items
        .iter()
        .map(|lv| lv.value().as_string().unwrap().clone())
        .collect()
}

#[test]
fn last_wins_empty_input() {
    let merged = LastWins.merge(&[]).unwrap();
    assert!(merged.is_empty());
}

#[test]
fn last_wins_keeps_last_value_for_same_name() {
    let parsed = vec![
        (payload(Some("app")), string_value("first")),
        (payload(Some("app")), string_value("second")),
    ];
    let merged = LastWins.merge(&parsed).unwrap();
    let (_, value) = merged.get(&Some("app".into())).unwrap();
    assert_eq!(value.value().as_string().unwrap(), "second");
}

#[test]
fn last_wins_groups_unnamed_entries() {
    let parsed = vec![
        (payload(None), string_value("first")),
        (payload(None), string_value("second")),
    ];
    let merged = LastWins.merge(&parsed).unwrap();
    let (_, value) = merged.get(&None).unwrap();
    assert_eq!(value.value().as_string().unwrap(), "second");
}

#[test]
fn last_wins_distinct_names() {
    let parsed = vec![
        (payload(Some("alpha")), string_value("a")),
        (payload(Some("beta")), string_value("b")),
    ];
    let merged = LastWins.merge(&parsed).unwrap();
    assert_eq!(merged.len(), 2);
    assert_eq!(
        merged
            .get(&Some("alpha".into()))
            .unwrap()
            .1
            .value()
            .as_string()
            .unwrap(),
        "a"
    );
    assert_eq!(
        merged
            .get(&Some("beta".into()))
            .unwrap()
            .1
            .value()
            .as_string()
            .unwrap(),
        "b"
    );
}

#[test]
fn deep_merge_empty_input() {
    let merged = DeepMerge::new().merge(&[]).unwrap();
    assert!(merged.is_empty());
}

#[test]
fn deep_merge_recurses_into_shared_map_keys() {
    let parsed = vec![
        (
            payload(Some("app")),
            map_value(&[("host", "localhost"), ("port", "8080")]),
        ),
        (
            payload(Some("app")),
            map_value(&[("port", "9090"), ("debug", "true")]),
        ),
    ];
    let merged = DeepMerge::new().merge(&parsed).unwrap();
    let (payloads, value) = merged.get(&Some("app".into())).unwrap();
    assert_eq!(payloads.len(), 2);
    let map = value.value().as_map().unwrap();
    assert_eq!(
        map.get("host").unwrap().value().as_string().unwrap(),
        "localhost"
    );
    assert_eq!(
        map.get("port").unwrap().value().as_string().unwrap(),
        "9090"
    );
    assert_eq!(
        map.get("debug").unwrap().value().as_string().unwrap(),
        "true"
    );
}

#[test]
fn deep_merge_scalar_overlay_replaces_map() {
    let parsed = vec![
        (payload(Some("app")), map_value(&[("mode", "auto")])),
        (payload(Some("app")), string_value("override")),
    ];
    let merged = DeepMerge::new().merge(&parsed).unwrap();
    let (_, value) = merged.get(&Some("app".into())).unwrap();
    assert_eq!(value.value().as_string().unwrap(), "override");
}

#[test]
fn deep_merge_null_overlay_deletes_map_key() {
    let parsed = vec![
        (
            payload(Some("app")),
            map_value(&[("host", "localhost"), ("port", "8080")]),
        ),
        (
            payload(Some("app")),
            located_map(vec![("port", null_value())]),
        ),
    ];
    let merged = DeepMerge::new().merge(&parsed).unwrap();
    let map = merged
        .get(&Some("app".into()))
        .unwrap()
        .1
        .value()
        .as_map()
        .unwrap();
    assert_eq!(
        map.get("host").unwrap().value().as_string().unwrap(),
        "localhost"
    );
    assert!(!map.contains_key("port"));
}

#[test]
fn deep_merge_null_overlay_only_does_not_insert_key() {
    let parsed = vec![
        (payload(Some("app")), map_value(&[("host", "localhost")])),
        (
            payload(Some("app")),
            located_map(vec![("debug", null_value())]),
        ),
    ];
    let merged = DeepMerge::new().merge(&parsed).unwrap();
    let map = merged
        .get(&Some("app".into()))
        .unwrap()
        .1
        .value()
        .as_map()
        .unwrap();
    assert!(map.contains_key("host"));
    assert!(!map.contains_key("debug"));
}

#[test]
fn deep_merge_null_overlay_deletes_nested_map_key() {
    let base = located_map(vec![(
        "server",
        map_value(&[("host", "localhost"), ("port", "8080")]),
    )]);
    let overlay = located_map(vec![("server", located_map(vec![("port", null_value())]))]);
    let parsed = vec![
        (payload(Some("app")), base),
        (payload(Some("app")), overlay),
    ];
    let merged = DeepMerge::new().merge(&parsed).unwrap();
    let server = merged
        .get(&Some("app".into()))
        .unwrap()
        .1
        .value()
        .as_map()
        .unwrap()
        .get("server")
        .unwrap()
        .value()
        .as_map()
        .unwrap();
    assert_eq!(
        server.get("host").unwrap().value().as_string().unwrap(),
        "localhost"
    );
    assert!(!server.contains_key("port"));
}

#[test]
fn last_wins_keeps_null_as_value() {
    // LastWins replaces the whole document; null-as-delete is DeepMerge-only.
    let parsed = vec![
        (payload(Some("app")), map_value(&[("port", "8080")])),
        (
            payload(Some("app")),
            located_map(vec![("port", null_value())]),
        ),
    ];
    let merged = LastWins.merge(&parsed).unwrap();
    let map = merged
        .get(&Some("app".into()))
        .unwrap()
        .1
        .value()
        .as_map()
        .unwrap();
    assert!(map.get("port").unwrap().value().is_null());
}

#[test]
fn deep_merge_unnamed_bucket() {
    let parsed = vec![
        (payload(None), map_value(&[("a", "1")])),
        (payload(None), map_value(&[("b", "2")])),
    ];
    let merged = DeepMerge::new().merge(&parsed).unwrap();
    let (payloads, value) = merged.get(&None).unwrap();
    assert_eq!(payloads.len(), 2);
    let map = value.value().as_map().unwrap();
    assert_eq!(map.get("a").unwrap().value().as_string().unwrap(), "1");
    assert_eq!(map.get("b").unwrap().value().as_string().unwrap(), "2");
}

#[test]
fn array_strategy_replace_is_default() {
    // Default merger uses `Replace`: the overlay list wins outright.
    let parsed = vec![
        (
            payload(Some("app")),
            list_value(vec![string_value("a"), string_value("b")]),
        ),
        (payload(Some("app")), list_value(vec![string_value("c")])),
    ];
    let merged = DeepMerge::new().merge(&parsed).unwrap();
    let list = merged
        .get(&Some("app".into()))
        .unwrap()
        .1
        .value()
        .as_list()
        .unwrap();
    assert_eq!(list_strings(list), strings(&["c"]));
}

#[test]
fn array_strategy_concat() {
    let out = merge_lists_via(
        list_value(vec![string_value("a"), string_value("b")]),
        list_value(vec![string_value("c")]),
        ArrayStrategy::Concat,
    );
    assert_eq!(list_strings(&out), strings(&["a", "b", "c"]));
}

#[test]
fn array_strategy_prepend() {
    let out = merge_lists_via(
        list_value(vec![string_value("a"), string_value("b")]),
        list_value(vec![string_value("c")]),
        ArrayStrategy::Prepend,
    );
    assert_eq!(list_strings(&out), strings(&["c", "a", "b"]));
}

#[test]
fn array_strategy_union_dedupes() {
    let out = merge_lists_via(
        list_value(vec![string_value("a"), string_value("b")]),
        list_value(vec![string_value("b"), string_value("c")]),
        ArrayStrategy::Union,
    );
    assert_eq!(list_strings(&out), strings(&["a", "b", "c"]));
}

#[test]
fn array_strategy_index_zips_and_appends_tail() {
    // Overlapping positions recurse (here scalars → overlay wins), then the longer list's tail
    // is appended.
    let out = merge_lists_via(
        list_value(vec![string_value("a"), string_value("b")]),
        list_value(vec![
            string_value("x"),
            string_value("y"),
            string_value("z"),
        ]),
        ArrayStrategy::Index,
    );
    assert_eq!(list_strings(&out), strings(&["x", "y", "z"]));
}

#[test]
fn array_strategy_index_recurses_into_maps() {
    let out = merge_lists_via(
        list_value(vec![map_value(&[("host", "a"), ("port", "1")])]),
        list_value(vec![map_value(&[("port", "2")])]),
        ArrayStrategy::Index,
    );
    assert_eq!(out.len(), 1);
    let map = out[0].value().as_map().unwrap();
    assert_eq!(map.get("host").unwrap().value().as_string().unwrap(), "a");
    assert_eq!(map.get("port").unwrap().value().as_string().unwrap(), "2");
}

#[test]
fn array_strategy_keyed_merges_matches_and_appends_rest() {
    // Base order preserved: `id=1` deep-merges, `id=2` (unmatched overlay) appends.
    let base = list_value(vec![
        map_value(&[("id", "1"), ("host", "a")]),
        map_value(&[("id", "3"), ("host", "c")]),
    ]);
    let overlay = list_value(vec![
        map_value(&[("id", "1"), ("port", "8080")]),
        map_value(&[("id", "2"), ("host", "b")]),
    ]);
    let out = merge_lists_via(base, overlay, ArrayStrategy::Keyed("id".into()));
    assert_eq!(out.len(), 3);
    // Element 0: id=1 with host from base + port from overlay.
    let e0 = out[0].value().as_map().unwrap();
    assert_eq!(e0.get("id").unwrap().value().as_string().unwrap(), "1");
    assert_eq!(e0.get("host").unwrap().value().as_string().unwrap(), "a");
    assert_eq!(e0.get("port").unwrap().value().as_string().unwrap(), "8080");
    // Element 1: id=3 untouched (base order preserved).
    assert_eq!(
        out[1]
            .value()
            .as_map()
            .unwrap()
            .get("id")
            .unwrap()
            .value()
            .as_string()
            .unwrap(),
        "3"
    );
    // Element 2: id=2 appended.
    assert_eq!(
        out[2]
            .value()
            .as_map()
            .unwrap()
            .get("id")
            .unwrap()
            .value()
            .as_string()
            .unwrap(),
        "2"
    );
}
