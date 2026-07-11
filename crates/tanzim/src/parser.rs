//! Configuration parsers: turn a raw [`Payload`](crate::loader::Payload) into a
//! [`LocatedValue`] tree that remembers its origin. Re-exports [`tanzim_parse`]; see that crate
//! for the [`Parse`] trait and the `env` / `json` / `yaml` / `toml` parsers.

pub use tanzim_parse::*;

use crate::loader;

/// A loaded payload paired with the value tree produced by parsing it.
///
/// Fields are private; access them through [`payload`](Self::payload) / [`value`](Self::value)
/// and their `_mut` variants.
#[derive(Debug, Clone, PartialEq)]
pub struct Parsed {
    payload: loader::Payload,
    value: LocatedValue,
}

impl Parsed {
    /// Pair a payload with the value produced by parsing it.
    pub fn new(payload: loader::Payload, value: LocatedValue) -> Self {
        Self { payload, value }
    }

    /// The loaded payload.
    pub fn payload(&self) -> &loader::Payload {
        &self.payload
    }

    /// Mutable access to the loaded payload.
    pub fn payload_mut(&mut self) -> &mut loader::Payload {
        &mut self.payload
    }

    /// The value produced by parsing the payload.
    pub fn value(&self) -> &LocatedValue {
        &self.value
    }

    /// Mutable access to the parsed value.
    pub fn value_mut(&mut self) -> &mut LocatedValue {
        &mut self.value
    }

    /// Split into the payload and its parsed value.
    pub fn into_parts(self) -> (loader::Payload, LocatedValue) {
        (self.payload, self.value)
    }
}
