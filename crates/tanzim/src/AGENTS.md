# tanzim/src

Facade crate. The crate root exposes only two types — `Config` and `Source` — everything else is a
per-concern module.

- `lib.rs` — module declarations; `pub use config::Config;` and `pub use source::Source;`
- `logging.rs` — `is_debug_level_enabled!` / `is_trace_level_enabled!` macros

## Stage modules (glob re-export of the backing crate, plus facade-owned types)

- `loader.rs` — `pub use tanzim_load::*;`
- `parser.rs` — `pub use tanzim_parse::*;` + `Parsed` (a `Payload` paired with its parsed `LocatedValue`)
- `merger.rs` — `pub use tanzim_merge::*;` + `Merged` (facade output map, shadows the raw
  `tanzim_merge::Merged` alias — reach the raw one as `tanzim_merge::Merged`), plus the pipeline's
  internal `pub(crate) enum Plan` and `pub(crate) fn group_by_source`
- `validator.rs` — `pub use tanzim_validate::*;`
- `value.rs` — `pub use tanzim_value::*;`
- `source.rs` — `pub use tanzim_source::*;`
- `entry.rs` — `Entry`: one merged entry (contributing payloads + combined value)

## `config.rs` — the single-configuration pipeline

- `Config` — load → parse → merge → unify → validate; `run()` returns one unified `Entry`,
  `try_deserialize::<T>()` returns one `T`. Error type is `config::Error` (kept off the crate root).
- Construct with `Config::default()` (feature-enabled loaders + parsers) or `Config::empty()`; there is no `new()`.
- Sources + merger are stored as one `merger::Plan` (a `merger::plan::MergePlan` tree); the simple
  builders append to it, `with_merge_plan` replaces it, mixing the two is `Error::PlanConflict`.
- `with_included_loaders` / `set_included_loaders` and `with_included_parsers` / `set_included_parsers` append feature-gated built-ins.
- `unify()` collapses all merge groups into one value using the configured merger (defaulting to `LastWins`).

## `pipeline.rs` — the multi-configuration pipeline

- `Pipeline` — load → parse → merge → validate; `run()` returns `Merged`
  (`HashMap<Option<String>, Entry>`), `try_deserialize::<T>()` returns a name-keyed map.
- Construct with the free functions `pipeline::default()` / `pipeline::empty()` (both delegate to the
  kept `Pipeline::default()` / `Pipeline::empty()`). Error type is `pipeline::Error`.
- `with_schema(Option<String>, schema)` registers per-entry validation schemas (`Schemas` map).
- Same build validation and included loader/parser helpers as `Config`.

## Testing

No tests here — they belong in `tests/`, not `src/`.
