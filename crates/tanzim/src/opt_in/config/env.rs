//! The [`Environment`] source helper.

use crate::source::{Source, SourceBuilder};

/// An environment-variable configuration source, mirroring the `config` crate's `Environment`.
///
/// [`with_prefix`](Self::with_prefix) restricts which variables are read; [`separator`](Self::separator)
/// splits each key into nested entry names. Converts into a [`Source`] for the `env` loader.
#[derive(Default)]
pub struct Environment {
    prefix: Option<String>,
    separator: Option<String>,
}

impl Environment {
    /// Only read variables whose name starts with `prefix`.
    pub fn with_prefix(prefix: impl Into<String>) -> Self {
        Self {
            prefix: Some(prefix.into()),
            separator: None,
        }
    }

    /// Split each variable name on `separator` to build nested keys (e.g. `.` for `APP_A.B`).
    pub fn separator(mut self, separator: impl Into<String>) -> Self {
        self.separator = Some(separator.into());
        self
    }
}

impl From<Environment> for Source {
    fn from(env: Environment) -> Self {
        let mut builder = SourceBuilder::new().with_source("env");
        if let Some(prefix) = env.prefix {
            builder = builder.with_option("prefix", prefix);
        }
        if let Some(separator) = env.separator {
            builder = builder.with_option("separator", separator);
        }
        // Infallible: the source name is always set and non-empty.
        builder
            .build()
            .expect("env source always has a source name")
    }
}
