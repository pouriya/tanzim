#![doc = include_str!("../README.md")]

use cfg_if::cfg_if;
use std::collections::HashMap;
use tanzim_load::Payload;
use tanzim_value::{LocatedValue, Map, Value};

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
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Other(#[from] Box<dyn std::error::Error + Send + Sync>),
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

/// Deep-merge merger: maps with the same name are merged recursively.
///
/// For each key in a map: if both the accumulated and incoming values are maps,
/// the merge recurses. Otherwise the incoming (overlay) value and its location win.
/// Payloads with `maybe_name == None` are grouped under the unnamed bucket (`None` key).
pub struct DeepMerge;

fn deep_merge_value(base: LocatedValue, overlay: LocatedValue) -> LocatedValue {
    if let (Value::Map(base_map), Value::Map(overlay_map)) = (&base.value, &overlay.value) {
        let mut result_map = Map::new();
        let base_entries = base_map.entries();
        let overlay_entries = overlay_map.entries();

        for (key, base_val) in base_entries {
            if let Some(overlay_val) = overlay_map.get(key) {
                result_map.insert(
                    key.clone(),
                    deep_merge_value(base_val.clone(), overlay_val.clone()),
                );
            } else {
                result_map.insert(key.clone(), base_val.clone());
            }
        }

        for (key, overlay_val) in overlay_entries {
            if !result_map.contains_key(key) {
                result_map.insert(key.clone(), overlay_val.clone());
            }
        }

        return LocatedValue {
            value: Value::Map(result_map),
            location: overlay.location.clone(),
        };
    }
    overlay
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
                let merged = deep_merge_value(existing.1.clone(), value.clone());
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
        LocatedValue {
            value: Value::String(text.to_string()),
            location: Location::at("mock", "test", None, None, None),
        }
    }

    fn map_value(entries: &[(&str, &str)]) -> LocatedValue {
        let mut map = Map::new();
        for (key, value) in entries {
            map.insert(key.to_string(), string_value(value));
        }
        LocatedValue {
            value: Value::Map(map),
            location: Location::at("mock", "test", None, None, None),
        }
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
        assert_eq!(value.value.as_string().unwrap(), "second");
    }

    #[test]
    fn last_wins_groups_unnamed_entries() {
        let parsed = vec![
            (payload(None), string_value("first")),
            (payload(None), string_value("second")),
        ];
        let merged = LastWins.merge(&parsed).unwrap();
        let (_, value) = merged.get(&None).unwrap();
        assert_eq!(value.value.as_string().unwrap(), "second");
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
                .value
                .as_string()
                .unwrap(),
            "a"
        );
        assert_eq!(
            merged
                .get(&Some("beta".into()))
                .unwrap()
                .1
                .value
                .as_string()
                .unwrap(),
            "b"
        );
    }

    #[test]
    fn deep_merge_empty_input() {
        let merged = DeepMerge.merge(&[]).unwrap();
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
        let merged = DeepMerge.merge(&parsed).unwrap();
        let (payloads, value) = merged.get(&Some("app".into())).unwrap();
        assert_eq!(payloads.len(), 2);
        let map = value.value.as_map().unwrap();
        assert_eq!(
            map.get("host").unwrap().value.as_string().unwrap(),
            "localhost"
        );
        assert_eq!(map.get("port").unwrap().value.as_string().unwrap(), "9090");
        assert_eq!(map.get("debug").unwrap().value.as_string().unwrap(), "true");
    }

    #[test]
    fn deep_merge_scalar_overlay_replaces_map() {
        let parsed = vec![
            (payload(Some("app")), map_value(&[("mode", "auto")])),
            (payload(Some("app")), string_value("override")),
        ];
        let merged = DeepMerge.merge(&parsed).unwrap();
        let (_, value) = merged.get(&Some("app".into())).unwrap();
        assert_eq!(value.value.as_string().unwrap(), "override");
    }

    #[test]
    fn deep_merge_unnamed_bucket() {
        let parsed = vec![
            (payload(None), map_value(&[("a", "1")])),
            (payload(None), map_value(&[("b", "2")])),
        ];
        let merged = DeepMerge.merge(&parsed).unwrap();
        let (payloads, value) = merged.get(&None).unwrap();
        assert_eq!(payloads.len(), 2);
        let map = value.value.as_map().unwrap();
        assert_eq!(map.get("a").unwrap().value.as_string().unwrap(), "1");
        assert_eq!(map.get("b").unwrap().value.as_string().unwrap(), "2");
    }
}
