# tanzim-parse

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

```rust,no_run
use tanzim_parse::{Parse, json::Json};

fn main() -> Result<(), tanzim_value::Error> {
    let value = Json::new().parse("file", "config.json", br#"{"port": 8080}"#)?;
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

Default features: `logging`, `env`.

## Relations

- Consumes `Payload` from [`tanzim-load`](../tanzim-load/).
- Depends on [`tanzim-value`](../tanzim-value/) for `LocatedValue`, `Value`, `Map`, `Error`.
- Produces `LocatedValue` trees consumed by [`tanzim-merge`](../tanzim-merge/).
- Full pipeline wired in [`tanzim`](../tanzim/).
