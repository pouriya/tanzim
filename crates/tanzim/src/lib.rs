#![doc = include_str!("../README.md")]
#![doc(test(no_crate_inject))]

//! # tanzim
//!
//! Load, parse, and merge configuration from declarative configuration sources.
//!
//! The pipeline lives under [`pipeline`] ([`pipeline::single::Single`] /
//! [`pipeline::multi::Multi`]); [`opt_in`] holds opinionated facades built on top of it.
//!
//! Workspace crates:
//!
//! - [`source`] — [`tanzim_source`] ([`tanzim_source::Source`])
//! - [`loader`] — [`tanzim_load`] ([`tanzim_load::Load`])
//! - [`parser`] — [`tanzim_parse`] ([`tanzim_parse::Parse`])
//! - [`merger`] — [`tanzim_merge`] ([`tanzim_merge::Merge`])
//! - [`validator`] — [`tanzim_validate`]
//! - [`value`] — [`tanzim_value`] ([`tanzim_value::Value`], [`tanzim_value::LocatedValue`])

pub use tanzim_load as loader;
pub use tanzim_merge as merger;
pub use tanzim_parse as parser;
pub use tanzim_source as source;
pub use tanzim_validate as validator;
pub use tanzim_value as value;

pub mod ext {
    //! Re-exported dependency crates.

    pub extern crate tanzim_load;
    pub extern crate tanzim_merge;
    pub extern crate tanzim_parse;
    pub extern crate tanzim_source;
    pub extern crate tanzim_validate;
}

mod logging;

pub mod opt_in;
pub mod pipeline;
