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
use tanzim::single::PipelineSingleBuilder;
use tanzim::merge::DeepMerge;

// Your own configuration type — deserialized directly from the merged tree.
#[derive(serde::Deserialize)]
struct Config {
    listen: Listen,
    remote: String,               // a URL
    log: Log,
    output: String,               // "stdout" / "stderr" / a file path
}
#[derive(serde::Deserialize)]
struct Listen {
    ip: std::net::IpAddr,
    port: u16,
}
#[derive(serde::Deserialize)]
struct Log {
    level: Level,
}
#[derive(serde::Deserialize)]
#[serde(rename_all = "lowercase")]
enum Level { Trace, Debug, Info, Warn, Error }

// `single` collapses every source into ONE unified configuration. Prefer
// `tanzim::multi::PipelineMultiBuilder` when your sources describe SEVERAL named configurations:
// it keeps them separate, and `try_deserialize::<T>()` returns a map keyed by entry name.
let config: Config = PipelineSingleBuilder::new()
    .with_included_loaders()             // env · file · http
    .with_included_parsers()             // env · yaml · toml · json
    .with_merger(DeepMerge)
    .with_source("env(prefix=APP_)")?
    .with_source("/etc/app.toml")?
    .with_source("https://cfg.tld/path/to/app.yaml")?
    .build()?
    .try_deserialize()?;
// A type mismatch yields an ergonomic, located error. Formatted with `{error:#}` it names what was
// expected and points a caret at the offending value — e.g. if `listen.port` held a string:
//
//   failed to deserialize configuration: invalid type: string "eighty", expected u16 at file:/etc/app.toml:2:8
//     1 | [listen]
//     2 | port = "eighty"
//       |        ^^^^^^^^
```

Full walkthrough, features, and per-stage recipes → [crates.io](https://crates.io/crates/tanzim) · [docs.rs](https://docs.rs/tanzim).

## License

Licensed under [BSD-3-Clause](LICENSE).
