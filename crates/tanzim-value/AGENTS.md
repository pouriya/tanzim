# tanzim-value

Foundational value types used throughout the tanzim pipeline.

## What lives here

- `Value` — six-variant dynamically typed enum (Bool, Int, Float, String, List, Map). No null.
- `LocatedValue` — a `Value` paired with a `Location` (source name, resource path, optional line/column).
- `Map` — ordered `Vec`-backed map of `String → LocatedValue`. Last inserted key wins on lookup.
- `Location` — human-readable source position used in error messages and diagnostics. `line`/`column`/`length` are 1-based and stored as `Option<NonZeroU32>` (compact, so `Error` stays small and avoids `clippy::result_large_err`). Build via `Location::at`, which accepts `Option<usize>` and converts internally — never expose `NonZeroU32` to callers.
- `Error` — parse-time error with optional source snippet; use `{error:#}` for the caret underline. Keep it under the `result_large_err` size threshold (no `Box` in return types); shrink fields rather than adding `#[allow]`.

## No external logic

This crate contains only type definitions and display formatting. It has no dependencies on loaders, parsers, or source parsing. All deserialization logic lives in `tanzim-parse`.

## src/ layout

- `value.rs` — `Value`, `LocatedValue`, `Map`, `Location`, `ValueType`
- `error.rs` — `Error` with alternate `{:#}` display (snippet + caret)
