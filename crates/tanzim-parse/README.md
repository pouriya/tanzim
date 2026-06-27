# tanzim-parse

Second stage of the tanzim pipeline: deserializes raw bytes into typed, source-located value trees.

## The `Deserialize` trait

Implement [`Deserialize`] to add a new configuration format. Every node in the returned
[`LocatedValue`] tree should carry a `Location` pointing to the source file and line for
accurate error messages.

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
use tanzim_parse::{Deserialize, Json};

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
