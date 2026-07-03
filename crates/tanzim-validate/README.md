# tanzim-validate
[**Package**](https://crates.io/crates/tanzim-validate)   |   [**Documentation**](https://docs.rs/tanzim-validate)   |   [**Repository**](https://github.com/pouriya/tanzim/tree/master/crates/tanzim-validate)

Validate and coerce [`tanzim-value`](https://crates.io/crates/tanzim-value)
configuration trees against a schema built from composable validators.

Validators do two jobs:

- **Check** that a value has the expected shape (type, range, length, required keys, …).
- **Coerce** values in place where the source format lost type information — e.g. a TOML/env
  string `"8080"` becomes an integer, an integral float `3.0` becomes `3`, and an empty map
  becomes an empty list.

Because coercion edits the value, the [`Validator`] trait works on `&mut Value`. Validate a
whole [`tanzim_value::LocatedValue`] node with the free [`validate`] function, which also
seeds the root location into any error.

Nested validators are passed by value — any `Validator` converts into the boxed form, so
there is no `Some(Box::new(...))` ceremony. Use the `_any` method variants when a key just
has to be present without validating its value.

```rust
use tanzim_validate::{validate, Integer, StaticMap, Str, Validator};
use tanzim_value::{LocatedValue, Location, Map, Value};

let schema = StaticMap::new()
    .required("host", Str::new().min_chars(1))
    .optional("port", Integer::new().range(1, 65535));

let loc = Location::at("file", "config.toml", None, None, None);
let mut map = Map::new();
map.insert("host".into(), LocatedValue { value: Value::String("localhost".into()), location: loc.clone() });
map.insert("port".into(), LocatedValue { value: Value::String("8080".into()), location: loc.clone() });
let mut root = LocatedValue { value: Value::Map(map), location: loc };

validate(&schema, &mut root).unwrap();
// "8080" was coerced to the integer 8080
assert_eq!(root.value.as_map().unwrap().get("port").unwrap().value.as_int(), Some(8080));
```

## Validators

Std-only validators (no extra dependencies). Each has its own Cargo feature and
all of them are enabled by `default`; disable default features to trim the set.

- Primitives: `Bool`, `Integer`, `Float`, `Number`, `Str`, `List`, `StaticMap`, `DynamicMap`
  (`boolean`, `integer`, `float`, `number`, `string`, `list`, `static_map`, `dynamic_map`).
- Choice & constraints: `Enum` (`enumeration`), `NonEmpty` (`non_empty`), `Percentage` (`percentage`).
- Combinator: `Either` (`either`) — accepts the value if either of two validators accepts it.
- Network: `Host`, `Domain`, `Email`, `Port`, `IpAddr`, `SocketAddr` (`net`).
- Filesystem: `Path` (`path`), with opt-in filesystem checks.

`float`/`integer` imply `number`; `net` implies `integer`.

`Number` accepts an integer or a float **without** converting between them. Sign
constraints live as builder methods on `Integer`, `Float`, and `Number`:
`.positive()`, `.non_negative()`, `.negative()`, `.non_positive()`.

Behind Cargo features (each pulls one external crate):

| feature | validators |
| --- | --- |
| `regex` | `Str::regex(...)`, `RegexPattern` |
| `url` | `Url` |
| `cidr` | `Cidr` |
| `uuid` | `Uuid` |
| `semver` | `Semver` |
| `encoding` | `Base64`, `Hex` |
| `duration` | `Duration` |
| `bytesize` | `ByteSize` |
| `datetime` | `DateTime`, `Date` |

`default` is the std-only validator set. `full` enables every validator plus
`schema` (but not `logging`/`tracing`).

## Building validators from a schema (`schema` feature)

With the off-by-default `schema` feature you can reconstruct a validator from a data
document instead of Rust code. A schema node is a map with a `"type"` tag plus that
validator's options (snake_case, matching the builder methods):

```json
{
  "type": "static_map",
  "fields": {
    "host": { "required": true,  "validator": { "type": "host" } },
    "port": { "required": false, "validator": { "type": "port", "privileged_ok": false } },
    "tags": { "required": false, "validator": {
        "type": "list", "unique": true, "items": { "type": "string", "min_chars": 1 } } },
    "mode": { "required": true, "validator": {
        "type": "either",
        "first":  { "type": "enum", "values": ["auto", "manual"] },
        "second": { "type": "integer", "min": 0 } } }
  }
}
```

The feature adds only `serde` as a dependency. Deserialize the document into a
[`SchemaValue`] with any serde data format (`serde_json`, etc.), then build:

```rust
# #[cfg(feature = "schema")] {
use tanzim_validate::{build_value, SchemaValue};

let doc: SchemaValue = serde_json::from_str(r#"{"type":"integer","min":1,"max":65535}"#).unwrap();
let validator = build_value(doc.value()).unwrap();
# }
```

Because the builder works on a `tanzim_value::Value`, the same path also accepts a tree
produced by `tanzim-parse` directly (no serde call needed) — which is how the `tanzim`
facade will wire it in.

Dispatch goes through a [`Registry`]; `Registry::with_builtins()` knows every built-in tag
(feature-gated ones included when their feature is on), and `Registry::register` adds custom
validator types. Schema-construction problems surface as a `SchemaError` carrying a field
path and, when built from a located tree, a source location.
