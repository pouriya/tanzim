# tanzim-load/examples

| File | Required features | Description |
|------|-------------------|-------------|
| `custom_loader.rs` | none | Custom `Load` impl for an in-memory source; shows the loader contract |
| `load_config.rs` | `env`, `file` | CLI: parse source strings, load with built-in loaders, print payloads |

`load_config.rs` compiles without features but prints a "no loaders" message at runtime.
