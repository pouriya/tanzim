#![doc = include_str!("../README.md")]

mod error;
#[cfg(feature = "serde")]
mod serde;
mod value;

pub use error::Error;
pub use value::{Comment, LocatedValue, Location, Map, Value, ValueType};
