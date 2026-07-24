//! Configuration merging: fold the parsed sources into one tree per entry name.
//!
//! Re-exports [`tanzim_merge`] (the [`Merge`] trait, the [`LastWins`] / [`DeepMerge`] strategies,
//! the [`Merged`] raw merge-stage map, and the [`plan`] fold-tree constructors), and adds the
//! facade's consumer-facing [`Entries`] / [`EntryName`] types plus the pipeline's internal
//! merge-plan plumbing.
//!
//! Custom [`Merge`] implementors return [`Merged`] (a `HashMap` keyed by `Option<String>`).
//! Application code consumes [`Entries`] from [`Pipeline`](crate::pipeline::Pipeline) /
//! [`Config`](crate::Config) instead — same grouping, without exposing `Option` keys.

pub use tanzim_merge::*;

use crate::entry::Entry;
use crate::parser::Parsed;
use crate::source::Source;
use std::fmt;
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

/// Owned name of a merged configuration entry.
///
/// [`Root`](Self::Root) is the unnamed bucket (payloads with `maybe_name = None`);
/// [`Named`](Self::Named) is an explicit entry name. [`Display`](fmt::Display) prints
/// `<root>` or the name.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EntryName {
    /// The unnamed / root entry bucket.
    Root,
    /// A named entry.
    Named(String),
}

impl EntryName {
    /// The root (unnamed) entry name.
    pub fn root() -> Self {
        Self::Root
    }

    /// A named entry.
    pub fn named(name: impl Into<String>) -> Self {
        Self::Named(name.into())
    }

    #[cfg(feature = "validate-schema")]
    pub(crate) fn from_option(name: Option<String>) -> Self {
        match name {
            None => Self::Root,
            Some(name) => Self::Named(name),
        }
    }

    pub(crate) fn into_option(self) -> Option<String> {
        match self {
            Self::Root => None,
            Self::Named(name) => Some(name),
        }
    }

    /// Borrowed view of this name.
    pub fn as_ref(&self) -> EntryNameRef<'_> {
        match self {
            Self::Root => EntryNameRef::Root,
            Self::Named(name) => EntryNameRef::Named(name.as_str()),
        }
    }
}

impl fmt::Display for EntryName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Root => write!(f, "<root>"),
            Self::Named(name) => write!(f, "{name}"),
        }
    }
}

/// Borrowed name of a merged configuration entry.
///
/// [`Display`](fmt::Display) prints `<root>` or the name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EntryNameRef<'a> {
    /// The unnamed / root entry bucket.
    Root,
    /// A named entry.
    Named(&'a str),
}

impl EntryNameRef<'_> {
    /// Clone into an owned [`EntryName`].
    pub fn to_owned(self) -> EntryName {
        match self {
            Self::Root => EntryName::Root,
            Self::Named(name) => EntryName::Named(name.to_string()),
        }
    }
}

impl fmt::Display for EntryNameRef<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Root => write!(f, "<root>"),
            Self::Named(name) => write!(f, "{name}"),
        }
    }
}

fn option_key_ref(name: &Option<String>) -> EntryNameRef<'_> {
    match name {
        None => EntryNameRef::Root,
        Some(name) => EntryNameRef::Named(name.as_str()),
    }
}

/// Named configuration entries produced by the merge stage (or by
/// [`Pipeline::try_deserialize`](crate::pipeline::Pipeline::try_deserialize)).
///
/// Fields are private. Prefer [`root`](Self::root) / [`named`](Self::named) over thinking about
/// the internal `Option<String>` key. The default type parameter is [`Entry`]; deserializing yields
/// `Entries<T>`.
///
/// Custom [`Merge`] implementors still return the raw [`Merged`] map; [`Pipeline`](crate::pipeline::Pipeline)
/// / [`Config`](crate::Config) convert that into `Entries` after the merge stage.
#[derive(Debug, Clone, PartialEq)]
pub struct Entries<T = Entry> {
    entries: std::collections::HashMap<Option<String>, T>,
}

impl<T> Default for Entries<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Entries<T> {
    /// An empty entries map.
    pub fn new() -> Self {
        Self {
            entries: std::collections::HashMap::new(),
        }
    }

    /// The number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether there are no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The root (unnamed) entry, if present.
    pub fn root(&self) -> Option<&T> {
        self.entries.get(&None)
    }

    /// Mutable access to the root (unnamed) entry, if present.
    pub fn root_mut(&mut self) -> Option<&mut T> {
        self.entries.get_mut(&None)
    }

    /// The entry named `name`, if present.
    pub fn named(&self, name: &str) -> Option<&T> {
        self.entries.get(&Some(name.to_string()))
    }

    /// Mutable access to the entry named `name`, if present.
    pub fn named_mut(&mut self, name: &str) -> Option<&mut T> {
        self.entries.get_mut(&Some(name.to_string()))
    }

    /// The entry for `name`, if present.
    pub fn get(&self, name: &EntryName) -> Option<&T> {
        match name {
            EntryName::Root => self.root(),
            EntryName::Named(name) => self.named(name),
        }
    }

    /// Mutable access to the entry for `name`, if present.
    pub fn get_mut(&mut self, name: &EntryName) -> Option<&mut T> {
        match name {
            EntryName::Root => self.root_mut(),
            EntryName::Named(name) => self.named_mut(name),
        }
    }

    /// Insert (or replace) the root entry, returning the previous value if any.
    pub fn insert_root(&mut self, value: T) -> Option<T> {
        self.entries.insert(None, value)
    }

    /// Insert (or replace) a named entry, returning the previous value if any.
    pub fn insert_named(&mut self, name: impl Into<String>, value: T) -> Option<T> {
        self.entries.insert(Some(name.into()), value)
    }

    /// Insert (or replace) the entry for `name`, returning the previous value if any.
    pub fn insert(&mut self, name: EntryName, value: T) -> Option<T> {
        self.entries.insert(name.into_option(), value)
    }

    /// Remove and return the root entry, if present.
    pub fn remove_root(&mut self) -> Option<T> {
        self.entries.remove(&None)
    }

    /// Remove and return the entry named `name`, if present.
    pub fn remove_named(&mut self, name: &str) -> Option<T> {
        self.entries.remove(&Some(name.to_string()))
    }

    /// Remove and return the entry for `name`, if present.
    pub fn remove(&mut self, name: &EntryName) -> Option<T> {
        match name {
            EntryName::Root => self.remove_root(),
            EntryName::Named(name) => self.remove_named(name),
        }
    }

    /// Whether the root entry is present.
    pub fn contains_root(&self) -> bool {
        self.entries.contains_key(&None)
    }

    /// Whether an entry named `name` is present.
    pub fn contains_named(&self, name: &str) -> bool {
        self.entries.contains_key(&Some(name.to_string()))
    }

    /// Whether an entry for `name` is present.
    pub fn contains(&self, name: &EntryName) -> bool {
        match name {
            EntryName::Root => self.contains_root(),
            EntryName::Named(name) => self.contains_named(name),
        }
    }

    /// The entry names, in arbitrary order.
    pub fn keys(&self) -> impl Iterator<Item = EntryNameRef<'_>> {
        self.entries.keys().map(option_key_ref)
    }

    /// Iterate over the entries by name, in arbitrary order.
    pub fn iter(&self) -> impl Iterator<Item = (EntryNameRef<'_>, &T)> {
        self.entries
            .iter()
            .map(|(name, value)| (option_key_ref(name), value))
    }

    /// Iterate mutably over the entries by name, in arbitrary order.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (EntryNameRef<'_>, &mut T)> {
        self.entries
            .iter_mut()
            .map(|(name, value)| (option_key_ref(name), value))
    }
}

impl Entries<Entry> {
    /// Wrap the raw map returned by a [`Merge`] into [`Entry`]-valued form.
    pub(crate) fn from_raw(raw: Merged) -> Self {
        let mut entries = std::collections::HashMap::with_capacity(raw.len());
        for (name, (payloads, value)) in raw {
            entries.insert(name, Entry::new(payloads, value));
        }
        Self { entries }
    }
}
