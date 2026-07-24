# tanzim-value

Foundational value types used throughout the tanzim pipeline.

## What lives here

- `Value` — six-variant dynamically typed enum (Bool, Int, Float, String, List, Map). No null.
- `LocatedValue` — a `Value` paired with a `Location` (full originating `tanzim_source::Source`, optional line/column).
- `Map` — ordered `Vec`-backed map of `String → LocatedValue`. Last inserted key wins on lookup.
- `Location` — holds the full originating `Source` (name, options, resource, `on_error` policy) so values can be traced to their declaration. `line`/`column`/`length` are 1-based and stored as `Option<NonZeroU32>`. Build via `Location::in_source` (real source) or `Location::at` (bare name/resource for synthetic origins); neither exposes `NonZeroU32` to callers.
- `Error` — parse-time error with optional source snippet; use `{error:#}` for the caret underline. `Error` boxes its `Location` field so it stays under the `result_large_err` size threshold. Shrink other fields rather than adding `#[allow]`.

## Serde (`serde` feature)

- `Value` / `LocatedValue` implement `Deserializer` (`try_deserialize::<T>()`).
- `Value::try_from_serialize` / `LocatedValue::try_from_serialize` implement `Serializer` (Rust
  `T: Serialize` → value tree). Nested nodes inherit the caller-supplied `Location` (e.g.
  `Location::at("defaults", …)` for synthetic origins).

## No external logic

This crate contains only type definitions, display formatting, and (optionally) serde bridges. It has
no dependencies on loaders, parsers, or source parsing.

## src/ layout

- `value.rs` — `Value`, `LocatedValue`, `Map`, `Location`, `ValueType`
- `error.rs` — `Error` with alternate `{:#}` display (snippet + caret)

## Testing

No tests in `src/`. Add/move tests to `tests/` (see workspace `AGENTS.md` for naming).
