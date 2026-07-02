# tanzim-merge/src

Single file: `lib.rs`.

Contains the `Merge` trait, both built-in implementations (`LastWins`, `DeepMerge`), the private `deep_merge_value` recursive helper, and the `Error` type.

When adding a new merger, implement `Merge` and add it to `lib.rs`. The merge output type is the `Merged` alias (`HashMap<Option<String>, (Vec<Payload>, LocatedValue)>`) — the `Vec` tracks which payloads contributed to each merged value. Return `Merged`, not the spelled-out type, so signatures stay readable and clippy's `type_complexity` lint stays quiet without `#[allow]`.
