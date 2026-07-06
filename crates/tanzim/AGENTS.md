# tanzim

Facade crate that wires the full load → parse → merge pipeline behind two modes: single and multi.

## Key types

### `single` module

- `Single` — the pipeline. Construct with `Single::default()` (feature-enabled loaders + parsers) or `Single::empty()`; there is no `new()`. `with_source(s)` / `add_source(s)` accept a string or `Source` and return `Result` (parse errors surface as `Error::Source`); `with_source_merged(s, merger)` binds a per-source merger. `with_merger` / `add_merger` set the global merger, returning `Result` (defaults to `LastWins` when unset — there is no `NoMerger` error). The pipeline holds a single `merger::plan::MergePlan` (see `pipeline::Plan`): the simple builders append to a root `Merge` node, while `with_merge_plan` / `add_merge_plan` replace it with an explicit tree you build yourself — the two styles are mutually exclusive (`Error::PlanConflict`). Also `with_loader`, `with_parser`, `with_included_loaders` / `set_included_loaders` / `with_included_parsers` / `set_included_parsers`.
- Stages: `load()`, `parse()`, `merge()`, `unify()`, `validate()`, `run()`, and `try_deserialize::<T>()`. `merge()` evaluates the configured `MergePlan` over the parsed payloads; the plan's `Source` leaves are the pipeline's sources (`sources()` walks them). Payloads are attributed back to their configured source by `pipeline::group_by_source` (loaders narrow a source's resource, so this is by containment, not equality).
- `Parsed` — a `(payload, value)` pair (private fields; `payload()` / `value()` accessors).
- `Merged` — grouped result keyed by entry name (`None` = unnamed bucket).
- `Error` — covers source parse errors, load errors, parse errors, merge errors, and missing-loader / missing-parser diagnostics.

### `multi` module

- `Multi` — same shape as `Single` (same source/merger/plan builders and stages); `run()` returns `Merged` (a map of named entries) instead of a single unified value.
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

- Errors are hand-written (no `thiserror`): each enum implements `Display`, `std::error::Error`
  (with `source()` returning the wrapped inner for transparent variants), and any needed `From`.
  Transparent variants forward the formatter with `std::fmt::Display::fmt(inner, f)` so `{error:#}`
  reaches the wrapped error's alternate form (source snippet / caret).
- `NoLoader` and `NoParser` include the source string without options via `source_display()`.
