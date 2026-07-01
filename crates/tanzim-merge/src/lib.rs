#![doc = include_str!("../README.md")]

use cfg_if::cfg_if;
use std::collections::HashMap;
use tanzim_load::Payload;
use tanzim_value::{LocatedValue, Map, Value};

/// Merge result: entry name → (contributing payloads, merged value).
///
/// Keys come from [`Payload::maybe_name`]: `Some("foo")` → `"foo"`, `None` → `""`.
pub type Merged = HashMap<String, (Vec<Payload>, LocatedValue)>;

/// Merges parsed payloads grouped by entry name.
///
/// The returned map keys are derived from [`Payload::maybe_name`]: `Some("foo")` → `"foo"`,
/// `None` → `""`. The value for each key is `(Vec<payload>, merged_value)`.
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
/// Payloads with `maybe_name == None` are grouped under an empty-string key.
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
            let key = match &payload.maybe_name {
                Some(name) => name.clone(),
                None => String::new(),
            };
            cfg_if! {
                if #[cfg(feature = "tracing")] {
                    tracing::trace!(msg = "Applied last-wins merge entry", name = key);
                } else if #[cfg(feature = "logging")] {
                    log::trace!("msg=\"Applied last-wins merge entry\" name={key}");
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
/// Payloads with `maybe_name == None` are grouped under an empty-string key.
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
            let key = match &payload.maybe_name {
                Some(name) => name.clone(),
                None => String::new(),
            };

            if let Some(existing) = result.get_mut(&key) {
                cfg_if! {
                    if #[cfg(feature = "tracing")] {
                        tracing::debug!(msg = "Deep-merging into existing entry", name = key);
                    } else if #[cfg(feature = "logging")] {
                        log::debug!("msg=\"Deep-merging into existing entry\" name={key}");
                    }
                }
                existing.0.push(payload.clone());
                let merged = deep_merge_value(existing.1.clone(), value.clone());
                existing.1 = merged;
            } else {
                cfg_if! {
                    if #[cfg(feature = "tracing")] {
                        tracing::trace!(msg = "Added new deep-merge entry", name = key);
                    } else if #[cfg(feature = "logging")] {
                        log::trace!("msg=\"Added new deep-merge entry\" name={key}");
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
