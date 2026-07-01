//! Custom loader backed by a closure.
//!
//! Use when configuration comes from a source that is not built-in.
//!
//! # Example
//!
//! ```
//! use tanzim_load::{closure::Closure, Error, Load, Payload, Source};
//!
//! # fn example() -> Result<(), tanzim_load::Error> {
//! let loader = Closure::new(
//!     "static",
//!     |source: Source| {
//!         Ok(vec![Payload {
//!             source: source.clone(),
//!             maybe_name: Some("demo".into()),
//!             maybe_format: Some("json".into()),
//!             content: br#"{"hello":"world"}"#.to_vec(),
//!         }])
//!     },
//!     "demo",
//! );
//! # Ok(())
//! # }
//! ```

use crate::{Error, Load, Payload, Source};

/// Boxed loader function: maps a [`Source`] to its loaded [`Payload`]s.
type LoaderFn = Box<dyn Fn(Source) -> Result<Vec<Payload>, Error> + Send + Sync + 'static>;

pub struct Closure {
    name: String,
    loader: LoaderFn,
    supported_source_list: Vec<String>,
}

impl Closure {
    pub fn new<N, L, S>(name: N, loader: L, source: S) -> Self
    where
        N: Into<String>,
        L: Fn(Source) -> Result<Vec<Payload>, Error> + Send + Sync + 'static,
        S: Into<String>,
    {
        Self {
            name: name.into(),
            loader: Box::new(loader),
            supported_source_list: vec![source.into()],
        }
    }

    pub fn with_name<N: AsRef<str>>(mut self, name: N) -> Self {
        self.name = name.as_ref().to_string();
        self
    }

    pub fn with_supported_source_list<S: AsRef<str>>(
        mut self,
        supported_source_list: Vec<S>,
    ) -> Self {
        self.supported_source_list = supported_source_list
            .into_iter()
            .map(|source| source.as_ref().to_string())
            .collect();
        self
    }
}

impl Load for Closure {
    fn name(&self) -> &str {
        self.name.as_str()
    }

    fn supported_source_list(&self) -> Vec<String> {
        self.supported_source_list.clone()
    }

    fn load(&self, source: Source) -> Result<Vec<Payload>, Error> {
        (self.loader)(source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tanzim_source::SourceBuilder;

    #[test]
    fn closure_loader_delegates_to_function() {
        let loader = Closure::new(
            "custom",
            |source: Source| {
                let resource = source.resource().to_string();
                Ok(vec![Payload {
                    source,
                    maybe_name: Some("demo".into()),
                    maybe_format: Some("txt".into()),
                    content: resource.into_bytes(),
                }])
            },
            "custom",
        );
        assert_eq!(loader.name(), "custom");
        assert_eq!(loader.supported_source_list(), vec!["custom".to_string()]);
        let source = SourceBuilder::new()
            .with_source("custom")
            .with_resource("hello")
            .build()
            .unwrap();
        let loaded = loader.load(source).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].content, b"hello");
    }
}
