# tanzim-parse

Second stage of the pipeline: deserializes raw bytes into typed, source-located value trees.

## Key types

- `Deserialize` — trait to implement for a new format parser. `parse()` returns a `LocatedValue` tree where every node carries its source file and line/column.
- `Error` (re-exported from `tanzim-value`) — parse error with optional snippet + caret via `{error:#}`.
- `LocatedValue`, `Value` — re-exported from `tanzim-value`.

## Built-in parsers

| Type | Feature | Formats |
|------|---------|---------|
| `Env` | `env` | `env` |
| `Json` | `json` | `json` |
| `Yaml` | `yaml` | `yml`, `yaml` |
| `Toml` | `toml` | `toml` |
| `closure::Closure` | — | user-defined |

## src/ layout

- `lib.rs` — `Deserialize` trait, re-exports
- `span.rs` — internal helpers for mapping format-specific spans to `Location`
- `env.rs`, `json.rs`, `yaml.rs`, `toml.rs` — format implementations (feature-gated)
- `closure.rs` — wraps any closure as a `Deserialize` implementor

## Adding a parser

Implement `Deserialize`. Return `LocatedValue` with locations attached to every node. Use `is_format_supported` to enable auto-detection when `Payload::format` is `None`.
