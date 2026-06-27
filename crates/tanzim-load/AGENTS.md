# tanzim-load

First stage of the pipeline: reads raw configuration bytes from a declared source.

## Key types

- `Load` — trait to implement for a new loader. `load()` returns `Vec<Payload>` (one per config entry found).
- `Payload` — one config entry: `source`, `name`, `format`, `content`. `name: Option<String>` is the entry name (`None` = unnamed); `format: Option<String>` hints the parser which format to use. Call `.normalize()` to lowercase `name` and `format`.
- `Error` — structured load error (NotFound, NoAccess, Timeout, InvalidOption, Duplicate, …).

## Built-in loaders

| Module | Feature | Source string |
|--------|---------|---------------|
| `env` | `env` | `env` |
| `file` | `file` | `file` |
| `http` | `http` | `http` |
| `closure` | — | user-defined |

## src/ layout

- `lib.rs` — `Load` trait, `Payload`, `Error`
- `env.rs` — reads environment variables, groups by separator prefix
- `file.rs` — reads files from a directory or single file path
- `http.rs` — fetches via a user-supplied closure (no HTTP client dependency)
- `closure.rs` — wraps any `Fn` as a `Load` implementor

## Adding a loader

Implement `Load`. Return one `Payload` per config entry. Set `name` to the entry name and `format` to the file extension / format hint so the parser stage can auto-select the right parser.
