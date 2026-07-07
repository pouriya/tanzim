#![doc = include_str!("../README.md")]

use std::error::Error as StdError;

pub use tanzim_source::{OptionValue, Options, Source};

pub mod closure;
#[cfg(feature = "env")]
pub mod env;
#[cfg(feature = "file")]
pub mod file;
#[cfg(feature = "http")]
pub mod http;

/// Raw bytes for one configuration entry, with its declaring [`Source`].
///
/// A loader returns one `Payload` per entry it finds. Fields:
///
/// - `source` — the concrete resource this entry came from. When one [`Source`] expands to
///   several entries (e.g. a directory of files), set this to the *specific* resource loaded,
///   not the original directory — clone the incoming source and narrow it with
///   [`Source::with_resource`]. Downstream stages surface it in diagnostics.
/// - `maybe_name` — the entry's name, or `None` for an unnamed payload. All `None`-named
///   payloads merge together into the root; distinct names stay separate. Named entries with the
///   same name also merge.
/// - `maybe_format` — a hint for the parser stage selecting the parser (`json`, `env`, …), or
///   `None` to let the parser infer/default. It is a hint, not a guarantee.
/// - `content` — the unparsed bytes, passed through verbatim to the parser.
///
/// **Lowercase convention:** built-in loaders lower-case `maybe_name` and `maybe_format` when
/// their Source `lowercase` option is `true` (the default). Custom loaders are encouraged to
/// follow the same convention so entry names merge predictably across sources.
#[derive(Debug, Clone, PartialEq)]
pub struct Payload {
    /// Concrete resource this entry was loaded from (narrowed from the incoming [`Source`]).
    pub source: Source,
    /// Entry name; `None` merges into the root alongside other unnamed payloads.
    pub maybe_name: Option<String>,
    /// Parser hint (e.g. `json`, `env`); `None` lets the parser infer or default.
    pub maybe_format: Option<String>,
    /// Unparsed bytes, forwarded verbatim to the parser stage.
    pub content: Vec<u8>,
}

/// Errors a [`Load`] implementation can return.
///
/// Each variant carries a `loader` field (set to your [`Load::name`]) so messages identify the
/// source. See [`Load`]'s "Choosing an error" section for guidance on which to pick.
///
/// [`Display`](std::fmt::Display) is one line by default. Use the alternate form (`{error:#}`) to
/// append the underlying cause chain — every wrapped `source` (and its sources, recursively) is
/// tacked on as `: <cause>`, so backend failures surface their real reason instead of just the
/// loader's summary.
#[derive(Debug)]
pub enum Error {
    /// The requested resource or entry does not exist and was not configured to be ignored.
    /// `item` names what was missing (e.g. `` `file "app.json"` ``).
    NotFound {
        loader: String,
        resource: String,
        item: String,
    },
    /// Access was denied (e.g. filesystem permissions, HTTP 401/403). `source` carries the
    /// underlying backend error.
    NoAccess {
        loader: String,
        resource: String,
        source: Box<dyn StdError + Send + Sync>,
    },
    /// The operation exceeded its deadline. `timeout_in_seconds` is the limit that was hit;
    /// `source` carries the underlying backend error.
    Timeout {
        loader: String,
        resource: String,
        timeout_in_seconds: u64,
        source: Box<dyn StdError + Send + Sync>,
    },
    /// A known option has the wrong type or value. `key` is the option name; `reason` explains the
    /// problem (commonly built from [`OptionValue::type_name`] on a type mismatch). Loaders only
    /// validate options they read — unknown keys are ignored.
    InvalidOption {
        loader: String,
        key: String,
        reason: String,
    },
    /// The resource string is empty or malformed for this loader (e.g. a required path is
    /// missing). `reason` explains what was expected.
    InvalidResource {
        loader: String,
        resource: String,
        reason: String,
    },
    /// Two entries resolve to the same `name` with differing formats (`format_1` vs `format_2`),
    /// so the loader cannot pick one unambiguously.
    Duplicate {
        loader: String,
        resource: String,
        name: String,
        format_1: String,
        format_2: String,
    },
    /// Catch-all backend failure that doesn't fit the variants above. `description` completes the
    /// phrase "could not {description}" (e.g. `"read contents of file"`); `source` carries the
    /// underlying error.
    Load {
        loader: String,
        resource: String,
        description: String,
        source: Box<dyn StdError + Send + Sync>,
    },
    /// Bridge for opaque errors via `?`/`From`, when none of the structured variants apply.
    Other(Box<dyn StdError + Send + Sync>),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NotFound {
                loader,
                resource,
                item,
            } => write!(
                f,
                "{loader} configuration loader could not find {item} at `{resource}`"
            )?,
            Error::NoAccess {
                loader, resource, ..
            } => write!(
                f,
                "{loader} configuration loader has no access to `{resource}`"
            )?,
            Error::Timeout {
                loader,
                resource,
                timeout_in_seconds,
                ..
            } => write!(
                f,
                "{loader} configuration loader reached timeout `{timeout_in_seconds}s` for `{resource}`"
            )?,
            Error::InvalidOption {
                loader,
                key,
                reason,
            } => write!(
                f,
                "{loader} configuration loader invalid option `{key}`: {reason}"
            )?,
            Error::InvalidResource {
                loader,
                resource,
                reason,
            } => write!(
                f,
                "{loader} configuration loader invalid resource `{resource}`: {reason}"
            )?,
            Error::Duplicate {
                loader,
                resource,
                name,
                format_1,
                format_2,
            } => write!(
                f,
                "{loader} configuration loader found duplicate configurations `{resource}/{name}.({format_1}|{format_2})`"
            )?,
            Error::Load {
                loader,
                resource,
                description,
                ..
            } => write!(
                f,
                "{loader} configuration loader could not {description} `{resource}`"
            )?,
            // Transparent: forward to the wrapped error's own message.
            Error::Other(source) => write!(f, "{source}")?,
        }
        // Alternate form appends the full underlying cause chain.
        if f.alternate() {
            let mut cause = StdError::source(self);
            while let Some(error) = cause {
                write!(f, ": {error}")?;
                cause = error.source();
            }
        }
        Ok(())
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Error::NoAccess { source, .. }
            | Error::Timeout { source, .. }
            | Error::Load { source, .. } => Some(&**source),
            // Transparent: delegate to the wrapped error so the chain continues past it.
            Error::Other(source) => source.source(),
            Error::NotFound { .. }
            | Error::InvalidOption { .. }
            | Error::InvalidResource { .. }
            | Error::Duplicate { .. } => None,
        }
    }
}

impl From<Box<dyn StdError + Send + Sync>> for Error {
    fn from(source: Box<dyn StdError + Send + Sync>) -> Self {
        Error::Other(source)
    }
}

/// Loads raw configuration bytes from a declared source.
///
/// Implement this to add a new source kind (protocol, service, database, …). This is the first
/// stage of the pipeline: it only *fetches bytes*, it does not parse them — [`Payload::content`]
/// is handed to the parser stage unchanged.
///
/// # Contract
///
/// - [`load`](Load::load) takes ownership of one [`Source`] and returns one [`Payload`] per
///   configuration entry found. A single source may expand to many entries (e.g. every file in a
///   directory) → return many payloads; finding nothing is `Ok(vec![])`, not an error.
/// - Set [`Payload::source`] on each entry to the *concrete* resource loaded, not the original
///   source — clone it and narrow with [`Source::with_resource`]. This keeps diagnostics precise.
/// - Use [`Payload::maybe_name`] for the entry name (`None` merges into the root with all other
///   unnamed entries) and [`Payload::maybe_format`] as a parser hint (e.g. `json`).
/// - Follow the lowercase convention: when your `lowercase` option is `true` (recommended
///   default), lower-case names and formats so entries merge predictably across sources.
///
/// # Reading options
///
/// Options declared on the source (e.g. `file(ignore=[not-found])`) are available via
/// [`Source::options`]. Look each up with [`Options::get`], convert with the typed accessors
/// ([`OptionValue::as_bool`], [`OptionValue::as_string`], [`OptionValue::as_list`], …), and on a
/// type mismatch build the `reason` from [`OptionValue::type_name`]. Only look up the options your
/// loader understands; ignore any others. See the `file` loader's `load` for a complete worked
/// pattern.
///
/// # Choosing an error
///
/// - [`Error::InvalidResource`] — the resource string is empty/malformed for this loader.
/// - [`Error::InvalidOption`] — a known option has the wrong type or value.
/// - [`Error::NotFound`] — the resource/entry doesn't exist (and isn't being ignored).
/// - [`Error::NoAccess`] — permission denied by the backend.
/// - [`Error::Timeout`] — a deadline was exceeded.
/// - [`Error::Duplicate`] — two entries collide on the same name with different formats.
/// - [`Error::Load`] — any other backend failure (`description` completes "could not …").
/// - [`Error::Other`] — bridge for an opaque error via `?`.
///
/// # Registering
///
/// Pass an instance to `tanzim::Config::with_loader`. The pipeline dispatches each source to the
/// first loader whose [`supported_source_list`](Load::supported_source_list) contains the source
/// string, so it may advertise several (e.g. `["http", "https"]`). For a one-off loader you don't
/// want to define a type for, use [`closure::Closure`] instead of implementing this trait.
///
/// # Example — collecting specific environment variables
///
/// A loader that reads the variable names listed in its `keys` option and returns them as one
/// `env`-format payload. It shows the whole contract: reading a typed option, mapping failures to
/// the right [`Error`] variant, and building a [`Payload`].
///
/// ```rust
/// use std::env;
/// use tanzim_load::{Error, Load, Payload, Source};
///
/// struct SelectedEnv;
///
/// impl Load for SelectedEnv {
///     fn name(&self) -> &str { "selected-env" }
///     fn supported_source_list(&self) -> Vec<String> { vec!["selected-env".into()] }
///
///     fn load(&self, source: Source) -> Result<Vec<Payload>, Error> {
///         // Read the `keys` option — a required list of variable names.
///         let value = source.options().get("keys").ok_or_else(|| Error::InvalidOption {
///             loader: self.name().into(),
///             key: "keys".into(),
///             reason: "required".into(),
///         })?;
///         let keys = value.as_list().ok_or_else(|| Error::InvalidOption {
///             loader: self.name().into(),
///             key: "keys".into(),
///             reason: format!("expected list, found {}", value.type_name()),
///         })?;
///
///         // Collect each requested variable into a `KEY="value"` line.
///         let mut lines = Vec::new();
///         for item in keys {
///             let key = item.as_string().ok_or_else(|| Error::InvalidOption {
///                 loader: self.name().into(),
///                 key: "keys".into(),
///                 reason: format!("expected string, found {}", item.type_name()),
///             })?;
///             let val = env::var(key).map_err(|_| Error::NotFound {
///                 loader: self.name().into(),
///                 resource: source.resource().into(),
///                 item: format!("environment variable `{key}`"),
///             })?;
///             lines.push(format!("{key}={val:?}"));
///         }
///
///         Ok(vec![Payload {
///             source,
///             maybe_name: None,                 // unnamed → merges into the config root
///             maybe_format: Some("env".into()), // parsed by the `env` parser
///             content: lines.join("\n").into_bytes(),
///         }])
///     }
/// }
///
/// # tanzim_testing::environment::run(|sandbox| {
/// #     sandbox.set_env("DB_HOST", "localhost")?;
/// #     sandbox.set_env("DB_PORT", "5432")?;
/// // The process environment holds `DB_HOST=localhost` and `DB_PORT=5432`.
/// let source = Source::parse("selected-env(keys=[DB_HOST,DB_PORT])").unwrap();
///
/// let payloads = SelectedEnv.load(source).unwrap();
/// let content = String::from_utf8_lossy(&payloads[0].content);
/// // The loader emits one `KEY="value"` line per requested variable.
/// assert!(content.contains(r#"DB_HOST="localhost""#));
/// assert!(content.contains(r#"DB_PORT="5432""#));
/// # Ok(())
/// # })
/// # .unwrap();
/// ```
pub trait Load {
    /// Human-readable name used in error messages.
    fn name(&self) -> &str;
    /// Source strings this loader handles (e.g. `["env"]`, `["file"]`, `["http", "https"]`).
    fn supported_source_list(&self) -> Vec<String>;
    /// Load raw bytes from the source. Returns one [`Payload`] per config entry found.
    fn load(&self, source: Source) -> Result<Vec<Payload>, Error>;
}
