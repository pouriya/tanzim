#![doc = include_str!("../README.md")]

use cfg_if::cfg_if;
use std::collections::HashMap;
use tanzim_load::Payload;
use tanzim_value::{LocatedValue, Map, Value};

pub mod plan;

/// Merge result: entry name → (contributing payloads, merged value).
///
/// Keys come from [`Payload::maybe_name`]: `Some("foo")` → `Some("foo")`, `None` → unnamed bucket (`None`).
pub type Merged = HashMap<Option<String>, (Vec<Payload>, LocatedValue)>;

/// Merges parsed payloads grouped by entry name.
///
/// The returned map keys are derived from [`Payload::maybe_name`]: `Some("foo")` → `Some("foo")`,
/// `None` → unnamed bucket (`None`). The value for each key is `(Vec<payload>, merged_value)`.
///
/// Implement this trait to define a custom merge strategy.
pub trait Merge {
    /// Merge `parsed_list` into a map keyed by entry name.
    ///
    /// Each tuple in `parsed_list` is `(payload, parsed_value)` as produced by
    /// the load and parse stages. The merger groups entries by name and combines values.
    fn merge(&self, parsed_list: &[(Payload, LocatedValue)]) -> Result<Merged, Error>;
}

/// Merge error type.
#[derive(Debug)]
pub enum Error {
    Other(Box<dyn std::error::Error + Send + Sync>),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            // Transparent: forward Display (and its alternate form) to the wrapped error.
            Self::Other(error) => std::fmt::Display::fmt(&**error, f),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            // Delegate past this transparent wrapper so the cause chain continues.
            Self::Other(error) => error.source(),
        }
    }
}

impl From<Box<dyn std::error::Error + Send + Sync>> for Error {
    fn from(error: Box<dyn std::error::Error + Send + Sync>) -> Self {
        Self::Other(error)
    }
}

/// Last-write-wins merger: each name keeps only its last-seen value.
///
/// Payloads with `maybe_name == None` are grouped under the unnamed bucket (`None` key).
pub struct LastWins;

impl Merge for LastWins {
    fn merge(&self, parsed_list: &[(Payload, LocatedValue)]) -> Result<Merged, Error> {
        cfg_if! {
            if #[cfg(feature = "tracing")] {
                tracing::debug!(msg = "Merging configuration with last-wins strategy", entry_count = parsed_list.len());
            } else if #[cfg(feature = "logging")] {
                log::debug!("msg=\"Merging configuration with last-wins strategy\" entry_count={}", parsed_list.len());
            }
        }
        let mut result: Merged = HashMap::new();
        for (payload, value) in parsed_list {
            let key = payload.maybe_name.clone();
            cfg_if! {
                if #[cfg(feature = "tracing")] {
                    tracing::trace!(msg = "Applied last-wins merge entry", name = ?key);
                } else if #[cfg(feature = "logging")] {
                    log::trace!("msg=\"Applied last-wins merge entry\" name={key:?}");
                }
            }
            result.insert(key, (vec![payload.clone()], value.clone()));
        }
        cfg_if! {
            if #[cfg(feature = "tracing")] {
                tracing::info!(msg = "Merged configuration with last-wins strategy", group_count = result.len());
            } else if #[cfg(feature = "logging")] {
                log::info!("msg=\"Merged configuration with last-wins strategy\" group_count={}", result.len());
            }
        }
        Ok(result)
    }
}

/// How [`DeepMerge`] combines two lists occupying the same key.
///
/// Only applies when *both* the base and overlay values at a position are lists; a list facing a
/// non-list (or vice versa) always falls through to "overlay wins", like any other scalar.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ArrayStrategy {
    /// Overlay replaces base outright (the default; matches pre-strategy behaviour).
    #[default]
    Replace,
    /// `base ++ overlay`.
    Concat,
    /// `overlay ++ base`.
    Prepend,
    /// Concatenate, then drop later elements equal (by [`Value`]) to an earlier one.
    Union,
    /// Zip by position: deep-merge overlapping indices, then append the longer list's tail.
    Index,
    /// Lists of maps merged by a shared key: each overlay element is deep-merged into the base
    /// element with an equal [`Value`] at `key`, else appended. Elements lacking the key (on
    /// either side) are appended. Base order is preserved.
    Keyed(String),
}

/// Deep-merge merger: maps with the same name are merged recursively.
///
/// For each key in a map: if both the accumulated and incoming values are maps, the merge
/// recurses. Two lists are combined according to the configured [`ArrayStrategy`] (default
/// [`Replace`](ArrayStrategy::Replace)). Otherwise the incoming (overlay) value and its location
/// win. Payloads with `maybe_name == None` are grouped under the unnamed bucket (`None` key).
#[derive(Debug, Clone, Default)]
pub struct DeepMerge {
    array_strategy: ArrayStrategy,
}

impl DeepMerge {
    /// A deep merger with the default [`Replace`](ArrayStrategy::Replace) array strategy.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set how same-key lists are combined.
    pub fn with_array_strategy(mut self, array_strategy: ArrayStrategy) -> Self {
        self.array_strategy = array_strategy;
        self
    }

    /// The configured array strategy.
    pub fn array_strategy(&self) -> &ArrayStrategy {
        &self.array_strategy
    }
}

fn deep_merge_value(
    base: LocatedValue,
    overlay: LocatedValue,
    strategy: &ArrayStrategy,
) -> LocatedValue {
    match (base.value(), overlay.value()) {
        (Value::Map(base_map), Value::Map(overlay_map)) => {
            let mut result_map = Map::new();
            for (key, base_val) in base_map.entries() {
                if let Some(overlay_val) = overlay_map.get(key) {
                    result_map.insert(
                        key.clone(),
                        deep_merge_value(base_val.clone(), overlay_val.clone(), strategy),
                    );
                } else {
                    result_map.insert(key.clone(), base_val.clone());
                }
            }
            for (key, overlay_val) in overlay_map.entries() {
                if !result_map.contains_key(key) {
                    result_map.insert(key.clone(), overlay_val.clone());
                }
            }
            LocatedValue::new(Value::Map(result_map), overlay.location().clone())
        }
        (Value::List(base_list), Value::List(overlay_list)) => {
            if matches!(strategy, ArrayStrategy::Replace) {
                return overlay;
            }
            let merged = merge_lists(base_list, overlay_list, strategy);
            LocatedValue::new(Value::List(merged), overlay.location().clone())
        }
        _ => overlay,
    }
}

fn merge_lists(
    base: &[LocatedValue],
    overlay: &[LocatedValue],
    strategy: &ArrayStrategy,
) -> Vec<LocatedValue> {
    match strategy {
        // Handled by the caller (returns the overlay `LocatedValue` untouched), but keep an arm
        // so this stays total.
        ArrayStrategy::Replace => overlay.to_vec(),
        ArrayStrategy::Concat => base.iter().chain(overlay).cloned().collect(),
        ArrayStrategy::Prepend => overlay.iter().chain(base).cloned().collect(),
        ArrayStrategy::Union => {
            let mut result: Vec<LocatedValue> = Vec::new();
            for item in base.iter().chain(overlay) {
                if !result
                    .iter()
                    .any(|existing| existing.value() == item.value())
                {
                    result.push(item.clone());
                }
            }
            result
        }
        ArrayStrategy::Index => {
            let overlap = base.len().min(overlay.len());
            let mut result = Vec::with_capacity(base.len().max(overlay.len()));
            for i in 0..overlap {
                result.push(deep_merge_value(
                    base[i].clone(),
                    overlay[i].clone(),
                    strategy,
                ));
            }
            result.extend(base[overlap..].iter().cloned());
            result.extend(overlay[overlap..].iter().cloned());
            result
        }
        ArrayStrategy::Keyed(key) => {
            let key_of = |lv: &LocatedValue| -> Option<Value> {
                lv.value()
                    .as_map()
                    .and_then(|m| m.get(key.as_str()))
                    .map(|v| v.value().clone())
            };
            let mut result: Vec<LocatedValue> = base.to_vec();
            for over in overlay {
                let matched = key_of(over).and_then(|ov_key| {
                    result
                        .iter()
                        .position(|b| key_of(b) == Some(ov_key.clone()))
                });
                match matched {
                    Some(i) => {
                        result[i] = deep_merge_value(result[i].clone(), over.clone(), strategy);
                    }
                    None => result.push(over.clone()),
                }
            }
            result
        }
    }
}

impl Merge for DeepMerge {
    fn merge(&self, parsed_list: &[(Payload, LocatedValue)]) -> Result<Merged, Error> {
        cfg_if! {
            if #[cfg(feature = "tracing")] {
                tracing::debug!(msg = "Merging configuration with deep-merge strategy", entry_count = parsed_list.len());
            } else if #[cfg(feature = "logging")] {
                log::debug!("msg=\"Merging configuration with deep-merge strategy\" entry_count={}", parsed_list.len());
            }
        }
        let mut result: Merged = HashMap::new();

        for (payload, value) in parsed_list {
            let key = payload.maybe_name.clone();

            if let Some(existing) = result.get_mut(&key) {
                cfg_if! {
                    if #[cfg(feature = "tracing")] {
                        tracing::debug!(msg = "Deep-merging into existing entry", name = ?key);
                    } else if #[cfg(feature = "logging")] {
                        log::debug!("msg=\"Deep-merging into existing entry\" name={key:?}");
                    }
                }
                existing.0.push(payload.clone());
                let merged =
                    deep_merge_value(existing.1.clone(), value.clone(), &self.array_strategy);
                existing.1 = merged;
            } else {
                cfg_if! {
                    if #[cfg(feature = "tracing")] {
                        tracing::trace!(msg = "Added new deep-merge entry", name = ?key);
                    } else if #[cfg(feature = "logging")] {
                        log::trace!("msg=\"Added new deep-merge entry\" name={key:?}");
                    }
                }
                result.insert(key, (vec![payload.clone()], value.clone()));
            }
        }

        cfg_if! {
            if #[cfg(feature = "tracing")] {
                tracing::info!(msg = "Merged configuration with deep-merge strategy", group_count = result.len());
            } else if #[cfg(feature = "logging")] {
                log::info!("msg=\"Merged configuration with deep-merge strategy\" group_count={}", result.len());
            }
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tanzim_load::Payload;
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
}
