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

/// The configured source leaves of a merge tree, in declared order.
///
/// Walks the [`MergePlan`] left-to-right, collecting every [`MergePlan::Source`] leaf so the load and
/// parse stages can resolve which sources to read regardless of how the tree was assembled (the
/// simple per-source builders and an explicit [`from_plan`](crate::Config::from_plan) tree share this
/// shape). [`MergePlan::Value`] leaves are skipped — they never go through load/parse.
pub(crate) fn leaves(plan: &MergePlan) -> Vec<&Source> {
    fn walk<'a>(node: &'a MergePlan, out: &mut Vec<&'a Source>) {
        match node {
            MergePlan::Source(source) => out.push(source),
            MergePlan::Value(_) => {}
            MergePlan::Merge { children, .. } => {
                for child in children {
                    walk(child, out);
                }
            }
        }
    }
    let mut out = Vec::new();
    walk(plan, &mut out);
    out
}

/// The top-level merger of a merge tree — the merger of a root [`Merge`](MergePlan::Merge) node, or
/// `None` for a bare [`Source`](MergePlan::Source) / [`Value`](MergePlan::Value) root (which has no
/// enclosing merger).
pub(crate) fn root_merger(plan: &MergePlan) -> Option<&dyn Merge> {
    match plan {
        MergePlan::Merge { merger, .. } => Some(merger.as_ref()),
        MergePlan::Source(_) | MergePlan::Value(_) => None,
    }
}

/// Append `child` to the children of a root [`Merge`](MergePlan::Merge) node — how the simple-fold
/// builders (`with_source` / `with_defaults` / `with_source_merged`) accumulate layers. A no-op for a
/// bare [`Source`](MergePlan::Source) / [`Value`](MergePlan::Value) root, which the simple builders
/// never construct.
pub(crate) fn push_child(plan: &mut MergePlan, child: MergePlan) {
    if let MergePlan::Merge { children, .. } = plan {
        children.push(child);
    }
}

/// Replace the merger of a root [`Merge`](MergePlan::Merge) node — how `with_merger` sets the global
/// merger. A no-op for a bare [`Source`](MergePlan::Source) / [`Value`](MergePlan::Value) root.
pub(crate) fn set_root_merger(plan: &mut MergePlan, merger: Box<dyn Merge + Send + Sync>) {
    if let MergePlan::Merge { merger: slot, .. } = plan {
        *slot = merger;
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
    /// An empty merged map.
    pub fn new() -> Self {
        Self {
            entries: std::collections::HashMap::new(),
        }
    }

    /// The number of merged entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether there are no merged entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The entry for `name` (`None` = the unnamed bucket), if present.
    pub fn get(&self, name: &Option<String>) -> Option<&Entry> {
        self.entries.get(name)
    }

    /// Mutable access to the entry for `name`, if present.
    pub fn get_mut(&mut self, name: &Option<String>) -> Option<&mut Entry> {
        self.entries.get_mut(name)
    }

    /// Insert (or replace) the entry for `name`, returning the previous entry if any.
    pub fn insert(&mut self, name: Option<String>, entry: Entry) -> Option<Entry> {
        self.entries.insert(name, entry)
    }

    /// Remove and return the entry for `name`, if present.
    pub fn remove(&mut self, name: &Option<String>) -> Option<Entry> {
        self.entries.remove(name)
    }

    /// Whether an entry for `name` is present.
    pub fn contains_key(&self, name: &Option<String>) -> bool {
        self.entries.contains_key(name)
    }

    /// The entry names, in arbitrary order.
    pub fn keys(&self) -> impl Iterator<Item = &Option<String>> {
        self.entries.keys()
    }

    /// Iterate over the entries by name, in arbitrary order.
    pub fn iter(&self) -> impl Iterator<Item = (&Option<String>, &Entry)> {
        self.entries.iter()
    }

    /// Iterate mutably over the entries by name, in arbitrary order.
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
