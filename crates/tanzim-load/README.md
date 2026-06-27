# tanzim-load

First stage of the tanzim pipeline: reads raw configuration bytes from a declared source.

## The `Load` trait

Implement [`Load`] to add a new source kind. Return one [`Payload`] per config entry found.
Use `Payload::name` for the entry name and `Payload::format` as a hint for the parser stage.

## Built-in loaders

| Module | Feature | Source string |
|--------|---------|---------------|
| `env` | `env` | `env` |
| `file` | `file` | `file` |
| `http` | `http` | `http` |
| `closure` | — | any (user-defined) |

## Example

```rust,no_run
use tanzim_load::{file::File, Load, Source};
use tanzim_source::SourceBuilder;

fn main() -> Result<(), tanzim_load::Error> {
    let loader = File::new();
    let source = SourceBuilder::new()
        .with_source("file")
        .with_resource("/etc/myapp")
        .build()
        .unwrap();
    for payload in loader.load(source)? {
        let payload = payload.normalize();
        println!(
            "resource={:?} name={:?} format={:?} bytes={}",
            payload.source.resource(),
            payload.name,
            payload.format,
            payload.content.len()
        );
    }
    Ok(())
}
```

## Features

| Feature | Enables |
|---------|---------|
| `env` | `env` loader (reads environment variables) |
| `file` | `file` loader (reads from filesystem) |
| `http` | `http` loader (user-provided fetch closure) |
| `logging` | emit log messages via the `log` crate |
| `tracing` | emit trace spans via the `tracing` crate |
| `full` | `env` + `file` + `http` |

Default features: `logging`, `env`.

## Relations

- Depends on [`tanzim-source`](../tanzim-source/) for `Source` and `Options`.
- Produces `Payload` values consumed by [`tanzim-parse`](../tanzim-parse/).
- Full pipeline wired in [`tanzim`](../tanzim/).
