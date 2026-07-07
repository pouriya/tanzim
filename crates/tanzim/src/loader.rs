//! Configuration loaders: turn a [`Source`] into raw
//! [`Payload`]s. Re-exports [`tanzim_load`]; see that crate for the [`Load`] trait and the
//! `env` / `file` / `http` / `closure` loaders.

pub use tanzim_load::*;
