# examples/full

Full-pipeline CLI example for `tanzim`.

Reads source strings from command-line arguments, runs load → parse → merge → validate, and
prints the merged configuration tree. Demonstrates env-var and file sources together, and
loads `schema.yml` with `serde_yaml` into `Schemas`, handing it to `ConfigBuilder::with_schemas`
so the merged output is validated (and coerced) after the merge stage.

Run via:
```bash
env 'APP_NAME.FOO.PORT=8080' \
cargo run -p tanzim --features full,tracing --example full -- \
    'env(prefix=APP_NAME,separator=.)' 'file:examples/full/etc'
```
