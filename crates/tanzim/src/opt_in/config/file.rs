//! The [`File`] source helper.

use crate::source::{OptionValue, Options, Source, SourceBuilder};

/// A filesystem configuration source, mirroring the `config` crate's `File`.
///
/// [`with_name`](Self::with_name) names a path (a file or a directory of files). Mark it
/// [`required(false)`](Self::required) to skip it silently when it is missing instead of aborting
/// the build. Converts into a [`Source`] for the `file` loader.
pub struct File {
    path: String,
    required: bool,
}

impl File {
    /// A configuration file (or directory) at `path`. Required by default.
    pub fn with_name(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            required: true,
        }
    }

    /// When `false`, a missing file is skipped instead of failing the build
    /// (via `on_error=(load=skip)`).
    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }
}

impl From<File> for Source {
    fn from(file: File) -> Self {
        let mut builder = SourceBuilder::new()
            .with_source("file")
            .with_resource(file.path);
        if !file.required {
            let mut on_error = Options::new();
            on_error.insert("load", "skip");
            builder = builder.with_option("on_error", OptionValue::from(on_error));
        }
        // Infallible: the source name is always set and non-empty.
        builder
            .build()
            .expect("file source always has a source name")
    }
}
