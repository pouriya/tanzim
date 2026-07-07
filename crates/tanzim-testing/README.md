# tanzim-testing

A tiny, dependency-light test sandbox for the `tanzim` workspace.

[`Environment`](environment::Environment) runs a closure inside a fresh temporary directory:
it `chdir`s into that directory, snapshots the whole process environment, and — when the closure
returns (or panics) — restores the environment and working directory and deletes the temporary
directory. Every run is serialized behind a process-global lock, so it is safe to use from tests
that the harness runs in parallel.

Use it to make tests and examples self-contained: create the files and environment variables a
piece of configuration code expects, exercise it, and let the sandbox clean up after itself.

## Example

```rust
use tanzim_testing::environment::run;

run(|env| {
    // The current directory is the sandbox for the duration of the run.
    env.write_file("app.json", b"{ \"port\": 8080 }")?;
    env.create_directory("logs")?;

    let contents = std::fs::read_to_string("app.json")?;
    assert!(contents.contains("8080"));
    Ok(())
})
.unwrap();
```

## Prior art

The idea is inspired by the `Jail` feature of the [`figment`](https://crates.io/crates/figment)
crate.
