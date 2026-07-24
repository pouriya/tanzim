# tanzim

Facade crate: wires load → parse → merge → validate for applications. Stage crates
(`tanzim-load`, `tanzim-parse`, `tanzim-merge`, …) stay usable on their own; this crate is the
composed surface (`Config` / `Pipeline`).

## Key types

- `Config` — single-configuration pipeline. Construct with `Config::builder()` /
  `Config::default()` / `Config::from_plan(plan)`. Stages: load → parse → merge → unify →
  validate. `run()` returns one unified `Entry`; `try_deserialize::<T>()` returns one `T`.
- `Pipeline` — multi-configuration pipeline (same builders/stages, no `unify`). `run()` returns
  `Entries`; `try_deserialize::<T>()` returns `Entries<T>`.
- `Entry` — one merged entry (contributing payloads + combined value).
- `Entries` / `EntryName` / `EntryNameRef` — named-entry map for `Pipeline` (and intermediate
  `Config` merge). Prefer `root()` / `named("db")`; root `Display` is `<root>`. Distinct from
  `tanzim_merge::Merged` (raw merge-stage `HashMap` for custom `Merge` implementors).
- `Schemas` — per-entry validators (`insert_root` / `insert_named`; feature `validate-schema`).
  Register with `with_root_schema` / `with_named_schema` / `with_schemas` on `Pipeline`.
- Errors — `config::Error` / `pipeline::Error` (kept off the crate root).

## Re-exports

| Name | Points to |
|------|-----------|
| `source` | `tanzim_source` |
| `loader` | `tanzim_load` |
| `parser` | `tanzim_parse` |
| `merger` | `tanzim_merge` (+ facade `Entries` / `EntryName`) |
| `validator` | `tanzim_validate` |
| `value` | `tanzim_value` |

## src/ layout

See [`src/AGENTS.md`](src/AGENTS.md).

## Error notes

- Errors are hand-written (no `thiserror`): each enum implements `Display`, `std::error::Error`
  (with `source()` returning the wrapped inner for transparent variants), and any needed `From`.
  Transparent variants forward the formatter with `std::fmt::Display::fmt(inner, f)` so `{error:#}`
  reaches the wrapped error's alternate form (source snippet / caret).
- `NoLoader` and `NoParser` include the source string without options via `source_display()`.

## Testing

No tests in `src/`. Add/move tests to `tests/` (see workspace `AGENTS.md` for naming).
`Entries` / `EntryName` → `tests/merger.rs`; pipeline integration → `tests/pipeline.rs`.
