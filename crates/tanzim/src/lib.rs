#![doc = include_str!("../README.md")]
#![doc(test(no_crate_inject))]

//! # tanzim
//!
//! Load, parse, and merge configuration from declarative configuration sources.
//!
//! Most users want [`Config`] — the single-configuration pipeline. Add a couple of sources and
//! [`try_deserialize`](Config::try_deserialize) into your own type. When your sources describe
//! several *named* configurations, reach for [`pipeline::Pipeline`] instead
//! (built with [`Pipeline::builder`](pipeline::Pipeline::builder) /
//! [`Pipeline::from_plan`](pipeline::Pipeline::from_plan)).
//!
//! Each stage is a module that re-exports its backing crate and adds the facade's own types:
//!
//! - [`source`] — the source-string format ([`Source`], [`tanzim_source`])
//! - [`loader`] — load a source into payloads ([`tanzim_load`])
//! - [`parser`] — parse a payload into a value tree ([`parser::Parsed`], [`tanzim_parse`])
//! - [`merger`] — fold sources together ([`merger::Merged`], the [`merge plan`](merger::plan), [`tanzim_merge`])
//! - [`validator`] — schema validation ([`tanzim_validate`])
//! - [`value`] — the core value types ([`value::Value`], [`value::LocatedValue`], [`tanzim_value`])
//! - [`entry`] — one merged [`Entry`](entry::Entry)

pub mod config;
pub mod entry;
pub mod loader;
pub mod merger;
pub mod parser;
pub mod pipeline;
pub mod source;
pub mod validator;
pub mod value;

mod logging;

pub use config::{BuilderState, Config, ConfigBuilder, ConfigStages, Plan, Sources};
pub use source::Source;
