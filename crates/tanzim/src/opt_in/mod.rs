//! Opinionated, batteries-included facades built on top of [`crate::pipeline`].
//!
//! Where [`crate::pipeline`] is unopinionated (you pick every loader, parser, and merger),
//! `opt_in` provides ready-made profiles that make sensible default choices for a common style of
//! configuration. Each submodule is a self-contained profile; today there is only [`config`],
//! which mirrors the ergonomics of the [`config`](https://docs.rs/config) crate. Future profiles
//! (e.g. a `figment`-style layered facade) would live here as sibling modules.

pub mod config;
