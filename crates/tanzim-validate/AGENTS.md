# tanzim-validate

Fourth stage of the pipeline: validates (and coerces) a configuration value against a schema.

## Key types

- `Validator` — trait with three required methods: `meta() -> &Meta`, `meta_mut() -> &mut Meta`,
  and `check(&self, value: &mut Value) -> Result<(), Error>` (the rule). `validate()` is a
  provided method: runs `check`, attaches this validator's `Meta` on error, then applies the
  output cast in `meta().convert` on success. Takes `&mut Value` (not `LocatedValue`) so it can
  **coerce in place** (numeric string → int, integral float → int, empty map ↔ empty list, etc.).
- `Meta` — human-facing metadata every validator carries: `name`, `description`, `examples`,
  `default`, and an optional output-cast `convert`. Set through the `WithMeta` blanket-implemented
  builder methods (`with_name`, `with_description`, `with_default`, `to_int`, …).
- `WithMeta` — blanket-implemented trait that adds fluent getters/setters for every `Validator`'s `Meta`.
- `validate(validator, &mut LocatedValue)` — free fn that validates a whole node and seeds the
  root `Location` into any error.
- `Error` / `ErrorKind` / `Segment` — validation failure with a breadcrumb `path` and an
  optional **boxed** `Location` (boxed to satisfy `clippy::result_large_err`).
- Nested validators are passed by value: a blanket `From<V: Validator> for Box<dyn Validator>`
  lets builder methods take `impl Into<Box<dyn Validator>>` (no `Some(Box::new(...))`).

## Validators

- Always on (std only): `Bool`, `Integer`, `Float`, `Number` (no int/float conversion), `Str`,
  `List`, `StaticMap`, `DynamicMap`, `Enum` (any value type), `NonEmpty`, `Percentage`,
  `Either` (accepts if either of two validators accepts), `Host`, `Domain`, `Email`, `Port`,
  `IpAddr`, `SocketAddr`, `Path`. Sign constraints are methods on `Integer`/`Float`/`Number`:
  `.positive()`, `.non_negative()`, `.negative()`, `.non_positive()`.
- Behind a feature each (one external crate per feature): `regex`, `url`, `cidr`, `uuid`,
  `semver`, `encoding` (`Base64`/`Hex`), `duration`, `bytesize`, `datetime`.

## schema feature (off by default; pulls `serde`)

Build a validator from a self-describing data document instead of Rust code.

- `SchemaValue(Value)` — newtype with a hand-written serde `Deserialize`, the bridge between
  any serde format (`serde_json` is a dev-dep for tests) and tanzim's `Value`.
- `Registry` — maps a `"type"` tag to a constructor; `with_builtins()` knows every built-in
  (feature-gated arms included when on), `register()` adds custom types. `build`/`build_value`
  dispatch; `Node` is the option-reader/recursion helper passed to constructors.
- `SchemaError` / `SchemaErrorKind` — schema-construction failures (distinct from `Error`).

Because the registry consumes a `Value`, a tree from `tanzim-parse` can be fed directly
(no serde call).

## src/ layout

One module per validator (`integer.rs`, `static_map.rs`, `net.rs`, …), `error.rs`, `number.rs`
(holds the shared `Sign` helper), and `schema.rs` (the `schema` feature). Examples in
`examples/` (`builder`, `schema`).

## Testing

No tests in `src/`. Add/move tests to `tests/` (see workspace `AGENTS.md` for naming).
