# tanzim-load/src

- `lib.rs` — `Load` trait, `Payload` struct, `Error` enum
- `env.rs` — env-var loader (feature `env`): groups vars by separator prefix into named payloads
- `file.rs` — filesystem loader (feature `file`): stem → name, extension → format
- `http.rs` — HTTP loader (feature `http`): user supplies the fetch closure
- `closure.rs` — adapter that wraps any closure as a `Load` implementor; the boxed function type is the `LoaderFn` alias (keeps the struct field free of `clippy::type_complexity` without `#[allow]`)

All loaders produce `Payload { source, maybe_name, maybe_format, content }`. The `maybe_name` field is `None` when the loader cannot determine an entry name (e.g. env without separator).
