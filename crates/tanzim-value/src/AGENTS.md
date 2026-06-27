# tanzim-value/src

Two source files — no sub-modules.

- `value.rs` — all value/map/location types and their `Display` impls
- `error.rs` — `Error` enum with the two-pass display (single-line default, multi-line `{:#}`)

Edit `value.rs` to add value accessors or change Map behaviour. Edit `error.rs` to change how parse errors are rendered to users.
