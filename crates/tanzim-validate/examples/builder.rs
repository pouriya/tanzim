//! Build a validator with the builder API, then validate and coerce a value.
//!
//! Run with: `cargo run -p tanzim-validate --example builder`

use tanzim_validate::{Integer, StaticMap, Str, validate};
use tanzim_value::{LocatedValue, Location, Map, Value};

fn main() {
    // A schema: `host` is a non-empty string, `port` is an integer in 1..=65535.
    let schema = StaticMap::new()
        .required("host", Str::new().min_chars(1))
        .optional("port", Integer::new().range(1, 65535));

    // A config where `port` arrived as a string (e.g. from an env var or TOML).
    let location = Location::at("example", "", None, None, None);
    let mut map = Map::new();
    map.insert(
        "host".to_string(),
        LocatedValue {
            value: Value::String("localhost".to_string()),
            location: location.clone(),
        },
    );
    map.insert(
        "port".to_string(),
        LocatedValue {
            value: Value::String("8080".to_string()),
            location: location.clone(),
        },
    );
    let mut root = LocatedValue {
        value: Value::Map(map),
        location,
    };

    match validate(&schema, &mut root) {
        Ok(()) => {
            let port = root.value.as_map().unwrap().get("port").unwrap();
            println!("valid — `port` was coerced from a string to {}", port.value);
        }
        Err(error) => eprintln!("invalid: {error}"),
    }
}
