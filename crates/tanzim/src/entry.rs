//! One merged configuration entry.
//!
//! [`Entry`] is what falls out of the merge stage for a single entry name: the payloads that
//! contributed to it and the combined [`LocatedValue`](crate::parser::LocatedValue). [`Config`]
//! collapses everything into one `Entry`; [`Pipeline`] keeps an [`Entries`](crate::merger::Entries)
//! map of them keyed by [`EntryName`](crate::merger::EntryName).
//!
//! [`Config`]: crate::Config
//! [`Pipeline`]: crate::pipeline::Pipeline

use crate::loader;
use crate::parser;

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

    /// The payloads that contributed to this entry.
    pub fn payloads(&self) -> &[loader::Payload] {
        &self.payloads
    }

    /// Mutable access to the contributing payloads.
    pub fn payloads_mut(&mut self) -> &mut Vec<loader::Payload> {
        &mut self.payloads
    }

    /// The combined value.
    pub fn value(&self) -> &parser::LocatedValue {
        &self.value
    }

    /// Mutable access to the combined value.
    pub fn value_mut(&mut self) -> &mut parser::LocatedValue {
        &mut self.value
    }

    /// Split into the contributing payloads and the combined value.
    pub fn into_parts(self) -> (Vec<loader::Payload>, parser::LocatedValue) {
        (self.payloads, self.value)
    }
}
