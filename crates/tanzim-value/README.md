# tanzim-value

Foundational value types for the tanzim pipeline.

## Types

- [`Value`] — `Bool`, `Int`, `Float`, `String`, `List`, `Map` (no null)
- [`LocatedValue`] — `Value` + [`Location`] (source name, resource, optional 1-based line/column)
- [`Map`] — ordered `Vec`-backed map; last inserted key wins on lookup
- [`Error`] — parse-time errors; use `{error:#}` for source snippet with caret underline

## Location

Positions (`line`, `column`, `length`) are 1-based and stored as
`Option<NonZeroU32>`. The compact representation keeps [`Location`] — and
therefore the [`Error`] that embeds it — small enough to return by value without
tripping `clippy::result_large_err`. Construct with [`Location::at`], which takes
ordinary `Option<usize>` positions and treats zero or out-of-range values as
absent, so callers never deal with `NonZeroU32` directly.

## Example

```rust
use tanzim_value::{Value, LocatedValue, Location, Map};

let mut map = Map::new();
let location = Location::at("env", "", None, None, None);
map.insert(
    "port".to_string(),
    LocatedValue { value: Value::Int(8080), location: location.clone() },
);
map.insert(
    "host".to_string(),
    LocatedValue { value: Value::String("localhost".to_string()), location },
);
assert!(map.contains_key("port"));
assert_eq!(map.len(), 2);
```

## Features

No optional features. This crate is always included as-is.

## Relations

- Used by all other tanzim crates.
- [`tanzim-parse`](../tanzim-parse/) produces `LocatedValue` trees from raw bytes.
- [`tanzim-merge`](../tanzim-merge/) consumes `LocatedValue` trees to produce merged maps.
