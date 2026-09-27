//! M3 data model: versioned nodes, captured reads, evaluation records, the five
//! dimensions, obligations and explained dispositions.
//!
//! Two rules shape every type here.
//!
//! First, there is no global `valid` flag. History, currency, business verdict,
//! current authority and coverage are separate, because a result can be
//! historically true and no longer usable without either being false.
//!
//! Second, a dependency is what a run actually consumed, not what somebody
//! mentioned. A captured read names the facet it consumed, the binding it used
//! and the comparator version that fingerprinted it.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::model::digest_of_bytes;

/// The identity of a logical object inside one world and scope.
pub type NodeId = String;

pub const IMPACT_PLAN_SCHEMA: &str = "world-impact-plan/v0-experimental";
pub const FACET_COMPARATOR_VERSION: &str = "facet-canonical-json/1";
pub const EVALUATOR_ENGINE_VERSION: &str = "world-impact/sequential/0";

/// Three natures are enough in the core. Domain notions such as a budget or an
/// Apple intent belong to an adapter, not here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeNature {
    /// Observed or imported from outside. Not derivable, therefore not
    /// "recomputable" merely because its payload happens to be JSON.
    Observed,
    /// The recorded output of a trusted deterministic evaluation.
    Derived,
    /// A human decision, recorded verbatim and never rewritten by the engine.
    Decision,
}

/// One immutable version of an object's content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NodeVersion {
    pub version: u64,
    pub payload_type: String,
    pub payload: serde_json::Value,
    /// The producer or originating activity, kept as a reference. The producer's
    /// private conversation is never an implicit condition of continuation.
    pub origin: String,
    /// A tombstone. Deletion is an explicit transition, never a silently missing
    /// reference.
    pub removed: bool,
}

impl NodeVersion {
    pub fn new(version: u64, payload_type: &str, payload: serde_json::Value, origin: &str) -> Self {
        Self {
            version,
            payload_type: payload_type.to_owned(),
            payload,
            origin: origin.to_owned(),
            removed: false,
        }
    }

    pub fn tombstone(version: u64, origin: &str) -> Self {
        Self {
            version,
            payload_type: "tombstone/v0".to_owned(),
            payload: serde_json::Value::Null,
            origin: origin.to_owned(),
            removed: true,
        }
    }

    pub fn digest(&self) -> String {
        digest_of_bytes(
            serde_json::to_string(&self.payload)
                .unwrap_or_default()
                .as_bytes(),
        )
    }
}

/// A closed set of immutable node versions, addressed by one world revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Snapshot {
    pub world: String,
    pub revision: u64,
    pub nodes: BTreeMap<NodeId, NodeVersion>,
    pub natures: BTreeMap<NodeId, NodeNature>,
}

impl Snapshot {
    pub fn new(world: &str, revision: u64) -> Self {
        Self {
            world: world.to_owned(),
            revision,
            nodes: BTreeMap::new(),
            natures: BTreeMap::new(),
        }
    }

    pub fn with_node(mut self, id: &str, nature: NodeNature, version: NodeVersion) -> Self {
        self.natures.insert(id.to_owned(), nature);
        self.nodes.insert(id.to_owned(), version);
        self
    }

    pub fn node(&self, id: &str) -> Option<&NodeVersion> {
        self.nodes.get(id)
    }

    pub fn nature(&self, id: &str) -> Option<NodeNature> {
        self.natures.get(id).copied()
    }

    /// A node that exists and is not a tombstone.
    pub fn live(&self, id: &str) -> Option<&NodeVersion> {
        self.node(id).filter(|version| !version.removed)
    }
}

/// The part of a value a run actually consumed.
///
/// A narrower projection than the whole value is a deterministic, typed and
/// versioned function. A similarity score is not an equality comparator and is
/// deliberately not representable here.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Facet {
    /// The whole canonical value.
    Whole,
    /// One named field of an object payload. Deterministic and typed by the
    /// payload's declared type.
    Field(String),
}

impl Facet {
    /// The value this facet exposes, or `None` when the facet is not present.
    pub fn value(&self, payload: &serde_json::Value) -> Option<serde_json::Value> {
        match self {
            Facet::Whole => Some(payload.clone()),
            Facet::Field(name) => payload.get(name).cloned(),
        }
    }

    /// The fingerprint of a consumed value, including absence.
    pub fn fingerprint(&self, value: Option<&serde_json::Value>) -> String {
        let material = serde_json::json!({
            "comparator": FACET_COMPARATOR_VERSION,
            "facet": self,
            "value": value,
        });
        digest_of_bytes(
            serde_json::to_string(&material)
                .unwrap_or_default()
                .as_bytes(),
        )
    }
}

/// How a run referred to the value it consumed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "binding", rename_all = "snake_case")]
pub enum ReadBinding {
    /// A historical reference to one immutable version. Its content cannot move,
    /// so its fingerprint never changes on its own.
    Pinned { version: u64 },
    /// The current version of the node in the snapshot being read.
    Current,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CapturedRead {
    pub subject: NodeId,
    pub binding: ReadBinding,
    pub facet: Facet,
    /// Fingerprint of the consumed value, or of its recorded absence.
    pub fingerprint: String,
    pub comparator: String,
    /// False when the run observed that nothing was there. A later creation has
    /// to invalidate it, which a present fingerprint would.
    pub present: bool,
}

impl CapturedRead {
    pub fn of(subject: &str, facet: Facet, value: Option<&serde_json::Value>) -> Self {
        let fingerprint = facet.fingerprint(value);
        Self {
            subject: subject.to_owned(),
            binding: ReadBinding::Current,
            facet,
            fingerprint,
            comparator: FACET_COMPARATOR_VERSION.to_owned(),
            present: value.is_some(),
        }
    }
}

/// A set-level read. A query for "every observation of this capability" depends
/// on the composition of the set, so adding a member has to invalidate it even
/// when nothing already read has changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QueryRead {
    pub query: String,
    pub query_version: u64,
    pub scope: String,
    /// The member set the query declared. A later check re-resolves this set,
    /// so a member that appears is visible to the read. Re-resolving only the
    /// members that happened to exist last time would make a new member
    /// invisible, which is the failure this field exists to prevent.
    pub declared_members: Vec<NodeId>,
    /// Fingerprint of the collection the query resolved against.
    pub collection: String,
    pub fingerprint: String,
    pub comparator: String,
}

impl QueryRead {
    /// Resolves a declared member set against a snapshot.
    pub fn resolve(snapshot: &Snapshot, declared: &[NodeId]) -> Vec<(NodeId, u64)> {
        declared
            .iter()
            .filter_map(|id| {
                snapshot
                    .live(id)
                    .map(|version| (id.clone(), version.version))
            })
            .collect()
    }

    /// The collection fingerprint of a member set. Order is irrelevant because
    /// a set is a set; a list whose order is semantic is not modelled as one.
    pub fn collection_fingerprint(members: &[(NodeId, u64)]) -> String {
        let mut normalized: Vec<String> = members
            .iter()
            .map(|(id, version)| format!("{id}@{version}"))
            .collect();
        normalized.sort();
        digest_of_bytes(normalized.join("\n").as_bytes())
    }
}

/// The engine version that produced a record, so a stale record is recognisable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Profile {
    /// Trusted deterministic evaluators, relevant inputs captured in a snapshot,
    /// no undeclared ambient access, an acyclic graph, pinned comparators. The
    /// profile for the equivalence property.
    ClosedDeterministic,
    /// Declared dependencies that are not exhaustively captured. Known impacts
    /// can be found; absence of a link is not a promise of universality.
    DeclaredPartial,
    /// Hidden inputs or an uninstrumented process. The answer is an obligation.
    Opaque,
}

impl Profile {
    /// Strength, where a closed profile is the strongest claim. A consumer may
    /// accept a weaker claim than it would permit; it may not accept a stronger
    /// one than it grants.
    pub fn rank(&self) -> u8 {
        match self {
            Self::Opaque => 0,
            Self::DeclaredPartial => 1,
            Self::ClosedDeterministic => 2,
        }
    }

    /// Whether this claim is at least as strong as `required`.
    pub fn covers(&self, required: Profile) -> bool {
        self.rank() >= required.rank()
    }

    /// Whether this claim is stronger than a configuration permits.
    pub fn exceeds(&self, allowed: Profile) -> bool {
        self.rank() > allowed.rank()
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ClosedDeterministic => "closed_deterministic",
            Self::DeclaredPartial => "declared_partial",
            Self::Opaque => "opaque",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// An output and a successful read set were published together.
    Completed,
    /// An attempt that did not finish. Recorded as an attempt, never presented as
    /// a completed evaluation.
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvaluationRecord {
    pub target: NodeId,
    pub target_version: u64,
    pub target_nature: NodeNature,
    pub world_revision: u64,
    pub evaluator: String,
    pub evaluator_version: String,
    pub engine: String,
    pub profile: Profile,
    pub reads: Vec<CapturedRead>,
    pub queries: Vec<QueryRead>,
    pub output: serde_json::Value,
    pub assurance_refs: Vec<String>,
    pub outcome: Outcome,
    /// Whether a run actually executed the evaluator. A reuse is not an
    /// execution, and a plan is not work.
    pub executed: bool,
}

impl EvaluationRecord {
    /// The facet of this record's output that a consumer would compare.
    pub fn output_fingerprint(&self) -> String {
        Facet::Whole.fingerprint(Some(&self.output))
    }
}

// ------------------------------------------------------------ the five dimensions

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HistoryState {
    /// Recorded at this world revision.
    Recorded { revision: u64 },
    /// Nothing was recorded for this object.
    Unrecorded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Currency {
    /// Serves the current request on the evidence captured.
    Reusable,
    /// Its content may be unchanged while its support moved. Not false, not
    /// usable without a check.
    NeedsRevalidation,
    Unknown,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Authority {
    Authorized,
    Refused,
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Coverage {
    /// The conclusion holds over the closed captured set.
    ClosedProfile,
    /// The conclusion holds over declared dependencies only.
    DeclaredPartial,
    /// The conclusion holds over an opaque zone.
    Opaque,
}

impl Coverage {
    /// A recomputation cannot promote a result to a guarantee the previous
    /// record did not have captured. A guarantee is only regained by capturing
    /// more, never by recomputing.
    pub fn strongest_of(recorded: Profile, produced: Profile) -> Self {
        Self::of(if recorded.rank() <= produced.rank() {
            recorded
        } else {
            produced
        })
    }

    pub fn of(profile: Profile) -> Self {
        match profile {
            Profile::ClosedDeterministic => Self::ClosedProfile,
            Profile::DeclaredPartial => Self::DeclaredPartial,
            Profile::Opaque => Self::Opaque,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Disposition {
    UnchangedInScope,
    ReusedAfterCheck,
    RecomputedSame,
    RecomputedChanged,
    NeedsEvidence,
    NeedsHumanReview,
    Blocked,
    Unsupported,
    Unknown,
}

impl Disposition {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::UnchangedInScope => "unchanged_in_scope",
            Self::ReusedAfterCheck => "reused_after_check",
            Self::RecomputedSame => "recomputed_same",
            Self::RecomputedChanged => "recomputed_changed",
            Self::NeedsEvidence => "needs_evidence",
            Self::NeedsHumanReview => "needs_human_review",
            Self::Blocked => "blocked",
            Self::Unsupported => "unsupported",
            Self::Unknown => "unknown",
        }
    }
}

/// What the engine concluded about one object, on five separate axes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Finding {
    pub subject: NodeId,
    pub history: HistoryState,
    pub currency: Currency,
    /// Carried through unchanged. The engine never computes a domain verdict and
    /// never turns `recomputed_same` into "satisfied".
    pub business_verdict: Option<String>,
    pub authority: Authority,
    pub coverage: Coverage,
    pub disposition: Disposition,
    pub output: Option<serde_json::Value>,
    pub explanation: Explanation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Explanation {
    /// A stable code. The human rendering comes from these fields, not from a
    /// model inventing a justification afterwards.
    pub code: String,
    pub subject: NodeId,
    pub versions: Vec<String>,
    pub facet: Option<Facet>,
    /// The change that made this object a concern, when there was one.
    pub trigger: Option<String>,
    /// The known dependency path, outermost first.
    pub path: Vec<NodeId>,
    /// What has to hold for work to continue.
    pub condition_to_continue: String,
}

impl Explanation {
    pub fn new(code: &str, subject: &str, condition: &str) -> Self {
        Self {
            code: code.to_owned(),
            subject: subject.to_owned(),
            versions: Vec::new(),
            facet: None,
            trigger: None,
            path: Vec::new(),
            condition_to_continue: condition.to_owned(),
        }
    }
}

// ------------------------------------------------------------------ obligations

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkType {
    PureRecompute,
    UniVerification,
    ExternalObservation,
    HumanReview,
    MissingReference,
}

impl WorkType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::PureRecompute => "pure_recompute",
            Self::UniVerification => "uni_verification",
            Self::ExternalObservation => "external_observation",
            Self::HumanReview => "human_review",
            Self::MissingReference => "missing_reference",
        }
    }
}

/// Structured work, not an automatic re-run of everything.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Obligation {
    pub target: NodeId,
    pub world_revision: u64,
    pub reason_code: String,
    pub work: WorkType,
    pub preconditions: Vec<String>,
    /// What would close this obligation. An agent saying "done" never does.
    pub expected_evidence: String,
    pub required_authority: String,
    /// Target plus relevant context plus work type plus profile. Repeating a plan
    /// does not duplicate a task; an incompatible context does not reuse one.
    pub dedup_key: String,
}

impl Obligation {
    pub fn new(
        target: &str,
        world_revision: u64,
        reason_code: &str,
        work: WorkType,
        profile: Profile,
        condition: &str,
    ) -> Self {
        let dedup_key = digest_of_bytes(
            format!(
                "{target}|{reason_code}|{}|{}",
                work.as_str(),
                profile.as_str()
            )
            .as_bytes(),
        );
        Self {
            target: target.to_owned(),
            world_revision,
            reason_code: reason_code.to_owned(),
            work,
            preconditions: vec![condition.to_owned()],
            expected_evidence: match work {
                WorkType::PureRecompute => {
                    "a re-evaluated record with a published read set".to_owned()
                }
                WorkType::UniVerification => {
                    "a new UNI decision through UNI's own interface".to_owned()
                }
                WorkType::ExternalObservation => {
                    "a fresh observation captured with its real context".to_owned()
                }
                WorkType::HumanReview => {
                    "a recorded human decision with a justification".to_owned()
                }
                WorkType::MissingReference => {
                    "a resolvable reference or an explicit permission".to_owned()
                }
            },
            required_authority: match work {
                WorkType::PureRecompute => "none beyond the configured trust".to_owned(),
                WorkType::UniVerification => "UNI decides".to_owned(),
                WorkType::ExternalObservation => "the observing application".to_owned(),
                WorkType::HumanReview => "the deciding human".to_owned(),
                WorkType::MissingReference => "whoever holds the reference".to_owned(),
            },
            dedup_key,
        }
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("unknown profile {0}")]
    UnknownProfile(String),
    #[error("executable cycle through {0}")]
    ExecutableCycle(NodeId),
    #[error("no evaluator registered for {0}")]
    NoEvaluator(String),
    #[error(
        "an evaluator claimed profile {claimed} but the allowed configuration grants at most {allowed}"
    )]
    ProfileNotGranted {
        evaluator: String,
        claimed: String,
        allowed: String,
    },
    #[error("the plan was computed against revision {planned} but the world is at {current}")]
    StalePlan { planned: u64, current: u64 },
    #[error("limit reached: {0}")]
    LimitReached(String),
    #[error(transparent)]
    Kernel(#[from] crate::KernelError),
}
