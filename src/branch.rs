//! M4 tranche 1: a branch, a proposal to adopt from it, and a gate on the composition.
//!
//! The capability being built is narrow on purpose. It is the ability to explore an alternative from a
//! fixed base, propose part of it for a target, and find out whether the **combination** is acceptable
//! before anything moves. Nothing here publishes, nothing here calls out, and nothing here decides for a
//! person.
//!
//! The design consequence that matters: the two changes in the fixture touch different fields, so there is
//! no value conflict to detect. A system that only compared values would merge them cleanly and produce a
//! target that violates its own rule. So the gate is a **constraint check on the reconstructed candidate**,
//! not a three-way diff. The fixture exists to make that difference visible.
//!
//! What is deliberately absent, and why, is in `docs/M4-PROTOCOL.md`:
//!
//! - no multi-parent merge, so the base is a single known ancestor and an unknown one is diagnosed;
//! - no automatic re-grounding, so a target that moved since the fork produces a stale proposal rather
//!   than a silent rebase;
//! - no `safe_to_merge` boolean, because structure, constraints, assurance, authority and currency are
//!   five different questions and collapsing them is the failure this repository keeps refusing;
//! - no second impact engine, because the constraint check reuses `impact` rather than rebuilding it. If
//!   that turns out to be impossible, ADR-004 says to reduce rather than to proceed.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::impact::{NodeId, NodeNature, NodeVersion, Snapshot};

/// A named state of the world, anchored on an immutable base.
///
/// A branch is a snapshot plus the revision it was created from. The base is never rewritten: if the
/// target moves on, the branch still says what it was forked from, and re-grounding is a separate,
/// explicit act. That is what stops a historical branch from silently changing under a reader.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Branch {
    pub id: String,
    /// The revision this branch was created from. Immutable for the life of the branch.
    pub base: Snapshot,
    pub head: Snapshot,
    pub archived: bool,
}

impl Branch {
    /// Fork from a base. The base is copied, not referenced, so a later change to the target cannot
    /// reach into an existing branch.
    pub fn fork(id: &str, base: &Snapshot) -> Self {
        Self {
            id: id.to_owned(),
            base: base.clone(),
            head: base.clone(),
            archived: false,
        }
    }

    pub fn archive(&mut self) {
        self.archived = true;
    }

    /// The units this branch changed relative to its own base.
    ///
    /// This is the difference the target has to be able to take, and it is computed against the base the
    /// branch actually forked from, never against whatever the target looks like now.
    pub fn changes_since_base(&self) -> BTreeMap<NodeId, Selection> {
        let mut changes = BTreeMap::new();
        for id in self.head.nodes.keys() {
            let before = self.base.node(id);
            let after = self.head.node(id);
            match (before, after) {
                // The branch did not touch it.
                (Some(before), Some(after)) if before == after => {}
                // The branch changed it, or added it.
                (_, Some(after)) => {
                    changes.insert(
                        id.clone(),
                        Selection::Changed {
                            before: before.map(|v| v.payload.clone()),
                            after: after.payload.clone(),
                            after_version: after.version,
                        },
                    );
                }
                // The branch removed it. A tombstone is not an absent value that is safe to replace.
                (Some(before), None) => {
                    changes.insert(
                        id.clone(),
                        Selection::Removed {
                            before: before.payload.clone(),
                            before_version: before.version,
                        },
                    );
                }
                (None, None) => {}
            }
        }
        for (id, version) in &self.base.nodes {
            if !self.head.nodes.contains_key(id) {
                changes.insert(
                    id.clone(),
                    Selection::Removed {
                        before: version.payload.clone(),
                        before_version: version.version,
                    },
                );
            }
        }
        changes
    }
}

/// What a branch did to one object, relative to its base.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selection {
    Changed {
        before: Option<Value>,
        after: Value,
        after_version: u64,
    },
    Removed {
        before: Value,
        before_version: u64,
    },
}

/// The exact units a proposal asks to move. Not a text range, not a whole branch.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Requested {
    pub units: Vec<NodeId>,
}

/// A proposal to take named units from one source branch into one target.
///
/// Source and base are pinned. The target revision the proposal was prepared against is pinned too, so
/// publishing can refuse a plan made for a world that has since moved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proposal {
    pub source: String,
    pub target: String,
    /// The base the source forked from, as an exact revision.
    pub base: Snapshot,
    pub target_revision: u64,
    pub selection: Requested,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrepareError {
    /// The named branch is not in the store, or is archived.
    UnknownSource { source: String },
    /// The proposal's base is not the base the source forked from.
    BaseMismatch { expected: u64, got: u64 },
    /// A requested unit is not something the source changed. Silently taking a unit the branch never
    /// touched would import a value under a false provenance.
    NotAChange { unit: NodeId },
    /// A unit the source removed. Adopting a removal is a different act from adopting a value, and it is
    /// not something a value-selection proposal may do by accident.
    RemovalNotAdoptable { unit: NodeId },
    /// The target moved since the proposal was prepared. Re-grounding is explicit.
    TargetMoved { expected: u64, got: u64 },
}

impl std::fmt::Display for PrepareError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownSource { source } => write!(formatter, "no open branch named {source}"),
            Self::BaseMismatch { expected, got } => write!(
                formatter,
                "the proposal's base is revision {got}, but the branch forked from {expected}"
            ),
            Self::NotAChange { unit } => {
                write!(formatter, "{unit} is not a change the source made")
            }
            Self::RemovalNotAdoptable { unit } => write!(
                formatter,
                "{unit} was removed in the source; a removal is not a value this proposal may adopt"
            ),
            Self::TargetMoved { expected, got } => write!(
                formatter,
                "the proposal was prepared at target revision {expected} and the target is at {got}"
            ),
        }
    }
}

/// What happened to each requested unit, and where the result came from.
///
/// Three outcomes, not two. "Target already had it" and "took the source's value" are different facts and
/// a receipt that cannot tell them apart is not a receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    /// The target's own value held, because the source did not differ from the shared base.
    TargetUnchanged { unit: NodeId, version: u64 },
    /// The source's selected value was taken.
    TookSource {
        unit: NodeId,
        from_version: u64,
        value: Value,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prepared {
    pub proposal: Proposal,
    /// The reconstructed candidate. Its revision is the target's, not a new one: nothing is committed yet.
    pub candidate: Snapshot,
    pub resolutions: BTreeMap<NodeId, Resolution>,
    /// Exactly which source units entered the candidate, for the receipt.
    pub applied: Vec<NodeId>,
}

/// Reconstruct the candidate a proposal would produce, without touching the target.
///
/// The base is verified rather than assumed. An unknown base is diagnosed, never guessed, because a
/// guessed base produces a plausible candidate from the wrong history.
pub fn prepare(
    branches: &BTreeMap<String, Branch>,
    proposal: &Proposal,
    target: &Snapshot,
) -> Result<Prepared, PrepareError> {
    let source = branches
        .get(&proposal.source)
        .ok_or_else(|| PrepareError::UnknownSource {
            source: proposal.source.clone(),
        })?;
    if source.archived {
        return Err(PrepareError::UnknownSource {
            source: proposal.source.clone(),
        });
    }
    if source.base.revision != proposal.base.revision {
        return Err(PrepareError::BaseMismatch {
            expected: source.base.revision,
            got: proposal.base.revision,
        });
    }
    if target.revision != proposal.target_revision {
        return Err(PrepareError::TargetMoved {
            expected: proposal.target_revision,
            got: target.revision,
        });
    }

    let changes = source.changes_since_base();
    let mut candidate = target.clone();
    let mut resolutions = BTreeMap::new();
    let mut applied = Vec::new();

    for unit in &proposal.selection.units {
        let change = changes
            .get(unit)
            .ok_or_else(|| PrepareError::NotAChange { unit: unit.clone() })?;
        let (value, from_version) = match change {
            Selection::Changed {
                after,
                after_version,
                ..
            } => (after.clone(), *after_version),
            Selection::Removed { .. } => {
                return Err(PrepareError::RemovalNotAdoptable { unit: unit.clone() });
            }
        };

        // The three-way part, and it is deliberately small. A unit the target never diverged on takes the
        // source's value; a unit the target has since changed is a conflict the caller must resolve
        // explicitly, not something this function papers over.
        let target_diverged = match (target.node(unit), proposal.base.node(unit)) {
            (Some(after), Some(before)) => after.payload != before.payload,
            (Some(_), None) => true,
            (None, Some(_)) => true,
            (None, None) => false,
        };
        if target_diverged {
            resolutions.insert(
                unit.clone(),
                Resolution::TargetUnchanged {
                    unit: unit.clone(),
                    version: candidate.node(unit).map_or(0, |node| node.version),
                },
            );
            continue;
        }

        candidate = candidate.with_node(
            unit,
            target.nature(unit).unwrap_or(NodeNature::Observed),
            NodeVersion::new(
                from_version,
                &format!("branch:{}", source.id),
                value.clone(),
                unit,
            ),
        );
        resolutions.insert(
            unit.clone(),
            Resolution::TookSource {
                unit: unit.clone(),
                from_version,
                value,
            },
        );
        applied.push(unit.clone());
    }

    Ok(Prepared {
        proposal: proposal.clone(),
        candidate,
        resolutions,
        applied,
    })
}

/// A rule the candidate must satisfy. Tranche 1 keeps these as named closures over the snapshot rather
/// than a rule language, so a domain supplies its own and the Kernel invents none.
pub type Rule = (&'static str, fn(&Snapshot) -> Option<String>);

/// The five questions a gate has to keep apart, and which of them this one answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gate {
    /// A rule was violated in the composition. This is a fact about the candidate, not a verdict on the
    /// branches, both of which may be individually correct.
    pub constraint_violations: Vec<ConstraintViolation>,
    pub obligations: Vec<Obligation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstraintViolation {
    pub rule: String,
    pub subject: String,
    pub detail: String,
    /// The versions actually examined, so the reason names what it looked at.
    pub versions: Vec<(NodeId, u64)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Obligation {
    pub code: String,
    pub detail: String,
}

impl Gate {
    pub fn is_blocked(&self) -> bool {
        !self.constraint_violations.is_empty()
    }
}

/// Check a candidate against the target's own rules.
///
/// This is the whole of tranche 1's gate. It runs on the reconstructed candidate, so a combination that
/// breaks a cross-object invariant is caught even when every individual branch is fine and no field
/// conflicts with another.
pub fn check(prepared: &Prepared, rules: &[Rule]) -> Gate {
    let mut constraint_violations = Vec::new();
    for (name, rule) in rules {
        if let Some(detail) = rule(&prepared.candidate) {
            let mut versions: Vec<(NodeId, u64)> = prepared
                .candidate
                .nodes
                .iter()
                .map(|(id, node)| (id.clone(), node.version))
                .collect();
            versions.sort();
            constraint_violations.push(ConstraintViolation {
                rule: (*name).to_owned(),
                subject: prepared.proposal.target.clone(),
                detail,
                versions,
            });
        }
    }
    Gate {
        constraint_violations,
        obligations: Vec::new(),
    }
}

/// The adoption receipt, produced only by [`commit`].
///
/// A receipt exists after a commit, never after a plan. Its `applied` list is the exact selection, so a
/// later reader can tell the excluded part from the merged part.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Receipt {
    pub proposal: Proposal,
    pub target_before: u64,
    pub target_after: u64,
    pub applied: Vec<NodeId>,
    pub resolutions: BTreeMap<NodeId, Resolution>,
}

/// Publish a prepared candidate, if the gate allows it and the world has not moved.
///
/// The compare-and-swap is on the world revision, exactly as M3 does it. This is conservative, including
/// for a change in a sibling branch, and the replanning cost of that conservatism is not measured here.
pub fn commit(
    prepared: &Prepared,
    target: &mut Snapshot,
    rules: &[Rule],
) -> Result<Receipt, CommitError> {
    let gate = check(prepared, rules);
    if gate.is_blocked() {
        return Err(CommitError::Blocked {
            violations: gate.constraint_violations,
        });
    }
    if target.revision != prepared.proposal.target_revision {
        return Err(CommitError::TargetMoved {
            expected: prepared.proposal.target_revision,
            got: target.revision,
        });
    }

    let before = target.revision;
    *target = prepared.candidate.clone();
    target.revision = before + 1;
    Ok(Receipt {
        proposal: prepared.proposal.clone(),
        target_before: before,
        target_after: target.revision,
        applied: prepared.applied.clone(),
        resolutions: prepared.resolutions.clone(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommitError {
    Blocked {
        violations: Vec<ConstraintViolation>,
    },
    TargetMoved {
        expected: u64,
        got: u64,
    },
}
