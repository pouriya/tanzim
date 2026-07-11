#![doc(test(no_crate_inject))]
#![deny(missing_docs)]

//! # tanzim
//!
//! A configuration pipeline that **loads, parses, merges, and validates** configuration from
//! declarative sources — files, environment variables, HTTP, and more — into your own types.
//! Every value remembers where it came from, so a bad value points a caret at the exact source,
//! line, and column.
//!
//! # Load and deserialize
//!
//! [`Config::default`] starts a builder with every feature-enabled loader and parser already
//! registered. Add a source or two and [`try_deserialize`](Config::try_deserialize) straight into
//! your own type. The example runs inside a throwaway sandbox that prepares the file.
//!
//! ```rust
//! # #[cfg(all(feature = "parse-toml", feature = "load-file"))]
//! # tanzim_testing::environment::run(|env| {
//! use serde::Deserialize;
//!
//! // app.toml, prepared in the sandbox:
//! //   file = "/var/log/app.log"
//! //   rotate_count = 5
//! # env.write_file("app.toml", b"file = \"/var/log/app.log\"\nrotate_count = 5\n")?;
//!
//! #[derive(Deserialize)]
//! struct LogRotation {
//!     file: String,
//!     rotate_count: u32,
//! }
//!
//! let config: LogRotation = tanzim::Config::default()
//!     .with_source(tanzim::source::file("app.toml"))
//!     .try_deserialize().unwrap();
//!
//! assert_eq!(config.file, "/var/log/app.log");
//! assert_eq!(config.rotate_count, 5);
//! # Ok(())
//! # })
//! # .unwrap();
//! ```
//!
//! No schema is needed to catch a **wrong type**: because every value keeps its origin, a
//! mismatch during deserialization still points at the offending value. Had the file said
//! `rotate_count = "five"`, `try_deserialize` would fail, and `{error:#}` renders (verified exactly
//! in `tests/doc.rs`):
//!
//! ```text
//! failed to deserialize configuration: invalid type: string "five", expected u32 at file:app.toml:2:16
//!   1 | file = "/var/log/app.log"
//!   2 | rotate_count = "five"
//!     |                ^^^^^^
//! ```
//!
//! # Validate before deserializing
//!
//! Validation is the differentiator: check **and coerce** the configuration before it reaches your
//! type. Build a validator with the fluent API and register it with
//! [`with_schema`](config::ConfigBuilder::with_schema). Here [`ByteSize`](validator::ByteSize) —
//! one of many [built-in validators](#validation) — turns the human-friendly `"10MB"` into a byte
//! count, and its metadata surfaces in error messages.
//!
//! ```rust
//! # #[cfg(all(feature = "parse-toml", feature = "load-file", feature = "validate-static_map", feature = "validate-bytesize", feature = "validate-non_empty"))]
//! # tanzim_testing::environment::run(|env| {
//! use serde::Deserialize;
//! use tanzim::validator::{ByteSize, NonEmpty, StaticMap};
//!
//! // app.toml, prepared in the sandbox:
//! //   file = "/var/log/app.log"
//! //   max_size = "10MB"          # human-friendly
//! # env.write_file("app.toml", b"file = \"/var/log/app.log\"\nmax_size = \"10MB\"\n")?;
//!
//! #[derive(Deserialize)]
//! struct LogRotation {
//!     file: String,
//!     max_size: u64, // bytes
//! }
//!
//! // Build the validator fluently; it validates *and* coerces in place. `ByteSize`
//! // accepts "10MB", "1GiB", … . Attach metadata that shows up in error messages.
//! let schema = StaticMap::new()
//!     .required("file", NonEmpty::new())
//!     .required(
//!         "max_size",
//!         ByteSize::new()
//!             .with_description("Rotate the log once it grows past this size.")
//!             .with_example("10MB"),
//!     );
//!
//! let config: LogRotation = tanzim::Config::default()
//!     .with_source(tanzim::source::file("app.toml"))
//!     .with_schema(schema)
//!     .try_deserialize().unwrap();
//!
//! assert_eq!(config.max_size, 10_000_000); // "10MB" coerced to bytes
//! # Ok(())
//! # })
//! # .unwrap();
//! ```
//!
//! Had the file said `max_size = "banana"`, validation would fail — and the caret, the description,
//! and the example all surface via `{error:#}` (verified exactly in `tests/doc.rs`):
//!
//! ```text
//! configuration failed validation: max_size: invalid byte size at file:app.toml:2:12
//!   Rotate the log once it grows past this size.
//!   example: "10MB"
//!   1 | file = "/var/log/app.log"
//!   2 | max_size = "banana"
//!     |            ^^^^^^^^
//! ```
//!
//! # At a glance
//!
//! | Stage | Built in | Feature-gated |
//! |-------|----------|---------------|
//! | **Load** | env, file | HTTP & custom via closures |
//! | **Parse** | env, JSON, YAML, TOML | — |
//! | **Merge** | `LastWins` (default), `DeepMerge`, arbitrary [merge plans](merger::plan) | — |
//! | **Validate** | fluent validators + declarative schemas, with per-value caret errors | 30+ built-in validators |
//!
//! Source strings use the `SOURCE[(OPTIONS)][:RESOURCE]` format (e.g. `env(prefix=APP_)`,
//! `file:app.toml`). Feature flags select loaders (`load-*`), parsers (`parse-*`), and validators
//! (`validate-*`); the defaults cover env + file loading, env parsing, and the standard validators.
//!
//! # For application authors
//!
//! The common path is [`Config`] — it collapses every source into one unified configuration value:
//! [`Config::default`] → [`with_source`](config::ConfigBuilder::with_source) →
//! [`with_schema`](config::ConfigBuilder::with_schema) (optional) →
//! [`try_deserialize`](Config::try_deserialize). Run the stages one at a time with
//! [`stages`](Config::stages) to inspect intermediate results.
//!
//! When your sources describe **several named configurations** at once (one file per service, or an
//! env `separator` that splits `APP_web__port` into entry `web`, key `port`), reach for
//! [`pipeline::Pipeline`] instead: same stages, but it keeps a map of named entries
//! (`None` = the unnamed bucket) and [`try_deserialize`](pipeline::Pipeline::try_deserialize)s each.
//!
//! # Validation
//!
//! Validators check and coerce a value, and carry human-facing metadata
//! ([`with_name`](validator::Integer::with_name), `with_description`, `with_example`,
//! `with_default`) that the alternate error form surfaces. There are three ways to build one:
//!
//! - **Fluent builder** — compose validators directly, as in the example above
//!   (`StaticMap::new().required("max_size", ByteSize::new())`). Pass it to `with_schema`.
//! - **Declarative schema** — deserialize a JSON/YAML schema document into a
//!   [`SchemaValue`](validator::SchemaValue) and turn it into a validator with
//!   [`build_value`](validator::build_value): a node is a map with a `"type"` tag plus options,
//!   e.g. `{"type": "static_map", "fields": {"max_size": {"validator": {"type": "bytesize"}}}}`.
//! - **The free [`validate`](validator::validate) function** — run any validator against a
//!   [`LocatedValue`](value::LocatedValue) yourself.
//!
//! The built-in catalog is broad (each behind a `validate-<name>` feature): `Integer`, `Float`,
//! `Number`, `Bool`, `Str`, `NonEmpty`, `Enum`, `Either`, `List`, `StaticMap`, `DynamicMap`, and
//! domain types such as [`ByteSize`](validator::ByteSize) (`"10MB"`), `Duration` (`"30s"`),
//! `Percentage` (`"80%"`), `Port`, `Url`, `Email`, `IpAddr`/`Cidr`/`SocketAddr`, `Path`, `Semver`,
//! `Uuid`, `Base64`/`Hex`, and `RegexPattern`.
//!
//! # For library authors
//!
//! Each stage is a trait you can implement to extend the pipeline; the facade re-exports the
//! backing crate from each stage module.
//!
//! | To add a… | Implement | In |
//! |-----------|-----------|----|
//! | source loader | [`Load`](loader::Load) | [`loader`] ([`tanzim_load`]) |
//! | format parser | [`Parse`](parser::Parse) | [`parser`] ([`tanzim_parse`]) |
//! | merge strategy | [`Merge`](merger::Merge) | [`merger`] ([`tanzim_merge`]) |
//! | validator | [`Validator`](validator::Validator) | [`validator`] ([`tanzim_validate`]) |
//! | a quick loader/parser | a `Closure` adapter | [`loader::closure`] / [`parser::closure`] |
//!
//! # Workspace crates
//!
//! Each stage re-exports its independently usable crate:
//!
//! - [`tanzim_source`] — parses the `SOURCE[(OPTIONS)][:RESOURCE]` source-string format.
//! - [`tanzim_load`] — the [`Load`](loader::Load) trait and the env / file / HTTP / closure loaders.
//! - [`tanzim_parse`] — the [`Parse`](parser::Parse) trait and the env / JSON / YAML / TOML parsers.
//! - [`tanzim_merge`] — the [`Merge`](merger::Merge) trait, `LastWins` / `DeepMerge`, and merge plans.
//! - [`tanzim_validate`] — the [`Validator`](validator::Validator) trait, the built-ins, and schemas.
//! - [`tanzim_value`] — the core [`Value`](value::Value) / [`LocatedValue`](value::LocatedValue) types.
//!
//! # Roadmap
//!
//! Planned work — a standalone CLI, format conversion, and a C-ABI shared library — lives in the
//! [ROADMAP](https://github.com/pouriya/tanzim/blob/master/ROADMAP.md).

pub mod config;
pub mod entry;
pub mod loader;
pub mod merger;
pub mod parser;
pub mod pipeline;
pub mod source;
pub mod validator;
pub mod value;

mod logging;

pub use config::Config;
