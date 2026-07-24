# tanzim-value
[**Package**](https://crates.io/crates/tanzim-value)   |   [**Documentation**](https://docs.rs/tanzim-value)   |   [**Repository**](https://github.com/pouriya/tanzim/tree/master/crates/tanzim-value)

Foundational value types for the tanzim pipeline.

## Types

- [`Value`] — `Bool`, `Int`, `Float`, `String`, `List`, `Map`, `Null`
- [`LocatedValue`] — `Value` + [`Location`] (full originating [`tanzim_source::Source`], optional 1-based line/column)
- [`Map`] — ordered `Vec`-backed map; last inserted key wins on lookup
- [`Error`] — parse-time and (with the `serde` feature) deserialize / serialize errors; use `{error:#}` for a source snippet with caret underline

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

## Serde (`serde` feature)

### Deserializing into your own types

[`Value`] and [`LocatedValue`] implement [`serde::Deserializer`](::serde::Deserializer), so a config
tree turns straight into your own structs. A [`LocatedValue`] runs the same [`Value`] deserializer
but, on failure, stamps the offending node's [`Location`] onto the error:

```rust
# // Deserialization lives behind the `serde` feature.
# #[cfg(feature = "serde")]
# {
use serde::Deserialize;
use tanzim_value::{Value, LocatedValue, Location, Map};

#[derive(Deserialize)]
struct Server {
    host: String,
    port: u16,
}

# let location = Location::at("file", "server.json", None, None, None);
# let mut map = Map::new();
# map.insert(
#     "host".to_string(),
#     LocatedValue::new(Value::String("localhost".to_string()), location.clone()),
# );
# map.insert(
#     "port".to_string(),
#     LocatedValue::new(Value::Int(8080), location.clone()),
# );
# let tree = LocatedValue::new(Value::Map(map), location);
// `tree` is a `LocatedValue` a parser would produce, here the map
// `{ host: "localhost", port: 8080 }` located at `server.json`.
let server: Server = tree.try_deserialize().unwrap();
assert_eq!(server.host, "localhost");
assert_eq!(server.port, 8080);
// On a type mismatch: `Err(Error::Deserialize { .. })` whose `Display` points at
// `source:resource:line:column`.
# }
```

### Serializing from your own types

The reverse direction — [`Value::try_from_serialize`] / [`LocatedValue::try_from_serialize`] — builds
a value tree from any `T: Serialize`. Use the located form when provenance matters: every node is
stamped with the supplied [`Location`] (e.g. `"defaults"` for programmatic defaults).

```rust
# #[cfg(feature = "serde")]
# {
use serde::Serialize;
use tanzim_value::{LocatedValue, Location};

#[derive(Serialize)]
struct Server {
    host: String,
    port: u16,
}

let tree = LocatedValue::try_from_serialize(
    &Server {
        host: "localhost".into(),
        port: 8080,
    },
    Location::at("defaults", "", None, None, None),
)
.unwrap();

assert_eq!(tree.location().source_name(), "defaults");
assert_eq!(
    tree.value().as_map().unwrap().get("port").unwrap().value().as_int(),
    Some(8080),
);
assert_eq!(
    tree.value().as_map().unwrap().get("port").unwrap().location().source_name(),
    "defaults",
);
# }
```

Map keys must serialize as strings; `None` / unit become [`Value::Null`]. Failures surface as
[`Error::Serialize`].

## Features

| Feature | Enables |
|---------|---------|
| `serde` | [`Deserializer`](::serde::Deserializer) / [`Serializer`](::serde::ser::Serializer) for [`Value`]/[`LocatedValue`], `try_deserialize` / `try_from_serialize`, and serde error impls for [`Error`] |

Off by default.

## Relations

- Depends on [`tanzim-source`](https://crates.io/crates/tanzim-source) for [`Location`]'s originating [`Source`](tanzim_source::Source).
