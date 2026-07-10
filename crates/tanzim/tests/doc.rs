//! Reproduces the two error scenarios shown on the crate-level landing page (`lib.rs`) and in the
//! README, so the exact `{error:#}` renderings pasted there stay real. Run with:
//!
//! ```text
//! cargo test -p tanzim --test doc --all-features -- --nocapture
//! ```
#![cfg(all(
    feature = "parse-toml",
    feature = "validate-static_map",
    feature = "validate-bytesize",
    feature = "validate-non_empty",
))]

use serde::Deserialize;
use tanzim::Config;
use tanzim::validator::{ByteSize, NonEmpty, StaticMap};
use tanzim_testing::environment::run;

#[derive(Debug, Deserialize)]
struct LogRotation1 {
    #[allow(dead_code)]
    file: String,
    #[allow(dead_code)]
    rotate_count: u32,
}

#[derive(Debug, Deserialize)]
struct LogRotation2 {
    #[allow(dead_code)]
    file: String,
    max_size: u64,
}

/// Example 1: a wrong-typed value fails to deserialize, and `{error:#}` points a caret at it —
/// no validator involved.
#[test]
fn example1_wrong_type_caret() {
    run(|env| {
        env.write_file(
            "app.toml",
            "file = \"/var/log/app.log\"\nrotate_count = \"five\"\n",
        )?;
        let error = Config::default()
            .with_source("file:app.toml")
            .unwrap()
            .try_deserialize::<LogRotation1>()
            .unwrap_err();

        println!("--- example1 default ---\n{error}\n");
        println!("--- example1 alternate ---\n{error:#}\n");

        let alternate = format!("{error:#}");
        assert!(alternate.contains("rotate_count = \"five\""), "{alternate}");
        assert!(alternate.contains('^'), "{alternate}");
        Ok(())
    })
    .unwrap();
}

/// Example 2 (error path): a `ByteSize` field rejects an invalid human size, with a caret under
/// the offending value.
#[test]
fn example2_bytesize_validation_error() {
    run(|env| {
        env.write_file(
            "app.toml",
            "file = \"/var/log/app.log\"\nmax_size = \"banana\"\n",
        )?;
        let schema = StaticMap::new().required("file", NonEmpty::new()).required(
            "max_size",
            ByteSize::new()
                .with_description("Rotate the log once it grows past this size.")
                .with_example("10MB"),
        );
        let error = Config::default()
            .with_source("file:app.toml")
            .unwrap()
            .with_schema(schema)
            .try_deserialize::<LogRotation2>()
            .unwrap_err();

        println!("--- example2 default ---\n{error}\n");
        println!("--- example2 alternate ---\n{error:#}\n");

        let alternate = format!("{error:#}");
        assert!(alternate.contains("max_size"), "{alternate}");
        Ok(())
    })
    .unwrap();
}

/// Example 2 (happy path): `"10MB"` is coerced to a byte count so the struct deserializes.
#[test]
fn example2_bytesize_coerces() {
    run(|env| {
        env.write_file(
            "app.toml",
            "file = \"/var/log/app.log\"\nmax_size = \"10MB\"\n",
        )?;
        let schema = StaticMap::new()
            .required("file", NonEmpty::new())
            .required("max_size", ByteSize::new());
        let cfg = Config::default()
            .with_source("file:app.toml")
            .unwrap()
            .with_schema(schema)
            .try_deserialize::<LogRotation2>()
            .unwrap();
        assert_eq!(cfg.max_size, 10_000_000);
        Ok(())
    })
    .unwrap();
}
