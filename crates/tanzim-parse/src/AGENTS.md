# tanzim-parse/src

- `lib.rs` — `Parse` trait definition and re-exports from `tanzim-value`
- `span.rs` — shared span-to-Location conversion helpers used by format modules
- `env.rs` — env format parser (feature `env`): `KEY=VALUE` lines
- `json.rs` — JSON parser with span info (feature `json`)
- `yaml.rs` — YAML parser with line numbers (feature `yaml`)
- `toml.rs` — TOML parser with span info (feature `toml`)
- `closure.rs` — adapter that wraps a `Fn` as a `Parse` implementor

Each format module must produce a `LocatedValue` where the `location` on every node points back to the source file and line. See `span.rs` for helpers.

No tests here — they belong in `tests/`, not `src/`.
