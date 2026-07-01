# tanzim-load

First stage of the tanzim pipeline: reads raw configuration bytes from a declared source.

## The `Load` trait

Implement [`Load`] to add a new source kind. Return one [`Payload`] per config entry found.
Use `Payload::maybe_name` for the entry name and `Payload::maybe_format` as a hint for the parser stage.

## Built-in loaders

| Module | Feature | Source string |
|--------|---------|---------------|
| `env` | `env` | `env` |
| `file` | `file` | `file` |
| `http` | `http` | `http` |
| `closure` | — | any (user-defined) |

## Example

```rust
use std::env;
use tanzim_load::{env::Env, Error, Load, Source};

fn main() -> Result<(), Error> {
    // SAFETY: example-only; single-threaded doctest env vars.
    unsafe {
        env::set_var("MY_APP_CFG.DEBUG", "true");
        env::set_var("MY_APP_CFG.NAME", "hello");
        env::set_var("MY_APP_CFG.DATABASE.HOST", "localhost");
    }

    let source = Source::parse(r#"env(prefix=MY_APP_,separator=".")"#).unwrap();

    let payloads = Env::new().load(source)?;
    assert_eq!(payloads.len(), 1);

    let payload = payloads[0].clone();
    assert_eq!(payload.maybe_name, Some("cfg".into()));
    assert_eq!(payload.maybe_format, Some("env".into()));

    let content = String::from_utf8_lossy(&payload.content);
    assert!(content.contains("DEBUG=\"true\""));
    assert!(content.contains("NAME=\"hello\""));
    assert!(content.contains("DATABASE.HOST=\"localhost\""));

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
