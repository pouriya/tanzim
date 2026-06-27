# examples/basic

Full-pipeline CLI example for `tanzim`.

Reads source strings from command-line arguments, runs load → parse → merge, and prints
the merged configuration tree. Demonstrates env-var and file sources together.

Run via:
```bash
env 'APP_NAME.FOO.PORT=8080' \
cargo run -p tanzim --features full --example basic -- \
    'env(prefix=APP_NAME,separator=.)' 'file:examples/basic/etc'
```
