#![doc = include_str!("../README.md")]

mod error;
mod value;

pub use error::Error;
pub use value::{LocatedValue, Location, Map, Value, ValueType};
