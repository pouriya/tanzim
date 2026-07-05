//! The configuration pipeline: **load → parse → merge → (unify) → validate**.
//!
//! Two entry points share the same stages but differ in their result shape:
//!
//! - [`single::Single`] collapses every source into one unified configuration value.
//! - [`multi::Multi`] keeps a map of named entries (`None` = the unnamed bucket).
//!
//! Construct either with `default()` (all feature-enabled loaders + parsers, no merger) or
//! `empty()` (nothing registered), add a merger and sources, then `run()` / `try_deserialize()`.
//!
//! Each submodule re-exports everything needed to build a pipeline, so
//! `use tanzim::pipeline::single::*;` (or `::multi::*`) is enough on its own.

use crate::loader;
use crate::parser;

pub mod multi;
pub mod single;

/// A loaded payload paired with the value tree produced by parsing it.
///
/// Fields are private; access them through [`payload`](Self::payload) / [`value`](Self::value)
/// and their `_mut` variants.
#[derive(Debug, Clone, PartialEq)]
pub struct Parsed {
    payload: loader::Payload,
    value: parser::LocatedValue,
}

impl Parsed {
    /// Pair a payload with the value produced by parsing it.
    pub fn new(payload: loader::Payload, value: parser::LocatedValue) -> Self {
        Self { payload, value }
    }

    pub fn payload(&self) -> &loader::Payload {
        &self.payload
    }

    pub fn payload_mut(&mut self) -> &mut loader::Payload {
        &mut self.payload
    }

    pub fn value(&self) -> &parser::LocatedValue {
        &self.value
    }

    pub fn value_mut(&mut self) -> &mut parser::LocatedValue {
        &mut self.value
    }

    /// Split into the payload and its parsed value.
    pub fn into_parts(self) -> (loader::Payload, parser::LocatedValue) {
        (self.payload, self.value)
    }
}

/// One merged entry: the payloads that contributed to it and the combined value.
///
/// Fields are private; access them through [`payloads`](Self::payloads) / [`value`](Self::value)
/// and their `_mut` variants.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    payloads: Vec<loader::Payload>,
    value: parser::LocatedValue,
}

impl Entry {
    /// Build an entry from its contributing payloads and combined value.
    pub fn new(payloads: Vec<loader::Payload>, value: parser::LocatedValue) -> Self {
        Self { payloads, value }
    }

    pub fn payloads(&self) -> &[loader::Payload] {
        &self.payloads
    }

    pub fn payloads_mut(&mut self) -> &mut Vec<loader::Payload> {
        &mut self.payloads
    }

    pub fn value(&self) -> &parser::LocatedValue {
        &self.value
    }

    pub fn value_mut(&mut self) -> &mut parser::LocatedValue {
        &mut self.value
    }

    /// Split into the contributing payloads and the combined value.
    pub fn into_parts(self) -> (Vec<loader::Payload>, parser::LocatedValue) {
        (self.payloads, self.value)
    }
}

/// Merged configuration keyed by entry name (`None` = the unnamed bucket).
///
/// Fields are private; navigate it through the map-like accessors ([`get`](Self::get),
/// [`iter`](Self::iter), [`keys`](Self::keys), …).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Merged {
    entries: std::collections::HashMap<Option<String>, Entry>,
}

impl Merged {
    pub fn new() -> Self {
        Self {
            entries: std::collections::HashMap::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn get(&self, name: &Option<String>) -> Option<&Entry> {
        self.entries.get(name)
    }

    pub fn get_mut(&mut self, name: &Option<String>) -> Option<&mut Entry> {
        self.entries.get_mut(name)
    }

    pub fn insert(&mut self, name: Option<String>, entry: Entry) -> Option<Entry> {
        self.entries.insert(name, entry)
    }

    pub fn remove(&mut self, name: &Option<String>) -> Option<Entry> {
        self.entries.remove(name)
    }

    pub fn contains_key(&self, name: &Option<String>) -> bool {
        self.entries.contains_key(name)
    }

    pub fn keys(&self) -> impl Iterator<Item = &Option<String>> {
        self.entries.keys()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Option<String>, &Entry)> {
        self.entries.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&Option<String>, &mut Entry)> {
        self.entries.iter_mut()
    }

    /// Wrap the raw map returned by a [`crate::merger::Merge`] into [`Entry`]-keyed form.
    pub(crate) fn from_raw(raw: crate::merger::Merged) -> Self {
        let mut entries = std::collections::HashMap::with_capacity(raw.len());
        for (name, (payloads, value)) in raw {
            entries.insert(name, Entry::new(payloads, value));
        }
        Self { entries }
    }
}
