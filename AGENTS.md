# tanzim workspace

Configuration pipeline: **load → parse → merge → validate**.

## Crates

| Crate | Purpose |
|-------|---------|
| `tanzim-value` | Core value types (`Value`, `LocatedValue`, `Map`, `Location`, `Error`) |
| `tanzim-source` | Source string parsing (`Source`, `SourceBuilder`, `Options`) |
| `tanzim-load` | Loading raw bytes from sources (`Load` trait, `Payload`) |
| `tanzim-parse` | Deserializing bytes into `LocatedValue` trees (`Deserialize` trait) |
| `tanzim-merge` | Merging parsed values by entry name (`Merge`, `LastWins`, `DeepMerge`, `Merged`) |
| `tanzim-validate` | Validating/coercing values (`Validator` trait, concrete validators, `schema` feature for building validators from data) |
| `tanzim` | Facade: `ConfigBuilder` / `Config` that wires the full pipeline |

## Pipeline

```
Source strings
  → Load::load(source)         → Vec<Payload>
  → Deserialize::parse(bytes)  → LocatedValue
  → Merge::merge(parsed_list)  → HashMap<name, (Vec<Payload>, LocatedValue)>   (the `Merged` alias)
```

`Config::run()` executes all three stages. Each stage can also be called individually via `Config::load()`, `Config::parse()`, and `Config::merge()`.

## Key conventions

- `Payload::name` is `Option<String>`: `None` means unnamed; all unnamed payloads share the `""` key in the merger.
- `Payload::format` is `Option<String>`: `None` means format is auto-detected by parsers via `is_format_supported`.
- Sources with `ignore_errors = true` swallow load and parse failures silently.

## Code style conventions

These apply to all crates, not just the pipeline:

- **Plain `for` loops over iterator method chains.** Prefer a `for` loop to `.map`/`.filter`/`.fold`/`.collect` chains. (When you do index a slice, still use `for x in &xs` / `.iter().enumerate()` to satisfy `needless_range_loop`.)
- **`match` over combinators for `Result`.** Use an explicit `match` instead of `map_err`, `and_then`, `or_else`, etc.
- **`if let Some(...)` over combinators for `Option`.** Use `if let` / `match` instead of `map`, `and_then`, `unwrap_or_else`, etc.
- **Don't extract single-use helpers.** If a function is called from exactly one place, inline it.

## Lint & style conventions

- **No `#[allow(...)]` anywhere.** Fix the root cause instead of suppressing a lint.
- **`make clippy` is the gate** — it runs `cargo clippy --workspace --all-features --all-targets --no-deps -- -D warnings`, so warnings (including in tests and examples) fail the build.
- **`clippy::type_complexity`** — extract a named `pub type` alias (e.g. `Merged`, `Parsed`, `LoaderFn`) rather than spelling out nested generic types in signatures.
- **`clippy::result_large_err`** — keep error enums small enough to return by value without `Box`. Shrink fields (e.g. `Option<NonZeroU32>` instead of `Option<usize>`) instead of boxing the error or allowing the lint.
- **`needless_range_loop`** — iterate with `for x in &xs` / `.iter().enumerate()`, not `for i in 0..xs.len()`.
- **`vec_init_then_push`** — when conditional (`#[cfg]`) construction prevents a `vec![…]` literal, use `Vec::extend([…])` rather than repeated `push`.

## Logging conventions

All logging uses `cfg_if` to select between the `tracing` and `logging` features at compile time. Never use `error!` — this is a library that propagates errors to the caller.

### Level guide

| Level | When to use |
|-------|-------------|
| `info` | Important success event (stage complete, network fetch done, source fully loaded) |
| `warn` | Intentionally ignored error (`ignore_errors` path) |
| `debug` | Before attempting something important — include the inputs/params that affect the outcome |
| `trace` | After completing a low-level operation — include rich detail about what was produced |

### Format rules

Every log call must start with a `msg` field whose value begins with a capital letter.
Additional structured fields follow as `key=value` pairs.

**`tracing` feature:**
```rust
tracing::info!(msg = "Loaded configuration payloads", payload_count = result.len());
tracing::warn!(msg = "Skipped load error for source", source = %display_val, error = ?e);
tracing::debug!(msg = "Loading configuration source", source = source_name, resource = resource);
tracing::trace!(msg = "Read configuration file", name = ?name, path = ?path, bytes = content.len());
```

**`logging` feature** — mirror the same key=value pairs in the format string:
```rust
log::info!("msg=\"Loaded configuration payloads\" payload_count={}", result.len());
log::warn!("msg=\"Skipped load error for source\" source={display_val} error={e:?}");
log::debug!("msg=\"Loading configuration source\" source={source_name} resource={resource}");
log::trace!("msg=\"Read configuration file\" name={name:?} path={path:?} bytes={}", content.len());
```

### Pattern

Wrap every call in `cfg_if!` — check `tracing` first, then `logging`:

```rust
use cfg_if::cfg_if;

cfg_if! {
    if #[cfg(feature = "tracing")] {
        tracing::debug!(msg = "Loading configuration source", source = source_name);
    } else if #[cfg(feature = "logging")] {
        log::debug!("msg=\"Loading configuration source\" source={source_name}");
    }
}
```
