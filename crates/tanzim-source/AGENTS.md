# tanzim-source

Parses the declarative source string format used to declare where configuration comes from.

## Format

```
SOURCE [(OPTIONS)] [:RESOURCE]
```

Examples: `env`, `env(prefix=APP_)`, `file:/etc/app`, `file(on_error=(load=skip)):.env`, `http(timeout=5,on_error=(load=skip)):https://...`

## Key types

- `Source` — parsed, validated source declaration. Has `source()`, `options()`, `resource()`, `on_error(Stage)` → `OnError`.
- `SourceBuilder` — builder for constructing `Source` programmatically.
- `Options` / `OptionValue` — ordered map and dynamically typed value used for loader options.
- `ParseError` — detailed parse error; use `{error:#}` for snippet + caret.

## src/ layout

- `lib.rs` — `Source`, `SourceBuilder`, `Options`, `OptionValue`, public API
- `parse.rs` — hand-written recursive descent parser + `Source: Display`
- `impls.rs` — `From`/`TryFrom` conversions for `Source`, `OptionValue`, `Options`
- `serde.rs` — (feature `serde`) serialize/deserialize `Source` as its canonical string

When editing the parser, remember that `Source: Display` must round-trip through `parse()`.
