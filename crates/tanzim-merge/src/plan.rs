//! A composable **merge tree**.
//!
//! A [`MergePlan`] is either a source leaf (the payloads parsed from one [`Source`]) or a merge of
//! child plans under a chosen [`Merge`] strategy. This lets callers express arbitrary folds such as
//! `last_wins(deep(A, B), C)` — deep-merge `A` and `B`, then last-wins the result with `C` — instead
//! of a single flat merge over every payload.
//!
//! Build trees with the constructor helpers ([`src`], [`deep`], [`last_wins`], [`merge_with`]) and
//! run one with [`evaluate`] against the pipeline's parsed `(payload, value)` pairs. The simple
//! per-source pipeline builders lower into exactly this shape, so both paths share one evaluator.
//!
//! ```
//! use tanzim_merge::plan::{deep, last_wins, src};
//!
//! // last_wins( deep(base, overrides), secrets ):
//! // deep-merge the two files (both keys survive), then let the env source win any conflicts.
//! let plan = last_wins(vec![
//!     deep(vec![src("file:base.toml")?, src("file:overrides.toml")?]),
//!     src("env(prefix=SECRET_)")?,
//! ]);
//!
//! // `plan` is a `MergePlan` tree; hand it to a `Single`/`Multi` pipeline via `with_merge_plan`,
//! // or evaluate it directly with `plan::evaluate` against source-grouped payloads.
//! # let _ = plan;
//! # Ok::<(), tanzim_merge::Error>(())
//! ```

use crate::{DeepMerge, Error, LastWins, Merge, Merged};
use tanzim_load::Payload;
use tanzim_source::Source;
use tanzim_value::LocatedValue;

/// A configured source paired with the parsed `(payload, value)` pairs attributed to it.
///
/// Callers group their parsed payloads by the *configured* source that produced them (loaders narrow
/// a source's resource per payload, so a payload's own `source` is not directly comparable to the
/// configured one). [`evaluate`] resolves each [`MergePlan::Source`] leaf against these groups.
pub type SourceGroup = (Source, Vec<(Payload, LocatedValue)>);

/// A node in a merge tree: a source leaf, or a merge of child nodes under one strategy.
pub enum MergePlan {
    /// Leaf: the parsed payloads originating from this [`Source`], unmerged.
    Source(Source),
    /// Merge each child (in order) and fold the results with `merger`.
    Merge {
        merger: Box<dyn Merge>,
        children: Vec<MergePlan>,
    },
}

/// A source-leaf node, parsing `s` into a [`Source`] now.
///
/// Returns [`Error::Other`] wrapping the [`tanzim_source::ParseError`] if `s` is not a valid source.
pub fn src<S>(s: S) -> Result<MergePlan, Error>
where
    S: TryInto<Source, Error = tanzim_source::ParseError>,
{
    match s.try_into() {
        Ok(source) => Ok(MergePlan::Source(source)),
        Err(error) => Err(Error::Other(Box::new(error))),
    }
}

/// A merge node folding `children` with a custom `merger`.
pub fn merge_with(merger: impl Merge + 'static, children: Vec<MergePlan>) -> MergePlan {
    MergePlan::Merge {
        merger: Box::new(merger),
        children,
    }
}

/// A merge node folding `children` with [`DeepMerge`] (default array strategy).
pub fn deep(children: Vec<MergePlan>) -> MergePlan {
    merge_with(DeepMerge::new(), children)
}

/// A merge node folding `children` with [`LastWins`].
pub fn last_wins(children: Vec<MergePlan>) -> MergePlan {
    merge_with(LastWins, children)
}

/// Evaluate a merge tree against source-attributed parsed pairs, producing a [`Merged`] map.
///
/// Post-order: each source leaf resolves to the pairs in the [`SourceGroup`] whose configured
/// [`Source`] equals the leaf's; each merge node evaluates its children in order, concatenates their
/// results, and folds them with the node's merger. A bare source at the root is grouped with
/// [`LastWins`] (there is no enclosing merger to apply).
pub fn evaluate(plan: &MergePlan, groups: &[SourceGroup]) -> Result<Merged, Error> {
    match plan {
        MergePlan::Source(_) => LastWins.merge(&eval_tuples(plan, groups)?),
        MergePlan::Merge { merger, children } => {
            let mut concat = Vec::new();
            for child in children {
                concat.extend(eval_tuples(child, groups)?);
            }
            merger.merge(&concat)
        }
    }
}

/// Evaluate a node to a flat list of `(payload, value)` pairs so leaves and inner nodes compose
/// uniformly: a leaf yields its source group's pairs; an inner node folds its children then flattens
/// the resulting [`Merged`] back to one carrier pair per name-group.
fn eval_tuples(
    node: &MergePlan,
    groups: &[SourceGroup],
) -> Result<Vec<(Payload, LocatedValue)>, Error> {
    match node {
        MergePlan::Source(source) => Ok(groups
            .iter()
            .find(|(configured, _)| configured == source)
            .map(|(_, tuples)| tuples.clone())
            .unwrap_or_default()),
        MergePlan::Merge { merger, children } => {
            let mut concat = Vec::new();
            for child in children {
                concat.extend(eval_tuples(child, groups)?);
            }
            Ok(flatten(merger.merge(&concat)?))
        }
    }
}

/// Flatten a [`Merged`] back into carrier pairs — one per name-group, reusing that group's first
/// contributing payload (with its `maybe_name` preserved) so a parent merger re-groups by name.
fn flatten(merged: Merged) -> Vec<(Payload, LocatedValue)> {
    let mut out = Vec::with_capacity(merged.len());
    for (name, (payloads, value)) in merged {
        let Some(mut carrier) = payloads.into_iter().next() else {
            continue;
        };
        carrier.maybe_name = name;
        out.push((carrier, value));
    }
    out
}
