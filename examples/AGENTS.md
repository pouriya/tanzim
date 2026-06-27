# tanzim workspace examples

Runnable examples for the `tanzim` facade crate (declared in `crates/tanzim/Cargo.toml`).

| Directory | Example | Description |
|-----------|---------|-------------|
| `basic/` | `basic` | CLI: pass source strings as args, build a `Config` with `ConfigBuilder`, call `run()`, print merged output |

Run with:
```bash
cargo run -p tanzim --features full --example basic -- 'env(prefix=APP_NAME,separator=.)' 'file:examples/basic/etc'
```
