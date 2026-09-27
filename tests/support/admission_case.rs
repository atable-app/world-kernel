//! Strict fixture model for the M1 equal-information admission corpus.
//!
//! Every field named in `docs/SPEC.md` is deserialized into a named type. No
//! field may stay an untyped `serde_json::Value`, and no unknown key, unknown
//! enum value or missing key is tolerated: a versioned corpus must fail loudly
//! rather than silently measure a weakened case.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use world_kernel::{GroundedChange, Patch, RejectionCode, WorldBootstrap};

pub const CASE_SCHEMA: &str = "world-kernel-admission-case/v1";
pub const CORPUS_PATH: &str = "tests/fixtures/admission-corpus.jsonl";

/// Every record must carry every key, including the ones whose value may be
/// `null`. Serde treats an absent `Option` field as `None`, so key presence is
/// checked before the typed parse instead of being assumed.
pub const CASE_FIELDS: &[&str] = &[
    "schema",
    "id",
    "family",
    "expected",
    "bootstrap",
    "prelude",
    "proposal",
    "current",
    "assurance",
    "fault",
    "notes",
];

/// The producer declares the view protocol it prepared a change under. v0
/// implements exactly one, and its completeness is the producer's own
/// declaration, not a trusted collector's proof.
pub const SUPPORTED_COVERAGE_PROFILES: &[&str] = &["closed-v1"];

/// The producer declares the operation class it needs admitted. v0 implements
/// native World publication only.
pub const SUPPORTED_INTENT_KINDS: &[&str] = &["publishCandidate"];

/// Declared view protocols that name a mechanism v0 does not have: a `none
/// exist` read (M4) and a bounded graph traversal.
pub const UNSUPPORTED_COVERAGE_PROFILES: &[&str] =
    &["closed-v1-negative-queries", "closed-v1-graph-traversal"];

/// Declared operation classes with no v0 implementation. External effect
/// dispatch is M3, and an uncertain effect must never be simulated as
/// confirmed.
pub const UNSUPPORTED_INTENT_KINDS: &[&str] = &["dispatchExternalEffect"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AdmissionCase {
    pub schema: String,
    pub id: String,
    pub family: CaseFamily,
    pub expected: ExpectedOutcome,
    pub bootstrap: WorldBootstrap,
    pub prelude: Vec<PreludeSubmission>,
    pub proposal: GroundedChange,
    pub current: CurrentFixture,
    pub assurance: AssuranceFixture,
    pub fault: Option<FaultPoint>,
    pub notes: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreludeSubmission {
    pub change: GroundedChange,
    pub grants: BTreeMap<String, BTreeSet<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CurrentFixture {
    pub grants: BTreeMap<String, BTreeSet<String>>,
    pub candidate_path: String,
    pub candidate_contents: String,
    pub candidate_contents_after_assurance: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssuranceFixture {
    pub mode: AssuranceMode,
    pub provider: String,
    pub evidence_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExpectedOutcome {
    pub status: ExpectedStatus,
    pub code: Option<BenchmarkCode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExpectedStatus {
    Committed,
    Rejected,
    Unsupported,
}

impl ExpectedStatus {
    /// A case is adverse when the correct answer is not a clean commit. The
    /// corpus declares this without a label: a case is adverse when it injects a
    /// fault or when it expects anything other than `committed`.
    pub fn is_adverse(self) -> bool {
        !matches!(self, Self::Committed)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaseFamily {
    CandidateExactness,
    Authority,
    WorldObjectConcurrency,
    IdempotencyRestart,
    CoverageLimits,
    AssuranceTrust,
    SchemaScopeIsolation,
    CommitInterruption,
    NegativeQueryDependency,
    UniEnvironmentAndPolicy,
    ReassessmentSemantics,
    AdversarialInput,
    GraphTermination,
    ExternalEffectUncertainty,
}

/// The corpus family, adverse and benign counts required by `docs/SPEC.md`.
pub const FAMILY_COUNTS: &[(CaseFamily, usize, usize)] = &[
    (CaseFamily::CandidateExactness, 6, 2),
    (CaseFamily::Authority, 5, 2),
    (CaseFamily::WorldObjectConcurrency, 7, 2),
    (CaseFamily::IdempotencyRestart, 5, 3),
    (CaseFamily::CoverageLimits, 4, 2),
    (CaseFamily::AssuranceTrust, 5, 2),
    (CaseFamily::SchemaScopeIsolation, 4, 2),
    (CaseFamily::CommitInterruption, 5, 1),
    (CaseFamily::NegativeQueryDependency, 4, 0),
    (CaseFamily::UniEnvironmentAndPolicy, 4, 1),
    (CaseFamily::ReassessmentSemantics, 3, 1),
    (CaseFamily::AdversarialInput, 3, 1),
    (CaseFamily::GraphTermination, 3, 0),
    (CaseFamily::ExternalEffectUncertainty, 2, 1),
];

pub const TOTAL_CASES: usize = 80;
pub const TOTAL_ADVERSE: usize = 60;
pub const TOTAL_BENIGN: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssuranceMode {
    Accepted,
    Rejected,
    EvidenceRequired,
    InvalidBundle,
    UnsupportedBundle,
    MissingRequiredEngine,
    RegistryDrift,
    WatchedFileDrift,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FaultPoint {
    BeforeObjectWrite,
    BeforeWorldRevision,
    BeforeEventAppend,
    BeforeSubmissionRecord,
    LostResponseAfterCommit,
}

/// The decision vocabulary shared by all three benchmark systems. It is closed:
/// every Kernel `RejectionCode`, every `UniCollectorError::code()` value the
/// corpus provokes, and `UNSUPPORTED` for a declared capability that no system
/// implements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BenchmarkCode {
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
    UniNotAccepted,
    CandidateNotCoveredByUni,
    CandidateChangedDuringVerification,
    UniCommandFailed,
    UniBundleInvalid,
    UniBundleUnsupported,
    Unsupported,
}

impl BenchmarkCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::UnsupportedSchema => "UNSUPPORTED_SCHEMA",
            Self::WrongWorld => "WRONG_WORLD",
            Self::StaleWorld => "STALE_WORLD",
            Self::Unauthorized => "UNAUTHORIZED",
            Self::IncompleteView => "INCOMPLETE_VIEW",
            Self::StaleDependency => "STALE_DEPENDENCY",
            Self::AssessmentMismatch => "ASSESSMENT_MISMATCH",
            Self::UntrustedAssessmentProvider => "UNTRUSTED_ASSESSMENT_PROVIDER",
            Self::CandidateMismatch => "CANDIDATE_MISMATCH",
            Self::PatchConflict => "PATCH_CONFLICT",
            Self::IdempotencyConflict => "IDEMPOTENCY_CONFLICT",
            Self::UniNotAccepted => "UNI_NOT_ACCEPTED",
            Self::CandidateNotCoveredByUni => "CANDIDATE_NOT_COVERED_BY_UNI",
            Self::CandidateChangedDuringVerification => "CANDIDATE_CHANGED_DURING_VERIFICATION",
            Self::UniCommandFailed => "UNI_COMMAND_FAILED",
            Self::UniBundleInvalid => "UNI_BUNDLE_INVALID",
            Self::UniBundleUnsupported => "UNI_BUNDLE_UNSUPPORTED",
            Self::Unsupported => "UNSUPPORTED",
        }
    }
}

impl From<&RejectionCode> for BenchmarkCode {
    fn from(code: &RejectionCode) -> Self {
        match code {
            RejectionCode::UnsupportedSchema => Self::UnsupportedSchema,
            RejectionCode::WrongWorld => Self::WrongWorld,
            RejectionCode::StaleWorld => Self::StaleWorld,
            RejectionCode::Unauthorized => Self::Unauthorized,
            RejectionCode::IncompleteView => Self::IncompleteView,
            RejectionCode::StaleDependency => Self::StaleDependency,
            RejectionCode::AssessmentMismatch => Self::AssessmentMismatch,
            RejectionCode::UntrustedAssessmentProvider => Self::UntrustedAssessmentProvider,
            RejectionCode::CandidateMismatch => Self::CandidateMismatch,
            RejectionCode::PatchConflict => Self::PatchConflict,
            RejectionCode::IdempotencyConflict => Self::IdempotencyConflict,
        }
    }
}

/// A capability a case declares and that no benchmark system implements.
///
/// The signal is read from the producer's own declared view protocol and
/// operation class, never from the expected outcome, so a system cannot pass a
/// case by looking at the answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnsupportedCapability {
    CoverageProfile(&'static str),
    IntentKind(&'static str),
}

impl UnsupportedCapability {
    pub fn describe(&self) -> String {
        match self {
            Self::CoverageProfile(profile) => {
                format!("coverage profile {profile} is a future view protocol")
            }
            Self::IntentKind(kind) => format!("intent kind {kind} has no v0 implementation"),
        }
    }
}

impl AdmissionCase {
    /// The capability this case requires, derived only from declared inputs.
    pub fn required_capability(&self) -> Option<UnsupportedCapability> {
        let profile = self.proposal.coverage.profile.as_str();
        if !SUPPORTED_COVERAGE_PROFILES.contains(&profile) {
            return Some(UnsupportedCapability::CoverageProfile(
                UNSUPPORTED_COVERAGE_PROFILES
                    .iter()
                    .copied()
                    .find(|candidate| *candidate == profile)
                    .unwrap_or("unknown"),
            ));
        }
        let kind = self.proposal.intent.kind.as_str();
        if !SUPPORTED_INTENT_KINDS.contains(&kind) {
            return Some(UnsupportedCapability::IntentKind(
                UNSUPPORTED_INTENT_KINDS
                    .iter()
                    .copied()
                    .find(|candidate| *candidate == kind)
                    .unwrap_or("unknown"),
            ));
        }
        None
    }

    /// The bytes the workspace holds once assurance has run.
    pub fn effective_candidate_contents(&self) -> &str {
        self.current
            .candidate_contents_after_assurance
            .as_deref()
            .unwrap_or(&self.current.candidate_contents)
    }

    pub fn expected_digest(&self) -> String {
        digest_bytes(self.effective_candidate_contents().as_bytes())
    }

    pub fn is_adverse(&self) -> bool {
        self.fault.is_some() || self.expected.status.is_adverse()
    }

    /// The exact-replay exception: a verbatim repeat of a prelude submission
    /// re-sends the envelope that was already admitted, including the base
    /// revision it was admitted at, and must return the stored outcome.
    pub fn is_exact_replay(&self) -> bool {
        self.prelude
            .iter()
            .any(|submission| submission.change == self.proposal)
    }
}

pub fn digest_bytes(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Corpus {
    pub cases: Vec<AdmissionCase>,
}

impl Corpus {
    pub fn parse(text: &str) -> Result<Self, CorpusError> {
        let mut cases = Vec::new();
        for (index, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let number = index + 1;
            cases.push(parse_case(line, number)?);
        }
        Ok(Self { cases })
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self, CorpusError> {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path).map_err(|error| CorpusError::Unreadable {
            path: path.display().to_string(),
            detail: error.to_string(),
        })?;
        Self::parse(&text)
    }

    pub fn load_checked() -> Result<Self, CorpusError> {
        let corpus = Self::load(corpus_path())?;
        let errors = corpus.validate();
        if errors.is_empty() {
            Ok(corpus)
        } else {
            Err(CorpusError::Invalid(errors))
        }
    }

    pub fn validate(&self) -> Vec<CorpusError> {
        let mut errors = Vec::new();
        self.check_shape(&mut errors);
        self.check_counts(&mut errors);
        errors
    }

    fn check_shape(&self, errors: &mut Vec<CorpusError>) {
        let mut seen = BTreeMap::new();
        for case in &self.cases {
            if case.schema != CASE_SCHEMA {
                errors.push(CorpusError::CaseSchema {
                    id: case.id.clone(),
                    schema: case.schema.clone(),
                });
            }
            if !valid_case_id(&case.id) {
                errors.push(CorpusError::CaseId {
                    id: case.id.clone(),
                    detail: "expected dotted lowercase segments ending in a three digit ordinal"
                        .to_owned(),
                });
            }
            if case.notes.trim().is_empty() {
                errors.push(CorpusError::Notes {
                    id: case.id.clone(),
                });
            }
            if let Some(previous) = seen.insert(case.id.clone(), case.family) {
                errors.push(CorpusError::DuplicateId {
                    id: case.id.clone(),
                    first: previous,
                    second: case.family,
                });
            }
            self.check_structural(case, errors);
        }
    }

    fn check_counts(&self, errors: &mut Vec<CorpusError>) {
        if self.cases.len() != TOTAL_CASES {
            errors.push(CorpusError::TotalCases {
                expected: TOTAL_CASES,
                found: self.cases.len(),
            });
        }
        let adverse = self.cases.iter().filter(|case| case.is_adverse()).count();
        if adverse != TOTAL_ADVERSE {
            errors.push(CorpusError::AdverseCases {
                expected: TOTAL_ADVERSE,
                found: adverse,
            });
        }
        let benign = self.cases.len() - adverse;
        if benign != TOTAL_BENIGN {
            errors.push(CorpusError::BenignCases {
                expected: TOTAL_BENIGN,
                found: benign,
            });
        }
        for (family, adverse_expected, benign_expected) in FAMILY_COUNTS {
            let adverse_found = self
                .cases
                .iter()
                .filter(|case| case.family == *family && case.is_adverse())
                .count();
            let benign_found = self
                .cases
                .iter()
                .filter(|case| case.family == *family && !case.is_adverse())
                .count();
            if adverse_found != *adverse_expected || benign_found != *benign_expected {
                errors.push(CorpusError::FamilyCounts {
                    family: *family,
                    adverse_expected: *adverse_expected,
                    adverse_found,
                    benign_expected: *benign_expected,
                    benign_found,
                });
            }
        }
        for case in &self.cases {
            if !FAMILY_COUNTS
                .iter()
                .any(|(family, _, _)| *family == case.family)
            {
                errors.push(CorpusError::UnlistedFamily {
                    id: case.id.clone(),
                    family: case.family,
                });
            }
        }
    }

    /// Per-case invariants that keep the corpus internally coherent, so a run
    /// measures admission rather than a broken fixture.
    ///
    /// This reads `expected` on purpose. It is a corpus self-check, not a
    /// decision oracle: the systems in `harness.rs` never see it.
    fn check_structural(&self, case: &AdmissionCase, errors: &mut Vec<CorpusError>) {
        let proposal = &case.proposal;
        let breaks_base_revision =
            case.expected.code == Some(BenchmarkCode::StaleWorld) || case.is_exact_replay();
        if !breaks_base_revision && proposal.base_revision != case.prelude.len() as u64 {
            errors.push(CorpusError::BaseRevision {
                id: case.id.clone(),
                prelude: case.prelude.len(),
                declared: proposal.base_revision,
            });
        }
        for (index, submission) in case.prelude.iter().enumerate() {
            if let Some(detail) =
                inadmissible(&submission.change, &submission.grants, &case.bootstrap)
            {
                errors.push(CorpusError::PreludeInadmissible {
                    id: case.id.clone(),
                    index,
                    detail,
                });
            }
        }
        if let Some(detail) = unsafe_candidate_path(&case.current.candidate_path) {
            errors.push(CorpusError::CandidatePath {
                id: case.id.clone(),
                path: case.current.candidate_path.clone(),
                detail: detail.to_owned(),
            });
        }
        if case.assurance.evidence_path.trim().is_empty() {
            errors.push(CorpusError::EvidencePath {
                id: case.id.clone(),
            });
        }

        // Only a deliberate producer error may publish bytes its assurance
        // never covered. Every other case must declare the digest the workspace
        // actually holds, so a stale digest can never hide inside an unrelated
        // rejection.
        let breaks_binding = matches!(
            case.expected.code,
            Some(BenchmarkCode::AssessmentMismatch) | Some(BenchmarkCode::CandidateMismatch)
        );
        if !breaks_binding && proposal.candidate.digest != case.expected_digest() {
            errors.push(CorpusError::CandidateDigest {
                id: case.id.clone(),
                declared: proposal.candidate.digest.clone(),
                contents: case.expected_digest(),
            });
        }
        if case.expected.status == ExpectedStatus::Committed
            && let Some(detail) = inadmissible(proposal, &case.current.grants, &case.bootstrap)
        {
            errors.push(CorpusError::CommittedCaseRejected {
                id: case.id.clone(),
                detail,
            });
        }
        match case.required_capability() {
            None if case.expected.status == ExpectedStatus::Unsupported => {
                errors.push(CorpusError::UnsupportedUndeclared {
                    id: case.id.clone(),
                })
            }
            Some(capability) if case.expected.status != ExpectedStatus::Unsupported => {
                errors.push(CorpusError::UnsupportedMisdeclared {
                    id: case.id.clone(),
                    detail: capability.describe(),
                })
            }
            _ => {}
        }
    }
}

/// Parses one JSONL record. Unknown keys, unknown enum values, missing keys and
/// shape errors all fail here, before any system sees a case.
pub fn parse_case(line: &str, number: usize) -> Result<AdmissionCase, CorpusError> {
    let value: serde_json::Value =
        serde_json::from_str(line).map_err(|error| CorpusError::Syntax {
            line: number,
            detail: error.to_string(),
        })?;
    let object = value.as_object().ok_or_else(|| CorpusError::Shape {
        line: number,
        detail: "a corpus record must be a JSON object".to_owned(),
    })?;
    for field in CASE_FIELDS {
        if !object.contains_key(*field) {
            return Err(CorpusError::MissingField {
                line: number,
                field: (*field).to_owned(),
            });
        }
    }
    serde_json::from_value(value).map_err(|error| CorpusError::Shape {
        line: number,
        detail: error.to_string(),
    })
}

fn valid_case_id(id: &str) -> bool {
    let mut segments = id.split('.');
    let ordinal = segments.next_back();
    let mut named: Vec<&str> = Vec::new();
    for segment in segments {
        named.push(segment);
    }
    let ordinal_ok = ordinal.is_some_and(|value| {
        value.len() == 3 && value.bytes().all(|byte| byte.is_ascii_digit()) && value != "000"
    });
    ordinal_ok
        && !named.is_empty()
        && named.iter().all(|segment| {
            !segment.is_empty()
                && segment.bytes().all(|byte| {
                    byte.is_ascii_lowercase()
                        || byte.is_ascii_digit()
                        || byte == b'-'
                        || byte == b'_'
                })
        })
}

/// Workspace-relative path safety. A symlink escape can only be detected after
/// canonicalization, so the collector owns that half of the rule and reports
/// `OUTSIDE_WORKSPACE`.
pub fn unsafe_candidate_path(path: &str) -> Option<&'static str> {
    if path.is_empty() {
        return Some("path is empty");
    }
    if path.starts_with('/') || path.starts_with('\\') {
        return Some("path is absolute");
    }
    if path.contains(':') {
        return Some("path names a drive or a scheme");
    }
    let segments: Vec<&str> = path.split('/').collect();
    if segments
        .iter()
        .any(|segment| *segment == ".." || *segment == "." || segment.is_empty())
    {
        return Some("path contains a relative or empty segment");
    }
    None
}

/// The admission requirements a prelude submission and a benign measured
/// proposal must satisfy before any system is asked to decide anything.
fn inadmissible(
    change: &GroundedChange,
    grants: &BTreeMap<String, BTreeSet<String>>,
    bootstrap: &WorldBootstrap,
) -> Option<String> {
    if change.world != bootstrap.world {
        return Some(format!(
            "targets {} in world {}",
            change.world, bootstrap.world
        ));
    }
    let permitted = grants
        .get(&change.actor)
        .is_some_and(|permissions| permissions.contains(&change.intent.kind));
    if !permitted {
        return Some(format!("{} may not {}", change.actor, change.intent.kind));
    }
    if change.coverage.truncated || !change.coverage.complete_for.contains(&change.intent.kind) {
        return Some(format!(
            "coverage {} does not include {}",
            change.coverage.profile, change.intent.kind
        ));
    }
    if change.intent.target != change.candidate.reference {
        return Some("intent target is not the candidate".to_owned());
    }
    let exact = change.assessments.iter().any(|assessment| {
        assessment.subject == change.candidate.reference
            && assessment.subject_digest == change.candidate.digest
    });
    if !exact {
        return Some("no assessment covers the exact candidate".to_owned());
    }
    let trusted = exact
        && change.assessments.iter().any(|assessment| {
            assessment.subject == change.candidate.reference
                && assessment.subject_digest == change.candidate.digest
                && bootstrap
                    .trusted_assurance_providers
                    .contains(&assessment.provider)
        });
    if !trusted {
        return Some("the exact candidate is covered only by untrusted providers".to_owned());
    }
    let published = change.patches.iter().any(|patch| match patch {
        Patch::PutObject {
            reference, digest, ..
        } => reference == &change.candidate.reference && digest == &change.candidate.digest,
    });
    if !published {
        return Some("no patch publishes the assessed candidate".to_owned());
    }
    None
}

pub fn corpus_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(CORPUS_PATH)
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CorpusError {
    #[error("cannot read {path}: {detail}")]
    Unreadable { path: String, detail: String },
    #[error("line {line} is not valid JSON: {detail}")]
    Syntax { line: usize, detail: String },
    #[error("line {line} is missing the required field {field}")]
    MissingField { line: usize, field: String },
    #[error("line {line} does not match {CASE_SCHEMA}: {detail}")]
    Shape { line: usize, detail: String },
    #[error("case {id} declares schema {schema}")]
    CaseSchema { id: String, schema: String },
    #[error("case {id} is not named like the corpus contract: {detail}")]
    CaseId { id: String, detail: String },
    #[error("case {id} has no notes")]
    Notes { id: String },
    #[error("case id {id} appears twice, in {first:?} and {second:?}")]
    DuplicateId {
        id: String,
        first: CaseFamily,
        second: CaseFamily,
    },
    #[error("corpus has {found} cases, contract requires {expected}")]
    TotalCases { expected: usize, found: usize },
    #[error("corpus has {found} adverse cases, contract requires {expected}")]
    AdverseCases { expected: usize, found: usize },
    #[error("corpus has {found} benign cases, contract requires {expected}")]
    BenignCases { expected: usize, found: usize },
    #[error(
        "family {family:?} has {adverse_found} adverse and {benign_found} benign cases, contract requires {adverse_expected} and {benign_expected}"
    )]
    FamilyCounts {
        family: CaseFamily,
        adverse_expected: usize,
        adverse_found: usize,
        benign_expected: usize,
        benign_found: usize,
    },
    #[error("case {id} uses family {family:?}, which the contract does not list")]
    UnlistedFamily { id: String, family: CaseFamily },
    #[error(
        "case {id} declares base revision {declared} but {prelude} prelude submissions advance the world"
    )]
    BaseRevision {
        id: String,
        prelude: usize,
        declared: u64,
    },
    #[error("case {id} prelude submission {index} could never be admitted: {detail}")]
    PreludeInadmissible {
        id: String,
        index: usize,
        detail: String,
    },
    #[error("case {id} candidate path {path} is not workspace relative: {detail}")]
    CandidatePath {
        id: String,
        path: String,
        detail: String,
    },
    #[error("case {id} has no assurance evidence path")]
    EvidencePath { id: String },
    #[error("case {id} declares candidate digest {declared} but its contents hash to {contents}")]
    CandidateDigest {
        id: String,
        declared: String,
        contents: String,
    },
    #[error("case {id} expects a commit but its own inputs are inadmissible: {detail}")]
    CommittedCaseRejected { id: String, detail: String },
    #[error("case {id} expects unsupported but declares no unimplemented capability")]
    UnsupportedUndeclared { id: String },
    #[error("case {id} does not expect unsupported although {detail}")]
    UnsupportedMisdeclared { id: String, detail: String },
    #[error("the corpus is invalid:\n{}", .0.iter().map(|error| format!("  - {error}")).collect::<Vec<_>>().join("\n"))]
    Invalid(Vec<CorpusError>),
}
