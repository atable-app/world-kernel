use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const EXPERIMENTAL_SCHEMA: &str = "world-change/v0-experimental";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorldBootstrap {
    pub world: String,
    pub trusted_assurance_providers: BTreeSet<String>,
    pub objects: Vec<ObjectRevision>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ObjectRevision {
    #[serde(rename = "ref")]
    pub reference: String,
    pub revision: u64,
    pub digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectState {
    pub revision: u64,
    pub digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldSnapshot {
    pub world: String,
    pub revision: u64,
    pub objects: BTreeMap<String, ObjectState>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GroundedChange {
    pub schema: String,
    pub world: String,
    pub proposal_id: String,
    pub idempotency_key: String,
    pub actor: String,
    pub base_revision: u64,
    pub intent: Intent,
    pub candidate: Candidate,
    pub reads: Vec<ObjectRevision>,
    pub coverage: Coverage,
    pub assessments: Vec<AcceptedAssessment>,
    pub patches: Vec<Patch>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Intent {
    pub kind: String,
    pub target: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Candidate {
    #[serde(rename = "ref")]
    pub reference: String,
    pub digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Coverage {
    pub profile: String,
    pub complete_for: BTreeSet<String>,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AcceptedAssessment {
    pub provider: String,
    pub subject: String,
    pub subject_digest: String,
    pub assessment_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase", deny_unknown_fields)]
pub enum Patch {
    PutObject {
        #[serde(rename = "ref")]
        reference: String,
        expected_revision: Option<u64>,
        digest: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum SubmissionOutcome {
    Committed(Receipt),
    Rejected(Rejection),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Receipt {
    pub proposal_id: String,
    pub world_revision: u64,
    pub event_sequence: u64,
    pub candidate_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rejection {
    pub code: RejectionCode,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RejectionCode {
    UnsupportedSchema,
    WrongWorld,
    StaleWorld,
    Unauthorized,
    IncompleteView,
    StaleDependency,
    AssessmentMismatch,
    UntrustedAssessmentProvider,
    CandidateMismatch,
    PatchConflict,
    IdempotencyConflict,
}

#[derive(Debug, Error)]
pub enum KernelError {
    #[error("storage error: {0}")]
    Storage(#[from] rusqlite::Error),
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("world already exists: {0}")]
    WorldAlreadyExists(String),
    #[error("world not found")]
    WorldNotFound,
    #[error("corrupt kernel state: {0}")]
    CorruptState(String),
}
