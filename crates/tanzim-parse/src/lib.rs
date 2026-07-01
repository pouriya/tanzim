#![doc = include_str!("../README.md")]

mod span;

pub use tanzim_value::{Error, LocatedValue, Value};

pub mod closure;

#[cfg(feature = "env")]
mod env;
#[cfg(feature = "json")]
mod json;
#[cfg(feature = "toml")]
mod toml;
#[cfg(feature = "yaml")]
mod yaml;

#[cfg(feature = "env")]
pub use env::Env;
#[cfg(feature = "json")]
pub use json::Json;
#[cfg(feature = "toml")]
pub use toml::Toml;
#[cfg(feature = "yaml")]
pub use yaml::Yaml;

/// Deserializes raw bytes into a [`LocatedValue`] tree for one format.
///
/// Implement this to add a new configuration format. Every node in the returned
/// tree should carry a [`tanzim_value::Location`] that points back to the
/// source file and line so that downstream error messages can show users exactly
/// where a bad value came from.
///
/// # Auto-detection
///
/// When a payload's `format` hint is `None`, the parse stage calls
/// [`is_format_supported`][Deserialize::is_format_supported] on each registered
/// parser in order. Return `Some(true)` if confident, `Some(false)` to skip, or `None`
/// if unsure (another parser may then claim the bytes).
///
/// # Example — custom CSV parser
///
/// ```rust
/// use tanzim_parse::{Deserialize, Error, LocatedValue, Value};
/// use tanzim_value::{Location, Map};
///
/// struct CsvParser;
///
/// impl Deserialize for CsvParser {
///     fn name(&self) -> &str { "csv" }
///     fn supported_format_list(&self) -> Vec<String> { vec!["csv".into()] }
///     fn is_format_supported(&self, bytes: &[u8]) -> Option<bool> {
///         Some(bytes.contains(&b','))
///     }
///     fn parse(&self, source: &str, resource: &str, bytes: &[u8])
///         -> Result<LocatedValue, Error>
///     {
///         let text = std::str::from_utf8(bytes).map_err(|_| Error::InvalidUtf8 {
///             location: Location::at(source, resource, None, None, None),
///         })?;
///         let mut map = Map::new();
///         for (line_idx, line) in text.lines().enumerate() {
///             if let Some((key, val)) = line.split_once(',') {
///                 let loc = Location::at(source, resource, Some(line_idx + 1), None, None);
///                 map.insert(key.trim().to_string(), LocatedValue {
///                     value: Value::String(val.trim().to_string()),
///                     location: loc,
///                 });
///             }
///         }
///         let root_loc = Location::at(source, resource, None, None, None);
///         Ok(LocatedValue { value: Value::Map(map), location: root_loc })
///     }
/// }
/// ```
pub trait Deserialize {
    /// Human-readable name used in error messages.
    fn name(&self) -> &str;
    /// Format extensions this parser handles (e.g. `["json"]`, `["yml", "yaml"]`).
    fn supported_format_list(&self) -> Vec<String>;
    /// Probe `bytes` for auto-detection when `Payload::maybe_format` is `None`.
    ///
    /// Return `Some(true)` if confident, `Some(false)` if definitely not this format,
    /// or `None` to abstain (another parser will be tried next).
    fn is_format_supported(&self, bytes: &[u8]) -> Option<bool>;
    /// Deserialize `bytes` into a [`LocatedValue`] tree.
    ///
    /// `source` is the source kind (e.g. `"file"`) and `resource` is the path or
    /// identifier; both are used to populate [`tanzim_value::Location`] on every
    /// node in the returned tree.
    fn parse(&self, source: &str, resource: &str, bytes: &[u8]) -> Result<LocatedValue, Error>;
}
