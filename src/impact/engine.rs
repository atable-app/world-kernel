//! The M3 engine: plan, re-evaluate, stop propagation, explain.
//!
//! Two entry points, and the difference between them is structural rather than
//! a promise.
//!
//! `full_recompute` is the oracle. It receives a snapshot and a target list. It
//! has no parameter through which an `EvaluationRecord`, an index or an
//! invalidation function could arrive, so it cannot call the incremental
//! strategy even by accident.
//!
//! `revise` is the incremental path. It decides reuse from the recorded read set
//! before running anything, so a consumer whose consumed facets did not move is
//! not re-evaluated at all.
//!
//! Reuse refreshes no evidence by magic. Content can be identical while its
//! provenance, its UNI evidence, its availability or its authority condition has
//! moved, which is why a node can be `reusable` on content and still carry an
//! obligation.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use super::model::{
    Authority, CapturedRead, Coverage, Currency, Disposition, EVALUATOR_ENGINE_VERSION, Error,
    EvaluationRecord, Explanation, Facet, Finding, HistoryState, IMPACT_PLAN_SCHEMA, NodeId,
    NodeNature, NodeVersion, Obligation, Outcome, Profile, QueryRead, ReadBinding, Snapshot,
    WorkType,
};

/// Bounds so a pathological graph ends in an explicit status instead of a
/// truncated success.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub max_nodes: usize,
    pub max_evaluations: usize,
    pub max_obligations: usize,
    pub max_explanation_path: usize,
    /// Reads one evaluator may perform, so a runaway evaluation ends in an
    /// explicit status rather than consuming the run.
    pub max_reads: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_nodes: 10_000,
            max_evaluations: 100_000,
            max_obligations: 10_000,
            max_explanation_path: 64,
            max_reads: 10_000,
        }
    }
}

/// What a run read while it worked. The journal is the only way an evaluator
/// touches the outside world, so a closed profile is enforceable at this seam.
pub trait ReadJournal {
    fn read(&mut self, subject: &str, facet: Facet) -> Result<Option<Value>, Error>;
    /// Records that the run looked for something that was not there.
    fn read_absence(&mut self, subject: &str) -> Result<(), Error>;
    /// Reads a set. The composition of the set is a dependency in its own right.
    fn read_query(&mut self, query: &Query) -> Result<Vec<(NodeId, u64)>, Error>;
    /// The branch a conditional read took, so a switch publishes a new read set.
    fn branch(&mut self) -> Option<String>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Query {
    pub query: String,
    pub query_version: u64,
    pub scope: String,
    /// The members a query resolves over, resolved by the caller so a query stays
    /// a set-level declaration rather than an ambient search.
    pub members: Vec<NodeId>,
}

/// A trusted, deterministic domain evaluator.
pub trait Evaluator: Send + Sync {
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    /// The profile the evaluator is granted by the consumer's configuration. An
    /// evaluator cannot award itself a stronger one.
    fn granted_profile(&self) -> Profile;
    fn evaluate(
        &self,
        input: &Value,
        journal: &mut dyn ReadJournal,
    ) -> Result<(Value, Profile), Error>;
}

/// The consumer's allowed configuration. Trust and the right to run an evaluator
/// come from here, never from a manifest in the data.
#[derive(Debug, Clone, Default)]
pub struct TrustConfiguration {
    /// Evaluator name and version to the strongest profile the consumer grants.
    pub granted: BTreeMap<(String, String), Profile>,
}

impl TrustConfiguration {
    pub fn grant(mut self, name: &str, version: &str, profile: Profile) -> Self {
        self.granted
            .insert((name.to_owned(), version.to_owned()), profile);
        self
    }

    fn allows(&self, evaluator: &str, version: &str) -> Option<Profile> {
        self.granted
            .get(&(evaluator.to_owned(), version.to_owned()))
            .copied()
    }
}

/// A journal that reads a snapshot and records what it handed out. Used by both
/// paths; the difference is whether the reader may fall back to a recorded value.
struct SnapshotJournal<'a> {
    snapshot: &'a Snapshot,
    reads: BTreeMap<(NodeId, String), (Facet, ReadBinding, String, bool)>,
    queries: Vec<QueryRead>,
    order: Vec<NodeId>,
    branches: Vec<String>,
    limits: Limits,
}

impl<'a> SnapshotJournal<'a> {
    fn new(snapshot: &'a Snapshot, limits: Limits) -> Self {
        Self {
            snapshot,
            reads: BTreeMap::new(),
            queries: Vec::new(),
            order: Vec::new(),
            branches: Vec::new(),
            limits,
        }
    }

    fn record_facet(
        &mut self,
        subject: &str,
        facet: &Facet,
        value: Option<Value>,
        binding: ReadBinding,
    ) -> Result<Option<Value>, Error> {
        if self.reads.len() >= self.limits.max_reads {
            return Err(Error::LimitReached(format!(
                "read budget of {} reached",
                self.limits.max_reads
            )));
        }
        let fingerprint = facet.fingerprint(value.as_ref());
        let present = value.is_some();
        self.reads.insert(
            (subject.to_owned(), format!("{facet:?}")),
            (facet.clone(), binding, fingerprint, present),
        );
        if !self.order.iter().any(|seen| seen == subject) {
            self.order.push(subject.to_owned());
        }
        Ok(value)
    }
}

impl ReadJournal for SnapshotJournal<'_> {
    fn read(&mut self, subject: &str, facet: Facet) -> Result<Option<Value>, Error> {
        let version = self.snapshot.node(subject);
        let value = version.and_then(|version| facet.value(&version.payload));
        let binding = match version {
            Some(version) => ReadBinding::Pinned {
                version: version.version,
            },
            None => ReadBinding::Current,
        };
        self.record_facet(subject, &facet, value, binding)
    }

    fn read_absence(&mut self, subject: &str) -> Result<(), Error> {
        self.record_facet(
            subject,
            &Facet::Whole,
            self.snapshot
                .live(subject)
                .map(|version| version.payload.clone()),
            ReadBinding::Current,
        )?;
        Ok(())
    }

    fn read_query(&mut self, query: &Query) -> Result<Vec<(NodeId, u64)>, Error> {
        let members = QueryRead::resolve(self.snapshot, &query.members);
        self.queries.push(QueryRead {
            query: query.query.clone(),
            query_version: query.query_version,
            scope: query.scope.clone(),
            declared_members: query.members.clone(),
            collection: QueryRead::collection_fingerprint(&members),
            fingerprint: QueryRead::collection_fingerprint(&members),
            comparator: super::model::FACET_COMPARATOR_VERSION.to_owned(),
        });
        Ok(members)
    }

    fn branch(&mut self) -> Option<String> {
        self.branches.last().cloned()
    }
}

/// A node that has been evaluated, with its record.
#[derive(Debug, Clone)]
pub struct Evaluated {
    pub record: EvaluationRecord,
    pub value: Value,
}

/// Runs one evaluator against a snapshot, honouring the trust configuration.
fn evaluate_one(
    snapshot: &Snapshot,
    id: &str,
    evaluators: &BTreeMap<String, Box<dyn Evaluator>>,
    trust: &TrustConfiguration,
    limits: Limits,
) -> Result<Evaluated, Error> {
    let Some(evaluator) = evaluators.get(id) else {
        return Err(Error::NoEvaluator(id.to_owned()));
    };
    let Some(version) = snapshot.node(id) else {
        return Err(Error::NoEvaluator(id.to_owned()));
    };
    let allowed = trust
        .allows(evaluator.name(), evaluator.version())
        .unwrap_or(Profile::Opaque);
    if evaluator.granted_profile().exceeds(allowed) {
        return Err(Error::ProfileNotGranted {
            evaluator: evaluator.name().to_owned(),
            claimed: evaluator.granted_profile().as_str().to_owned(),
            allowed: allowed.as_str().to_owned(),
        });
    }

    let mut journal = SnapshotJournal::new(snapshot, limits);
    let (output, claimed) = evaluator.evaluate(&version.payload, &mut journal)?;

    let reads: Vec<CapturedRead> = journal
        .order
        .iter()
        .flat_map(|subject| reads_of(&journal, subject))
        .collect();

    Ok(Evaluated {
        record: EvaluationRecord {
            target: id.to_owned(),
            target_version: version.version,
            target_nature: snapshot.nature(id).unwrap_or(NodeNature::Derived),
            world_revision: snapshot.revision,
            evaluator: evaluator.name().to_owned(),
            evaluator_version: evaluator.version().to_owned(),
            engine: EVALUATOR_ENGINE_VERSION.to_owned(),
            profile: claimed,
            reads,
            queries: journal.queries.clone(),
            output: output.clone(),
            assurance_refs: Vec::new(),
            outcome: Outcome::Completed,
            executed: true,
        },
        value: output,
    })
}

/// Every captured read of one subject, in a stable order.
fn reads_of(journal: &SnapshotJournal<'_>, subject: &str) -> Vec<CapturedRead> {
    journal
        .reads
        .iter()
        .filter(|((key, _), _)| key == subject)
        .map(|(_, (facet, binding, fingerprint, present))| CapturedRead {
            subject: subject.to_owned(),
            binding: binding.clone(),
            facet: facet.clone(),
            fingerprint: fingerprint.clone(),
            comparator: super::model::FACET_COMPARATOR_VERSION.to_owned(),
            present: *present,
        })
        .collect()
}

/// The oracle. Full recomputation with no recorded state anywhere.
///
/// Its signature is the independence guarantee: there is no way to hand it an
/// `EvaluationRecord`, an index or an invalidation function.
pub fn full_recompute(
    snapshot: &Snapshot,
    targets: &[NodeId],
    evaluators: &BTreeMap<String, Box<dyn Evaluator>>,
    trust: &TrustConfiguration,
    limits: Limits,
) -> Result<BTreeMap<NodeId, Evaluated>, Error> {
    if targets.len() > limits.max_nodes {
        return Err(Error::LimitReached(format!(
            "{} targets above {}",
            targets.len(),
            limits.max_nodes
        )));
    }
    let mut out = BTreeMap::new();
    for target in targets {
        let evaluated = evaluate_one(snapshot, target, evaluators, trust, limits)?;
        out.insert(target.clone(), evaluated);
    }
    Ok(out)
}

/// Why a recorded read still holds, or not, against a newer snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadOutcome {
    /// The consumed value is identical. The consumer may be reused.
    Unchanged,
    /// The consumed value moved. The consumer must be re-evaluated.
    Changed { before: String, after: String },
    /// The record's own definition moved, so no comparison is possible.
    DefinitionMoved,
}

fn check_read(read: &CapturedRead, snapshot: &Snapshot) -> ReadOutcome {
    if read.comparator != super::model::FACET_COMPARATOR_VERSION {
        return ReadOutcome::DefinitionMoved;
    }
    let current: Option<&NodeVersion> = match &read.binding {
        // A pinned reference addresses immutable content, so its own value
        // cannot move. Its support can, which is why this is not a silent reuse.
        ReadBinding::Pinned { version } => snapshot
            .node(&read.subject)
            .filter(|node| node.version == *version)
            .or_else(|| snapshot.node(&read.subject)),
        ReadBinding::Current => snapshot.node(&read.subject),
    };
    let value = current.and_then(|version| read.facet.value(&version.payload));
    let after = read.facet.fingerprint(value.as_ref());
    if after == read.fingerprint {
        ReadOutcome::Unchanged
    } else {
        ReadOutcome::Changed {
            before: read.fingerprint.clone(),
            after,
        }
    }
}

fn check_query(read: &QueryRead, snapshot: &Snapshot) -> ReadOutcome {
    if read.comparator != super::model::FACET_COMPARATOR_VERSION {
        return ReadOutcome::DefinitionMoved;
    }
    // The initial profile does not re-derive the query. It re-resolves the same
    // declared member set and notices whether that set moved, which is the
    // conservative behaviour the protocol allows.
    let members = QueryRead::resolve(snapshot, &read.declared_members);
    let after = QueryRead::collection_fingerprint(&members);
    if after == read.collection {
        ReadOutcome::Unchanged
    } else {
        ReadOutcome::Changed {
            before: read.collection.clone(),
            after,
        }
    }
}

/// A computed impact plan. It performs no external effect and publishes nothing.
#[derive(Debug, Clone, PartialEq)]
pub struct Revised {
    pub base_revision: u64,
    pub target_revision: u64,
    pub findings: BTreeMap<NodeId, Finding>,
    pub obligations: Vec<Obligation>,
    pub evaluations: usize,
    pub reused: usize,
}

impl Revised {
    pub fn why(&self, node: &str) -> Option<&Explanation> {
        self.findings.get(node).map(|finding| &finding.explanation)
    }

    pub fn why_reused(&self, node: &str) -> Option<&Explanation> {
        self.findings
            .get(node)
            .filter(|finding| {
                matches!(
                    finding.disposition,
                    Disposition::UnchangedInScope
                        | Disposition::ReusedAfterCheck
                        | Disposition::RecomputedSame
                )
            })
            .map(|finding| &finding.explanation)
    }
}

/// Incremental revision from a previous set of records to a newer snapshot.
#[allow(clippy::too_many_arguments)]
pub fn revise(
    base: &Snapshot,
    target: &Snapshot,
    recorded: &BTreeMap<NodeId, EvaluationRecord>,
    requested: &[NodeId],
    evaluators: &BTreeMap<String, Box<dyn Evaluator>>,
    trust: &TrustConfiguration,
    limits: Limits,
) -> Result<Revised, Error> {
    if base.world != target.world {
        return Err(Error::StalePlan {
            planned: target.revision,
            current: base.revision,
        });
    }
    if requested.len() > limits.max_nodes {
        return Err(Error::LimitReached(format!(
            "{} targets above {}",
            requested.len(),
            limits.max_nodes
        )));
    }

    // A dependency path is carried for explanation only. An executable cycle is
    // refused, not hidden by cutting an edge.
    let order = topological_order(recorded, requested)?;

    let frontier = impact_frontier(&order, recorded, target, trust, evaluators);

    // A working view starts as the target snapshot and receives each recomputed
    // value as it is produced, so a consumer reads the fresh value of the
    // dependency it was just given. Reading the target snapshot directly would
    // hand a chain the stale value of the node above it, and a chain could then
    // never agree with a full recompute.
    let mut working = target.clone();

    let mut findings: BTreeMap<NodeId, Finding> = BTreeMap::new();
    let mut obligations: Vec<Obligation> = Vec::new();
    let mut current_records: BTreeMap<NodeId, EvaluationRecord> = recorded.clone();
    let mut evaluations = 0usize;
    let mut reused = 0usize;

    for id in &order {
        let nature = target.nature(id).unwrap_or(NodeNature::Derived);
        if !super::is_recomputable(Some(nature)) {
            // An observed input or a recorded decision is not something the
            // engine recomputes. A decision whose declared support moved becomes
            // work for a human; nothing is substituted.
            let support = declared_support(target, id);
            let moved_support: Vec<&NodeId> = support
                .iter()
                .filter(|node| !frontier.get(*node).map(Vec::is_empty).unwrap_or(true))
                .collect();
            let _ = &moved_support;
            let (disposition, code, obligation) = if moved_support.is_empty() {
                (
                    Disposition::UnchangedInScope,
                    "no_change_reaches_this_object",
                    None,
                )
            } else {
                (
                    Disposition::NeedsHumanReview,
                    "recorded_decision_support_moved",
                    Some(Obligation::new(
                        id,
                        target.revision,
                        "recorded_decision_support_moved",
                        WorkType::HumanReview,
                        Profile::ClosedDeterministic,
                        "a human decides again; the engine does not substitute a new decision",
                    )),
                )
            };
            let mut explanation = Explanation::new(
                code,
                id,
                if moved_support.is_empty() {
                    "nothing that could reach this object moved"
                } else {
                    "the support this decision relied on moved; the decision itself is untouched"
                },
            );
            explanation.path = moved_support.iter().map(|node| (*node).clone()).collect();
            if let Some(obligation) = obligation {
                obligations.push(obligation);
            }
            findings.insert(
                id.clone(),
                Finding {
                    subject: id.clone(),
                    history: HistoryState::Recorded {
                        revision: base.revision,
                    },
                    currency: if moved_support.is_empty() {
                        Currency::Reusable
                    } else {
                        Currency::NeedsRevalidation
                    },
                    business_verdict: None,
                    authority: if moved_support.is_empty() {
                        Authority::Authorized
                    } else {
                        Authority::Unresolved
                    },
                    coverage: Coverage::of(Profile::Opaque),
                    disposition,
                    output: target.node(id).map(|node| node.payload.clone()),
                    explanation,
                },
            );
            continue;
        }

        let Some(previous) = recorded.get(id) else {
            // No record at all: an unknown starting point is reported as unknown
            // rather than assumed fresh.
            findings.insert(
                id.clone(),
                Finding {
                    subject: id.clone(),
                    history: HistoryState::Unrecorded,
                    currency: Currency::Unknown,
                    business_verdict: None,
                    authority: Authority::Unresolved,
                    coverage: Coverage::Opaque,
                    disposition: Disposition::Unknown,
                    output: None,
                    explanation: Explanation::new(
                        "no_recorded_evaluation",
                        id,
                        "an evaluation must exist before currency can be judged",
                    ),
                },
            );
            obligations.push(Obligation::new(
                id,
                target.revision,
                "no_recorded_evaluation",
                WorkType::PureRecompute,
                Profile::Opaque,
                "a first evaluation at this revision",
            ));
            continue;
        };

        // A moved evaluator or comparator definition invalidates the record
        // itself. An old fingerprint must not be reused as if its definition had
        // not changed.
        let evaluator_moved = evaluators
            .get(id)
            .map(|evaluator| {
                evaluator.name() != previous.evaluator
                    || evaluator.version() != previous.evaluator_version
            })
            .unwrap_or(true);

        let mut read_moved: Option<(String, ReadOutcome)> = None;
        for read in &previous.reads {
            let outcome = check_read(read, &working);
            if outcome != ReadOutcome::Unchanged {
                read_moved = Some((read.subject.clone(), outcome));
                break;
            }
        }
        let mut query_moved: Option<(String, ReadOutcome)> = None;
        if read_moved.is_none() {
            for query in &previous.queries {
                let outcome = check_query(query, &working);
                if outcome != ReadOutcome::Unchanged {
                    query_moved = Some((query.query.clone(), outcome));
                    break;
                }
            }
        }

        if !evaluator_moved && read_moved.is_none() && query_moved.is_none() {
            // Nothing the consumer consumed moved. It is not re-evaluated at all.
            // Whether a change could reach it at all is a separate statement, and
            // the two are reported differently.
            reused += 1;
            let reached = !frontier.get(id).map(Vec::is_empty).unwrap_or(true);
            findings.insert(
                id.clone(),
                Finding {
                    subject: id.clone(),
                    history: HistoryState::Recorded {
                        revision: base.revision,
                    },
                    currency: Currency::Reusable,
                    business_verdict: previous.business_verdict(),
                    authority: Authority::Authorized,
                    coverage: Coverage::of(previous.profile),
                    disposition: if reached {
                        Disposition::ReusedAfterCheck
                    } else {
                        Disposition::UnchangedInScope
                    },
                    output: Some(previous.output.clone()),
                    explanation: {
                        let mut explanation = Explanation::new(
                            if reached {
                                "consumed_facets_unchanged"
                            } else {
                                "no_change_reaches_this_object"
                            },
                            id,
                            "every consumed facet and collection still matches the record",
                        );
                        explanation.versions = vec![format!("{}@{}", id, previous.target_version)];
                        explanation.path = if reached {
                            frontier.get(id).cloned().unwrap_or_default()
                        } else {
                            path_to(id, &order)
                        };
                        explanation
                    },
                },
            );
            current_records.insert(id.clone(), previous.clone());
            continue;
        }

        if !evaluators.contains_key(id) {
            obligations.push(Obligation::new(
                id,
                target.revision,
                "no_evaluator_for_changed_consumer",
                WorkType::MissingReference,
                Profile::Opaque,
                "an evaluator must be registered before this node can be re-evaluated",
            ));
            findings.insert(
                id.clone(),
                Finding {
                    subject: id.clone(),
                    history: HistoryState::Recorded {
                        revision: base.revision,
                    },
                    currency: Currency::Unsupported,
                    business_verdict: None,
                    authority: Authority::Unresolved,
                    coverage: Coverage::Opaque,
                    disposition: Disposition::Unsupported,
                    output: None,
                    explanation: Explanation::new(
                        "no_evaluator_registered",
                        id,
                        "an evaluator must be registered before this node can be re-evaluated",
                    ),
                },
            );
            continue;
        };

        if evaluations >= limits.max_evaluations {
            return Err(Error::LimitReached(format!(
                "evaluation budget of {} reached",
                limits.max_evaluations
            )));
        }
        evaluations += 1;

        let evaluated = evaluate_one(&working, id, evaluators, trust, limits)?;
        let same = evaluated.record.output_fingerprint() == previous.output_fingerprint();
        let trigger = if evaluator_moved {
            Some(format!(
                "evaluator {}@{}",
                previous.evaluator, previous.evaluator_version
            ))
        } else {
            read_moved
                .as_ref()
                .map(|(subject, outcome)| format!("{subject}: {outcome:?}"))
                .or_else(|| {
                    query_moved
                        .as_ref()
                        .map(|(query, outcome)| format!("{query}: {outcome:?}"))
                })
        };
        let path = path_to(id, &order);
        let mut explanation = if same {
            Explanation::new(
                "recomputed_same",
                id,
                "the consumed values moved but the produced value did not",
            )
        } else {
            Explanation::new(
                "recomputed_changed",
                id,
                "the produced value moved and its consumers must be examined",
            )
        };
        explanation.versions = vec![format!("{}@{}", id, evaluated.record.target_version)];
        explanation.trigger = trigger;
        explanation.path = path;

        // A node is re-evaluated, not simply overwritten. A decision record is
        // never replaced by the engine; it becomes something to re-examine.
        let disposition = match target.nature(id) {
            Some(NodeNature::Decision) if !same => Disposition::NeedsHumanReview,
            _ if same => Disposition::RecomputedSame,
            _ => Disposition::RecomputedChanged,
        };
        if disposition == Disposition::NeedsHumanReview {
            obligations.push(Obligation::new(
                id,
                target.revision,
                "recorded_decision_support_moved",
                WorkType::HumanReview,
                Profile::ClosedDeterministic,
                "a human decides again; the engine does not substitute a new decision",
            ));
        }

        working.nodes.insert(
            id.clone(),
            NodeVersion::new(
                evaluated.record.target_version,
                &format!("derived/{}", evaluated.record.evaluator),
                evaluated.value.clone(),
                &evaluated.record.evaluator,
            ),
        );
        let mut record = evaluated.record.clone();
        record.executed = true;
        findings.insert(
            id.clone(),
            Finding {
                subject: id.clone(),
                history: HistoryState::Recorded {
                    revision: base.revision,
                },
                currency: if same {
                    Currency::Reusable
                } else {
                    Currency::NeedsRevalidation
                },
                business_verdict: previous.business_verdict(),
                authority: Authority::Authorized,
                coverage: Coverage::strongest_of(previous.profile, record.profile),
                disposition,
                output: Some(evaluated.value.clone()),
                explanation,
            },
        );
        current_records.insert(id.clone(), record);
    }

    Ok(Revised {
        base_revision: base.revision,
        target_revision: target.revision,
        findings,
        obligations,
        evaluations,
        reused,
    })
}

impl EvaluationRecord {
    /// The domain verdict is carried, never computed here.
    fn business_verdict(&self) -> Option<String> {
        self.output
            .get("verdict")
            .and_then(Value::as_str)
            .map(str::to_owned)
    }
}

/// A deterministic order with dependencies before consumers, and executable
/// cycles refused rather than broken.
fn topological_order(
    recorded: &BTreeMap<NodeId, EvaluationRecord>,
    requested: &[NodeId],
) -> Result<Vec<NodeId>, Error> {
    let mut order: Vec<NodeId> = Vec::new();
    let mut state: BTreeMap<NodeId, u8> = BTreeMap::new();

    fn visit(
        id: &str,
        recorded: &BTreeMap<NodeId, EvaluationRecord>,
        state: &mut BTreeMap<NodeId, u8>,
        order: &mut Vec<NodeId>,
    ) -> Result<(), Error> {
        match state.get(id) {
            Some(2) => return Ok(()),
            Some(1) => return Err(Error::ExecutableCycle(id.to_owned())),
            _ => {}
        }
        state.insert(id.to_owned(), 1);
        if let Some(record) = recorded.get(id) {
            for read in &record.reads {
                if recorded.contains_key(&read.subject) && read.subject != id {
                    visit(&read.subject, recorded, state, order)?;
                }
            }
        }
        state.insert(id.to_owned(), 2);
        order.push(id.to_owned());
        Ok(())
    }

    for id in requested {
        visit(id, recorded, &mut state, &mut order)?;
    }
    Ok(order)
}

/// The support a recorded decision declares.
///
/// This is a relationship that explains, not a computational dependency. It
/// never causes a recomputation of the decision itself, and the engine never
/// substitutes a new decision for the recorded one.
fn declared_support(snapshot: &Snapshot, id: &str) -> Vec<NodeId> {
    snapshot
        .node(id)
        .and_then(|version| version.payload.get("supports"))
        .and_then(Value::as_array)
        .map(|members| {
            members
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// A node is directly concerned when something it actually consumed moved, or
/// when the definition that produced it moved.
fn directly_concerned(record: &EvaluationRecord, target: &Snapshot) -> bool {
    let moved_definition = record.target_version
        != target
            .node(&record.target)
            .map(|n| n.version)
            .unwrap_or(u64::MAX)
        || record.engine.as_str() != super::model::EVALUATOR_ENGINE_VERSION;
    if moved_definition {
        return true;
    }
    record
        .reads
        .iter()
        .any(|read| check_read(read, target) != ReadOutcome::Unchanged)
        || record
            .queries
            .iter()
            .any(|query| check_query(query, target) != ReadOutcome::Unchanged)
}

/// The impact frontier, propagated towards the requested targets while keeping
/// explaining paths.
///
/// A node no change can reach is out of scope, which is a different statement
/// from a node that was reached and held. Both are reported, neither is implied
/// by the other.
fn impact_frontier(
    order: &[NodeId],
    recorded: &BTreeMap<NodeId, EvaluationRecord>,
    target: &Snapshot,
    trust: &TrustConfiguration,
    evaluators: &BTreeMap<String, Box<dyn Evaluator>>,
) -> BTreeMap<NodeId, Vec<NodeId>> {
    let mut frontier: BTreeMap<NodeId, Vec<NodeId>> = BTreeMap::new();
    for id in order {
        let mut reasons: Vec<NodeId> = Vec::new();
        if let Some(record) = recorded.get(id) {
            if directly_concerned(record, target) {
                reasons.push(id.clone());
            }
            let definition_moved = evaluators
                .get(id)
                .map(|evaluator| {
                    evaluator.name() != record.evaluator
                        || evaluator.version() != record.evaluator_version
                })
                .unwrap_or(true);
            if definition_moved {
                reasons.push(id.clone());
            }
            // Propagation is transitive through *reached* nodes only. A subject
            // that nothing can reach is not a reason, otherwise the frontier
            // would fill with nodes no change touched.
            let reached =
                |node: &str| -> bool { !frontier.get(node).map(Vec::is_empty).unwrap_or(true) };
            for read in &record.reads {
                if read.subject != *id && reached(&read.subject) {
                    reasons.push(read.subject.clone());
                }
            }
            for support in declared_support(target, id) {
                if support != *id && reached(&support) {
                    reasons.push(support);
                }
            }
        }
        reasons.sort();
        reasons.dedup();
        frontier.insert(id.clone(), reasons);
    }
    let _ = trust;
    frontier
}

fn path_to(id: &str, order: &[NodeId]) -> Vec<NodeId> {
    let index = order.iter().position(|node| node == id).unwrap_or(0);
    if index == 0 {
        vec![id.to_owned()]
    } else {
        let mut path = vec![id.to_owned()];
        path.extend(order[..index].iter().rev().cloned());
        path.truncate(8);
        path
    }
}

/// Publication is a conservative compare-and-swap on the whole world revision.
///
/// Any concurrent change forces replanning. Finer per-read-set validation may
/// come later and is not assumed safe here.
pub fn assert_publishable(plan: &Revised, current_world_revision: u64) -> Result<(), Error> {
    if plan.target_revision != current_world_revision {
        return Err(Error::StalePlan {
            planned: plan.target_revision,
            current: current_world_revision,
        });
    }
    Ok(())
}

pub fn plan_schema() -> &'static str {
    IMPACT_PLAN_SCHEMA
}

/// A node that is not a computation and is therefore never recomputed by the
/// engine, whatever its payload happens to look like.
pub fn is_recomputable(nature: Option<NodeNature>) -> bool {
    matches!(nature, Some(NodeNature::Derived))
}

/// The set of nodes a snapshot declares as derived, which is what an incremental
/// pass may consider recomputing.
pub fn derivable(snapshot: &Snapshot) -> BTreeSet<NodeId> {
    snapshot
        .natures
        .iter()
        .filter(|(_, nature)| **nature == NodeNature::Derived)
        .map(|(id, _)| id.clone())
        .collect()
}
