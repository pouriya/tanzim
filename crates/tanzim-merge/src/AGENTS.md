# tanzim-merge/src

Two files: `lib.rs` and `plan.rs` (`pub mod plan`).

`lib.rs` contains the `Merge` trait, both built-in implementations (`LastWins`, `DeepMerge`), the private `deep_merge_value` recursive helper, and the `Error` type. `DeepMerge` carries an `ArrayStrategy` (`Replace` default, plus `Concat`/`Prepend`/`Union`/`Index`/`Keyed`) selecting how two same-key lists combine; build it with `DeepMerge::new().with_array_strategy(..)`. In `deep_merge_value`, an overlay `Value::Null` at a map key removes the key (unset); it does not store `Null`.

When adding a new merger, implement `Merge` and add it to `lib.rs`. The merge output type is the `Merged` alias (`HashMap<Option<String>, (Vec<Payload>, LocatedValue)>`) — the `Vec` tracks which payloads contributed to each merged value. Return `Merged`, not the spelled-out type, so signatures stay readable and clippy's `type_complexity` lint stays quiet without `#[allow]`.

`plan.rs` is the composable merge tree: `MergePlan` is a `Source` leaf, a `Value(Box<ValueLeaf>)` leaf (skips load/parse — pre-built values; boxed to keep the enum small), or a `Merge { merger: Box<dyn Merge>, children }` (the merger is owned by `Box`, not shared — callers hold one tree and mutate it in place). Build with `src` / `value` / `named_value` / `deep` / `last_wins` / `merge_with`; `src` returns `Result` (wraps a source `ParseError` in `Error::Other`). `evaluate(plan, groups)` folds post-order against `SourceGroup`s — `(configured Source, its attributed (Payload, LocatedValue) pairs)` — so source leaves resolve by configured source, value leaves yield their stored pair, and inner nodes flatten each child `Merged` back to one carrier pair per name-group before the parent merges. Keep the module doctest at the top of `plan.rs` compiling.

No tests here — they belong in `tests/`, not `src/`.
