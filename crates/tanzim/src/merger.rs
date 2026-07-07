//! Configuration merging: fold the parsed sources into one tree per entry name.
//!
//! Re-exports [`tanzim_merge`] (the [`Merge`] trait, the [`LastWins`] / [`DeepMerge`] strategies,
//! and the [`plan`] fold-tree constructors), and adds the facade's own [`Merged`] output type
//! plus the pipeline's internal merge-plan plumbing.

pub use tanzim_merge::*;

use crate::entry::Entry;
use crate::parser::Parsed;
use crate::source::Source;
use tanzim_merge::plan::{MergePlan, SourceGroup};

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
    /// [`LastWins`]). `merger_set` records whether a global merger was explicitly chosen, so
    /// a still-pristine simple plan can be replaced by `with_merge_plan`.
    Simple { root: MergePlan, merger_set: bool },
    /// A complete tree supplied via `with_merge_plan`.
    Explicit(MergePlan),
}

impl Plan {
    /// A pristine simple plan: an empty root that folds with [`LastWins`].
    pub(crate) fn simple() -> Self {
        Plan::Simple {
            root: MergePlan::Merge {
                merger: Box::new(LastWins),
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
    pub(crate) fn set_merger(&mut self, merger: Box<dyn Merge>) {
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
    pub(crate) fn configured_merger(&self) -> Option<&dyn Merge> {
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
pub(crate) fn group_by_source(sources: &[&Source], parsed: &[Parsed]) -> Vec<SourceGroup> {
    let mut groups: Vec<SourceGroup> = sources.iter().map(|s| ((*s).clone(), Vec::new())).collect();
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

    /// Wrap the raw map returned by a [`Merge`] into [`Entry`]-keyed form.
    pub(crate) fn from_raw(raw: tanzim_merge::Merged) -> Self {
        let mut entries = std::collections::HashMap::with_capacity(raw.len());
        for (name, (payloads, value)) in raw {
            entries.insert(name, Entry::new(payloads, value));
        }
        Self { entries }
    }
}
