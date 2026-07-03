#![doc = include_str!("../README.md")]

pub use tanzim_source::Source;
pub use tanzim_value::{Error, LocatedValue, Value};

pub mod closure;
pub mod span;

#[cfg(feature = "env")]
pub mod env;
#[cfg(feature = "json")]
pub mod json;
#[cfg(feature = "toml")]
pub mod toml;
#[cfg(feature = "yaml")]
pub mod yaml;

/// Parses raw bytes into a [`LocatedValue`] tree for one format.
///
/// Implement this to add a new configuration format. This is the second pipeline stage: it turns
/// the raw bytes a loader produced into a typed, source-located value tree for merging.
///
/// # Contract
///
/// - [`parse`](Parse::parse) returns one [`LocatedValue`] tree per payload. `source` carries the
///   source kind (e.g. `"file"`), the resource path/identifier, and any loader options.
/// - Every node in the tree — including the root — should carry a [`tanzim_value::Location`] that
///   points back to the source, resource, and line/column, so downstream error messages can show
///   users exactly where a bad value came from. Use [`tanzim_value::Location::at`] to build them.
/// - [`supported_format_list`](Parse::supported_format_list) may return several extensions
///   for one parser (e.g. `["yml", "yaml"]`). When a payload carries no format hint, selection
///   instead falls back to probing — see Auto-detection below.
///
/// # Auto-detection
///
/// When a payload's `format` hint is `None`, the parse stage calls
/// [`is_format_supported`][Parse::is_format_supported] on each registered
/// parser in order. Return `Some(true)` if confident, `Some(false)` to skip, or `None`
/// if unsure (another parser may then claim the bytes).
///
/// # Choosing an error
///
/// Failures are reported with [`tanzim_value::Error`]; every variant except `Parse` carries a
/// [`Location`](tanzim_value::Location):
///
/// - [`Error::InvalidUtf8`] — the bytes aren't valid UTF-8.
/// - [`Error::Parse`] — a syntax or structural error; set `location` when you can pinpoint it,
///   otherwise `None`.
/// - [`Error::UnsupportedType`] — a value of a type that has no configuration representation
///   (e.g. a date-time).
///
/// # Registering
///
/// Pass an instance to `tanzim::Config::with_parser`. The pipeline picks a parser by the payload's
/// format hint when present, otherwise it probes each parser with
/// [`is_format_supported`](Parse::is_format_supported). For a one-off parser you don't want
/// to define a type for, use [`closure::Closure`] instead of implementing this trait.
///
/// # Example — custom CSV parser
///
/// ```rust
/// use tanzim_parse::{Parse, Source};
/// use tanzim_source::SourceBuilder;
/// use tanzim_value::{Error, LocatedValue, Location, Map, Value};
///
/// struct CsvParser;
///
/// impl Parse for CsvParser {
///     fn name(&self) -> &str { "csv" }
///     fn supported_format_list(&self) -> Vec<String> { vec!["csv".into()] }
///     fn is_format_supported(&self, bytes: &[u8]) -> Option<bool> {
///         Some(bytes.contains(&b','))
///     }
///     fn parse(&self, source: &Source, bytes: &[u8]) -> Result<LocatedValue, Error> {
///         let source_name = source.source();
///         let resource = source.resource();
///         let text = match std::str::from_utf8(bytes) {
///             Ok(value) => value,
///             Err(_) => {
///                 return Err(Error::InvalidUtf8 {
///                     location: Box::new(Location::at(source_name, resource, None, None, None)),
///                 });
///             }
///         };
///         let mut map = Map::new();
///         for (line_idx, line) in text.lines().enumerate() {
///             if let Some((key, val)) = line.split_once(',') {
///                 let loc = Location::at(source_name, resource, Some(line_idx + 1), None, None);
///                 map.insert(key.trim().to_string(), LocatedValue::new(
///                     Value::String(val.trim().to_string()),
///                     loc,
///                 ));
///             }
///         }
///         let root_loc = Location::at(source_name, resource, None, None, None);
///         Ok(LocatedValue::new(Value::Map(map), root_loc))
///     }
/// }
///
/// let source = SourceBuilder::new()
///     .with_source("file")
///     .with_resource("config.csv")
///     .build()
///     .unwrap();
/// let value = CsvParser
///     .parse(&source, b"host,127.0.0.1\nport,8080\n")
///     .unwrap();
///
/// let map = value.value().as_map().unwrap();
/// assert_eq!(map.get("host").unwrap().value().as_string().unwrap(), "127.0.0.1");
/// assert_eq!(map.get("port").unwrap().value().as_string().unwrap(), "8080");
/// // `port` is a string — this parser stores every field verbatim.
/// ```
pub trait Parse {
    /// Human-readable name used in error messages.
    fn name(&self) -> &str;
    /// Format extensions this parser handles (e.g. `["json"]`, `["yml", "yaml"]`).
    fn supported_format_list(&self) -> Vec<String>;
    /// Probe `bytes` for auto-detection when `Payload::maybe_format` is `None`.
    ///
    /// Return `Some(true)` if confident, `Some(false)` if definitely not this format,
    /// or `None` to abstain (another parser will be tried next).
    fn is_format_supported(&self, bytes: &[u8]) -> Option<bool>;
    /// Parse `bytes` into a [`LocatedValue`] tree.
    ///
    /// `source` carries the source kind (e.g. `"file"`), the resource path or
    /// identifier, and any loader options. Use [`Source::source`], [`Source::resource`],
    /// and [`Source::options`] to access them. Every node in the returned tree should
    /// carry a [`tanzim_value::Location`] built from those values.
    fn parse(&self, source: &Source, bytes: &[u8]) -> Result<LocatedValue, Error>;
}
