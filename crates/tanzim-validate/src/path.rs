use crate::Validator;
use crate::error::{Error, ErrorKind};
use tanzim_value::{Value, ValueType};

/// (`path` feature) The kind of filesystem entry a [`Path`] must point at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathKind {
    Dir,
    File,
    Symlink,
}

/// (`path` feature) Accepts a filesystem path string.
///
/// Format checks (absolute/relative, extension) never touch the filesystem. The
/// existence, kind, and permission checks do, and only when explicitly requested.
/// `readable`/`writable` consult OS permission flags where available; where the OS
/// exposes no such flag the check is a no-op that accepts.
#[derive(Debug, Clone, Default)]
pub struct Path {
    absolute: bool,
    relative: bool,
    extensions: Vec<String>,
    must_exist: bool,
    kind: Option<PathKind>,
    readable: bool,
    writable: bool,
}

impl Path {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn absolute(mut self) -> Self {
        self.absolute = true;
        self.relative = false;
        self
    }

    pub fn relative(mut self) -> Self {
        self.relative = true;
        self.absolute = false;
        self
    }

    /// Require the path to end in one of the allowed extensions (compared case-insensitively).
    pub fn extension(mut self, extension: impl Into<String>) -> Self {
        self.extensions.push(extension.into());
        self
    }

    /// Require the path to exist on the filesystem.
    pub fn must_exist(mut self) -> Self {
        self.must_exist = true;
        self
    }

    /// Require the path to point at the given kind of entry (implies existence).
    pub fn kind(mut self, kind: PathKind) -> Self {
        self.kind = Some(kind);
        self
    }

    /// Require the path to be readable (implies existence).
    pub fn readable(mut self) -> Self {
        self.readable = true;
        self
    }

    /// Require the path to be writable (implies existence).
    pub fn writable(mut self) -> Self {
        self.writable = true;
        self
    }

    fn touches_filesystem(&self) -> bool {
        self.must_exist || self.kind.is_some() || self.readable || self.writable
    }
}

/// Whether the file mode grants read permission. On non-unix targets there is no such
/// flag, so this accepts (returns `true`).
#[cfg(unix)]
fn is_readable(metadata: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o444 != 0
}

#[cfg(not(unix))]
fn is_readable(_metadata: &std::fs::Metadata) -> bool {
    true
}

impl Validator for Path {
    fn validate(&self, value: &mut Value) -> Result<(), Error> {
        let text = match value {
            Value::String(text) => text,
            other => {
                return Err(Error::new(ErrorKind::Type {
                    expected: ValueType::String,
                    found: other.type_name(),
                }));
            }
        };

        let path = std::path::Path::new(text.as_str());

        if self.absolute && !path.is_absolute() {
            return Err(Error::new(ErrorKind::Format {
                expected: "absolute path",
            }));
        }
        if self.relative && path.is_absolute() {
            return Err(Error::new(ErrorKind::Format {
                expected: "relative path",
            }));
        }

        if !self.extensions.is_empty() {
            let mut matched = false;
            if let Some(extension) = path.extension() {
                for allowed in &self.extensions {
                    if extension.eq_ignore_ascii_case(allowed) {
                        matched = true;
                        break;
                    }
                }
            }
            if !matched {
                return Err(Error::new(ErrorKind::Format {
                    expected: "allowed file extension",
                }));
            }
        }

        if !self.touches_filesystem() {
            return Ok(());
        }

        let metadata = match std::fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(_) => {
                return Err(Error::new(ErrorKind::Format {
                    expected: "existing path",
                }));
            }
        };

        if let Some(kind) = self.kind {
            let file_type = metadata.file_type();
            let ok = match kind {
                PathKind::Dir => file_type.is_dir(),
                PathKind::File => file_type.is_file(),
                PathKind::Symlink => file_type.is_symlink(),
            };
            if !ok {
                let expected = match kind {
                    PathKind::Dir => "directory",
                    PathKind::File => "file",
                    PathKind::Symlink => "symlink",
                };
                return Err(Error::new(ErrorKind::Format { expected }));
            }
        }

        if self.readable && !is_readable(&metadata) {
            return Err(Error::new(ErrorKind::Format {
                expected: "readable path",
            }));
        }
        if self.writable && metadata.permissions().readonly() {
            return Err(Error::new(ErrorKind::Format {
                expected: "writable path",
            }));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn string(text: &str) -> Value {
        Value::String(text.to_string())
    }

    #[test]
    fn absolute_and_relative() {
        assert!(
            Path::new()
                .absolute()
                .validate(&mut string("/etc/app"))
                .is_ok()
        );
        assert!(Path::new().absolute().validate(&mut string("app")).is_err());
        assert!(
            Path::new()
                .relative()
                .validate(&mut string("app/conf"))
                .is_ok()
        );
    }

    #[test]
    fn extension_filter() {
        assert!(
            Path::new()
                .extension("toml")
                .validate(&mut string("a.toml"))
                .is_ok()
        );
        assert!(
            Path::new()
                .extension("toml")
                .validate(&mut string("a.json"))
                .is_err()
        );
    }

    #[test]
    fn must_exist_uses_filesystem() {
        // The crate manifest is guaranteed to exist when tests run.
        let manifest = env!("CARGO_MANIFEST_DIR");
        let mut here = string(manifest);
        assert!(
            Path::new()
                .must_exist()
                .kind(PathKind::Dir)
                .validate(&mut here)
                .is_ok()
        );
        let mut missing = string("/this/path/should/not/exist/xyzzy");
        assert!(Path::new().must_exist().validate(&mut missing).is_err());
    }

    #[test]
    fn format_only_never_touches_fs() {
        // A non-existent path passes when no fs check is requested.
        let mut value = string("/nope/not/here.toml");
        assert!(
            Path::new()
                .absolute()
                .extension("toml")
                .validate(&mut value)
                .is_ok()
        );
    }
}
