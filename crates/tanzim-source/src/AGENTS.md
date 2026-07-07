# tanzim-source/src

- `lib.rs` — public API: `Source`, `SourceBuilder`, `Options`, `OptionValue`, `Error`
- `parse.rs` — recursive descent parser for the `SOURCE[(OPTIONS)][:RESOURCE]` grammar and `Display` impl
- `impls.rs` — `From`/`TryFrom` blanket conversions
- `serde.rs` — serde glue (feature-gated)

The grammar lives only in `parse.rs`. Display (canonical form) is also in `parse.rs` and must round-trip.

No tests here — they belong in `tests/`, not `src/`.
