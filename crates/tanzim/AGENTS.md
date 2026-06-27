# tanzim

Facade crate that wires the full load → parse → merge pipeline behind a single `Config` / `ConfigBuilder` API.

## Key types

- `ConfigBuilder` — fluent builder. `with_source(s)` parses the source string (returns `Result`); `with_loader`, `with_parser`, `with_merger` register pipeline components. `build()` produces a `Config`.
- `Config` — owns the pipeline components. Exposes `load()`, `parse()`, `merge()`, and `run()` (all three). Also provides `with_*` setters that return `Self` for post-build reconfiguration.
- `Parsed` — type alias `(loader::Payload, parser::LocatedValue)`; one parsed payload. `parse()` returns `Vec<Parsed>`.
- `Merged` — type alias for `merge::Merged` (`HashMap<String, (Vec<Payload>, LocatedValue)>`). `merge()` and `run()` return it. Use these aliases instead of the spelled-out types so signatures stay readable and clippy's `type_complexity` lint stays quiet without `#[allow]`.
- `Error` — covers source parse errors, load errors, parse errors, merge errors, and missing-loader / missing-parser diagnostics.

## Re-exports

| Name | Points to |
|------|-----------|
| `source` | `tanzim_source` |
| `loader` | `tanzim_load` |
| `parser` | `tanzim_parse` |
| `merge` | `tanzim_merge` |

## src/ layout

- `lib.rs` — `Error`, `ConfigBuilder`, `Config`, `source_display` helper, `logging` module include
- `logging.rs` — `is_debug_level_enabled!` / `is_trace_level_enabled!` macros for conditional logging

## Error notes

- Field names in `Error` variants must not be `source` (thiserror reserves that for the error chain).
- `NoLoader` and `NoParser` include the source string without options via `source_display()`.
