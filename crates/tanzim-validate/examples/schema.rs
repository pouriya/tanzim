//! Build a validator from a JSON schema document, then validate and coerce a value.
//!
//! Run with: `cargo run -p tanzim-validate --features schema --example schema`

use tanzim_validate::{SchemaValue, build_value, validate};
use tanzim_value::{LocatedValue, Location};

fn main() {
    // A schema document — could come from any serde format (here JSON).
    let schema_json = r#"{
        "type": "static_map",
        "fields": {
            "host": { "required": true, "validator": { "type": "host" } },
            "port": { "required": true, "validator": { "type": "port" } }
        }
    }"#;
    let schema: SchemaValue = serde_json::from_str(schema_json).expect("parse schema");
    let validator = build_value(schema.value()).expect("build validator");

    // A config document with a string `port` that the `port` validator will coerce.
    let config_json = r#"{ "host": "example.com", "port": "443" }"#;
    let config: SchemaValue = serde_json::from_str(config_json).expect("parse config");
    let mut value = LocatedValue {
        value: config.into_value(),
        location: Location::at("example", "", None, None, None),
    };

    match validate(validator.as_ref(), &mut value) {
        Ok(()) => println!("valid — coerced config: {value:#}"),
        Err(error) => eprintln!("invalid: {error}"),
    }
}
