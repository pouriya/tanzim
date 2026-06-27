# tanzim/src

- `lib.rs` — entire public API: `Error`, `ConfigBuilder`, `Config`, pipeline methods, re-exports
- `logging.rs` — `is_debug_level_enabled!` and `is_trace_level_enabled!` macros

All pipeline logic (load, parse, merge) is in `lib.rs`. The file is intentionally self-contained so the full pipeline is easy to follow in one place.
