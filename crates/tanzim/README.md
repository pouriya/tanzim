# tanzim

[![crates.io](https://img.shields.io/crates/v/tanzim.svg)](https://crates.io/crates/tanzim)
[![docs.rs](https://docs.rs/tanzim/badge.svg)](https://docs.rs/tanzim)
[![CI](https://github.com/pouriya/tanzim/actions/workflows/rust.yml/badge.svg)](https://github.com/pouriya/tanzim/actions/workflows/rust.yml)
[![license](https://img.shields.io/crates/l/tanzim.svg)](https://github.com/pouriya/tanzim/blob/master/LICENSE)

![load · parse · merge · validate](https://raw.githubusercontent.com/pouriya/tanzim/refs/heads/master/pipeline.svg)

A configuration pipeline that **loads, parses, merges, and validates** configuration from
declarative sources — files, environment variables, HTTP, and more — into your own types. Every
value remembers where it came from, so a bad value points a caret at the exact source, line, and
column. Built on the Rust 2024 edition (Rust ≥ 1.85).

## Simple example

[`Config::default`] registers every feature-enabled loader and parser. Add a source string and
call [`try_deserialize`] to get your type in one chain.

```rust
// Suppose `app.toml` contains:
//
//   file = "/var/log/app.log"
//   rotate_count = 5

use serde::Deserialize;
use tanzim::Config;

#[derive(Deserialize)]
struct LogRotation {
    file: String,
    rotate_count: u32,
}

let config: LogRotation = Config::default()
    .with_source("file:app.toml")
    .try_deserialize()
    .unwrap();

assert_eq!(config.file, "/var/log/app.log");
assert_eq!(config.rotate_count, 5);

// Had `rotate_count` been `"five"`, `try_deserialize` would fail.
// Formatted with `{error:#}` (verified in `tests/doc.rs`):
//
//   failed to deserialize configuration: invalid type: string "five", expected u32
//   at file:app.toml:2:16
//     1 | file = "/var/log/app.log"
//     2 | rotate_count = "five"
//       |                ^^^^^^
```

## Advanced example

Supply an explicit merge plan and a schema that both validates and coerces values before
deserialization. Here `app.toml` (system defaults) and `user/app.toml` (user overrides) share the
filename stem `app` and are **deep-merged** — keys only in the system file are kept; keys in both
take the user's value. The env source (`APP_*`) is then applied last and wins any remaining
conflicts. [`ByteSize`] turns the human-friendly `"20MB"` into a byte count, and its description
and example surface in any error message.

```rust
// Suppose `app.toml` contains:
//
//   file = "/var/log/app.log"
//   max_size = "100MB"
//
// Suppose `user/app.toml` contains (partial override — only changes max_size):
//
//   max_size = "10MB"
//
// And the environment has APP_FILE=/var/log/app.log APP_MAX_SIZE=20MB

use serde::Deserialize;
use tanzim::{
    Config,
    merger::plan::{deep, last_wins, src},
    validator::{ByteSize, NonEmpty, StaticMap},
};

#[derive(Deserialize)]
struct LogRotation {
    file: String,
    max_size: u64, // bytes, coerced by ByteSize
}

// deep-merge the two files, then let env win any remaining conflicts.
let plan = last_wins(vec![
    deep(vec![
        src("file:app.toml").unwrap(),
        src("file:user/app.toml").unwrap(),
    ]),
    src("env(prefix=APP_)").unwrap(),
]);

let schema = StaticMap::new()
    .required("file", NonEmpty::new())
    .required(
        "max_size",
        ByteSize::new()
            .with_description("Rotate the log once it grows past this size.")
            .with_example("10MB"),
    );

let config: LogRotation = Config::from_plan(plan)
    .with_default_loaders()
    .with_default_parsers()
    .with_schema(schema)
    .try_deserialize()
    .unwrap();

assert_eq!(config.max_size, 20_000_000); // env wins: "20MB" coerced to bytes
assert_eq!(config.file, "/var/log/app.log");

// Had `user/app.toml` said `max_size = "banana"`, validation would fail.
// Formatted with `{error:#}` (verified in `tests/doc.rs`):
//
//   configuration failed validation: max_size: invalid byte size at file:user/app.toml:1:12
//     Rotate the log once it grows past this size.
//     example: "10MB"
//     1 | max_size = "banana"
//       |            ^^^^^^^^
```

## Why tanzim

- **Validation with caret errors** — check *and coerce* configuration before it reaches your type;
  a failure points a caret at the exact source, line, and column, and can even suggest a valid
  value.
- **Many formats** — environment variables, JSON, YAML, and TOML out of the box.
- **Flexible merging** — last-wins or deep merge in declared order, or an arbitrary merge plan.
- **Per-value provenance** — every value knows where it came from, which is what powers the located
  errors.
- **Composable** — each stage (load, parse, merge, validate) is a trait you can implement to extend
  the pipeline.

## Features

| Feature | Enables |
|---------|---------|
| `load-env` / `load-file` / `load-http-closure` | env / filesystem / HTTP (closure-based) loaders |
| `parse-env` / `parse-json` / `parse-yaml` / `parse-toml` | format parsers |
| `validate-default` | the standard, dependency-free validators |
| `validate-schema` | schema machinery (`with_schema`, the validation stage) |
| `validate-<name>` | one validator (e.g. `validate-bytesize`, `validate-url`), pulling in `validate-schema` |
| `validate-full` | every validator + schema |
| `logging` / `tracing` | optional log integration across all crates |

Defaults: `load-env`, `load-file`, `load-http-closure`, `parse-env`, `validate-default`,
`validate-schema`.

## Documentation

**Full documentation, guides, and runnable examples → [docs.rs/tanzim](https://docs.rs/tanzim).**
The API docs cover application use (`Config`), multiple named configurations
(`pipeline::Pipeline`), the full validator catalog, and how to extend each stage.

## Workspace crates

`tanzim` re-exports each stage as a module (`tanzim::loader`, `tanzim::parser`, …), but every crate
is independently usable:

- [`tanzim-source`](https://docs.rs/tanzim-source) — parses the `SOURCE[(OPTIONS)][:RESOURCE]` source-string format.
- [`tanzim-load`](https://docs.rs/tanzim-load) — the `Load` trait and the env / file / HTTP / closure loaders.
- [`tanzim-parse`](https://docs.rs/tanzim-parse) — the `Parse` trait and the env / JSON / YAML / TOML parsers.
- [`tanzim-merge`](https://docs.rs/tanzim-merge) — the `Merge` trait, `LastWins` / `DeepMerge`, and merge plans.
- [`tanzim-validate`](https://docs.rs/tanzim-validate) — the `Validator` trait, the built-in validators, and the schema machinery.
- [`tanzim-value`](https://docs.rs/tanzim-value) — the core `Value` / `LocatedValue` types shared by every stage.
- [`tanzim-testing`](https://docs.rs/tanzim-testing) — the sandboxed `Environment` used by the examples and tests.

## MSRV, license, and contributing

Requires the Rust 2024 edition (Rust ≥ 1.85). Licensed under [MIT](https://github.com/pouriya/tanzim/blob/master/LICENSE).
Issues and pull requests are welcome at [github.com/pouriya/tanzim](https://github.com/pouriya/tanzim).

## Roadmap

Planned work — a standalone CLI, format conversion, and a C-ABI shared library — lives in the [ROADMAP](https://github.com/pouriya/tanzim/blob/master/ROADMAP.md).
