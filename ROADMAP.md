# Roadmap

Planned work for tanzim. Each entry lists what it is, why it matters, and how far
along it is. The pipeline today ends at validation; everything below is about
turning the resolved configuration back into something the outside world can use —
another format, a command line, or a shared library.

## 1. `tanzim-emit` — write-back and format conversion

**Status: 0% — not started.**

A new pipeline stage that runs in the opposite direction from parsing: it takes a
resolved value tree and writes it back out as bytes in a chosen format. Where the
existing `Parse` trait turns bytes into a `LocatedValue`, `tanzim-emit` will
provide an `Emit` trait that turns a `LocatedValue` back into bytes.

This unlocks true format conversion — read configuration in as env, YAML, TOML, or
JSON, and emit it as any of the others. Because tanzim already carries every
value's origin and stores comments on `LocatedValue`, emission should preserve
comments wherever the target format can represent them (TOML via `toml_edit` is the
first candidate), rather than flattening the configuration into anonymous data.

Lossy conversions must be explicit: emitting to a flat `KEY=VALUE` env format
collapses nesting, and some formats cannot represent every shape the value tree
allows. In those cases the emitter should warn clearly rather than silently drop
information.

This crate is a prerequisite for the CLI's write-out commands and for the shared
library's serialize path, so it comes first.

## 2. `tanzim` CLI — the pipeline as a standalone tool

**Status: 10% — early scaffolding in place.**

A single cross-platform command-line binary that exposes the whole pipeline to
people who are not writing Rust. It should let an operator load, parse, merge,
validate, emit, and serve configuration entirely from the shell, so the same
resolution logic an application relies on can be run and inspected outside that
application.

Planned capabilities:

- **load / parse / merge** — resolve one or more declared sources into a single
  merged configuration, with clear precedence.
- **validate** — check the resolved configuration against a schema and exit with a
  meaningful status code, printing caret-underlined errors that point at the exact
  source, line, and column.
- **emit** — write the resolved configuration out in a chosen format (built on
  `tanzim-emit`), including provenance annotations that record where each value
  came from.
- **serve** — expose the resolved configuration over HTTP, re-resolving on demand
  so other services can fetch it without embedding tanzim themselves.

The CLI must behave well as a Unix citizen: predictable exit codes, human-readable
diagnostics on stderr, machine-readable output on stdout, and cross-platform
support for Linux, macOS, and Windows.

## 3. `libtanzim` — shared library

**Status: not started.**

A shared library that makes tanzim callable from languages other than Rust. It
should expose a stable interface for loading, resolving, and emitting configuration
so that programs written in C, Go, Python, and similar can drive the pipeline
without reimplementing it.

The design needs to keep tanzim's defining feature — per-value source location —
reachable across the boundary, so consumers can not only read the resolved
configuration but also ask where any given value came from. A simpler
serialize-to-string path (built on `tanzim-emit`) should cover callers that only
want the final resolved document. The library must ship as a genuinely usable
artifact: a reviewed, versioned interface and cross-platform builds that are
exercised from a non-Rust program before the feature is considered done.
