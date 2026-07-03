//! Filesystem loader (`file` feature).
//!
//! Reads a single configuration file, or every file in a directory.
//!
//! **Source:** `file` (the resource is the file or directory path and is required; an empty
//! resource is rejected with [`Error::InvalidResource`])
//!
//! # Behaviour
//!
//! - If the resource is a **directory**, each regular file in it becomes one entry; sub-entries
//!   that are not regular files are skipped (with a warning). Entries are returned in a
//!   deterministic order (sorted by path).
//! - If the resource is a **single file**, it becomes one entry.
//! - `maybe_name` comes from the filename stem and `maybe_format` from the extension; either may
//!   be `None` (e.g. `README` has no format, `.env` has no name). Both are lower-cased when
//!   `lowercase = true` (the default).
//! - Each entry's [`Payload::source`] is narrowed to that file's path, so
//!   diagnostics point at the exact file rather than the directory.
//! - Missing paths and permission errors normally surface as
//!   [`Error::NotFound`] / [`Error::NoAccess`];
//!   the `ignore` option downgrades them to a skipped entry instead.
//!
//! # Options
//!
//! - `ignore` — list of `not-found` and/or `no-access` (default `[]`)
//! - `lowercase` — boolean (default `true`; whether to lowercase entry names and formats)
//!
//! # Example
//!
//! ```text
//! file:/path/to/config.json
//! file(ignore=[not-found]):/optional/config
//! ```

use crate::{Error, Load, Payload, Source};
use cfg_if::cfg_if;
use std::{
    fs, io,
    path::{Path, PathBuf},
};

pub const NAME: &str = "File";
pub const SOURCE: &str = "file";
const IGNORE_NOT_FOUND: &str = "not-found";
const IGNORE_NO_ACCESS: &str = "no-access";

/// Loader for the `file` source: reads a single file or every file in a directory.
///
/// See the [module docs](self) for how names/formats are derived and how the `ignore` and
/// `lowercase` options behave. Stateless — construct with [`File::new`].
///
/// # Example
///
/// ```
/// use tanzim_load::{file::File, Load};
/// use tanzim_source::SourceBuilder;
///
/// // `ignore=[not-found]` turns a missing path into an empty result instead of an error,
/// // so this example is self-contained.
/// let source = SourceBuilder::new()
///     .with_source("file")
///     .with_resource("/path/to/config") // a file or a directory
///     .with_option("ignore", vec!["not-found"])
///     .build()
///     .unwrap();
///
/// let payloads = File::new().load(source).unwrap();
/// assert!(payloads.is_empty()); // nothing at that path, and not-found is ignored
/// ```
#[derive(Default, Clone, Debug)]
pub struct File;

impl File {
    /// Create a filesystem loader. Configuration comes from the source's options, not the type.
    pub fn new() -> Self {
        Default::default()
    }

    fn should_ignore(ignore: &[String], kind: io::ErrorKind) -> bool {
        match kind {
            io::ErrorKind::NotFound => ignore.iter().any(|item| item == IGNORE_NOT_FOUND),
            io::ErrorKind::PermissionDenied => ignore.iter().any(|item| item == IGNORE_NO_ACCESS),
            _ => false,
        }
    }

    fn info<P: AsRef<Path>>(path: P, lowercase: bool) -> Option<(Option<String>, Option<String>)> {
        let path = path.as_ref();
        if !path.is_file() {
            cfg_if! {
                if #[cfg(feature = "tracing")] {
                    tracing::warn!(msg = "Ignored configuration file directory entry", path = ?path, reason = "not a file");
                } else if #[cfg(feature = "logging")] {
                    log::warn!("msg=\"Ignored configuration file directory entry\" path={path:?} reason=\"not a file\"");
                }
            }
            return None;
        }

        let maybe_name = if let Some(stem) = path.file_stem() {
            let trimmed = stem.to_str().unwrap_or_default().trim();
            if trimmed.is_empty() {
                None
            } else {
                if lowercase {
                    let lower = trimmed.to_lowercase();
                    if lower != trimmed {
                        cfg_if! {
                            if #[cfg(feature = "tracing")] {
                                tracing::debug!(msg = "Lowercased configuration file entry name", from = trimmed, to = lower.as_str(), path = ?path);
                            } else if #[cfg(feature = "logging")] {
                                log::debug!("msg=\"Lowercased configuration file entry name\" from={trimmed} to={lower} path={path:?}");
                            }
                        }
                    }
                    Some(lower)
                } else {
                    Some(trimmed.to_string())
                }
            }
        } else {
            None
        };

        let maybe_format = if let Some(extension) = path.extension() {
            if let Some(extension_str) = extension.to_str() {
                let trimmed = extension_str.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    if lowercase {
                        let lower = trimmed.to_lowercase();
                        if lower != trimmed {
                            cfg_if! {
                                if #[cfg(feature = "tracing")] {
                                    tracing::debug!(msg = "Lowercased configuration file entry format", from = trimmed, to = lower.as_str(), path = ?path);
                                } else if #[cfg(feature = "logging")] {
                                    log::debug!("msg=\"Lowercased configuration file entry format\" from={trimmed} to={lower} path={path:?}");
                                }
                            }
                        }
                        Some(lower)
                    } else {
                        Some(trimmed.to_string())
                    }
                }
            } else {
                None
            }
        } else {
            None
        };

        Some((maybe_name, maybe_format))
    }
}

impl Load for File {
    fn name(&self) -> &str {
        NAME
    }

    fn supported_source_list(&self) -> Vec<String> {
        vec![SOURCE.to_string()]
    }

    fn load(&self, source: Source) -> Result<Vec<Payload>, Error> {
        let options = source.options().clone();
        let resource = source.resource().to_string();

        for key in options.keys() {
            if key != "ignore" && key != "lowercase" {
                return Err(Error::InvalidOption {
                    loader: NAME.to_string(),
                    key: key.to_string(),
                    reason: "unknown option".into(),
                });
            }
        }

        let ignore =
            match options.get("ignore") {
                None => Vec::new(),
                Some(value) => {
                    let list = value.as_list().ok_or_else(|| Error::InvalidOption {
                        loader: NAME.to_string(),
                        key: "ignore".to_string(),
                        reason: format!("expected list, found {}", value.type_name()),
                    })?;
                    let mut ignore = Vec::with_capacity(list.len());
                    for item in list {
                        ignore.push(item.as_string().cloned().ok_or_else(|| {
                            Error::InvalidOption {
                                loader: NAME.to_string(),
                                key: "ignore".to_string(),
                                reason: format!("expected string, found {}", item.type_name()),
                            }
                        })?);
                    }
                    ignore
                }
            };

        for item in &ignore {
            if item != IGNORE_NOT_FOUND && item != IGNORE_NO_ACCESS {
                return Err(Error::InvalidOption {
                    loader: NAME.to_string(),
                    key: "ignore".into(),
                    reason: format!(
                        "unknown ignore value `{item}` (expected `not-found` or `no-access`)"
                    ),
                });
            }
        }

        let lowercase = match options.get("lowercase") {
            None => true,
            Some(value) => value.as_bool().ok_or_else(|| Error::InvalidOption {
                loader: NAME.to_string(),
                key: "lowercase".to_string(),
                reason: format!("expected boolean, found {}", value.type_name()),
            })?,
        };

        if resource.is_empty() {
            return Err(Error::InvalidResource {
                loader: NAME.to_string(),
                resource: resource.to_string(),
                reason: "resource (file or directory path) is required".into(),
            });
        }

        cfg_if! {
            if #[cfg(feature = "tracing")] {
                tracing::debug!(msg = "Loading configuration from filesystem", resource = resource, lowercase = lowercase);
            } else if #[cfg(feature = "logging")] {
                log::debug!("msg=\"Loading configuration from filesystem\" resource={resource} lowercase={lowercase}");
            }
        }

        let path = PathBuf::from(&resource);

        // Each entry: (name, format, path, source_for_this_entry)
        let list: Vec<(Option<String>, Option<String>, PathBuf, Source)> = if path.is_dir() {
            let entry_list = match fs::read_dir(&path) {
                Ok(entry_list) => entry_list,
                Err(error) if Self::should_ignore(&ignore, error.kind()) => {
                    cfg_if! {
                        if #[cfg(feature = "tracing")] {
                            tracing::warn!(msg = "Ignored configuration file directory", path = ?path, reason = ?error);
                        } else if #[cfg(feature = "logging")] {
                            log::debug!("msg=\"Ignored configuration file directory\" path={path:?} reason={error:?}");
                        }
                    }
                    return Ok(Vec::new());
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    return Err(Error::NotFound {
                        loader: NAME.to_string(),
                        resource: resource.to_string(),
                        item: format!("directory `{path:?}`"),
                    });
                }
                Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
                    return Err(Error::NoAccess {
                        loader: NAME.to_string(),
                        resource: resource.to_string(),
                        source: error.into(),
                    });
                }
                Err(error) => {
                    return Err(Error::Load {
                        loader: NAME.to_string(),
                        resource: resource.to_string(),
                        description: "load directory file list".into(),
                        source: error.into(),
                    });
                }
            };

            let mut filtered_entry_list = Vec::new();
            for maybe_entry in entry_list {
                let entry = match maybe_entry {
                    Ok(entry) => entry,
                    Err(error) if Self::should_ignore(&ignore, error.kind()) => {
                        cfg_if! {
                            if #[cfg(feature = "tracing")] {
                                tracing::warn!(msg = "Ignored configuration file directory entry", path = ?path, reason = ?error);
                            } else if #[cfg(feature = "logging")] {
                                log::warn!("msg=\"Ignored configuration file directory entry\" path={path:?} reason={error:?}");
                            }
                        }
                        continue;
                    }
                    Err(error) => {
                        return Err(Error::Load {
                            loader: NAME.to_string(),
                            resource: resource.to_string(),
                            description: "load directory file list".into(),
                            source: error.into(),
                        });
                    }
                };

                let entry_path = entry.path();
                let (maybe_name, maybe_format) = if let Some((maybe_name, maybe_format)) =
                    Self::info(&entry_path, lowercase)
                {
                    (maybe_name, maybe_format)
                } else {
                    cfg_if! {
                        if #[cfg(feature = "tracing")] {
                            tracing::warn!(msg = "Ignored configuration file directory entry", path = ?entry_path, reason = "not a file");
                        } else if #[cfg(feature = "logging")] {
                            log::warn!("msg=\"Ignored configuration file directory entry\" path={entry_path:?} reason=\"not a file\"");
                        }
                    }
                    continue;
                };
                filtered_entry_list.push((
                    maybe_name,
                    maybe_format,
                    entry_path.clone(),
                    source
                        .clone()
                        .with_resource(entry_path.to_string_lossy().to_string()),
                ));
            }

            filtered_entry_list
                .sort_by_key(|(_name, _format, entry_path, _source)| entry_path.clone());
            filtered_entry_list
        } else if path.is_file() {
            let (maybe_name, maybe_format) =
                if let Some((maybe_name, maybe_format)) = Self::info(&path, lowercase) {
                    (maybe_name, maybe_format)
                } else {
                    // unreachable
                    return Err(Error::InvalidResource {
                        loader: NAME.to_string(),
                        resource: resource.to_string(),
                        reason: "resource is not a regular file".into(),
                    });
                };
            Vec::from([(
                maybe_name,
                maybe_format,
                path.clone(),
                source
                    .clone()
                    .with_resource(path.to_string_lossy().to_string()),
            )])
        } else if path.exists() {
            return Err(Error::InvalidResource {
                loader: NAME.to_string(),
                resource: resource.to_string(),
                reason: "resource is not a directory or regular file".into(),
            });
        } else if Self::should_ignore(&ignore, io::ErrorKind::NotFound) {
            return Ok(Vec::new());
        } else {
            return Err(Error::NotFound {
                loader: NAME.to_string(),
                resource: resource.to_string(),
                item: format!("path `{path:?}`"),
            });
        };

        let mut payload_list = Vec::with_capacity(list.len());
        for (maybe_name, maybe_format, path, source) in list {
            let content = match fs::read(&path) {
                Ok(content) => Some(content),
                Err(error) if Self::should_ignore(&ignore, error.kind()) => {
                    cfg_if! {
                        if #[cfg(feature = "tracing")] {
                            tracing::warn!(msg = "Ignored configuration file", path = ?path, reason = ?error);
                        } else if #[cfg(feature = "logging")] {
                            log::warn!("msg=\"Ignored configuration file\" path={path:?} reason={error:?}");
                        }
                    }
                    None
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    return Err(Error::NotFound {
                        loader: NAME.to_string(),
                        resource: resource.to_string(),
                        item: format!("file `{path:?}`"),
                    });
                }
                Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
                    return Err(Error::NoAccess {
                        loader: NAME.to_string(),
                        resource: resource.to_string(),
                        source: error.into(),
                    });
                }
                Err(error) => {
                    return Err(Error::Load {
                        loader: NAME.to_string(),
                        resource: resource.to_string(),
                        description: format!("read contents of file `{path:?}`"),
                        source: error.into(),
                    });
                }
            };
            if let Some(content) = content {
                cfg_if! {
                    if #[cfg(feature = "tracing")] {
                        tracing::trace!(
                            msg = "Read configuration file",
                            name = ?maybe_name.as_deref().unwrap_or("<empty>"),
                            format = ?maybe_format.as_deref().unwrap_or("<empty>"),
                            path = ?path,
                            bytes = content.len(),
                        );
                    } else if #[cfg(feature = "logging")] {
                        log::trace!(
                            "msg=\"Read configuration file\" name={} format={} path={} bytes={}",
                            maybe_name.as_deref().unwrap_or("<empty>"),
                            maybe_format.as_deref().unwrap_or("<empty>"),
                            path.to_string_lossy(),
                            content.len(),
                        );
                    }
                }
                payload_list.push(Payload {
                    source,
                    maybe_name,
                    maybe_format,
                    content,
                });
            }
        }
        cfg_if! {
            if #[cfg(feature = "tracing")] {
                tracing::info!(msg = "Loaded configuration from filesystem", file_count = payload_list.len(), resource = resource);
            } else if #[cfg(feature = "logging")] {
                log::info!("msg=\"Loaded configuration from filesystem\" file_count={} resource={resource}", payload_list.len());
            }
        }
        Ok(payload_list)
    }
}

#[cfg(all(test, feature = "file"))]
mod tests {
    use super::*;
    use std::fs;
    use tanzim_source::SourceBuilder;
    use tempdir::TempDir;

    fn make_source(resource: &str) -> Source {
        SourceBuilder::new()
            .with_source("file")
            .with_resource(resource)
            .build()
            .unwrap()
    }

    #[test]
    fn load_resolves_name_and_format_from_path() {
        let tmp = TempDir::new("tanzim-file-name-format").unwrap();
        fs::write(tmp.path().join("foo.JSON"), b"{}").unwrap();
        fs::write(tmp.path().join("README"), b"x").unwrap();
        fs::write(tmp.path().join(".env"), b"x").unwrap();
        let resource = tmp.path().display().to_string();
        let loaded = File::new().load(make_source(&resource)).unwrap();

        let mut foo = None;
        let mut readme = None;
        let mut dotenv = None;
        for payload in &loaded {
            if payload.maybe_name == Some("foo".to_string()) {
                foo = Some(payload);
            } else if payload.maybe_name == Some("readme".to_string()) {
                readme = Some(payload);
            } else if payload.maybe_name == Some(".env".to_string()) {
                dotenv = Some(payload);
            }
        }

        assert_eq!(foo.expect("foo").maybe_format, Some("json".to_string()));
        assert!(readme.expect("readme").maybe_format.is_none());
        assert!(dotenv.expect(".env").maybe_format.is_none());
    }

    #[test]
    fn load_reads_files_with_and_without_extension() {
        let tmp = TempDir::new("tanzim-file-edge-names").unwrap();
        fs::write(tmp.path().join("foo.json"), br#"{"hello":"world"}"#).unwrap();
        fs::write(tmp.path().join("README"), b"no extension").unwrap();
        fs::write(tmp.path().join(".env"), b"KEY=value").unwrap();
        let resource = tmp.path().display().to_string();
        let loaded = File::new().load(make_source(&resource)).unwrap();
        assert_eq!(loaded.len(), 3);

        let mut foo = None;
        let mut readme = None;
        let mut dotenv = None;
        for payload in &loaded {
            if payload.maybe_name == Some("foo".to_string()) {
                foo = Some(payload);
            } else if payload.maybe_name == Some("readme".to_string()) {
                readme = Some(payload);
            } else if payload.maybe_name == Some(".env".to_string()) {
                dotenv = Some(payload);
            }
        }

        let foo = foo.expect("foo payload");
        assert_eq!(foo.maybe_format, Some("json".to_string()));

        let readme = readme.expect("readme payload");
        assert!(readme.maybe_format.is_none());

        let dotenv = dotenv.expect(".env payload");
        assert!(dotenv.maybe_format.is_none());
    }

    #[test]
    fn load_reads_files_from_directory() {
        let tmp = TempDir::new("tanzim-file").unwrap();
        fs::write(tmp.path().join("foo.json"), br#"{"hello":"world"}"#).unwrap();
        let resource = tmp.path().display().to_string();
        let loaded = File::new().load(make_source(&resource)).unwrap();
        assert_eq!(loaded.len(), 1);
        let payload = &loaded[0];
        assert_eq!(payload.maybe_name, Some("foo".to_string()));
        assert_eq!(payload.maybe_format, Some("json".to_string()));
        // Source resource updated to full file path
        assert!(payload.source.resource().ends_with("foo.json"));
    }

    #[test]
    fn load_ignores_not_found_when_configured() {
        let source = SourceBuilder::new()
            .with_source("file")
            .with_resource("/no/such/path")
            .with_option("ignore", vec!["not-found"])
            .build()
            .unwrap();
        let loaded = File::new().load(source).unwrap();
        assert!(loaded.is_empty());
    }

    #[test]
    fn load_requires_resource() {
        let source = SourceBuilder::new().with_source("file").build().unwrap();
        let error = File::new().load(source).unwrap_err();
        assert!(matches!(error, Error::InvalidResource { .. }));
    }

    #[test]
    fn load_single_file_path() {
        let tmp = TempDir::new("tanzim-file-single").unwrap();
        let file_path = tmp.path().join("solo.json");
        fs::write(&file_path, br#"{"ok":true}"#).unwrap();
        let loaded = File::new()
            .load(make_source(&file_path.display().to_string()))
            .unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].maybe_name.as_deref(), Some("solo"));
        assert_eq!(loaded[0].source.resource(), file_path.display().to_string());
    }

    #[test]
    fn load_rejects_unknown_option() {
        let source = SourceBuilder::new()
            .with_source("file")
            .with_resource("/tmp")
            .with_option("bogus", true)
            .build()
            .unwrap();
        let error = File::new().load(source).unwrap_err();
        assert!(matches!(error, Error::InvalidOption { .. }));
    }

    #[test]
    fn load_rejects_invalid_ignore_list() {
        let source = SourceBuilder::new()
            .with_source("file")
            .with_resource("/tmp")
            .with_option("ignore", "not-a-list")
            .build()
            .unwrap();
        let error = File::new().load(source).unwrap_err();
        assert!(matches!(error, Error::InvalidOption { key, .. } if key == "ignore"));
    }

    #[test]
    fn load_rejects_unknown_ignore_value() {
        let source = SourceBuilder::new()
            .with_source("file")
            .with_resource("/tmp")
            .with_option("ignore", vec!["bogus"])
            .build()
            .unwrap();
        let error = File::new().load(source).unwrap_err();
        assert!(matches!(error, Error::InvalidOption { key, .. } if key == "ignore"));
    }

    #[test]
    fn load_preserves_case_when_lowercase_disabled() {
        let tmp = TempDir::new("tanzim-file-case").unwrap();
        fs::write(tmp.path().join("Demo.JSON"), b"{}").unwrap();
        let source = SourceBuilder::new()
            .with_source("file")
            .with_resource(tmp.path().display().to_string())
            .with_option("lowercase", false)
            .build()
            .unwrap();
        let loaded = File::new().load(source).unwrap();
        assert_eq!(loaded[0].maybe_name.as_deref(), Some("Demo"));
        assert_eq!(loaded[0].maybe_format.as_deref(), Some("JSON"));
    }

    #[test]
    fn load_reports_not_found_for_missing_path() {
        let source = SourceBuilder::new()
            .with_source("file")
            .with_resource("/no/such/tanzim-file-path")
            .build()
            .unwrap();
        let error = File::new().load(source).unwrap_err();
        assert!(matches!(error, Error::NotFound { .. }));
    }
}
