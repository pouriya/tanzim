//! The configuration pipeline: **load → parse → merge → (unify) → validate**.
//!
//! Two entry points share the same stages but differ in their result shape:
//!
//! - [`single::Single`] collapses every source into one unified configuration value.
//! - [`multi::Multi`] keeps a map of named entries (`None` = the unnamed bucket).
//!
//! Construct either with `default()` (all feature-enabled loaders + parsers) or `empty()` (nothing
//! registered), add sources (and optionally a merger — it defaults to [`merger::LastWins`]), then
//! `run()` / `try_deserialize()`.
//!
//! Each submodule re-exports everything needed to build a pipeline, so
//! `use tanzim::pipeline::single::*;` (or `::multi::*`) is enough on its own.

use crate::loader;
use crate::merger;
use crate::merger::plan::MergePlan;
use crate::parser;
use crate::source::Source;

pub mod multi;
pub mod single;

/// How a pipeline's merge tree is configured.
///
/// The simple source builders (`add_source` / `with_source_merged` / `with_merger`) and the advanced
/// `with_merge_plan` are mutually exclusive: the simple builders accumulate into a root [`Merge`]
/// node, while `with_merge_plan` supplies a complete tree the caller built themselves. Mixing them is
/// a configuration error — either let the pipeline build the plan, or build it yourself, not both.
///
/// [`Merge`]: MergePlan::Merge
pub(crate) enum Plan {
    /// Built from the simple builders: the root is always a [`MergePlan::Merge`] whose `children`
    /// are the sources in declared order and whose merger is the global merger (defaulting to
    /// [`merger::LastWins`]). `merger_set` records whether a global merger was explicitly chosen, so
    /// a still-pristine simple plan can be replaced by [`with_merge_plan`](single::Single::with_merge_plan).
    Simple { root: MergePlan, merger_set: bool },
    /// A complete tree supplied via `with_merge_plan`.
    Explicit(MergePlan),
}

impl Plan {
    /// A pristine simple plan: an empty root that folds with [`merger::LastWins`].
    pub(crate) fn simple() -> Self {
        Plan::Simple {
            root: MergePlan::Merge {
                merger: Box::new(merger::LastWins),
                children: Vec::new(),
            },
            merger_set: false,
        }
    }

    pub(crate) fn is_explicit(&self) -> bool {
        matches!(self, Plan::Explicit(_))
    }

    /// Whether an explicit merge plan may still replace this one — i.e. no source or merger has been
    /// configured through the simple builders yet.
    pub(crate) fn is_pristine(&self) -> bool {
        matches!(
            self,
            Plan::Simple { root: MergePlan::Merge { children, .. }, merger_set: false }
                if children.is_empty()
        )
    }

    /// Append a source (or per-source pre-merge) child to the simple root. The caller guarantees the
    /// plan is not explicit.
    pub(crate) fn push_child(&mut self, child: MergePlan) {
        if let Plan::Simple {
            root: MergePlan::Merge { children, .. },
            ..
        } = self
        {
            children.push(child);
        }
    }

    /// Set the global merger on the simple root. The caller guarantees the plan is not explicit.
    pub(crate) fn set_merger(&mut self, merger: Box<dyn merger::Merge>) {
        if let Plan::Simple {
            root: MergePlan::Merge { merger: slot, .. },
            merger_set,
        } = self
        {
            *slot = merger;
            *merger_set = true;
        }
    }

    /// Replace a pristine simple plan with an explicit tree. The caller guarantees [`is_pristine`].
    ///
    /// [`is_pristine`]: Self::is_pristine
    pub(crate) fn set_explicit(&mut self, tree: MergePlan) {
        *self = Plan::Explicit(tree);
    }

    /// The tree to evaluate.
    pub(crate) fn tree(&self) -> &MergePlan {
        match self {
            Plan::Simple { root, .. } => root,
            Plan::Explicit(tree) => tree,
        }
    }

    /// The global merger chosen via `with_merger`, if any — the root merger of a simple plan whose
    /// merger was set. `None` for the default or for an explicit tree (which has no single global
    /// merger).
    pub(crate) fn configured_merger(&self) -> Option<&dyn merger::Merge> {
        match self {
            Plan::Simple {
                root: MergePlan::Merge { merger, .. },
                merger_set: true,
            } => Some(merger.as_ref()),
            _ => None,
        }
    }

    /// The configured source leaves, in declared order.
    pub(crate) fn leaves(&self) -> Vec<&Source> {
        fn walk<'a>(node: &'a MergePlan, out: &mut Vec<&'a Source>) {
            match node {
                MergePlan::Source(source) => out.push(source),
                MergePlan::Merge { children, .. } => {
                    for child in children {
                        walk(child, out);
                    }
                }
            }
        }
        let mut out = Vec::new();
        walk(self.tree(), &mut out);
        out
    }
}

/// Attribute parsed payloads back to the configured source that produced them, preserving declared
/// source order, so a [`MergePlan`] can resolve its [`Source`] leaves.
///
/// Loaders narrow a source's *resource* per payload (e.g. `file:dir` → `file:dir/app.toml`) while
/// preserving scheme and options, so a payload's own `source` is not equal to the configured one.
/// Each payload is matched to the configured source it was narrowed from — identical apart from its
/// resource, whose resource contains the payload's, most specific (longest) winning. Unmatched
/// payloads (which should not arise from `run`) are dropped.
pub(crate) fn group_by_source(
    sources: &[&Source],
    parsed: &[Parsed],
) -> Vec<merger::plan::SourceGroup> {
    let mut groups: Vec<merger::plan::SourceGroup> =
        sources.iter().map(|s| ((*s).clone(), Vec::new())).collect();
    for item in parsed {
        let payload_source = &item.payload().source;
        let mut best: Option<(usize, usize)> = None;
        for (index, configured) in sources.iter().enumerate() {
            let resource = configured.resource();
            let same_but_resource =
                payload_source.clone().with_resource(resource.to_string()) == **configured;
            let narrowed = payload_source.resource();
            let resource_contains = resource.is_empty()
                || resource == narrowed
                || narrowed.starts_with(&format!("{resource}/"))
                || narrowed.starts_with(&format!("{resource}\\"));
            if same_but_resource
                && resource_contains
                && best.is_none_or(|(_, best_len)| resource.len() > best_len)
            {
                best = Some((index, resource.len()));
            }
        }
        if let Some((index, _)) = best {
            groups[index]
                .1
                .push((item.payload().clone(), item.value().clone()));
        }
    }
    groups
}

/// A loaded payload paired with the value tree produced by parsing it.
///
/// Fields are private; access them through [`payload`](Self::payload) / [`value`](Self::value)
/// and their `_mut` variants.
#[derive(Debug, Clone, PartialEq)]
pub struct Parsed {
    payload: loader::Payload,
    value: parser::LocatedValue,
}

impl Parsed {
    /// Pair a payload with the value produced by parsing it.
    pub fn new(payload: loader::Payload, value: parser::LocatedValue) -> Self {
        Self { payload, value }
    }

    pub fn payload(&self) -> &loader::Payload {
        &self.payload
    }

    pub fn payload_mut(&mut self) -> &mut loader::Payload {
        &mut self.payload
    }

    pub fn value(&self) -> &parser::LocatedValue {
        &self.value
    }

    pub fn value_mut(&mut self) -> &mut parser::LocatedValue {
        &mut self.value
    }

    /// Split into the payload and its parsed value.
    pub fn into_parts(self) -> (loader::Payload, parser::LocatedValue) {
        (self.payload, self.value)
    }
}

/// One merged entry: the payloads that contributed to it and the combined value.
///
/// Fields are private; access them through [`payloads`](Self::payloads) / [`value`](Self::value)
/// and their `_mut` variants.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    payloads: Vec<loader::Payload>,
    value: parser::LocatedValue,
}

impl Entry {
    /// Build an entry from its contributing payloads and combined value.
    pub fn new(payloads: Vec<loader::Payload>, value: parser::LocatedValue) -> Self {
        Self { payloads, value }
    }

    pub fn payloads(&self) -> &[loader::Payload] {
        &self.payloads
    }

    pub fn payloads_mut(&mut self) -> &mut Vec<loader::Payload> {
        &mut self.payloads
    }

    pub fn value(&self) -> &parser::LocatedValue {
        &self.value
    }

    pub fn value_mut(&mut self) -> &mut parser::LocatedValue {
        &mut self.value
    }

    /// Split into the contributing payloads and the combined value.
    pub fn into_parts(self) -> (Vec<loader::Payload>, parser::LocatedValue) {
        (self.payloads, self.value)
    }
}

/// Merged configuration keyed by entry name (`None` = the unnamed bucket).
///
/// Fields are private; navigate it through the map-like accessors ([`get`](Self::get),
/// [`iter`](Self::iter), [`keys`](Self::keys), …).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Merged {
    entries: std::collections::HashMap<Option<String>, Entry>,
}

impl Merged {
    pub fn new() -> Self {
        Self {
            entries: std::collections::HashMap::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn get(&self, name: &Option<String>) -> Option<&Entry> {
        self.entries.get(name)
    }

    pub fn get_mut(&mut self, name: &Option<String>) -> Option<&mut Entry> {
        self.entries.get_mut(name)
    }

    pub fn insert(&mut self, name: Option<String>, entry: Entry) -> Option<Entry> {
        self.entries.insert(name, entry)
    }

    pub fn remove(&mut self, name: &Option<String>) -> Option<Entry> {
        self.entries.remove(name)
    }

    pub fn contains_key(&self, name: &Option<String>) -> bool {
        self.entries.contains_key(name)
    }

    pub fn keys(&self) -> impl Iterator<Item = &Option<String>> {
        self.entries.keys()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Option<String>, &Entry)> {
        self.entries.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&Option<String>, &mut Entry)> {
        self.entries.iter_mut()
    }

    /// Wrap the raw map returned by a [`crate::merger::Merge`] into [`Entry`]-keyed form.
    pub(crate) fn from_raw(raw: crate::merger::Merged) -> Self {
        let mut entries = std::collections::HashMap::with_capacity(raw.len());
        for (name, (payloads, value)) in raw {
            entries.insert(name, Entry::new(payloads, value));
        }
        Self { entries }
    }
}
