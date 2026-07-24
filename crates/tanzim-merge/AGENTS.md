# tanzim-merge

Third stage of the pipeline: groups parsed payloads by entry name and combines their values.

## Key types

- `Merge` — trait to implement for a custom merge strategy. Takes a flat list of `(Payload, LocatedValue)` and returns `Merged` grouped by entry name.
- `Merged` — public type alias for `HashMap<Option<String>, (Vec<Payload>, LocatedValue)>`. Use it instead of writing the raw type out (keeps signatures clear and avoids `clippy::type_complexity` without `#[allow]`).
- `LastWins` — built-in: for each name, the last-seen value replaces any previous.
- `DeepMerge` — built-in: maps with the same name are merged recursively. Same-name lists follow a configurable `ArrayStrategy` (default `Replace`; also `Concat`/`Prepend`/`Union`/`Index`/`Keyed`), set via `DeepMerge::new().with_array_strategy(..)`. The overlay value/location wins at every other leaf.
- `ArrayStrategy` — enum selecting how two same-key lists combine in `DeepMerge`.
- `Error` — merge error, wraps `Box<dyn Error + Send + Sync>`.
- `plan::MergePlan` — a composable merge tree (`plan` module): a `Source` leaf, a `Value` leaf (pre-built `LocatedValue` that skips load/parse), or a `Merge { merger: Box<dyn Merge>, children }`. Build with `plan::src` / `value` / `named_value` / `deep` / `last_wins` / `merge_with`; fold with `plan::evaluate` against `SourceGroup`s (configured source → its attributed `(Payload, LocatedValue)` pairs; value leaves ignore groups).

## Grouping key

`Payload::maybe_name` (`Option<String>`) maps directly to the `Merged` key:
- `Some("foo")` → key `Some("foo")`
- `None` → key `None` (all unnamed payloads share this bucket)

## src/ layout

- `lib.rs` — the `Merge` trait, `LastWins`, `DeepMerge` + `ArrayStrategy`, `deep_merge_value`/`merge_lists` helpers, `Error`
- `plan.rs` — the merge tree: `MergePlan`, the `src`/`value`/`named_value`/`deep`/`last_wins`/`merge_with` constructors, `SourceGroup`, and `evaluate`

Add new merge strategies in `lib.rs`; tree/composition logic lives in `plan.rs`.

## Testing

No tests in `src/`. Add/move tests to `tests/` (see workspace `AGENTS.md` for naming).
