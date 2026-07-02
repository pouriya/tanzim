# tanzim workspace examples

Runnable examples for the `tanzim` facade crate (declared in `crates/tanzim/Cargo.toml`).

| Directory | Example | Description |
|-----------|---------|-------------|
| `full/` | `full` | CLI: pass source strings as args, build a `PipelineMulti` with `PipelineMultiBuilder`, call `run()`, print merged output |

Run with:
```bash
cargo run -p tanzim --features full,tracing --example full -- 'env(prefix=APP_NAME,separator=.)' 'file:examples/full/etc'
```
