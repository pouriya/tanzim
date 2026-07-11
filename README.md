# tanzim

![load from anywhere · parse anything · merge safely · validate intelligently](pipeline.svg)

## Why

Real configuration never lives in one place or in one format. It arrives from environment variables, files, whole directories, and remote endpoints — written in env, YAML, TOML, or JSON — and it has to be combined with clear precedence and checked before anything trusts it. The usual answer is a pile of glue code stitched across several unrelated libraries, and somewhere in that pile a value quietly loses track of *where it came from*.

`tanzim` treats the whole thing as **one pipeline** instead of a pile of glue. A value flows from its source through every stage while carrying its origin the entire way, so when something is wrong the error can point at the exact file, line, and column that caused it.

## Principles

These are the properties of the project as a whole — the reasons the pieces fit together the
way they do. Each stage's own README covers the details of *how*.

- **Located everything.** Every value *and every error* remembers its exact source, line, and column. Errors render a caret-underlined snippet pointing straight at the offending input.
- **Pluggable at every stage.** Loading, parsing, merging, and validating are each just a trait. Bring your own source kind, format, merge strategy, or validator without forking anything.
- **Pay for what you use.** Every source, format, and validator is feature-gated. Take the whole pipeline through the facade, or depend on a single stage crate on its own.
- **Declarative sources.** Say *where* configuration comes from with short strings like `env(prefix=APP_)` or `file(on_error=(load=skip)):/etc/app` — not hand-wired setup code.
- **Validation is part of the pipeline.** Schema-driven checking and coercion is a first-class final stage, not something you bolt on afterward.
- **One config, or many.** Collapse everything into a single unified value, or keep named entries as a map — whichever fits how your application reads its configuration.

## Using in Rust

### Simple example

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
    .with_source(tanzim::source::file("app.toml"))
    .try_deserialize()
    .unwrap();

assert_eq!(config.file, "/var/log/app.log");
assert_eq!(config.rotate_count, 5);

// Had `rotate_count` been `"five"`, `try_deserialize` would fail.
// Formatted with `{error:#}` (verified in `crates/tanzim/tests/doc.rs`):
//
//   failed to deserialize configuration: invalid type: string "five", expected u32
//   at file:app.toml:2:16
//     1 | file = "/var/log/app.log"
//     2 | rotate_count = "five"
//       |                ^^^^^^
```

### Advanced example

Supply an explicit merge plan and a schema that both validates and coerces values before
deserialization. Here `app.toml` (system defaults) and `user/app.toml` (user overrides) share the
filename stem `app` and are **deep-merged** — keys only in the system file are kept; keys in both
take the user's value. The env source (`APP_*`) is then applied last and wins any remaining
conflicts. `ByteSize` turns the human-friendly `"20MB"` into a byte count, and its description
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
// Formatted with `{error:#}` (verified in `crates/tanzim/tests/doc.rs`):
//
//   configuration failed validation: max_size: invalid byte size at file:user/app.toml:1:12
//     Rotate the log once it grows past this size.
//     example: "10MB"
//     1 | max_size = "banana"
//       |            ^^^^^^^^
```

Full walkthrough, features, and per-stage recipes → [crates.io](https://crates.io/crates/tanzim) · [docs.rs](https://docs.rs/tanzim).

## License

Licensed under [MIT](LICENSE).
