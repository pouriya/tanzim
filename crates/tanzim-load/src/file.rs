//! Filesystem loader (`file` feature).
//!
//! Reads a directory of configuration files, or a single file.
//! Name and format come from the filename stem and extension; either may be empty.
//!
//! **Source:** `file`
//!
//! # Options
//!
//! - `ignore` — list of `not-found` and/or `no-access` (default `[]`)
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
    collections::HashMap,
    fs, io,
    path::{Path, PathBuf},
};

pub const NAME: &str = "File";
pub const SOURCE: &str = "file";
const IGNORE_NOT_FOUND: &str = "not-found";
const IGNORE_NO_ACCESS: &str = "no-access";

#[derive(Default, Clone, Debug)]
pub struct File;

impl File {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn get_name_and_format<P: AsRef<Path>>(path: P) -> (Option<String>, Option<String>) {
        let path = path.as_ref();

        let name = if let Some(stem) = path.file_stem() {
            if let Some(name) = stem.to_str() {
                if name.is_empty() {
                    None
                } else {
                    Some(name.to_lowercase())
                }
            } else {
                None
            }
        } else {
            None
        };

        let format = if let Some(extension) = path.extension() {
            if let Some(format) = extension.to_str() {
                if format.is_empty() {
                    None
                } else {
                    Some(format.to_lowercase())
                }
            } else {
                None
            }
        } else {
            None
        };

        (name, format)
    }

    fn should_ignore(ignore: &[String], kind: io::ErrorKind) -> bool {
        match kind {
            io::ErrorKind::NotFound => ignore.iter().any(|item| item == IGNORE_NOT_FOUND),
            io::ErrorKind::PermissionDenied => ignore.iter().any(|item| item == IGNORE_NO_ACCESS),
            _ => false,
        }
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
            if key != "ignore" {
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

        if resource.is_empty() {
            return Err(Error::InvalidResource {
                loader: NAME.to_string(),
                resource: resource.to_string(),
                reason: "resource is required".into(),
            });
        }

        cfg_if! {
            if #[cfg(feature = "tracing")] {
                tracing::debug!(msg = "Loading configuration from filesystem", resource = resource);
            } else if #[cfg(feature = "logging")] {
                log::debug!("msg=\"Loading configuration from filesystem\" resource={resource}");
            }
        }

        let path = PathBuf::from(&resource);

        // Each entry: (name, format, path, source_for_this_entry)
        let list: Vec<(Option<String>, Option<String>, PathBuf, Source)> = if path.is_dir() {
            let entries = match fs::read_dir(&path) {
                Ok(entries) => entries,
                Err(error) if Self::should_ignore(&ignore, error.kind()) => return Ok(Vec::new()),
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

            let mut raw_list = Vec::new();
            for maybe_entry in entries {
                let Ok(entry) = maybe_entry else {
                    continue;
                };
                let entry_path = entry.path();
                if !entry_path.is_file() {
                    continue;
                }
                let (name, format) = Self::get_name_and_format(&entry_path);
                cfg_if! {
                    if #[cfg(feature = "tracing")] {
                        tracing::trace!(msg="Detected configuration file", name=?name, path=?entry_path);
                    } else if #[cfg(feature = "logging")] {
                        log::trace!("msg=\"Detected configuration file\" name={name:?} path={entry_path:?}");
                    }
                }
                raw_list.push((name, format, entry_path));
            }

            // Duplicate-name check
            let mut names: HashMap<String, String> = HashMap::with_capacity(raw_list.len());
            for (name_opt, format_opt, _path) in &raw_list {
                let name = match name_opt {
                    None => continue,
                    Some(n) => n.clone(),
                };
                let format = match format_opt {
                    Some(f) => f.clone(),
                    None => String::new(),
                };
                if let Some(other_format) = names.get(&name) {
                    return Err(Error::Duplicate {
                        loader: NAME.to_string(),
                        resource: resource.to_string(),
                        name,
                        format_1: other_format.clone(),
                        format_2: format,
                    });
                }
                names.insert(name, format);
            }

            // Build final list with per-file sources (resource = full file path)
            raw_list
                .into_iter()
                .map(|(name, format, entry_path)| {
                    let mut entry_source = source.clone();
                    entry_source.set_resource(entry_path.to_string_lossy().into_owned());
                    (name, format, entry_path, entry_source)
                })
                .collect()
        } else if path.is_file() {
            let (name, format) = Self::get_name_and_format(&path);
            vec![(name, format, path, source)]
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
        for (name, format, path, entry_source) in list {
            let content = match fs::read(&path) {
                Ok(content) => Some(content),
                Err(error) if Self::should_ignore(&ignore, error.kind()) => None,
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
                        tracing::trace!(msg = "Read configuration file", name = ?name, path = ?path, bytes = content.len());
                    } else if #[cfg(feature = "logging")] {
                        log::trace!("msg=\"Read configuration file\" name={name:?} path={path:?} bytes={}", content.len());
                    }
                }
                payload_list.push(Payload {
                    source: entry_source,
                    name,
                    format,
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
    fn get_name_and_format_from_path() {
        let path = PathBuf::from("/tmp/foo.JSON");
        assert_eq!(
            File::get_name_and_format(&path),
            (Some("foo".into()), Some("json".into()))
        );

        let path = PathBuf::from("/tmp/README");
        assert_eq!(
            File::get_name_and_format(&path),
            (Some("readme".into()), None)
        );

        let path = PathBuf::from("/tmp/.env");
        assert_eq!(
            File::get_name_and_format(&path),
            (Some(".env".into()), None)
        );
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
            if payload.name == Some("foo".to_string()) {
                foo = Some(payload);
            } else if payload.name == Some("readme".to_string()) {
                readme = Some(payload);
            } else if payload.name == Some(".env".to_string()) {
                dotenv = Some(payload);
            }
        }

        let foo = foo.expect("foo payload");
        assert_eq!(foo.format, Some("json".to_string()));

        let readme = readme.expect("readme payload");
        assert!(readme.format.is_none());

        let dotenv = dotenv.expect(".env payload");
        assert!(dotenv.format.is_none());
    }

    #[test]
    fn load_reads_files_from_directory() {
        let tmp = TempDir::new("tanzim-file").unwrap();
        fs::write(tmp.path().join("foo.json"), br#"{"hello":"world"}"#).unwrap();
        let resource = tmp.path().display().to_string();
        let loaded = File::new().load(make_source(&resource)).unwrap();
        assert_eq!(loaded.len(), 1);
        let payload = &loaded[0];
        assert_eq!(payload.name, Some("foo".to_string()));
        assert_eq!(payload.format, Some("json".to_string()));
        // Source resource updated to full file path
        assert!(payload.source.resource().ends_with("foo.json"));
    }

    #[test]
    fn load_errors_on_duplicate_formats() {
        let tmp = TempDir::new("tanzim-file-dup").unwrap();
        fs::write(tmp.path().join("foo.json"), b"{}").unwrap();
        fs::write(tmp.path().join("foo.yaml"), b"hello: world").unwrap();
        let resource = tmp.path().display().to_string();
        let error = File::new().load(make_source(&resource)).unwrap_err();
        assert!(matches!(error, Error::Duplicate { .. }));
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
}
