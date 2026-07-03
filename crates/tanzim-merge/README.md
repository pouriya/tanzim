# tanzim-merge
[**Package**](https://crates.io/crates/tanzim-merge)   |   [**Documentation**](https://docs.rs/tanzim-merge)   |   [**Repository**](https://github.com/pouriya/tanzim/tree/master/crates/tanzim-merge)

Third stage of the tanzim pipeline: groups parsed payloads by entry name and merges their values.

## The `Merge` trait

Implement [`Merge`] to define a custom merge strategy. The output is
[`Merged`] — a `HashMap<Option<String>, (Vec<Payload>, LocatedValue)>` keyed by entry
name, where the `Vec<Payload>` records which payloads contributed to each merged
value.

## Grouping key

`Payload::maybe_name` determines the map key:
- `Some("foo")` → key `Some("foo")`
- `None` → key `None` (all unnamed payloads share this bucket)

## Built-in strategies

| Type | Behaviour |
|------|-----------|
| `LastWins` | Last value for each name fully replaces any previous value |
| `DeepMerge` | Maps are merged recursively; the overlay value wins at each non-map leaf |

## Example

```rust
use tanzim_merge::{DeepMerge, LastWins, Merge};
use tanzim_load::Payload;
use tanzim_source::Source;
use tanzim_value::{LocatedValue, Location, Map, Value};

let source = Source::parse("env").unwrap();

let make_entry = |name: Option<&str>, key: &str, val: &str| {
    let loc = Location::at("env", "", None, None, None);
    let mut map = Map::new();
    map.insert(key.to_string(), LocatedValue::new(Value::String(val.to_string()), loc.clone()));
    let payload = Payload {
        source: source.clone(),
        maybe_name: name.map(str::to_string),
        maybe_format: Some("env".into()),
        content: vec![],
    };
    (payload, LocatedValue::new(Value::Map(map), loc))
};

let list = vec![
    make_entry(Some("db"), "host", "primary"),
    make_entry(Some("db"), "host", "replica"),
];

// LastWins: second entry fully replaces the first
let merged = LastWins.merge(&list).unwrap();
let db = merged.get(&Some("db".to_string())).unwrap();
let host = db.1.value().as_map().unwrap().get("host").unwrap();
assert_eq!(host.value().as_string().unwrap(), "replica");
```

## Features

No default features. Opt-in `logging` / `tracing` emit log messages / trace
spans via the `log` / `tracing` crates.

## Relations

- Consumes `LocatedValue` from [`tanzim-parse`](https://crates.io/crates/tanzim-parse).
- Uses `Payload` from [`tanzim-load`](https://crates.io/crates/tanzim-load) (which embeds `Source`) to track provenance.
- Full pipeline wired in [`tanzim`](https://crates.io/crates/tanzim).
