# tanzim-value
[**Package**](https://crates.io/crates/tanzim-value)   |   [**Documentation**](https://docs.rs/tanzim-value)   |   [**Repository**](https://github.com/pouriya/tanzim/tree/master/crates/tanzim-value)

Foundational value types for the tanzim pipeline.

## Types

- [`Value`] — `Bool`, `Int`, `Float`, `String`, `List`, `Map` (no null)
- [`LocatedValue`] — `Value` + [`Location`] (full originating [`tanzim_source::Source`], optional 1-based line/column)
- [`Map`] — ordered `Vec`-backed map; last inserted key wins on lookup
- [`Error`] — parse-time errors; use `{error:#}` for source snippet with caret underline

## Location

[`Location`] holds the full originating [`tanzim_source::Source`] (name, options, resource,
including any `on_error` policy) so any value or error can be traced back to how it was
declared. Positions (`line`, `column`, `length`) are 1-based and stored as `Option<NonZeroU32>`.
[`Error`] boxes its [`Location`] field so results stay small enough to return by value without
tripping `clippy::result_large_err`. Construct via [`Location::in_source`] (real source) or
[`Location::at`] (bare name/resource for synthetic origins); neither exposes `NonZeroU32` to
callers.

## Example

```rust
use tanzim_value::{Value, LocatedValue, Location, Map};

let mut map = Map::new();
let location = Location::at("env", "", None, None, None);
map.insert(
    "port".to_string(),
    LocatedValue::new(Value::Int(8080), location.clone()),
);
map.insert(
    "host".to_string(),
    LocatedValue::new(Value::String("localhost".to_string()), location),
);
assert!(map.contains_key("port"));
assert_eq!(map.len(), 2);
```

## Features

No optional features. This crate is always included as-is.

## Relations

- Used by all other tanzim crates.
- [`tanzim-parse`](https://crates.io/crates/tanzim-parse) produces `LocatedValue` trees from raw bytes.
- [`tanzim-merge`](https://crates.io/crates/tanzim-merge) consumes `LocatedValue` trees to produce merged maps.
