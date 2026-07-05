# tanzim
[**Package**](https://crates.io/crates/tanzim)   |   [**Documentation**](https://docs.rs/tanzim)   |   [**Repository**](https://github.com/pouriya/tanzim/tree/master/crates/tanzim)

Facade crate for a small, composable configuration pipeline: **load → parse → merge**.

`tanzim` lets you describe *where* configuration comes from with short source
strings (environment variables, files, HTTP, …), parse each source into a
typed value tree that remembers its origin, and merge everything into one map
keyed by entry name. Every value keeps its source location, so errors point at
the exact file, line, and column.

## Pipeline

```text
"env(prefix=APP_)"  "file:/etc/app"          ← source strings
        │
        ▼  load     Load::load(source)            → Vec<Payload>      (raw bytes + maybe name + maybe format)
        ▼  parse    Parse::parse(bytes)      → LocatedValue      (typed tree + Location)
        ▼  merge    Merge::merge(parsed)           → HashMap<name, …>  (grouped + combined)
        │
        ▼
   merged configuration
```

`pipeline::multi::Multi::run()` (or `pipeline::single::Single::run()` for a unified value) executes
all stages and returns the merged configuration; `try_deserialize::<T>()` runs the pipeline and
deserializes the result straight into your own type (single: one `T`; multi: a map keyed by entry
name). Each stage is also callable on its own via `load()`, `parse()`, and `merge()` — useful for
inspecting intermediate results or building a custom pipeline.

## Workspace crates

`tanzim` re-exports each stage so you rarely depend on them directly, but they
are independently usable:

| Re-export | Crate | Responsibility |
|-----------|-------|----------------|
| `source` | [`tanzim-source`](https://docs.rs/tanzim-source/latest/tanzim_source/) | Parse the `SOURCE[(OPTIONS)][:RESOURCE]` source-string format into a validated [`source::Source`] |
| `loader` | [`tanzim-load`](https://docs.rs/tanzim-load/latest/tanzim_load/) | `Load` trait + `env`/`file`/`http`/`closure` loaders → `Payload` |
| `parser` | [`tanzim-parse`](https://docs.rs/tanzim-parse/latest/tanzim_parse/) | `Parse` trait + `env`/`json`/`yaml`/`toml` parsers → `LocatedValue` |
| `merger` | [`tanzim-merge`](https://docs.rs/tanzim-merge/latest/tanzim_merge/) | `Merge` trait + `LastWins`/`DeepMerge` strategies → grouped map |
| `validator` | [`tanzim-validate`](https://docs.rs/tanzim-validate/latest/tanzim_validate/) | `Validator` trait + built-in validators + optional schema machinery |
| `value` | [`tanzim-value`](https://docs.rs/tanzim-value/latest/tanzim_value/) | Core `Value`, `LocatedValue`, `Map`, `Location`, `Error` types shared by every stage |

## Key concepts

- **Source strings** use the [`tanzim-source`](https://docs.rs/tanzim-source/latest/tanzim_source/) format
  `SOURCE [(OPTIONS)] [:RESOURCE]`, e.g. `env(prefix=APP_)`, `file:/etc/app`.
- **Named entries** — each `Payload` carries an optional `maybe_name`. The merger groups
  by name; unnamed payloads (`maybe_name == None`) all share the `None` key.
- **Format auto-detection** — if a payload has no `maybe_format`, parsers are probed via
  `is_format_supported`; otherwise the format hint selects the parser.
- **`on_error`** — the reserved option `on_error=(<stage>=skip)` (where `<stage>` is `load`,
  `parse`, or `validate`) makes a source silently skip failures for that stage instead of
  aborting the pipeline. E.g. `file(on_error=(load=skip)):.env`.
- **Located errors** — `Error` renders one line by default; use `{error:#}` for a
  source snippet with a caret underline.
- **Result types** — `parse()` returns `Vec<Parsed>` (a struct pairing each `Payload` with its
  parsed value, read via `payload()` / `value()`); `merge()` and multi `run()` return `Merged`, a
  map of named `Entry` values (each with `payloads()` + `value()`); single `run()` returns one
  unified `Entry`. All fields are private — use the accessors.
- **Typed configuration** — `try_deserialize::<T>()` deserializes the result into any
  `serde::Deserialize` type; errors point at the offending source `file:line:column`.

## Features

| Feature | Enables |
|---------|---------|
| `load-env` / `load-file` / `load-http` | env / filesystem / HTTP (closure-based) loaders |
| `parse-env` | env parser |
| `parse-json` / `parse-yaml` / `parse-toml` | format parsers |
| `validate-default` | the std-only validators (no extra dependencies) |
| `validate-schema` | schema machinery (`with_schema`, the validation stage) |
| `validate-<name>` | one validator (e.g. `validate-url`, `validate-regex`), pulling in `validate-schema` |
| `validate-full` | every validator + schema |
| `full` | all loaders + all parsers + `validate-full` (examples, smoke test) |
| `logging` / `tracing` | optional log integration across all crates |

Defaults: `load-env`, `load-file`, `load-http`, `parse-env`, `validate-default`, `validate-schema`.

Use individual workspace crates if you only need one stage — see [`tanzim-load`](https://docs.rs/tanzim-load/latest/tanzim_load/), [`tanzim-parse`](https://docs.rs/tanzim-parse/latest/tanzim_parse/), [`tanzim-merge`](https://docs.rs/tanzim-merge/latest/tanzim_merge/).

## Quick start

Everything you need lives under `pipeline::multi` (or `pipeline::single`), so a single glob import
is enough. `Multi::default()` pre-registers every feature-enabled loader and parser; pick a merger
and add sources (as parsed [`source::Source`] values).

```rust,ignore
use tanzim::pipeline::multi::*; // Multi, Source, DeepMerge, ...
use serde::Deserialize;

// One type per configuration section; each named entry is deserialized into `Entry`
// (absent sections stay `None`, unknown keys are ignored).
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Entry {
    sqlite: Option<Sqlite>,
    logging: Option<Logging>,
    https: Option<Https>,
}
#[derive(Debug, Deserialize)]
struct Sqlite {
    file: String,
    #[serde(default)]
    recreate: bool,
}
#[derive(Debug, Deserialize)]
struct Logging {
    level: String,
}
#[derive(Debug, Deserialize)]
struct Https {
    insecure: bool,
    #[serde(default)]
    follow_redirects: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let configs: std::collections::HashMap<Option<String>, Entry> = Multi::default()
        .with_merger(DeepMerge)
        .with_source(Source::parse("env(prefix=MY_APP_,separator=.)")?)
        .with_source(Source::parse("file:examples/full/etc")?)
        .try_deserialize()?;

    for (name, entry) in &configs {
        let display = name.as_deref().unwrap_or("(unnamed)");
        println!("{display}: {entry:?}");
    }
    Ok(())
}
```

### Opinionated facade

If you'd rather not wire the pipeline yourself, [`opt_in::config`] offers a
[`config`](https://docs.rs/config)-style layer with sensible defaults (all loaders + parsers, deep
merge):

```rust,ignore
use tanzim::opt_in::config::{Config, Environment, File};

let config = Config::builder()
    .add_source(File::with_name("config").required(false))
    .add_source(Environment::with_prefix("APP").separator("."))
    .build()?;

let port: u16 = config.get("server.port")?;
let name = config.get_string("server.name")?;
```

## Examples

```shell
make examples        # run every example
make example-full    # full pipeline over env + file sources
```

`example-full` reads the source strings declared in the `Makefile` and the
sample config under `examples/full/etc/`. The individual stage crates ship their
own examples too (see each crate's `examples/` directory).

## Development

```shell
make all          # build every crate
make test         # run unit tests + doctests across the workspace
make clippy       # lint with `-D warnings` across all targets and features
make check-style  # cargo fmt --check
make docs         # build rustdoc for the whole workspace
```
