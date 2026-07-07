//! A sandboxed temporary [`Environment`] for tests and examples.
//!
//! [`Environment::run`] executes a closure inside a freshly created temporary directory. On entry it
//! changes the working directory into that sandbox and snapshots the whole process environment; on
//! exit (whether the closure returns or panics) it restores the environment and working directory and
//! deletes the sandbox. Every run is serialized behind a process-global lock, so parallel tests do not
//! race on the shared working directory / environment — with the caveat that `Environment`-based tests
//! effectively run one at a time.
//!
//! ```
//! use tanzim_testing::environment::run;
//!
//! let read_back = run(|env| {
//!     env.write_file("hello.txt", b"world")?;
//!     Ok(std::fs::read_to_string("hello.txt")?)
//! })
//! .unwrap();
//! assert_eq!(read_back, "world");
//! ```

use cfg_if::cfg_if;
use std::error::Error as StdError;
use std::ffi::{OsStr, OsString};
use std::fmt::{self, Display, Formatter};
use std::path::{Component, Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::Instant;

/// Process-global lock. Serializes every [`Environment::run`] so concurrent test threads cannot stomp
/// on the shared working directory / environment. A `std` `Mutex` with a `const` initializer keeps the
/// crate dependency-free.
static ENV_LOCK: Mutex<()> = Mutex::new(());

/// Errors returned by [`Environment`] operations.
///
/// [`Display`] is one line by default; the alternate form (`{error:#}`) appends the underlying cause
/// chain, so wrapped [`std::io::Error`]s surface their real reason.
#[derive(Debug)]
pub enum Error {
    /// A method that requires an active sandbox was called outside of [`Environment::run`].
    Inactive,
    /// A filesystem or environment operation failed. `action` describes what was attempted and `path`
    /// names the target when there is one.
    Io {
        /// What was being attempted, e.g. `"create the file"`.
        action: String,
        /// The target path, when the failing operation had one.
        path: Option<PathBuf>,
        /// The underlying cause.
        source: std::io::Error,
    },
    /// A `create_*` / `write_file` path was absolute; sandbox paths must be relative.
    NotRelative {
        /// The offending path.
        path: PathBuf,
    },
    /// A path resolved outside the sandbox directory (e.g. it contained a `..` component).
    Escapes {
        /// The offending path.
        path: PathBuf,
    },
    /// A user error `?`-converted from inside a `run` closure.
    Other(Box<dyn StdError + Send + Sync>),
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Error::Inactive => write!(
                f,
                "the sandbox environment is not active; call this inside `Environment::run`"
            )?,
            Error::Io {
                action,
                path,
                source: _,
            } => match path {
                Some(path) => write!(f, "could not {action} `{}`", path.display())?,
                None => write!(f, "could not {action}")?,
            },
            Error::NotRelative { path } => write!(
                f,
                "path `{}` must be relative to the sandbox",
                path.display()
            )?,
            Error::Escapes { path } => {
                write!(f, "path `{}` escapes the sandbox directory", path.display())?
            }
            Error::Other(source) => write!(f, "{source}")?,
        }
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
            Error::Io { source, .. } => Some(source),
            Error::Other(source) => source.source(),
            Error::Inactive | Error::NotRelative { .. } | Error::Escapes { .. } => None,
        }
    }
}

impl From<Box<dyn StdError + Send + Sync>> for Error {
    fn from(source: Box<dyn StdError + Send + Sync>) -> Self {
        Error::Other(source)
    }
}

impl From<std::io::Error> for Error {
    fn from(source: std::io::Error) -> Self {
        Error::Other(Box::new(source))
    }
}

/// A sandboxed temporary environment. Build one with [`Environment::temporary`] and enter it with
/// [`Environment::run`], or use the free [`run`] function to do both at once.
pub struct Environment {
    entered: Option<Entered>,
}

/// State captured while a sandbox is active. Restored and torn down in [`Environment`]'s `Drop`.
struct Entered {
    directory: PathBuf,
    saved_cwd: PathBuf,
    saved_env: Vec<(OsString, OsString)>,
    started: Instant,
    // Held for the whole run; released only after `Drop` finishes cleaning up. Declared last so it is
    // dropped last.
    _guard: MutexGuard<'static, ()>,
}

/// Run `f` inside a fresh sandbox. Shorthand for [`Environment::temporary`] followed by
/// [`Environment::run`].
pub fn run<T>(f: impl FnOnce(&mut Environment) -> Result<T, Error>) -> Result<T, Error> {
    Environment::temporary().run(f)
}

impl Environment {
    /// Create a fresh, not-yet-entered environment. The temporary directory is created later, inside
    /// [`run`](Environment::run), while the global lock is held.
    pub fn temporary() -> Self {
        Self { entered: None }
    }

    /// The active sandbox directory (the canonicalized temporary directory), or `None` before
    /// [`run`](Environment::run) has entered.
    pub fn directory(&self) -> Option<&Path> {
        match &self.entered {
            Some(entered) => Some(&entered.directory),
            None => None,
        }
    }

    /// Enter the sandbox and run `f` inside it.
    ///
    /// Acquires the global lock, creates a temporary directory (falling back to the current directory
    /// if the system temporary directory is not writable), snapshots the environment and working
    /// directory, and `chdir`s into the sandbox. When `f` returns — or panics — the environment and
    /// working directory are restored and the sandbox is deleted before the lock is released.
    ///
    /// User errors convert into [`Error::Other`], so `?` works inside the closure for any
    /// `Box<dyn Error + Send + Sync>` or [`std::io::Error`].
    pub fn run<T>(
        mut self,
        f: impl FnOnce(&mut Environment) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let guard = match ENV_LOCK.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let started = Instant::now();

        cfg_if! {
            if #[cfg(feature = "tracing")] {
                tracing::debug!(msg = "Entering sandbox environment");
            } else if #[cfg(feature = "logging")] {
                log::debug!("msg=\"Entering sandbox environment\"");
            }
        }

        let saved_cwd = match std::env::current_dir() {
            Ok(cwd) => cwd,
            Err(source) => {
                return Err(Error::Io {
                    action: String::from("read the current directory"),
                    path: None,
                    source,
                });
            }
        };
        let saved_env = std::env::vars_os().collect();

        let nanos = match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
            Ok(elapsed) => elapsed.as_nanos(),
            Err(_) => 0,
        };
        let name = format!("tanzim-testing-{}-{}", std::process::id(), nanos);

        let target = std::env::temp_dir().join(&name);
        let created = match std::fs::create_dir(&target) {
            Ok(()) => target,
            Err(source) => {
                if source.kind() != std::io::ErrorKind::PermissionDenied {
                    return Err(Error::Io {
                        action: String::from("create the sandbox directory"),
                        path: Some(target),
                        source,
                    });
                }
                let fallback = saved_cwd.join(&name);
                match std::fs::create_dir(&fallback) {
                    Ok(()) => fallback,
                    Err(source) => {
                        return Err(Error::Io {
                            action: String::from("create the sandbox directory"),
                            path: Some(fallback),
                            source,
                        });
                    }
                }
            }
        };
        let directory = match std::fs::canonicalize(&created) {
            Ok(directory) => directory,
            Err(source) => {
                let _ = std::fs::remove_dir_all(&created);
                return Err(Error::Io {
                    action: String::from("resolve the sandbox directory"),
                    path: Some(created),
                    source,
                });
            }
        };

        cfg_if! {
            if #[cfg(feature = "tracing")] {
                tracing::info!(msg = "Created sandbox directory", path = ?directory);
            } else if #[cfg(feature = "logging")] {
                log::info!("msg=\"Created sandbox directory\" path={directory:?}");
            }
        }

        let cwd_target = directory.clone();
        self.entered = Some(Entered {
            directory,
            saved_cwd,
            saved_env,
            started,
            _guard: guard,
        });

        match std::env::set_current_dir(&cwd_target) {
            Ok(()) => {}
            Err(source) => {
                return Err(Error::Io {
                    action: String::from("enter the sandbox directory"),
                    path: Some(cwd_target),
                    source,
                });
            }
        }

        cfg_if! {
            if #[cfg(feature = "tracing")] {
                tracing::trace!(msg = "Changed working directory into sandbox", path = ?cwd_target);
            } else if #[cfg(feature = "logging")] {
                log::trace!("msg=\"Changed working directory into sandbox\" path={cwd_target:?}");
            }
        }

        f(&mut self)
    }

    /// Remove every environment variable from the process. The full environment was snapshotted on
    /// entry, so it is restored when the sandbox is dropped. A no-op if called outside of
    /// [`run`](Environment::run).
    pub fn clear_env(&mut self) {
        if self.entered.is_none() {
            return;
        }

        cfg_if! {
            if #[cfg(feature = "tracing")] {
                tracing::debug!(msg = "Clearing environment variables");
            } else if #[cfg(feature = "logging")] {
                log::debug!("msg=\"Clearing environment variables\"");
            }
        }

        for (key, _) in std::env::vars_os() {
            // SAFETY: guarded by ENV_LOCK; single-threaded within the sandbox.
            unsafe { std::env::remove_var(&key) };
        }
    }

    /// Set the environment variable `key` to `value` for the duration of the sandbox. The full
    /// environment was snapshotted on entry, so this is undone when the sandbox is dropped — use it
    /// instead of a hand-rolled `unsafe { std::env::set_var(..) }` so tests stay self-contained.
    /// Returns [`Error::Inactive`] when called outside of [`run`](Environment::run).
    pub fn set_env(
        &mut self,
        key: impl AsRef<OsStr>,
        value: impl AsRef<OsStr>,
    ) -> Result<(), Error> {
        if self.entered.is_none() {
            return Err(Error::Inactive);
        }
        let key = key.as_ref();
        let value = value.as_ref();

        cfg_if! {
            if #[cfg(feature = "tracing")] {
                tracing::debug!(msg = "Setting environment variable", key = ?key);
            } else if #[cfg(feature = "logging")] {
                log::debug!("msg=\"Setting environment variable\" key={key:?}");
            }
        }

        // SAFETY: guarded by ENV_LOCK; single-threaded within the sandbox.
        unsafe { std::env::set_var(key, value) };

        cfg_if! {
            if #[cfg(feature = "tracing")] {
                tracing::trace!(msg = "Set environment variable value", key = ?key, value = ?value);
            } else if #[cfg(feature = "logging")] {
                log::trace!("msg=\"Set environment variable value\" key={key:?} value={value:?}");
            }
        }
        cfg_if! {
            if #[cfg(feature = "tracing")] {
                tracing::info!(msg = "Set environment variable", key = ?key);
            } else if #[cfg(feature = "logging")] {
                log::info!("msg=\"Set environment variable\" key={key:?}");
            }
        }
        Ok(())
    }

    /// Create an empty file at `path` (relative to the sandbox), truncating any existing file. The
    /// sandbox is the current directory during [`run`](Environment::run), so read it back with the
    /// same relative path.
    pub fn create_file(&mut self, path: impl AsRef<Path>) -> Result<(), Error> {
        let directory = match &self.entered {
            Some(entered) => entered.directory.clone(),
            None => return Err(Error::Inactive),
        };
        let full = resolve(&directory, path.as_ref())?;
        let _existed = full.exists();

        cfg_if! {
            if #[cfg(feature = "tracing")] {
                tracing::debug!(msg = "Creating file", path = ?full);
            } else if #[cfg(feature = "logging")] {
                log::debug!("msg=\"Creating file\" path={full:?}");
            }
        }

        create_parents(&full)?;
        match std::fs::File::create(&full) {
            Ok(_) => {}
            Err(source) => {
                return Err(Error::Io {
                    action: String::from("create the file"),
                    path: Some(full),
                    source,
                });
            }
        }
        confirm_within(&directory, &full)?;

        cfg_if! {
            if #[cfg(feature = "tracing")] {
                tracing::info!(msg = "Created file", path = ?full, recreated = _existed);
            } else if #[cfg(feature = "logging")] {
                log::info!("msg=\"Created file\" path={full:?} recreated={_existed}");
            }
        }
        Ok(())
    }

    /// Create a fresh file at `path` (relative to the sandbox), truncating any existing file, and write
    /// `contents` to it.
    pub fn write_file(
        &mut self,
        path: impl AsRef<Path>,
        contents: impl AsRef<[u8]>,
    ) -> Result<(), Error> {
        let directory = match &self.entered {
            Some(entered) => entered.directory.clone(),
            None => return Err(Error::Inactive),
        };
        let full = resolve(&directory, path.as_ref())?;
        let bytes = contents.as_ref();
        let _existed = full.exists();

        cfg_if! {
            if #[cfg(feature = "tracing")] {
                tracing::debug!(msg = "Writing file", path = ?full, bytes = bytes.len());
            } else if #[cfg(feature = "logging")] {
                log::debug!("msg=\"Writing file\" path={full:?} bytes={}", bytes.len());
            }
        }

        create_parents(&full)?;
        match std::fs::write(&full, bytes) {
            Ok(()) => {}
            Err(source) => {
                return Err(Error::Io {
                    action: String::from("write the file"),
                    path: Some(full),
                    source,
                });
            }
        }
        confirm_within(&directory, &full)?;

        cfg_if! {
            if #[cfg(feature = "tracing")] {
                tracing::trace!(
                    msg = "Wrote file contents",
                    path = ?full,
                    contents = %String::from_utf8_lossy(bytes),
                );
            } else if #[cfg(feature = "logging")] {
                log::trace!(
                    "msg=\"Wrote file contents\" path={full:?} contents={}",
                    String::from_utf8_lossy(bytes),
                );
            }
        }
        cfg_if! {
            if #[cfg(feature = "tracing")] {
                tracing::info!(
                    msg = "Wrote file",
                    path = ?full,
                    bytes = bytes.len(),
                    recreated = _existed,
                );
            } else if #[cfg(feature = "logging")] {
                log::info!(
                    "msg=\"Wrote file\" path={full:?} bytes={} recreated={_existed}",
                    bytes.len(),
                );
            }
        }
        Ok(())
    }

    /// Create a directory (and any missing parents) at `path`, relative to the sandbox.
    pub fn create_directory(&mut self, path: impl AsRef<Path>) -> Result<(), Error> {
        let directory = match &self.entered {
            Some(entered) => entered.directory.clone(),
            None => return Err(Error::Inactive),
        };
        let full = resolve(&directory, path.as_ref())?;
        let _existed = full.exists();

        cfg_if! {
            if #[cfg(feature = "tracing")] {
                tracing::debug!(msg = "Creating directory", path = ?full);
            } else if #[cfg(feature = "logging")] {
                log::debug!("msg=\"Creating directory\" path={full:?}");
            }
        }

        match std::fs::create_dir_all(&full) {
            Ok(()) => {}
            Err(source) => {
                return Err(Error::Io {
                    action: String::from("create the directory"),
                    path: Some(full),
                    source,
                });
            }
        }
        confirm_within(&directory, &full)?;

        cfg_if! {
            if #[cfg(feature = "tracing")] {
                tracing::info!(msg = "Created directory", path = ?full, recreated = _existed);
            } else if #[cfg(feature = "logging")] {
                log::info!("msg=\"Created directory\" path={full:?} recreated={_existed}");
            }
        }
        Ok(())
    }
}

impl Drop for Environment {
    fn drop(&mut self) {
        let entered = match self.entered.take() {
            Some(entered) => entered,
            None => return,
        };

        cfg_if! {
            if #[cfg(feature = "tracing")] {
                tracing::trace!(msg = "Restoring environment and removing sandbox");
            } else if #[cfg(feature = "logging")] {
                log::trace!("msg=\"Restoring environment and removing sandbox\"");
            }
        }

        for (key, _) in std::env::vars_os() {
            // SAFETY: guarded by ENV_LOCK; single-threaded within the sandbox.
            unsafe { std::env::remove_var(&key) };
        }
        for (key, value) in &entered.saved_env {
            // SAFETY: guarded by ENV_LOCK; single-threaded within the sandbox.
            unsafe { std::env::set_var(key, value) };
        }

        match std::env::set_current_dir(&entered.saved_cwd) {
            Ok(()) => {}
            Err(_source) => {
                cfg_if! {
                    if #[cfg(feature = "tracing")] {
                        tracing::warn!(
                            msg = "Could not restore working directory",
                            path = ?entered.saved_cwd,
                            error = ?_source,
                        );
                    } else if #[cfg(feature = "logging")] {
                        log::warn!(
                            "msg=\"Could not restore working directory\" path={:?} error={_source:?}",
                            entered.saved_cwd,
                        );
                    }
                }
            }
        }

        match std::fs::remove_dir_all(&entered.directory) {
            Ok(()) => {
                cfg_if! {
                    if #[cfg(feature = "tracing")] {
                        tracing::info!(msg = "Removed sandbox directory", path = ?entered.directory);
                    } else if #[cfg(feature = "logging")] {
                        log::info!(
                            "msg=\"Removed sandbox directory\" path={:?}",
                            entered.directory,
                        );
                    }
                }
            }
            Err(_source) => {
                cfg_if! {
                    if #[cfg(feature = "tracing")] {
                        tracing::warn!(
                            msg = "Could not remove sandbox directory",
                            path = ?entered.directory,
                            error = ?_source,
                        );
                    } else if #[cfg(feature = "logging")] {
                        log::warn!(
                            "msg=\"Could not remove sandbox directory\" path={:?} error={_source:?}",
                            entered.directory,
                        );
                    }
                }
            }
        }

        let _held = entered.started.elapsed();
        cfg_if! {
            if #[cfg(feature = "tracing")] {
                tracing::info!(msg = "Released sandbox lock", held_seconds = _held.as_secs_f64());
            } else if #[cfg(feature = "logging")] {
                log::info!("msg=\"Released sandbox lock\" held_seconds={}", _held.as_secs_f64());
            }
        }
    }
}

/// Join `relative` onto the sandbox `directory`, rejecting absolute paths and any `..` component that
/// could escape the sandbox.
fn resolve(directory: &Path, relative: &Path) -> Result<PathBuf, Error> {
    if relative.is_absolute() {
        return Err(Error::NotRelative {
            path: relative.to_path_buf(),
        });
    }
    for component in relative.components() {
        if matches!(component, Component::ParentDir) {
            return Err(Error::Escapes {
                path: relative.to_path_buf(),
            });
        }
    }
    Ok(directory.join(relative))
}

/// Create any missing parent directories for `full`.
fn create_parents(full: &Path) -> Result<(), Error> {
    match full.parent() {
        Some(parent) => match std::fs::create_dir_all(parent) {
            Ok(()) => Ok(()),
            Err(source) => Err(Error::Io {
                action: String::from("create parent directories"),
                path: Some(parent.to_path_buf()),
                source,
            }),
        },
        None => Ok(()),
    }
}

/// Defense in depth: confirm the just-created `full` canonicalizes to somewhere inside `directory`.
fn confirm_within(directory: &Path, full: &Path) -> Result<(), Error> {
    let canonical = match std::fs::canonicalize(full) {
        Ok(canonical) => canonical,
        Err(source) => {
            return Err(Error::Io {
                action: String::from("resolve the created path"),
                path: Some(full.to_path_buf()),
                source,
            });
        }
    };
    if canonical.starts_with(directory) {
        Ok(())
    } else {
        Err(Error::Escapes {
            path: full.to_path_buf(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_and_source() {
        let error = Error::NotRelative {
            path: PathBuf::from("/abs"),
        };
        assert!(error.to_string().contains("relative"));
        assert!(StdError::source(&error).is_none());

        let inner = std::io::Error::new(std::io::ErrorKind::NotFound, "nope");
        let error = Error::Io {
            action: String::from("create the file"),
            path: Some(PathBuf::from("x")),
            source: inner,
        };
        assert!(error.to_string().contains("create the file"));
        assert!(StdError::source(&error).is_some());
        assert!(format!("{error:#}").contains("nope"));
    }

    #[test]
    fn resolve_rejects_absolute_and_parent() {
        let directory = Path::new("/sandbox");
        assert!(matches!(
            resolve(directory, Path::new("/abs")),
            Err(Error::NotRelative { .. })
        ));
        assert!(matches!(
            resolve(directory, Path::new("../up")),
            Err(Error::Escapes { .. })
        ));
        assert!(resolve(directory, Path::new("a/b.txt")).is_ok());
    }

    // All sandbox behavior lives in one test so that only a single test mutates the process-global
    // working directory / environment; parallel tests would otherwise race despite the lock.
    #[test]
    fn sandbox_lifecycle() {
        let before = std::env::current_dir().unwrap();

        // SAFETY: this is the only test that mutates the process environment.
        unsafe { std::env::set_var("TANZIM_TESTING_PRE", "keep") };

        let sandbox = run(|env| {
            let directory = env.directory().unwrap().to_path_buf();
            assert_eq!(std::env::current_dir().unwrap(), directory);

            env.create_file("empty.txt")?;
            assert!(std::fs::metadata("empty.txt").unwrap().is_file());

            env.write_file("cfg/app.json", b"{\"port\":8080}")?;
            assert_eq!(
                std::fs::read_to_string("cfg/app.json").unwrap(),
                "{\"port\":8080}"
            );
            env.write_file("cfg/app.json", b"{}")?;
            assert_eq!(std::fs::read_to_string("cfg/app.json").unwrap(), "{}");

            env.create_directory("logs")?;
            assert!(std::fs::metadata("logs").unwrap().is_dir());

            assert!(matches!(
                env.create_file("/etc/passwd"),
                Err(Error::NotRelative { .. })
            ));
            assert!(matches!(
                env.create_file("../escape.txt"),
                Err(Error::Escapes { .. })
            ));

            env.set_env("TANZIM_TESTING_INNER", "1")?;
            assert_eq!(std::env::var("TANZIM_TESTING_INNER").unwrap(), "1");
            env.clear_env();
            assert!(std::env::var("TANZIM_TESTING_PRE").is_err());

            Ok(directory)
        })
        .unwrap();

        assert_eq!(std::env::current_dir().unwrap(), before);
        assert!(!sandbox.exists());
        assert!(std::env::var("TANZIM_TESTING_INNER").is_err());
        assert_eq!(std::env::var("TANZIM_TESTING_PRE").unwrap(), "keep");

        let converted: Result<(), Error> = run(|_env| {
            let io = std::io::Error::other("boom");
            Err(io)?;
            Ok(())
        });
        assert!(matches!(converted, Err(Error::Other(_))));

        let panicked = std::panic::catch_unwind(|| {
            let _ = run(|_env| -> Result<(), Error> { panic!("boom") });
        });
        assert!(panicked.is_err());
        assert_eq!(std::env::current_dir().unwrap(), before);

        // SAFETY: this is the only test that mutates the process environment.
        unsafe { std::env::remove_var("TANZIM_TESTING_PRE") };
    }
}
