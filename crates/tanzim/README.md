# tanzim
[**Package**](https://crates.io/crates/tanzim)   |   [**Documentation**](https://docs.rs/tanzim)   |   [**Repository**](https://github.com/tanzim-rs/tanzim)

Facade crate for a small, composable configuration pipeline: **load → parse → merge**.

`tanzim` lets you describe *where* configuration comes from with short source
strings (environment variables, files, HTTP, …), deserialize each source into a
typed value tree that remembers its origin, and merge everything into one map
keyed by entry name. Every value keeps its source location, so errors point at
the exact file, line, and column.

## Pipeline

```text
"env(prefix=APP_)"  "file:/etc/app"          ← source strings
        │
        ▼  load     Load::load(source)            → Vec<Payload>      (raw bytes + name + format)
        ▼  parse    Deserialize::parse(bytes)      → LocatedValue      (typed tree + Location)
        ▼  merge    Merge::merge(parsed)           → HashMap<name, …>  (grouped + combined)
        │
        ▼
   merged configuration
```

`Config::run()` executes all three stages. Each stage is also callable on its
own via `Config::load()`, `Config::parse()`, and `Config::merge()` — useful for
inspecting intermediate results or building a custom pipeline.

## Workspace crates

`tanzim` re-exports each stage so you rarely depend on them directly, but they
are independently usable:

| Re-export | Crate | Responsibility |
|-----------|-------|----------------|
| `source` | [`tanzim-source`](crates/tanzim-source/README.md) | Parse the `SOURCE[(OPTIONS)][?][:RESOURCE]` source-string format into a validated [`Source`] |
| `loader` | [`tanzim-load`](crates/tanzim-load/README.md) | `Load` trait + `env`/`file`/`http`/`closure` loaders → `Payload` |
| `parser` | [`tanzim-parse`](crates/tanzim-parse/README.md) | `Deserialize` trait + `env`/`json`/`yaml`/`toml` parsers → `LocatedValue` |
| `merge` | [`tanzim-merge`](crates/tanzim-merge/README.md) | `Merge` trait + `LastWins`/`DeepMerge` strategies → grouped map |
| — | [`tanzim-value`](crates/tanzim-value/README.md) | Core `Value`, `LocatedValue`, `Map`, `Location`, `Error` types shared by every stage |

## Key concepts

- **Source strings** use the [`tanzim-source`](crates/tanzim-source/README.md) format
  `SOURCE [(OPTIONS)] [?] [:RESOURCE]`, e.g. `env(prefix=APP_)`, `file?:.env`.
- **Named entries** — each `Payload` carries an optional `name`. The merger groups
  by name; unnamed payloads (`name == None`) all share the `""` key.
- **Format auto-detection** — if a payload has no `format`, parsers are probed via
  `is_format_supported`; otherwise the format hint selects the parser.
- **`ignore_errors` (`?`)** — sources marked with `?` swallow load/parse failures
  silently instead of aborting the pipeline.
- **Located errors** — [`Error`] renders one line by default; use `{error:#}` for a
  source snippet with a caret underline.
- **Result aliases** — `parse()` returns `Vec<Parsed>` and `merge()`/`run()` return
  `Merged` (`HashMap<String, (Vec<Payload>, LocatedValue)>`).

## Features

| Feature | Enables |
|---------|---------|
| `env` | env loader + env parser |
| `file` | filesystem loader |
| `http` | HTTP loader (closure-based, no HTTP client dependency) |
| `json` / `yaml` / `toml` | format parsers |
| `full` | all of the above (examples, smoke test) |
| `logging` / `tracing` | optional log integration |

Use individual workspace crates if you only need one stage — see [tanzim-load/README.md](crates/tanzim-load/README.md), [tanzim-parse/README.md](crates/tanzim-parse/README.md), [tanzim-merge/README.md](crates/tanzim-merge/README.md).

## Quick start

```rust,no_run
use tanzim::ConfigBuilder;
use tanzim::loader::{env::Env, file::File};
use tanzim::parser::{Env as EnvParser, Json, Yaml, Toml};
use tanzim::merge::DeepMerge;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let merged = ConfigBuilder::new()
        .with_loader(Env::new())
        .with_loader(File::new())
        .with_parser(EnvParser::new())
        .with_parser(Json::new())
        .with_parser(Yaml::new())
        .with_parser(Toml::new())
        .with_merger(DeepMerge)
        .with_source("env(prefix=MY_APP_,separator=.)")?
        .with_source("file:examples/basic/etc")?
        .build()
        .run()?;

    for (name, (_sources, value)) in &merged {
        let display = if name.is_empty() { "(unnamed)" } else { name.as_str() };
        println!("{display}: {value}");
    }
    Ok(())
}
```

## Examples

```shell
make examples        # run every example
make example-basic   # full pipeline over env + file sources
```

`example-basic` reads the source strings declared in the `Makefile` and the
sample config under `examples/basic/etc/`. The individual stage crates ship their
own examples too (see each crate's `examples/` directory).

## Development

```shell
make all          # build every crate
make test         # run unit tests + doctests across the workspace
make clippy       # lint with `-D warnings` across all targets and features
make check-style  # cargo fmt --check
make docs         # build rustdoc for the whole workspace
```
