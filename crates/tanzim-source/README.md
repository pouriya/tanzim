# tanzim-source
[**Package**](https://crates.io/crates/tanzim-source)   |   [**Documentation**](https://docs.rs/tanzim-source)   |   [**Repository**](https://github.com/pouriya/tanzim/tree/master/crates/tanzim-source)

Declarative configuration source: where to load from, loader options, and resource (address).

## Format

```text
SOURCE [(OPTIONS)] [?] [:RESOURCE]
```

| Part | Meaning |
|------|---------|
| `SOURCE` | Loader kind (opaque string, e.g. `env`, `file`, `http`, `custom`) |
| `(OPTIONS)` | Optional loader options as `key=value` pairs |
| `?` | Ignore errors when loading this source |
| `:RESOURCE` | Optional address (path, URL, …); may be empty |

### Source examples

```text
env
env(prefix=APP_)
file:/etc/app/config.json
file?:.env
http(headers=(Authorization="TOKEN"),timeout=3s)?:https://example.com/config.yml
custom(k=v,list=[1,2,3.14,""],inner-kv=(foo=bar,baz=qux)):oops
```

## Parsing

```rust
use tanzim_source::Source;

let env = Source::parse("env(prefix=APP_)")?;
let file: Source = "file:/etc/app/config.json".parse()?;
let http = Source::try_from("http(headers=(Authorization=TOKEN))?:https://config.tld")?;

assert_eq!(env.source(), "env");
assert_eq!(env.to_string(), "env(prefix=APP_)");
assert_eq!(file.resource(), "/etc/app/config.json");
assert!(http.ignore_errors());
assert_eq!(
  http
    .options().get("headers").unwrap()
    .as_map().unwrap()
    .get("Authorization").unwrap()
    .as_string().unwrap(),
  "TOKEN"
);

# Ok::<(), tanzim_source::Error>(())
```

##### Format Rules

- **`SOURCE` and option keys** — ASCII letters, digits, `-`, `_`, `.` (non-empty).
- **No whitespace** anywhere except inside `"quoted"` strings.
- **Option values** — try, in order: boolean, integer, float, list, map; otherwise unquoted string.
  - Booleans: `true` / `false` (case-insensitive).
  - Integers: base-10, optional leading `-` (no `+`).
  - Floats: digits, `.`, optional leading `-` (e.g. `3.14`; not `.5`).
  - Lists: `[value,value,…]`
  - Maps: `(key=value,…)` (same value grammar).
  - Unquoted strings: letters, digits, `-`, `_`, `.` only (e.g. `APP_`, `3s`).
  - Quoted strings: `"…"` with escapes `\"`, `\\`, `\n`, `\r`, `\t`.
- **Empty value** — error; use `""`.
- **Trailing commas** — error (`(a=1,)` / `[1,]`).
- **Duplicate keys** in `(OPTIONS)` — last wins.
- **`?` (ignore errors)** — must come after `SOURCE` or after the closing `)` of `(OPTIONS)`; never before `(OPTIONS)`.

Build programmatically with [`SourceBuilder`](https://docs.rs/tanzim-source/latest/tanzim_source/struct.SourceBuilder.html):

```rust
use tanzim_source::SourceBuilder;

let source = SourceBuilder::new()
    .with_source("env")
    .with_option("prefix", "APP")
    .with_ignore_errors(false)
    .build()?;
# Ok::<(), tanzim_source::Error>(())
```

Try sources from the command line:

```shell
cargo run -p tanzim-source --example parse_sources -- \
  'env(prefix=APP_)' \
  'file:/etc/app/config.json'
```

## Ergonomic Errors
Use `{error:#}` for multi-line errors with caret:

```rust
use tanzim_source::Source;
let error = Source::parse("bad?(k=v)").unwrap_err();
println!("{error:#}")
```
```text
invalid configuration source at column 4: configuration source: skip marker `?` must come after options `(...)`; use `source(...)?` not `source?(...)`
  bad?(k=v)
     ^
```
Use `{error}` for one-line errors.


## Cargo features

| Feature | Enables |
|---------|---------|
| `serde` | `Serialize` / `Deserialize` for `Source` as its canonical string |

## Relations

- Used by all other tanzim crates to represent where and how to load from.
- [`tanzim-load`](../tanzim-load/) consumes `Source` fields (`source`, `options`, `resource`) in its loaders.
- Full pipeline wired in [`tanzim`](../tanzim/).
