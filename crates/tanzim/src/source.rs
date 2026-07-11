//! Configuration sources: the `SOURCE[(OPTIONS)][:RESOURCE]` source-string format parsed into a
//! validated [`Source`], plus typed builders for the built-in loaders.
//!
//! Re-exports [`tanzim_source`]. On top of the string form, the [`env`], [`file`], and [`http`]
//! free functions return typed builders that mirror each loader's options and convert into a
//! [`Source`], so they slot straight into
//! [`Config::with_source`](crate::Config)/[`Pipeline::with_source`](crate::Pipeline):
//!
//! ```
//! use tanzim::{Config, source::{env, file}};
//!
//! let _ = Config::default()
//!     .with_source(env().with_prefix("APP_").with_separator("__"))
//!     .with_source(file("app.toml").skip_not_found());
//! ```
//!
//! Each builder is available only when its loader feature (`load-env`, `load-file`,
//! `load-http-closure`) is enabled. Only the options you set are emitted; the loaders supply their
//! own defaults for the rest.

pub use tanzim_source::*;

/// The parsed URL type accepted by [`http`] (re-exported from the HTTP loader).
#[cfg(feature = "load-http-closure")]
pub use tanzim_load::http::Url;

#[cfg(feature = "load-http-closure")]
use std::time::Duration;

/// Which stages a source should skip (via the reserved `on_error` option) instead of failing the
/// pipeline. Shared by the loader builders below.
#[cfg(any(
    feature = "load-env",
    feature = "load-file",
    feature = "load-http-closure"
))]
#[derive(Default, Clone, Debug, PartialEq, Eq)]
struct SkipStages {
    load: bool,
    parse: bool,
    validate: bool,
}

#[cfg(any(
    feature = "load-env",
    feature = "load-file",
    feature = "load-http-closure"
))]
impl SkipStages {
    /// Apply the collected skip stages to `source` as its `on_error` option, if any were set.
    fn apply(self, mut source: Source) -> Source {
        if !(self.load || self.parse || self.validate) {
            return source;
        }
        let mut map = Options::default();
        if self.load {
            map.insert(Stage::Load.as_str(), "skip");
        }
        if self.parse {
            map.insert(Stage::Parse.as_str(), "skip");
        }
        if self.validate {
            map.insert(Stage::Validate.as_str(), "skip");
        }
        source.set_option("on_error", map);
        source
    }
}

// ---------------------------------------------------------------------------
// env
// ---------------------------------------------------------------------------

/// Builder for an `env` [`Source`], mirroring the options of the environment-variable loader.
///
/// Create one with [`env`], then convert into a [`Source`] (directly or via
/// [`Config::with_source`](crate::Config)).
#[cfg(feature = "load-env")]
#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub struct EnvSource {
    prefix: Option<String>,
    strip_prefix: Option<bool>,
    separator: Option<String>,
    lowercase: Option<bool>,
    skip: SkipStages,
}

#[cfg(feature = "load-env")]
impl EnvSource {
    /// Only read variables whose names start with `prefix` (otherwise the loader auto-detects one).
    pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = Some(prefix.into());
        self
    }

    /// Whether to strip the prefix from derived entry names (defaults to `true` when a prefix is set).
    pub fn strip_prefix(mut self, strip_prefix: bool) -> Self {
        self.strip_prefix = Some(strip_prefix);
        self
    }

    /// Split each variable name once on `separator` into an entry name and a content key.
    pub fn with_separator(mut self, separator: impl Into<String>) -> Self {
        self.separator = Some(separator.into());
        self
    }

    /// Whether to lower-case derived entry names (default `true`).
    pub fn lowercase(mut self, lowercase: bool) -> Self {
        self.lowercase = Some(lowercase);
        self
    }

    /// Skip this source instead of failing the pipeline when its load stage errors.
    pub fn skip_load_error(mut self) -> Self {
        self.skip.load = true;
        self
    }

    /// Skip this source instead of failing the pipeline when its parse stage errors.
    pub fn skip_parse_error(mut self) -> Self {
        self.skip.parse = true;
        self
    }

    /// Skip this source instead of failing the pipeline when its validate stage errors.
    pub fn skip_validate_error(mut self) -> Self {
        self.skip.validate = true;
        self
    }
}

#[cfg(feature = "load-env")]
impl From<EnvSource> for Source {
    fn from(builder: EnvSource) -> Self {
        let mut source = Source::named("env");
        if let Some(prefix) = builder.prefix {
            source.set_option("prefix", prefix);
        }
        if let Some(strip_prefix) = builder.strip_prefix {
            source.set_option("strip_prefix", strip_prefix);
        }
        if let Some(separator) = builder.separator {
            source.set_option("separator", separator);
        }
        if let Some(lowercase) = builder.lowercase {
            source.set_option("lowercase", lowercase);
        }
        builder.skip.apply(source)
    }
}

/// Start building an `env` source that reads configuration from environment variables.
///
/// ```
/// use tanzim::source::{env, Source};
///
/// let source = Source::from(env().with_prefix("APP_").with_separator("__"));
/// assert_eq!(source.to_string(), "env(prefix=APP_,separator=__)");
/// ```
#[cfg(feature = "load-env")]
pub fn env() -> EnvSource {
    EnvSource::default()
}

// ---------------------------------------------------------------------------
// file
// ---------------------------------------------------------------------------

/// Builder for a `file` [`Source`], mirroring the options of the filesystem loader.
///
/// Create one with [`file`], then convert into a [`Source`] (directly or via
/// [`Config::with_source`](crate::Config)).
#[cfg(feature = "load-file")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileSource {
    resource: String,
    skip_kinds: Vec<&'static str>,
    lowercase: Option<bool>,
    skip: SkipStages,
}

#[cfg(feature = "load-file")]
impl FileSource {
    fn push_skip_kind(&mut self, kind: &'static str) {
        if !self.skip_kinds.contains(&kind) {
            self.skip_kinds.push(kind);
        }
    }

    /// Downgrade a missing path to an empty result instead of a load error.
    pub fn skip_not_found(mut self) -> Self {
        self.push_skip_kind("not-found");
        self
    }

    /// Downgrade a permission error to an empty result instead of a load error.
    pub fn skip_no_access(mut self) -> Self {
        self.push_skip_kind("no-access");
        self
    }

    /// Whether to lower-case derived entry names and formats (default `true`).
    pub fn lowercase(mut self, lowercase: bool) -> Self {
        self.lowercase = Some(lowercase);
        self
    }

    /// Skip this source instead of failing the pipeline when its load stage errors.
    pub fn skip_load_error(mut self) -> Self {
        self.skip.load = true;
        self
    }

    /// Skip this source instead of failing the pipeline when its parse stage errors.
    pub fn skip_parse_error(mut self) -> Self {
        self.skip.parse = true;
        self
    }

    /// Skip this source instead of failing the pipeline when its validate stage errors.
    pub fn skip_validate_error(mut self) -> Self {
        self.skip.validate = true;
        self
    }
}

#[cfg(feature = "load-file")]
impl From<FileSource> for Source {
    fn from(builder: FileSource) -> Self {
        let mut source = Source::named("file");
        source.set_resource(builder.resource);
        if !builder.skip_kinds.is_empty() {
            source.set_option("skip", builder.skip_kinds);
        }
        if let Some(lowercase) = builder.lowercase {
            source.set_option("lowercase", lowercase);
        }
        builder.skip.apply(source)
    }
}

/// Start building a `file` source that reads `path` (a single file or a directory).
///
/// ```
/// use tanzim::source::{file, Source};
///
/// let source = Source::from(file("app.toml").skip_not_found());
/// assert_eq!(source.source(), "file");
/// assert_eq!(source.resource(), "app.toml");
/// ```
#[cfg(feature = "load-file")]
pub fn file(path: impl Into<String>) -> FileSource {
    FileSource {
        resource: path.into(),
        skip_kinds: Vec::new(),
        lowercase: None,
        skip: SkipStages::default(),
    }
}

// ---------------------------------------------------------------------------
// http
// ---------------------------------------------------------------------------

/// Builder for an `http` [`Source`], mirroring the options of the HTTP loader.
///
/// Create one with [`http`], then convert into a [`Source`] (directly or via
/// [`Config::with_source`](crate::Config)).
#[cfg(feature = "load-http-closure")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpSource {
    resource: String,
    headers: Vec<(String, String)>,
    timeout: Option<Duration>,
    insecure: Option<bool>,
    lowercase: Option<bool>,
    skip: SkipStages,
}

#[cfg(feature = "load-http-closure")]
impl HttpSource {
    /// Add a request header. May be called repeatedly; order is preserved.
    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    /// Add several request headers at once.
    pub fn with_headers<K, V>(mut self, headers: impl IntoIterator<Item = (K, V)>) -> Self
    where
        K: Into<String>,
        V: Into<String>,
    {
        self.headers.extend(
            headers
                .into_iter()
                .map(|(key, value)| (key.into(), value.into())),
        );
        self
    }

    /// Set the request timeout (default 15 seconds; rounded down to whole seconds).
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Allow invalid TLS certificates (default `false`).
    pub fn insecure(mut self, insecure: bool) -> Self {
        self.insecure = Some(insecure);
        self
    }

    /// Whether to lower-case returned entry names and formats (default `true`).
    pub fn lowercase(mut self, lowercase: bool) -> Self {
        self.lowercase = Some(lowercase);
        self
    }

    /// Skip this source instead of failing the pipeline when its load stage errors.
    pub fn skip_load_error(mut self) -> Self {
        self.skip.load = true;
        self
    }

    /// Skip this source instead of failing the pipeline when its parse stage errors.
    pub fn skip_parse_error(mut self) -> Self {
        self.skip.parse = true;
        self
    }

    /// Skip this source instead of failing the pipeline when its validate stage errors.
    pub fn skip_validate_error(mut self) -> Self {
        self.skip.validate = true;
        self
    }
}

#[cfg(feature = "load-http-closure")]
impl From<HttpSource> for Source {
    fn from(builder: HttpSource) -> Self {
        let mut source = Source::named("http");
        source.set_resource(builder.resource);
        if !builder.headers.is_empty() {
            let mut map = Options::default();
            for (name, value) in builder.headers {
                map.insert(name, value);
            }
            source.set_option("headers", map);
        }
        if let Some(timeout) = builder.timeout {
            source.set_option("timeout", timeout.as_secs() as i64);
        }
        if let Some(insecure) = builder.insecure {
            source.set_option("insecure", insecure);
        }
        if let Some(lowercase) = builder.lowercase {
            source.set_option("lowercase", lowercase);
        }
        builder.skip.apply(source)
    }
}

/// Start building an `http` source that fetches `url`.
///
/// ```
/// use tanzim::source::{http, Source, Url};
///
/// let source = Source::from(http(Url::parse("https://example.com/c.json").unwrap()));
/// assert_eq!(source.source(), "http");
/// assert_eq!(source.resource(), "https://example.com/c.json");
/// ```
#[cfg(feature = "load-http-closure")]
pub fn http(url: Url) -> HttpSource {
    HttpSource {
        resource: url.to_string(),
        headers: Vec::new(),
        timeout: None,
        insecure: None,
        lowercase: None,
        skip: SkipStages::default(),
    }
}
