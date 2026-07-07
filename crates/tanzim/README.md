# tanzim

[**Package**](https://crates.io/crates/tanzim)   |   [**Documentation**](https://docs.rs/tanzim)   |   [**Repository**](https://github.com/pouriya/tanzim/tree/master/crates/tanzim)

![load · parse · merge · validate](https://raw.githubusercontent.com/pouriya/tanzim/refs/heads/master/pipeline.svg)

Facade crate for a small, composable configuration pipeline: **load → parse → merge → validate**.

`tanzim` lets you describe *where* configuration comes from with short source strings (environment
variables, files, HTTP, …), parse each source into a typed value tree that remembers its origin, and
merge everything into your own type. Every value keeps its source location, so when something is
wrong the error points at the exact file, line, and column.

Two entry points share the same stages:

- [`Config`](https://docs.rs/tanzim/latest/tanzim/struct.Config.html) — **collapses every source into one unified configuration value.** This is what most applications want.
- [`pipeline::Pipeline`](https://docs.rs/tanzim/latest/tanzim/pipeline/struct.Pipeline.html) — **keeps a map of *named* entries** (`None` = the unnamed bucket), for when your sources describe several configurations at once.

## Quick start — `Config`

Add a couple of sources and deserialize straight into your own type. The example below runs inside a
throwaway sandbox (from [`tanzim-testing`](https://docs.rs/tanzim-testing)) that prepares the file and
environment variable, so it is fully self-contained.

```rust
# #[cfg(feature = "parse-toml")]
# tanzim_testing::environment::run(|env| {
use serde::Deserialize;
use tanzim::merger::DeepMerge;

// A config file plus one environment override, prepared in the sandbox.
env.write_file("app.toml", b"[server]\nhost = \"localhost\"\nport = 8080\n")?;
env.set_env("APP_app__server__host", "0.0.0.0")?;

#[derive(Deserialize)]
struct AppConfig {
    server: Server,
}
#[derive(Deserialize)]
struct Server {
    host: String,
    port: u16,
}

// `Config::default()` pre-registers every feature-enabled loader and parser.
// Sources are read in declared order; a deep merger folds later sources into
// earlier ones field-by-field, so the env var wins `host` while `port` survives.
let config: AppConfig = tanzim::Config::default()
    .with_merger(DeepMerge::new()).unwrap()
    .with_source("file:app.toml").unwrap()
    .with_source("env(prefix=APP_,separator=__)").unwrap()
    .try_deserialize().unwrap();

assert_eq!(config.server.host, "0.0.0.0"); // overridden by the env var
assert_eq!(config.server.port, 8080);      // kept from the file
# Ok(())
# })
# .unwrap();
```

`Config` collapses everything into one value. When your sources describe **several named
configurations**, reach for [`Pipeline`](#pipeline--multiple-named-entries). And for any fold beyond a
straight last-wins / deep merge in declared order, build a [merge plan](#merging--merge-plans).

## Merging & merge plans

The merge stage folds the sources together. Pick a global merger with `with_merger` (it defaults to
`LastWins` — later sources replace earlier ones):

- [`LastWins`](https://docs.rs/tanzim-merge) — later source replaces the earlier value wholesale.
- [`DeepMerge`](https://docs.rs/tanzim-merge) — recursively merges maps, so sibling fields from different sources survive.
- `with_source_merged(source, merger)` binds a merger to a single source's payloads before the global fold.

The simple builders always fold in declared order. For an **arbitrary fold** — deep-merge some
sources, then last-wins the result against others — build a [`MergePlan`](https://docs.rs/tanzim-merge)
yourself with the `merger::plan` constructors (`src`, `deep`, `last_wins`, `merge_with`) and hand it
to `with_merge_plan`. The plan's `src(..)` leaves become the pipeline's sources:

```rust
# #[cfg(feature = "parse-toml")]
# tanzim_testing::environment::run(|env| {
use serde::Deserialize;
use tanzim::merger::plan::{deep, src};

// Two files that describe the *same* entry (same file name in different dirs),
// so their fields merge instead of landing in separate entries.
env.write_file("base/app.toml", b"host = \"0.0.0.0\"\nport = 80\n")?;
env.write_file("prod/app.toml", b"port = 8080\n")?;

#[derive(Deserialize)]
struct App {
    host: String,
    port: u16,
}

// Build the fold yourself: deep-merge `base` under `prod`.
let config: App = tanzim::Config::default()
    .with_merge_plan(deep(vec![
        src("file:base/app.toml").unwrap(),
        src("file:prod/app.toml").unwrap(),
    ]))
    .unwrap()
    .try_deserialize()
    .unwrap();

assert_eq!(config.host, "0.0.0.0"); // kept from base
assert_eq!(config.port, 8080);      // overridden by prod
# Ok(())
# })
# .unwrap();
```

The simple source builders and `with_merge_plan` are mutually exclusive — mixing them yields
`Error::PlanConflict`. Either let the pipeline build the plan, or build it yourself.

## `Pipeline` — multiple named entries

`Pipeline` keeps every entry separate, keyed by name. Names come from the source: a **file's** name
is its filename stem (`web.toml` → `web`); an **env** source with a `separator` splits each variable
once — the left part is the entry name, the right part is the key within it (`APP_web__port` →
entry `web`, key `port`). Build one with the free [`pipeline::default()`] / [`pipeline::empty()`]
functions.

### 1. Named entries, deep-merged

```rust
# #[cfg(feature = "parse-toml")]
# tanzim_testing::environment::run(|env| {
use serde::Deserialize;
use std::collections::HashMap;
use tanzim::merger::DeepMerge;

// A config directory: one file per named entry, plus an env override for `web`.
env.write_file("etc/web.toml", b"name = \"web-service\"\nurl = \"http://localhost:8080\"\n")?;
env.write_file("etc/db.toml", b"url = \"postgres://localhost/app\"\n")?;
env.set_env("APP_web__url", "http://0.0.0.0:80")?;

#[derive(Deserialize)]
struct Service {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    url: Option<String>,
}

let services: HashMap<Option<String>, Service> = tanzim::pipeline::default()
    .with_merger(DeepMerge::new()).unwrap()
    .with_source("file:etc").unwrap()
    .with_source("env(prefix=APP_,separator=__)").unwrap()
    .try_deserialize().unwrap();

// `web` merges its file entry with the env override, field-by-field:
assert_eq!(services[&Some("web".into())].name.as_deref(), Some("web-service")); // kept from file
assert_eq!(services[&Some("web".into())].url.as_deref(), Some("http://0.0.0.0:80")); // env override
assert_eq!(services[&Some("db".into())].url.as_deref(), Some("postgres://localhost/app"));
# Ok(())
# })
# .unwrap();
```

### 2. Per-entry schema validation

Register a schema per entry name with `with_schema`; the validation stage runs after merging and
coerces/checks each entry (requires the `validate-schema` feature, on by default). A failure surfaces
as `pipeline::Error::Validate`, pointing at the offending value.

```rust
# #[cfg(all(feature = "parse-json", feature = "validate-schema"))]
# tanzim_testing::environment::run(|env| {
use tanzim::validator::SchemaValue;

// The data under validation, written to `etc/web.json` (`port` is out of range on purpose):
//
//   {
//     "host": "0.0.0.0",
//     "port": 70000
//   }
env.write_file("etc/web.json", b"{\n  \"host\": \"0.0.0.0\",\n  \"port\": 70000\n}\n")?;

// A schema for the `web` entry: `port` must be an integer in 1..=65535.
let schema = serde_json::from_str::<SchemaValue>(
    r#"{
        "type": "static_map",
        "allow_unknown": true,
        "fields": {
            "port": { "required": true, "validator": { "type": "integer", "min": 1, "max": 65535 } }
        }
    }"#,
)
.unwrap()
.into_value();

// Builder pattern: `default()` registers the loaders + parsers, then add the source and the
// per-entry schema. Schemas are keyed by entry name, so the error names the entry it came from.
let error = tanzim::pipeline::default()
    .with_source("file:etc").unwrap()
    .with_schema(Some("web".into()), schema)
    .run()
    .unwrap_err();

assert!(matches!(error, tanzim::pipeline::Error::Validate { .. }));

// `{error:#}` shows the full message — the entry name, the reason, and a caret at the value:
//
//   configuration `Some("web")` failed validation: port: 70000 is above the maximum 65535 at file:etc/web.json:3:11
//     1 | {
//     2 |   "host": "0.0.0.0",
//     3 |   "port": 70000
//       |           ^^^^^
//     4 | }
assert!(format!("{error:#}").contains(r#"configuration `Some("web")` failed validation"#));
assert!(format!("{error:#}").contains("above the maximum 65535"));
# Ok(())
# })
# .unwrap();
```

#### Richer validator errors — names, notes, and examples

Every validator carries human-facing metadata: a **name**, a **description**, and **example**
values (each with an optional note). Attach it with the `with_name` / `with_description` /
`with_example` / `with_example_noted` builder methods on any validator from `tanzim::validator`.
When validation fails, the alternate error form (`{error:#}`) surfaces all of it, so the reader sees
not just *what* was wrong but *what a valid value looks like*:

```rust
# #[cfg(feature = "validate-integer")]
# {
use tanzim::validator::{self, Integer};
use tanzim::value::{LocatedValue, Location, Value};

// A validator with metadata attached.
let port = Integer::new()
    .min(1)
    .max(65535)
    .with_name("port")
    .with_description("TCP port the server listens on")
    .with_example_noted(Value::Int(8080), "common HTTP dev port")
    .with_example(Value::Int(443));

// Validate a value that is out of range.
let mut value = LocatedValue::new(
    Value::Int(70000),
    Location::at("file", "app.toml", Some(3), Some(11), Some(5)),
);
let error = validator::validate(&port, &mut value).unwrap_err();

// `{error:#}` renders the name, the description, and every example with its note:
//
//   port: 70000 is above the maximum 65535 at file:app.toml:3:11
//     TCP port the server listens on
//     example: 8080 (common HTTP dev port)
//     example: 443
let pretty = format!("{error:#}");
assert!(pretty.contains("port:"));                                  // the name
assert!(pretty.contains("TCP port the server listens on"));         // the description
assert!(pretty.contains("example: 8080 (common HTTP dev port)"));   // an example with a note
assert!(pretty.contains("example: 443"));                           // an example without a note
# }
```

### 3. Inspecting the merged result

Every stage is callable on its own (`load` → `parse` → `merge`), and each merged `Entry` keeps its
provenance — the payloads that contributed to it — alongside the combined value.

```rust
# #[cfg(feature = "parse-toml")]
# tanzim_testing::environment::run(|env| {
env.write_file("etc/web.toml", b"port = 8080\n")?;
env.write_file("etc/db.toml", b"url = \"postgres://localhost\"\n")?;

let pipeline = tanzim::pipeline::default().with_source("file:etc").unwrap();

// Run the stages by hand to inspect intermediate results.
let loaded = pipeline.load().unwrap();        // Vec<Payload>
let parsed = pipeline.parse(&loaded).unwrap(); // Vec<Parsed> — payload paired with its value tree
let merged = pipeline.merge(&parsed).unwrap(); // Merged — a map keyed by entry name

let web = merged.get(&Some("web".into())).unwrap();
assert_eq!(web.payloads().len(), 1); // one source contributed to `web`
assert!(web.value().value().as_map().unwrap().contains_key("port"));

let mut names: Vec<String> = merged.keys().flatten().cloned().collect();
names.sort();
assert_eq!(names, ["db".to_string(), "web".to_string()]);
# Ok(())
# })
# .unwrap();
```

## Things worth knowing

- **Source strings** use the [`tanzim-source`](https://docs.rs/tanzim-source) format
  `SOURCE[(OPTIONS)][:RESOURCE]`, e.g. `env(prefix=APP_)`, `file:/etc/app`, `file:app.toml`.
- **Named entries** — the env `prefix`/`separator` options and a file's name determine the entry
  each value lands in (see [above](#pipeline--multiple-named-entries)). Without a `separator`, all
  env variables merge into the single unnamed entry.
- **`on_error`** — the reserved option `on_error=(<stage>=skip)` (stage: `load`, `parse`, or
  `validate`) makes a source silently skip failures for that stage instead of aborting:

  ```rust
  # #[cfg(feature = "parse-toml")]
  # tanzim_testing::environment::run(|env| {
  env.write_file("app.toml", b"port = 8080\n")?;
  // `missing.toml` is absent, but the source skips the load error instead of failing the run.
  let merged = tanzim::pipeline::default()
      .with_source("file:app.toml").unwrap()
      .with_source("file(on_error=(load=skip)):missing.toml").unwrap()
      .run().unwrap();
  assert!(merged.contains_key(&Some("app".into())));
  # Ok(())
  # })
  # .unwrap();
  ```

- **Located errors** — an `Error` renders one line by default; format it with `{error:#}` for a
  source snippet with a caret under the offending value:

  ```rust
  # #[cfg(feature = "parse-toml")]
  # tanzim_testing::environment::run(|env| {
  use serde::Deserialize;
  env.write_file("app.toml", b"port = \"not-a-number\"\n")?;
  #[derive(Debug, Deserialize)]
  struct App { port: u16 }
  let error = tanzim::Config::default()
      .with_source("file:app.toml").unwrap()
      .try_deserialize::<App>()
      .unwrap_err();

  // `{error}` is one line; `{error:#}` adds the source snippet with a caret under the value.
  assert!(format!("{error:#}").contains("app.toml"));
  //
  // The alternate form renders like this — the caret points at the offending value:
  //
  //   failed to deserialize configuration: invalid type: string "not-a-number", expected u16 at file:app.toml:1:8
  //     1 | port = "not-a-number"
  //       |        ^^^^^^^^^^^^^^
  # Ok(())
  # })
  # .unwrap();
  ```

- **Format auto-detection** — a payload with no format hint is probed against every registered
  parser; a hint (e.g. a file extension) selects the parser directly.
- **Result types** — `parse()` returns `Vec<`[`parser::Parsed`]`>`; `merge()` and `Pipeline::run()`
  return [`merger::Merged`], a map of named [`entry::Entry`] values; `Config::run()` returns one
  unified `Entry`. Fields are private — use the accessors.

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

## Examples

```shell
make examples        # run every example
make example-full    # full pipeline over env + file sources
```

`example-full` reads the source strings declared in the `Makefile` and the sample config under
`examples/full/etc/`.

## Workspace crates

`tanzim` re-exports each stage as a module (`tanzim::loader`, `tanzim::parser`, …) so you rarely
depend on them directly, but every crate is independently usable:

- [`tanzim-source`](https://docs.rs/tanzim-source) — parses the `SOURCE[(OPTIONS)][:RESOURCE]` source-string format into a validated `Source`.
- [`tanzim-load`](https://docs.rs/tanzim-load) — the `Load` trait plus the `env` / `file` / `http` / `closure` loaders; turns a source into raw `Payload`s.
- [`tanzim-parse`](https://docs.rs/tanzim-parse) — the `Parse` trait plus the `env` / `json` / `yaml` / `toml` parsers; turns a payload into a location-aware `LocatedValue`.
- [`tanzim-merge`](https://docs.rs/tanzim-merge) — the `Merge` trait, the `LastWins` / `DeepMerge` strategies, and the `MergePlan` fold tree.
- [`tanzim-validate`](https://docs.rs/tanzim-validate) — the `Validator` trait, the built-in validators, and the schema machinery.
- [`tanzim-value`](https://docs.rs/tanzim-value) — the core `Value` / `LocatedValue` / `Map` / `Location` / `Error` types shared by every stage.
- [`tanzim-testing`](https://docs.rs/tanzim-testing) — the sandboxed `Environment` used by the examples and tests above.

## Development

```shell
make all          # build every crate
make test         # run unit tests + doctests across the workspace
make clippy       # lint with `-D warnings` across all targets and features
make check-style  # cargo fmt --check
make docs         # build rustdoc for the whole workspace
```
