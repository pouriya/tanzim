# tanzim

Facade crate that wires the full load → parse → merge pipeline behind two modes: single and multi.

## Key types

### `single` module

- `PipelineSingleBuilder` — fluent builder. `with_source(s)` parses the source string (returns `Result`); `with_loader`, `with_parser`, `with_merger` register pipeline components. `build()` returns `Result<PipelineSingle, Error>` and errors when loaders, parsers, or merger are missing.
- `PipelineSingle` — owns the pipeline components. Exposes `load()`, `parse()`, `merge()`, `unify()`, `validate()`, and `run()`. Also provides `with_*` setters and `with_included_loaders` / `set_included_loaders` / `with_included_parsers` / `set_included_parsers`.
- `Parsed` — type alias `(loader::Payload, parser::LocatedValue)`.
- `Merged` — type alias for `merge::Merged` (`HashMap<Option<String>, (Vec<Payload>, LocatedValue)>`).
- `Error` — covers source parse errors, load errors, parse errors, merge errors, missing components, and missing-loader / missing-parser diagnostics.

### `multi` module

- `PipelineMultiBuilder` / `PipelineMulti` — same builder pattern as single; `run()` returns `Merged` instead of a unified value.
- `Schemas` — `HashMap<Option<String>, validate::Value>` (feature `validate-schema`).
- `with_schema(Option<String>, schema)` and `with_schemas(Schemas)` register validation schemas per merged entry name.

## Re-exports

| Name | Points to |
|------|-----------|
| `source` | `tanzim_source` |
| `loader` | `tanzim_load` |
| `parser` | `tanzim_parse` |
| `merge` | `tanzim_merge` |
| `validate` | `tanzim_validate` |

## src/ layout

- `lib.rs` — `pub mod single`, `pub mod multi`, re-exports, `logging` module include
- `logging.rs` — `is_debug_level_enabled!` / `is_trace_level_enabled!` macros for conditional logging

## Error notes

- Field names in `Error` variants must not be `source` (thiserror reserves that for the error chain).
- `NoLoader` and `NoParser` include the source string without options via `source_display()`.
