//! Reproduces the error scenarios shown in the crate-level docs (`lib.rs`) and both READMEs,
//! so the exact `{error:#}` renderings pasted there stay real. Run with:
//!
//! ```text
//! cargo test -p tanzim --test doc --all-features -- --nocapture
//! ```
#![cfg(all(
    feature = "load-env",
    feature = "parse-env",
    feature = "parse-toml",
    feature = "validate-static_map",
    feature = "validate-bytesize",
    feature = "validate-non_empty",
))]

use serde::Deserialize;
use tanzim::Config;
use tanzim::merger::plan::{deep, last_wins, src};
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

// ── Simple example tests (Config::default + with_source) ─────────────────────

/// Simple example (happy path): a TOML file loads and deserializes straight into the target type.
#[test]
fn example_simple_happy_path() {
    run(|env| {
        env.write_file("app.toml", "file = \"/var/log/app.log\"\nrotate_count = 5\n")?;
        let cfg = Config::default()
            .with_source("file:app.toml")
            .try_deserialize::<LogRotation1>()
            .unwrap();
        assert_eq!(cfg.file, "/var/log/app.log");
        assert_eq!(cfg.rotate_count, 5);
        Ok(())
    })
    .unwrap();
}

/// Simple example (error path): a wrong-typed value fails to deserialize, and `{error:#}` points
/// a caret at the offending value — no validator involved.
#[test]
fn example1_wrong_type_caret() {
    run(|env| {
        env.write_file(
            "app.toml",
            "file = \"/var/log/app.log\"\nrotate_count = \"five\"",
        )?;
        let error = Config::default()
            .with_source("file:app.toml")
            .try_deserialize::<LogRotation1>()
            .unwrap_err();

        println!("--- example1 default ---\n{error}\n");
        println!("--- example1 alternate ---\n{error:#}\n");

        // The exact `{error:#}` rendering shown in the `lib.rs` docs and READMEs.
        let expected = "\
failed to deserialize configuration: invalid type: string \"five\", expected u32 at file:app.toml:2:16
  1 | file = \"/var/log/app.log\"
  2 | rotate_count = \"five\"
    |                ^^^^^^";
        assert_eq!(format!("{error:#}"), expected);
        Ok(())
    })
    .unwrap();
}

// ── Advanced example tests (Config::from_plan + with_schema) ─────────────────
//
// The file loader derives `maybe_name` from the filename stem, so both `app.toml` (system) and
// `user/app.toml` (user) share the stem `"app"` and land in the same named bucket. `deep` then
// merges them recursively — keys only in the system file are preserved; keys in both take the
// user's value.  The env source is unnamed (`None` bucket), so in `unify` the unnamed bucket is
// appended last and `LastWins` lets env override the merged file config entirely. For the error
// path the env `APP_MAX_SIZE` is set to `"banana"` so validation fails at the env source.

/// Advanced example (happy path): system and user `app.toml` files are deep-merged; the env
/// source (`APP_MAX_SIZE=20MB`) wins the final config, and `ByteSize` coerces it to bytes.
#[test]
fn example_advanced_happy_path() {
    run(|env| {
        env.write_file("app.toml", "file = \"/var/log/app.log\"\nmax_size = \"100MB\"\n")?;
        env.write_file("user/app.toml", "max_size = \"10MB\"\n")?;
        env.set_env("APP_FILE", "/var/log/app.log")?;
        env.set_env("APP_MAX_SIZE", "20MB")?;
        let plan = last_wins(vec![
            deep(vec![
                src("file:app.toml").unwrap(),
                src("file:user/app.toml").unwrap(),
            ]),
            src("env(prefix=APP_)").unwrap(),
        ]);
        let schema = StaticMap::new()
            .required("file", NonEmpty::new())
            .required(
                "max_size",
                ByteSize::new()
                    .with_description("Rotate the log once it grows past this size.")
                    .with_example("10MB"),
            );
        let cfg = Config::from_plan(plan)
            .with_default_loaders()
            .with_default_parsers()
            .with_schema(schema)
            .try_deserialize::<LogRotation2>()
            .unwrap();
        assert_eq!(cfg.max_size, 20_000_000); // env wins: "20MB" coerced to bytes
        assert_eq!(cfg.file, "/var/log/app.log");
        Ok(())
    })
    .unwrap();
}

/// Advanced example (error path): a bad value in `user/app.toml` fails `ByteSize` validation;
/// env is not in the plan so the caret points at the exact file location.
#[test]
fn example_advanced_validation_error() {
    run(|env| {
        env.write_file("app.toml", "file = \"/var/log/app.log\"\nmax_size = \"100MB\"\n")?;
        env.write_file("user/app.toml", "max_size = \"banana\"\n")?;
        let plan = deep(vec![
            src("file:app.toml").unwrap(),
            src("file:user/app.toml").unwrap(),
        ]);
        let schema = StaticMap::new()
            .required("file", NonEmpty::new())
            .required(
                "max_size",
                ByteSize::new()
                    .with_description("Rotate the log once it grows past this size.")
                    .with_example("10MB"),
            );
        let error = Config::from_plan(plan)
            .with_default_loaders()
            .with_default_parsers()
            .with_schema(schema)
            .try_deserialize::<LogRotation2>()
            .unwrap_err();

        println!("--- example_advanced default ---\n{error}\n");
        println!("--- example_advanced alternate ---\n{error:#}\n");

        // The exact `{error:#}` rendering shown in the READMEs.
        let expected = "\
configuration failed validation: max_size: invalid byte size at file:user/app.toml:1:12
  Rotate the log once it grows past this size.
  example: \"10MB\"
  1 | max_size = \"banana\"
    |            ^^^^^^^^
  2 | ";
        assert_eq!(format!("{error:#}"), expected);
        Ok(())
    })
    .unwrap();
}

// ── Retained: example2 tests referenced from lib.rs comments ─────────────────

/// Example 2 (error path): a `ByteSize` field rejects an invalid human size, with a caret under
/// the offending value.
#[test]
fn example2_bytesize_validation_error() {
    run(|env| {
        env.write_file(
            "app.toml",
            "file = \"/var/log/app.log\"\nmax_size = \"banana\"",
        )?;
        let schema = StaticMap::new().required("file", NonEmpty::new()).required(
            "max_size",
            ByteSize::new()
                .with_description("Rotate the log once it grows past this size.")
                .with_example("10MB"),
        );
        let error = Config::default()
            .with_source("file:app.toml")
            .with_schema(schema)
            .try_deserialize::<LogRotation2>()
            .unwrap_err();

        println!("--- example2 default ---\n{error}\n");
        println!("--- example2 alternate ---\n{error:#}\n");

        // The exact `{error:#}` rendering shown in the `lib.rs` docs.
        let expected = "\
configuration failed validation: max_size: invalid byte size at file:app.toml:2:12
  Rotate the log once it grows past this size.
  example: \"10MB\"
  1 | file = \"/var/log/app.log\"
  2 | max_size = \"banana\"
    |            ^^^^^^^^";
        assert_eq!(format!("{error:#}"), expected);
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
            .with_schema(schema)
            .try_deserialize::<LogRotation2>()
            .unwrap();
        assert_eq!(cfg.max_size, 10_000_000);
        Ok(())
    })
    .unwrap();
}
