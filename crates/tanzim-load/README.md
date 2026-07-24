# tanzim-load
[**Package**](https://crates.io/crates/tanzim-load)   |   [**Documentation**](https://docs.rs/tanzim-load)   |   [**Repository**](https://github.com/pouriya/tanzim/tree/master/crates/tanzim-load)

First stage of the tanzim pipeline: reads raw configuration bytes from a declared source.

## The `Load` trait

Implement [`Load`] to add a new source kind (protocol, service, database, …). It only *fetches
bytes* — parsing happens in a later stage. The contract:

- Return one [`Payload`] per config entry found; a single source may expand to many entries.
  Finding nothing is `Ok(vec![])`, not an error.
- Set `Payload::source` on each entry to the concrete resource loaded (narrow the incoming source
  with `Source::with_resource`) so diagnostics stay precise.
- `Payload::maybe_name` is the entry name (`None` merges into the root); `Payload::maybe_format`
  is a parser hint (e.g. `json`).
- Read options off the source with `Source::options()` and the typed `OptionValue` accessors.
  Only validate options your loader reads; ignore unknown keys. Pick the [`Error`] variant that
  matches the failure (`InvalidResource`, `InvalidOption`, `NotFound`, `NoAccess`, `Timeout`,
  `Duplicate`, `Load`).

Register a loader with the load stage that walks sources and calls [`Load::load`]; it's dispatched
by the source strings its `supported_source_list()` returns. For a quick, stateless adapter, use
`closure::Closure` instead of a full `impl Load`. See the [`Load`] rustdoc for worked details.

## Built-in loaders

| Module | Feature | Source string |
|--------|---------|---------------|
| `env` | `env` | `env` |
| `file` | `file` | `file` |
| `http` | `http-closure` | `http` |
| `closure` | — | any (user-defined) |

## Example

```rust
use tanzim_load::{env::Env, Load, Source};

# tanzim_testing::environment::run(|env| {
#     env.set_env("MY_APP_CFG.DEBUG", "true")?;
#     env.set_env("MY_APP_CFG.NAME", "hello")?;
#     env.set_env("MY_APP_CFG.DATABASE.HOST", "localhost")?;
// The process environment holds three variables:
//   MY_APP_CFG.DEBUG=true
//   MY_APP_CFG.NAME=hello
//   MY_APP_CFG.DATABASE.HOST=localhost
// The `MY_APP_` prefix marks them as ours and `.` groups each one under the `cfg` entry.
let source = Source::parse(r#"env(prefix=MY_APP_,separator=".")"#).unwrap();

let payloads = Env::new().load(source).unwrap();
assert_eq!(payloads.len(), 1);

let payload = payloads[0].clone();
assert_eq!(payload.maybe_name, Some("cfg".into()));
assert_eq!(payload.maybe_format, Some("env".into()));

let content = String::from_utf8_lossy(&payload.content);
assert!(content.contains("DEBUG=\"true\""));
assert!(content.contains("NAME=\"hello\""));
assert!(content.contains("DATABASE.HOST=\"localhost\""));
# Ok(())
# })
# .unwrap();
```

## Features

| Feature | Enables |
|---------|---------|
| `env` | `env` loader (reads environment variables) |
| `file` | `file` loader (reads from filesystem) |
| `http-closure` | `http` loader (user-provided fetch closure) |
| `logging` | emit log messages via the `log` crate |
| `tracing` | emit trace spans via the `tracing` crate |
| `full` | `env` + `file` + `http-closure` |

Default features: `env`, `file`, `http-closure`. Logging/tracing are opt-in.

## Relations

- Depends on [`tanzim-source`](https://crates.io/crates/tanzim-source) for `Source` and `Options`.
- Produces `Payload` values consumed by [`tanzim-parse`](https://crates.io/crates/tanzim-parse).
- Full pipeline wired in [`tanzim`](https://crates.io/crates/tanzim).
