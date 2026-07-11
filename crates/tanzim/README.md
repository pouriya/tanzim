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

## Example

```rust
use serde::Deserialize;
use tanzim::Config;

#[derive(Deserialize)]
struct LogRotation {
    file: String,
    rotate_count: u32,
}

fn load() -> Result<LogRotation, tanzim::config::Error> {
    // `Config::default()` registers every feature-enabled loader and parser;
    // this reads `app.toml` deserializes it.
    Config::default().with_source("file:app.toml").try_deserialize()
}
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

## Validation errors

Validators coerce human-friendly inputs (`ByteSize` turns `"10MB"` into a byte count) and reject
bad ones with a caret — plus the validator's own description and example. Given a schema with a
`ByteSize` field and `max_size = "banana"`, the alternate error form (`{error:#}`) renders:

```text
configuration failed validation: max_size: invalid byte size at file:app.toml:2:12
  Rotate the log once it grows past this size.
  example: "10MB"
  1 | file = "/var/log/app.log"
  2 | max_size = "banana"
    |            ^^^^^^^^
```

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
