# tanzim-parse
[**Package**](https://crates.io/crates/tanzim-parse)   |   [**Documentation**](https://docs.rs/tanzim-parse)   |   [**Repository**](https://github.com/pouriya/tanzim/tree/master/crates/tanzim-parse)

Second stage of the tanzim pipeline: parses raw bytes into typed, source-located value trees.

## The `Parse` trait

Implement [`Parse`] to add a new configuration format. It turns the raw bytes a loader
produced into a typed, source-located value tree. The contract:

- `parse` returns one [`LocatedValue`] tree per payload, given the `source` kind and `resource`
  identifier the bytes came from.
- Every node — including the root — should carry a `Location` (source, resource, line/column) so
  downstream error messages can point at the exact value. Build them with `Location::at`.
- A parser is selected by the payload's format hint against `supported_format_list` (which may
  list several extensions, e.g. `yml`/`yaml`); with no hint, the stage probes each parser via
  `is_format_supported` (`Some(true)`/`Some(false)`/`None` to abstain).
- Report failures with the matching [`Error`] variant (`InvalidUtf8`, `Parse`, `UnsupportedNull`,
  `UnsupportedType`).

Register a parser with `tanzim::Config::with_parser`. For a quick, stateless adapter, use
`closure::Closure` instead of a full `impl Parse`. See the [`Parse`] rustdoc and the
example below for worked details.

## Built-in parsers

| Type | Feature | Formats |
|------|---------|---------|
| `Env` | `env` | `env` |
| `Json` | `json` | `json` |
| `Yaml` | `yaml` | `yml`, `yaml` |
| `Toml` | `toml` | `toml` |
| `closure::Closure` | — | custom |

## Example

```rust,ignore
use tanzim_parse::{Parse, json::Json};
use tanzim_source::SourceBuilder;

fn main() -> Result<(), tanzim_value::Error> {
    let source = SourceBuilder::new()
        .with_source("file")
        .with_resource("config.json")
        .build()
        .unwrap();
    let value = Json::new().parse(&source, br#"{"port": 8080}"#)?;
    let map = value.value.as_map().unwrap();
    let port = map.get("port").unwrap();
    println!("port={port}  location={}", port.location);
    Ok(())
}
```

## Features

| Feature | Enables |
|---------|---------|
| `env` | `env` format parser (KEY=VALUE lines) |
| `json` | JSON parser with source spans |
| `yaml` | YAML parser with line numbers |
| `toml` | TOML parser with source spans |
| `logging` | emit log messages via the `log` crate |
| `tracing` | emit trace spans via the `tracing` crate |
| `full` | `env` + `json` + `yaml` + `toml` |

Default features: `env`. Logging/tracing are opt-in.

## Relations

- Consumes `Payload` from [`tanzim-load`](https://docs.rs/tanzim-load/latest/tanzim_load/).
- Depends on [`tanzim-value`](https://docs.rs/tanzim-value/latest/tanzim_value/) for `LocatedValue`, `Value`, `Map`, `Error`.
- Produces `LocatedValue` trees consumed by [`tanzim-merge`](https://docs.rs/tanzim-merge/latest/tanzim_merge/).
- Full pipeline wired in [`tanzim`](https://docs.rs/tanzim/latest/tanzim/).
