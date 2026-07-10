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

Describe your sources as strings and deserialize the merged configuration straight into your own type:

```rust,no_run
use tanzim::merger::DeepMerge;

// Your own configuration type — deserialized directly from the merged tree.
#[derive(serde::Deserialize)]
struct Config {
    listen: Listen,
    remote: String,
    log_level: Log,
    output: String,
}

#[derive(serde::Deserialize)]
struct Listen {
    ip: std::net::IpAddr,
    port: u16,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "lowercase")]
enum Level { Trace, Debug, Info, Warn, Error }

// `Config` collapses every source into ONE unified configuration. Reach for
// `tanzim::pipeline::Pipeline` when your sources describe SEVERAL named configurations.
let config: Config = tanzim::Config::default() // env · file loaders + env · yaml · toml · json parsers
    .with_merger(DeepMerge::new())?      // optional — defaults to `LastWins` when unset
    .with_source("env(prefix=APP_)")?
    .with_source("/etc/app.toml")?
    .try_deserialize()?;
// Any failure yields an ergonomic, located error. Formatted with `{error:#}` it names what was
// expected and points a caret at the offending value — e.g. if `listen.port` held a string:
//
//   failed to deserialize configuration: invalid type: string "eighty", expected u16 at file:/etc/app.toml:2:8
//     1 | [listen]
//     2 | port = "eighty"
//       |        ^^^^^^^^
```

### Advanced: explicit merge plan + schema

Reach for an explicit `MergePlan` when a single flat merge isn't enough, and a schema when you
want the shape checked before it ever reaches your struct. `Either` accepts a value if either of
two validators does — here, `logging.output` must be `stdout`/`stderr` *or* an absolute file path.
Every validator carries human-facing `Meta`: a `name`, a `description`, and noted `examples`, all
of which surface in a validation failure:

```rust,no_run
use tanzim::{
    merger::plan::{deep, last_wins, src},
    validator::{Either, Enum, Path, StaticMap, Value, validate},
};

// last_wins(deep(base, overrides), env): deep-merge the two files, then let the env
// source win any remaining conflicts — instead of one flat merge across all three.
let plan = last_wins(vec![
    deep(vec![src("file:base.toml")?, src("file:overrides.toml")?]),
    src("env(prefix=APP_)")?,
]);

let output = Either::new(
    Enum::new([Value::String("stdout".into()), Value::String("stderr".into())]),
    Path::new().absolute(),
)
.with_name("Log output")
.with_description("Where log lines are written: `stdout`, `stderr`, or an absolute file path")
.with_example_noted(Value::String("stdout".into()), "write to standard output")
.with_example_noted(
    Value::String("/var/log/app.log".into()),
    "write to an absolute file path",
);
let schema = StaticMap::new().required("logging", StaticMap::new().required("output", output));

let mut entry = tanzim::Config::default().with_merge_plan(plan)?.run()?;
validate(&schema, entry.value_mut())?;
// A validation failure carries the offending value's location; `{error:#}` renders a
// caret-underlined snippet plus the name/description/examples attached above — e.g. if
// `logging.output` held the relative path `"app.log"`:
//
//   Log output: logging.output: no alternative matched: (`"app.log"` is not an allowed
//   value) or (invalid absolute path) at file:overrides.toml:2:10
//     Where log lines are written: `stdout`, `stderr`, or an absolute file path
//     example: "stdout" (write to standard output)
//     example: "/var/log/app.log" (write to an absolute file path)
//     1 | [logging]
//     2 | output = "app.log"
//       |          ^^^^^^^^^
```

Full walkthrough, features, and per-stage recipes → [crates.io](https://crates.io/crates/tanzim) · [docs.rs](https://docs.rs/tanzim).

## License

Licensed under [MIT](LICENSE).
