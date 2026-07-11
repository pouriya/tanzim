#![doc = include_str!("../README.md")]
#![deny(missing_docs)]

use cfg_if::cfg_if;
use std::collections::HashMap;
use tanzim_load::Payload;
use tanzim_value::{LocatedValue, Map, Value};

pub mod plan;

/// Merge result: entry name → (contributing payloads, merged value).
///
/// Keys come from [`Payload::maybe_name`]: `Some("foo")` → `Some("foo")`, `None` → unnamed bucket (`None`).
///
/// # Examples
///
/// ```rust
/// use tanzim_load::Payload;
/// use tanzim_merge::{LastWins, Merge};
/// use tanzim_source::SourceBuilder;
/// use tanzim_value::{LocatedValue, Location, Value};
///
/// let source = SourceBuilder::new().with_source("mock").build()?;
/// let payload = Payload {
///     source,
///     maybe_name: Some("app".to_string()),
///     maybe_format: None,
///     content: Vec::new(),
/// };
/// let value = LocatedValue::new(
///     Value::String("hello".to_string()),
///     Location::at("mock", "test", None, None, None),
/// );
///
/// // `Merged` is what every `Merge` implementation returns.
/// let merged = LastWins.merge(&[(payload, value)])?;
/// let (payloads, value) = &merged[&Some("app".to_string())];
/// assert_eq!(payloads.len(), 1);
/// assert_eq!(value.value().as_string().unwrap(), "hello");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub type Merged = HashMap<Option<String>, (Vec<Payload>, LocatedValue)>;

/// Merges parsed payloads grouped by entry name.
///
/// The returned map keys are derived from [`Payload::maybe_name`]: `Some("foo")` → `Some("foo")`,
/// `None` → unnamed bucket (`None`). The value for each key is `(Vec<payload>, merged_value)`.
///
/// Implement this trait to define a custom merge strategy.
///
/// # Examples
///
/// ```rust
/// use tanzim_load::Payload;
/// use tanzim_merge::{LastWins, Merge};
/// use tanzim_source::SourceBuilder;
/// use tanzim_value::{LocatedValue, Location, Value};
///
/// let source = SourceBuilder::new().with_source("mock").build()?;
/// let older = Payload {
///     source: source.clone(),
///     maybe_name: None,
///     maybe_format: None,
///     content: Vec::new(),
/// };
/// let newer = older.clone();
///
/// let parsed = vec![
///     (older, LocatedValue::new(Value::from(1isize), Location::at("mock", "a", None, None, None))),
///     (newer, LocatedValue::new(Value::from(2isize), Location::at("mock", "b", None, None, None))),
/// ];
///
/// // `LastWins` is one built-in `Merge` implementation: the later payload wins.
/// let merged = LastWins.merge(&parsed)?;
/// assert_eq!(merged[&None].1.value().as_int(), Some(2));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
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
    /// A wrapped, opaque error (e.g. from parsing a [`plan::src`] string).
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
///
/// # Examples
///
/// ```rust
/// use tanzim_load::Payload;
/// use tanzim_merge::{LastWins, Merge};
/// use tanzim_source::SourceBuilder;
/// use tanzim_value::{LocatedValue, Location, Value};
///
/// let source = SourceBuilder::new().with_source("mock").build()?;
/// let payload = || Payload {
///     source: source.clone(),
///     maybe_name: Some("app".to_string()),
///     maybe_format: None,
///     content: Vec::new(),
/// };
/// let base = LocatedValue::new(Value::from("base"), Location::at("mock", "a", None, None, None));
/// let overlay = LocatedValue::new(Value::from("overlay"), Location::at("mock", "b", None, None, None));
///
/// let merged = LastWins.merge(&[(payload(), base), (payload(), overlay)])?;
/// assert_eq!(merged[&Some("app".to_string())].1.value().as_string().unwrap(), "overlay");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
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
///
/// # Examples
///
/// ```rust
/// use tanzim_load::Payload;
/// use tanzim_merge::{DeepMerge, Merge};
/// use tanzim_source::SourceBuilder;
/// use tanzim_value::{LocatedValue, Location, Map, Value};
///
/// let source = SourceBuilder::new().with_source("mock").build()?;
/// let payload = || Payload {
///     source: source.clone(),
///     maybe_name: None,
///     maybe_format: None,
///     content: Vec::new(),
/// };
/// let location = || Location::at("mock", "test", None, None, None);
///
/// let mut base_map = Map::new();
/// base_map.insert("host".to_string(), LocatedValue::new(Value::from("localhost"), location()));
/// base_map.insert("port".to_string(), LocatedValue::new(Value::from(80isize), location()));
/// let base = LocatedValue::new(Value::Map(base_map), location());
///
/// let mut overlay_map = Map::new();
/// overlay_map.insert("port".to_string(), LocatedValue::new(Value::from(443isize), location()));
/// let overlay = LocatedValue::new(Value::Map(overlay_map), location());
///
/// // Both maps are unnamed, so they merge into the same (`None`) bucket, recursively.
/// let merged = DeepMerge::new().merge(&[(payload(), base), (payload(), overlay)])?;
/// let result = merged[&None].1.value().as_map().unwrap();
/// assert_eq!(result.get("host").unwrap().value().as_string().unwrap(), "localhost");
/// assert_eq!(result.get("port").unwrap().value().as_int(), Some(443));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
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
