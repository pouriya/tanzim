# tanzim/src

- `lib.rs` — public API: `pub mod single` and `pub mod multi`, each self-contained with its own pipeline types and logic
- `logging.rs` — `is_debug_level_enabled!` and `is_trace_level_enabled!` macros

## single module

- `PipelineSingleBuilder` / `PipelineSingle` — load → parse → merge → unify → validate; returns `(Vec<Payload>, LocatedValue)`
- `build()` errors when no loaders, parsers, or merger are registered
- `with_included_loaders` / `set_included_loaders` and `with_included_parsers` / `set_included_parsers` append feature-gated built-ins
- `unify()` collapses all merge groups into one value using the configured merger

## multi module

- `PipelineMultiBuilder` / `PipelineMulti` — load → parse → merge → validate; returns `Merged` (`HashMap<Option<String>, ...>`)
- Same build validation and included loader/parser helpers as single
- `with_schema(Option<String>, schema)` registers per-entry validation schemas
