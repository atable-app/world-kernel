//! System A3: a competent application-level incremental engine.
//!
//! It is given the same fixture, the same evaluators, the same snapshots, the same trust scope and the
//! same ordered target list as the World Kernel core. It chooses its own representation.
//!
//! The mechanism is the obvious competent one and nothing more: a cache entry per target holding the read
//! set and the produced value, re-fingerprinting the reads against the current snapshot, reuse when they
//! all hold and re-execution when one does not. There is no portable graph, no five dimensions, no
//! obligations, no explanations, no coverage profiles and no publication step.
//!
//! It is not a strawman. Its cost is the thing under measurement: if the portable machinery in `src/impact`
//! buys nothing an application cannot already do at a similar price, the reduction is the honest result.

use std::collections::BTreeMap;

use super::impact_core::{
    Evaluator, Facet, Limits, NodeId, ReadJournal, Snapshot, TrustConfiguration,
};
use serde_json::Value;

/// What one target consumed, in the application's own terms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppCacheEntry {
    pub reads: Vec<(String, String)>,
    pub value: Value,
}

#[derive(Debug, Clone, Default)]
pub struct AppCache {
    entries: BTreeMap<NodeId, AppCacheEntry>,
}

impl AppCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn entry(&self, target: &str) -> Option<&AppCacheEntry> {
        self.entries.get(target)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AppOutcome {
    pub values: BTreeMap<NodeId, Value>,
    /// Evaluator invocations. Reuse is not an invocation.
    pub executions: usize,
    pub reused: usize,
}

/// Runs the application mechanism over an ordered target list.
///
/// The order is a parameter because an application knows its own dependency order. The core derives the
/// order from recorded reads. Both are informed; neither is given an answer.
/// System A3: the application, judging the snapshot it was given.
pub fn revise_app(
    snapshot: &Snapshot,
    targets: &[NodeId],
    cache: &mut AppCache,
    evaluators: &BTreeMap<NodeId, Box<dyn Evaluator>>,
    trust: &TrustConfiguration,
    limits: Limits,
) -> Result<AppOutcome, super::impact_core::Error> {
    revise_against(snapshot, targets, cache, evaluators, trust, limits, &[])
}

/// System B3: the same application, told which references UNI declared stale.
///
/// This wrapper is the whole of B3. It adds no mechanism, so anything B3 does
/// differently from A3 is attributable to the declaration rather than to
/// something the application invented.
pub fn revise_b3(
    snapshot: &Snapshot,
    targets: &[NodeId],
    cache: &mut AppCache,
    evaluators: &BTreeMap<NodeId, Box<dyn Evaluator>>,
    trust: &TrustConfiguration,
    limits: Limits,
    stale_assurances: &[String],
) -> Result<AppOutcome, super::impact_core::Error> {
    revise_against(
        snapshot,
        targets,
        cache,
        evaluators,
        trust,
        limits,
        stale_assurances,
    )
}

fn revise_against(
    snapshot: &Snapshot,
    targets: &[NodeId],
    cache: &mut AppCache,
    evaluators: &BTreeMap<NodeId, Box<dyn Evaluator>>,
    trust: &TrustConfiguration,
    limits: Limits,
    // References a third party declared no longer current. An entry that relied on
    // one of them is not reusable, whatever the snapshot says. The application does
    // not re-check a declaration it was handed, which is the point of B3.
    stale_assurances: &[String],
) -> Result<AppOutcome, super::impact_core::Error> {
    let mut values: BTreeMap<NodeId, Value> = BTreeMap::new();
    let mut executions = 0usize;
    let mut reused = 0usize;
    let mut working = snapshot.clone();

    for target in targets {
        // The application caches on whole-value identity. It has no notion of a
        // consumed facet, so a change to any part of a node invalidates a consumer
        // of that node. That coarser granularity is the mechanism under test, not
        // a handicap: a team that needed finer reuse would have to build it.
        let reusable = cache.entries.get(target).is_some_and(|entry| {
            // A stale reference invalidates the entry on the authority of the
            // declaration, not of a re-read. The application does not re-check it,
            // which is the point: it consumes the answer instead of computing one.
            let relied_on_stale = entry
                .reads
                .iter()
                .any(|(subject, _)| stale_assurances.contains(subject));
            !relied_on_stale
                && entry.reads.iter().all(|(subject, fingerprint)| {
                    match working.live(subject) {
                        Some(version) => {
                            Facet::Whole.fingerprint(Some(&version.payload)) == *fingerprint
                        }
                        // A read that recorded an absence still holds if it is absent.
                        None => Facet::Whole.fingerprint(None) == *fingerprint,
                    }
                })
        });

        if reusable {
            let entry = cache.entries.get(target).expect("the entry was just found");
            values.insert(target.clone(), entry.value.clone());
            working.nodes.insert(
                target.clone(),
                super::impact_core::NodeVersion::new(1, "app/cached", entry.value.clone(), target),
            );
            reused += 1;
            continue;
        }

        if executions >= limits.max_evaluations {
            return Err(super::impact_core::Error::LimitReached(format!(
                "evaluation budget of {} reached",
                limits.max_evaluations
            )));
        }
        let Some(evaluator) = evaluators.get(target) else {
            return Err(super::impact_core::Error::NoEvaluator(target.clone()));
        };
        if trust
            .allows(evaluator.name(), evaluator.version())
            .is_none()
        {
            return Err(super::impact_core::Error::NoEvaluator(target.clone()));
        }

        let mut journal = AppJournal {
            snapshot: &working,
            reads: Vec::new(),
            max_reads: limits.max_reads,
        };
        let (value, _profile) = evaluator.evaluate(&Value::Null, &mut journal)?;
        executions += 1;

        let entry = AppCacheEntry {
            reads: journal.reads,
            value: value.clone(),
        };
        cache.entries.insert(target.clone(), entry);
        values.insert(target.clone(), value.clone());
        working.nodes.insert(
            target.clone(),
            super::impact_core::NodeVersion::new(1, "app/derived", value, target),
        );
    }

    Ok(AppOutcome {
        values,
        executions,
        reused,
    })
}

/// A journal that records whole-value reads. The application compares whole
/// values; it has no facet concept, which is one of the things under measurement.
struct AppJournal<'a> {
    snapshot: &'a Snapshot,
    reads: Vec<(String, String)>,
    max_reads: usize,
}

impl ReadJournal for AppJournal<'_> {
    fn read(
        &mut self,
        subject: &str,
        facet: Facet,
    ) -> Result<Option<Value>, super::impact_core::Error> {
        if self.reads.len() >= self.max_reads {
            return Err(super::impact_core::Error::LimitReached(
                "read budget reached".into(),
            ));
        }
        let version = self.snapshot.node(subject);
        let value = version.and_then(|version| facet.value(&version.payload));
        // Whole-value identity, deliberately: the application has no facet
        // comparator to record.
        self.reads.push((
            subject.to_owned(),
            Facet::Whole.fingerprint(version.map(|version| &version.payload)),
        ));
        Ok(value)
    }

    fn read_absence(&mut self, subject: &str) -> Result<(), super::impact_core::Error> {
        self.reads
            .push((subject.to_owned(), Facet::Whole.fingerprint(None)));
        Ok(())
    }

    fn read_query(
        &mut self,
        query: &super::impact_core::Query,
    ) -> Result<Vec<(NodeId, u64)>, super::impact_core::Error> {
        let members = super::impact_core::QueryRead::resolve(self.snapshot, &query.members);
        self.reads.push((
            query.query.clone(),
            super::impact_core::QueryRead::collection_fingerprint(&members),
        ));
        Ok(members)
    }

    fn branch(&mut self) -> Option<String> {
        None
    }
}
