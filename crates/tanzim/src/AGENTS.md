# tanzim/src

Facade crate. The crate root exposes only two types — `Config` and `Source` — everything else is a
per-concern module.

- `lib.rs` — module declarations; `pub use config::{Config, ConfigBuilder, ConfigStages, Sources, Plan, BuilderState};` and `pub use source::Source;`
- `logging.rs` — `is_debug_level_enabled!` / `is_trace_level_enabled!` macros

## Stage modules (glob re-export of the backing crate, plus facade-owned types)

- `loader.rs` — `pub use tanzim_load::*;`
- `parser.rs` — `pub use tanzim_parse::*;` + `Parsed` (a `Payload` paired with its parsed `LocatedValue`)
- `merger.rs` — `pub use tanzim_merge::*;` (includes the raw `Merged` alias for merge implementors)
  + `Entries` / `EntryName` / `EntryNameRef` (consumer map; `root()` / `named()` hide
  `Option<String>` keys), plus the pipeline's internal `pub(crate)` merge-tree helpers
  (`leaves`, `root_merger`, `push_child`, `set_root_merger`) and `group_by_source`
- `validator.rs` — `pub use tanzim_validate::*;`
- `value.rs` — `pub use tanzim_value::*;`
- `source.rs` — `pub use tanzim_source::*;`
- `entry.rs` — `Entry`: one merged entry (contributing payloads + combined value)

## `config.rs` — the single-configuration pipeline

- Typestate builder: `ConfigBuilder<State>` where `State` is `Sources` or `Plan` (sealed
  `BuilderState`). `Config::builder()` → `ConfigBuilder<Sources>` (simple-fold: exposes
  `with_defaults` / `with_value` / `with_source` / `add_source` / `with_source_merged` /
  `with_merger`); `Config::from_plan(plan)` → `ConfigBuilder<Plan>` (carries a
  `merger::plan::MergePlan` tree, no source builders). Mixing modes is a **compile error** — there
  is no runtime `PlanConflict`. Builders are infallible (bad source strings / serialize failures are
  deferred to `run`). There is no `new()`.
- `build()` produces the runnable `Config`; the builder's `run()` / `try_deserialize()` are sugar for
  `build().run()` / `build().try_deserialize()`. `Config` — load → parse → merge → unify → validate;
  `run()` returns one unified `Entry`, `try_deserialize::<T>()` returns one `T`. Error type is
  `config::Error` (kept off the crate root).
- Stage methods live behind `Config::stages() -> ConfigStages<'_>` (`load`/`parse`/`merge`/`unify`/`validate`).
- `with_default_loaders` / `set_default_loaders` and `with_default_parsers` / `set_default_parsers`
  append/replace the feature-gated built-ins.
- Sources + merger are stored as one `merger::plan::MergePlan` tree with a `merger_set` flag; the
  simple builders append/replace via the `merger::{push_child, set_root_merger}` helpers.
- `unify()` collapses all merge groups into one value using the configured merger (defaulting to `LastWins`).
- Stored trait objects are `Box<dyn Load/Parse/Merge + Send + Sync>`, so `Config`/`ConfigBuilder` are
  `Send + Sync`.

## `pipeline.rs` — the multi-configuration pipeline

- `PipelineBuilder<State>` mirrors `ConfigBuilder<State>` (shares the `Sources`/`Plan` markers from
  `config`). Construct with `Pipeline::builder()` / `Pipeline::from_plan(plan)`. `Pipeline` — load →
  parse → merge → validate (no `unify`); `run()` returns `Entries` (`Entries<Entry>`),
  `try_deserialize::<T>()` returns `Entries<T>`. Stages behind `Pipeline::stages() -> PipelineStages<'_>`.
  Error type is `pipeline::Error`.
- `with_root_schema` / `with_named_schema` / `with_schemas(Schemas)` register per-entry validation
  schemas (`Schemas` is a struct with the same root/named insert helpers).
- Same default loader/parser helpers as `Config`.

## Testing

No tests here — they belong in `tests/`, not `src/`.
