# tanzim-merge

Third stage of the pipeline: groups parsed payloads by entry name and combines their values.

## Key types

- `Merge` — trait to implement for a custom merge strategy. Takes a flat list of `(Payload, LocatedValue)` and returns `Merged` grouped by entry name.
- `Merged` — public type alias for `HashMap<String, (Vec<Payload>, LocatedValue)>`. Use it instead of writing the raw type out (keeps signatures clear and avoids `clippy::type_complexity` without `#[allow]`).
- `LastWins` — built-in: for each name, the last-seen value replaces any previous.
- `DeepMerge` — built-in: maps with the same name are merged recursively; the later value wins at each leaf. Location is preserved from the overlay value.
- `Error` — merge error, wraps `Box<dyn Error + Send + Sync>`.

## Grouping key

`Payload::maybe_name` (`Option<String>`) maps to a `String` key:
- `Some("foo")` → key `"foo"`
- `None` → key `""` (all unnamed payloads share this bucket)

## src/ layout

- `lib.rs` — everything: `Merge` trait, `LastWins`, `DeepMerge`, `deep_merge_value` helper, `Error`

There is intentionally only one source file and no examples directory. Add new merge strategies here.
