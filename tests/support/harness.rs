//! The shared benchmark driver.
//!
//! Every system receives the same deserialized `AdmissionCase`, the same
//! workspace bytes and the same current authority. The harness never decides a
//! case: `expected` is read only to report and score a result, exactly as the
//! SPEC output contract requires.
//!
//! The order of operations is fixed by the corpus contract:
//!
//! 1. write `candidateContents` and the assurance contract into a workspace;
//! 2. open the world and admit the prelude submissions in order;
//! 3. run the assurance step;
//! 4. replace the candidate with `candidateContentsAfterAssurance` if declared;
//! 5. admit the measured proposal.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::PathBuf,
    time::Instant,
};

use serde::Serialize;
use serde_json::{Value, json};
use tempfile::TempDir;
use world_kernel::{
    GroundedChange, Kernel, Patch, StaticAuthority, SubmissionOutcome, WorldBootstrap,
};

use super::admission_case::{
    AdmissionCase, BenchmarkCode, CaseFamily, Corpus, ExpectedStatus, FaultPoint, PreludeSubmission,
};
use super::assurance::{AssuranceOutcome, expected_code_for};
use super::baseline_a::{
    AppAuthority, BaselineA, DeclaredRead, DeclaredView, GateError, GateOutcome, PublishRequest,
    REQUEST_VERSION, VerifierClaim, WriteIntent, code_from_assurance,
};
use super::baseline_b;
use super::system_c::{self, Ablation, CurrentState};

/// The sibling scope is always created so that a wrong world is a genuine scope
/// test rather than a test of an absent name.
pub const SIBLING_SUFFIX: &str = "-sibling";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SystemId {
    A,
    B,
    C,
}

impl SystemId {
    pub const ALL: [SystemId; 3] = [SystemId::A, SystemId::B, SystemId::C];

    pub fn label(&self) -> &'static str {
        match self {
            Self::A => "A",
            Self::B => "B",
            Self::C => "C",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ActualStatus {
    Committed,
    Rejected,
    Unsupported,
    /// A harness or storage failure. A case that produces this has failed the
    /// run; it is never a valid measurement.
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ActualOutcome {
    pub status: ActualStatus,
    pub code: Option<BenchmarkCode>,
}

impl ActualOutcome {
    fn committed() -> Self {
        Self {
            status: ActualStatus::Committed,
            code: None,
        }
    }

    fn rejected(code: BenchmarkCode) -> Self {
        Self {
            status: ActualStatus::Rejected,
            code: Some(code),
        }
    }

    fn unsupported() -> Self {
        Self {
            status: ActualStatus::Unsupported,
            code: Some(BenchmarkCode::Unsupported),
        }
    }

    fn error() -> Self {
        Self {
            status: ActualStatus::Error,
            code: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CaseResult {
    pub case_id: String,
    pub system: SystemId,
    pub actual: ActualOutcome,
    pub expected: ExpectedOutcomeRecord,
    pub correct: bool,
    pub duration_micros: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExpectedOutcomeRecord {
    pub status: ExpectedStatus,
    pub code: Option<BenchmarkCode>,
}

#[derive(Debug)]
pub enum RunError {
    Storage(String),
    PreludeDidNotCommit {
        case_id: String,
        index: usize,
        detail: String,
    },
    InterruptedFaultDidNotInterrupt {
        case_id: String,
        detail: String,
    },
    FaultAdvancedState {
        case_id: String,
        detail: String,
    },
}

impl std::fmt::Display for RunError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Storage(detail) => write!(formatter, "storage failure: {detail}"),
            Self::PreludeDidNotCommit {
                case_id,
                index,
                detail,
            } => write!(
                formatter,
                "case {case_id} prelude {index} did not advance the world: {detail}"
            ),
            Self::InterruptedFaultDidNotInterrupt { case_id, detail } => write!(
                formatter,
                "case {case_id} was expected to be interrupted but was not: {detail}"
            ),
            Self::FaultAdvancedState { case_id, detail } => write!(
                formatter,
                "case {case_id} advanced state across an interruption: {detail}"
            ),
        }
    }
}

/// The workspace a case runs in, holding the exact bytes the fixture declared.
pub struct CaseWorkspace {
    pub directory: TempDir,
    pub database: PathBuf,
}

impl CaseWorkspace {
    fn prepare(case: &AdmissionCase) -> Result<Self, RunError> {
        let directory =
            tempfile::tempdir().map_err(|error| RunError::Storage(error.to_string()))?;
        let candidate = directory.path().join(&case.current.candidate_path);
        if let Some(parent) = candidate.parent() {
            fs::create_dir_all(parent).map_err(|error| RunError::Storage(error.to_string()))?;
        }
        fs::write(&candidate, case.current.candidate_contents.as_bytes())
            .map_err(|error| RunError::Storage(error.to_string()))?;
        fs::write(
            directory.path().join(&case.assurance.evidence_path),
            b"VERSION 0.1\nDOMAIN software\nINTENT release\n",
        )
        .map_err(|error| RunError::Storage(error.to_string()))?;
        let database = directory.path().join("world.db");
        Ok(Self {
            directory,
            database,
        })
    }

    /// The bytes the fixture declares for the moment after assurance ran.
    fn apply_after_assurance(&self, case: &AdmissionCase) -> Result<(), RunError> {
        if let Some(contents) = &case.current.candidate_contents_after_assurance {
            fs::write(
                self.directory.path().join(&case.current.candidate_path),
                contents.as_bytes(),
            )
            .map_err(|error| RunError::Storage(error.to_string()))?;
        }
        Ok(())
    }

    fn candidate_bytes(&self, case: &AdmissionCase) -> Result<Vec<u8>, RunError> {
        fs::read(self.directory.path().join(&case.current.candidate_path))
            .map_err(|error| RunError::Storage(error.to_string()))
    }
}

pub fn sibling_world(world: &str) -> String {
    format!("{world}{SIBLING_SUFFIX}")
}

fn grants_map(grants: &BTreeMap<String, BTreeSet<String>>) -> BTreeMap<String, BTreeSet<String>> {
    grants.clone()
}

/// One system under test, so the harness drives all three the same way.
enum Driver {
    Application { gate: BaselineA, use_uni_seam: bool },
    Kernel { kernel: Kernel },
}

impl Driver {
    fn revision(&self) -> Result<u64, RunError> {
        match self {
            Self::Application { gate, .. } => gate
                .revision()
                .map_err(|error| RunError::Storage(describe(error))),
            Self::Kernel { kernel } => kernel
                .snapshot()
                .map(|snapshot| snapshot.revision)
                .map_err(|error| RunError::Storage(error.to_string())),
        }
    }

    fn assure(&self, case: &AdmissionCase, workspace: &CaseWorkspace) -> AssuranceOutcome {
        let uses_uni_seam = match self {
            Self::Application { use_uni_seam, .. } => *use_uni_seam,
            // C is UNI plus the Kernel by definition.
            Self::Kernel { .. } => true,
        };
        if uses_uni_seam {
            baseline_b::assure(
                workspace.directory.path(),
                case.assurance.mode,
                &case.assurance.provider,
                &case.current.candidate_path,
                &case.assurance.evidence_path,
                &case.proposal.candidate.reference,
            )
        } else {
            super::assurance::app_verify(
                case.assurance.mode,
                &case.assurance.provider,
                workspace.directory.path(),
                &case.current.candidate_path,
                &case.proposal.candidate.reference,
            )
        }
    }

    fn submit(
        &mut self,
        case: &AdmissionCase,
        workspace: &CaseWorkspace,
        assurance: &AssuranceOutcome,
        ablation: Option<Ablation>,
    ) -> Result<ActualOutcome, RunError> {
        match self {
            Self::Application { gate, .. } => {
                let request = application_request(&case.proposal);
                let authority = AppAuthority::new(grants_map(&case.current.grants));
                match gate.apply(&request, assurance, &authority) {
                    Ok(GateOutcome::Applied { .. }) => Ok(ActualOutcome::committed()),
                    Ok(GateOutcome::Refused(code)) => Ok(ActualOutcome::rejected(code)),
                    Err(error) => Err(RunError::Storage(describe(error))),
                }
            }
            Self::Kernel { kernel } => {
                // The collector refused, so no accepted assessment exists and
                // the change is never presented to the Kernel.
                if let Some(code) = assurance.refusal_code() {
                    return Ok(ActualOutcome::rejected(code_from_assurance(code)));
                }
                let change = match ablation {
                    None => case.proposal.clone(),
                    Some(ablation) => {
                        let state = current_state(case, kernel, workspace)?;
                        system_c::ablate(&case.proposal, ablation, &state)
                    }
                };
                let authority = StaticAuthority::new(grants_map(&case.current.grants));
                match kernel.submit(&change, &authority) {
                    Ok(outcome) => {
                        let (committed, code) = system_c::outcome_to_benchmark(&outcome);
                        Ok(if committed {
                            ActualOutcome::committed()
                        } else {
                            ActualOutcome::rejected(code.expect("a rejection carries a code"))
                        })
                    }
                    Err(error) => Err(RunError::Storage(error.to_string())),
                }
            }
        }
    }
}

fn describe(error: GateError) -> String {
    match error {
        GateError::Storage(error) => error.to_string(),
        GateError::Interrupted(detail) => detail,
        GateError::Serialization(error) => error.to_string(),
    }
}

fn current_state(
    case: &AdmissionCase,
    kernel: &Kernel,
    workspace: &CaseWorkspace,
) -> Result<CurrentState, RunError> {
    let snapshot = kernel
        .snapshot()
        .map_err(|error| RunError::Storage(error.to_string()))?;
    let candidate = workspace.candidate_bytes(case)?;
    Ok(CurrentState {
        object_digests: snapshot
            .objects
            .into_iter()
            .map(|(name, state)| (name, state.revision, state.digest))
            .collect(),
        candidate_digest: system_c::workspace_digest(&String::from_utf8_lossy(&candidate)),
    })
}

/// The application-specific translation of the same envelope.
///
/// Every declared field is carried over, including the ones a given
/// implementation never reads, so no system receives less information than
/// another.
pub fn application_request(change: &GroundedChange) -> PublishRequest {
    PublishRequest {
        request_version: if change.schema == "world-change/v0-experimental" {
            REQUEST_VERSION.to_owned()
        } else {
            change.schema.clone()
        },
        scope: change.world.clone(),
        request_key: change.idempotency_key.clone(),
        principal: change.actor.clone(),
        operation: change.intent.kind.clone(),
        target: change.intent.target.clone(),
        subject: change.candidate.reference.clone(),
        subject_digest: change.candidate.digest.clone(),
        observed_revision: change.base_revision,
        declared_reads: change
            .reads
            .iter()
            .map(|read| DeclaredRead {
                subject: read.reference.clone(),
                revision: read.revision,
                digest: read.digest.clone(),
            })
            .collect(),
        declared_view: DeclaredView {
            profile: change.coverage.profile.clone(),
            covers: change.coverage.complete_for.clone(),
            truncated: change.coverage.truncated,
        },
        verifier_claims: change
            .assessments
            .iter()
            .map(|assessment| VerifierClaim {
                verifier: assessment.provider.clone(),
                subject: assessment.subject.clone(),
                subject_digest: assessment.subject_digest.clone(),
            })
            .collect(),
        write_intents: change
            .patches
            .iter()
            .map(|patch| match patch {
                Patch::PutObject {
                    reference,
                    expected_revision,
                    digest,
                } => WriteIntent {
                    subject: reference.clone(),
                    expect_revision: *expected_revision,
                    digest: digest.clone(),
                },
            })
            .collect(),
    }
}

fn build_driver(
    system: SystemId,
    case: &AdmissionCase,
    workspace: &CaseWorkspace,
) -> Result<Driver, RunError> {
    let trusted = case.bootstrap.trusted_assurance_providers.clone();
    let objects = case.bootstrap.objects.clone();
    match system {
        SystemId::A | SystemId::B => Ok(Driver::Application {
            gate: BaselineA::create(
                &workspace.database,
                &case.bootstrap.world,
                trusted,
                &objects,
            )
            .map_err(|error| RunError::Storage(describe(error)))?,
            use_uni_seam: system == SystemId::B,
        }),
        SystemId::C => {
            let kernel = Kernel::create(
                &workspace.database,
                WorldBootstrap {
                    world: case.bootstrap.world.clone(),
                    trusted_assurance_providers: trusted,
                    objects,
                },
            )
            .map_err(|error| RunError::Storage(error.to_string()))?;
            Kernel::create(
                &workspace.database,
                WorldBootstrap {
                    world: sibling_world(&case.bootstrap.world),
                    trusted_assurance_providers: BTreeSet::new(),
                    objects: Vec::new(),
                },
            )
            .map_err(|error| RunError::Storage(error.to_string()))?;
            Ok(Driver::Kernel { kernel })
        }
    }
}

/// Opens a second instance over the same storage, which is what a restarted
/// process does. The first instance is dropped by the caller's assignment.
fn reopen_driver(
    system: SystemId,
    case: &AdmissionCase,
    workspace: &CaseWorkspace,
) -> Result<Driver, RunError> {
    match system {
        SystemId::A | SystemId::B => Ok(Driver::Application {
            gate: BaselineA::reopen(&workspace.database, &case.bootstrap.world)
                .map_err(|error| RunError::Storage(describe(error)))?,
            use_uni_seam: system == SystemId::B,
        }),
        SystemId::C => Ok(Driver::Kernel {
            kernel: Kernel::open(&workspace.database, &case.bootstrap.world)
                .map_err(|error| RunError::Storage(error.to_string()))?,
        }),
    }
}

/// The assurance already established for a prior commit.
///
/// A prelude submission is world-state preparation, not a measured decision:
/// its own envelope carries the assurance that was established when it was
/// first admitted. Only the measured proposal goes through the case's declared
/// assurance step.
fn prior_assurance(submission: &PreludeSubmission) -> AssuranceOutcome {
    let change = &submission.change;
    let provider = change
        .assessments
        .first()
        .map(|assessment| assessment.provider.clone())
        .unwrap_or_else(|| "uni".to_owned());
    AssuranceOutcome::Bound {
        provider,
        subject: change.candidate.reference.clone(),
        subject_digest: change.candidate.digest.clone(),
    }
}

fn run_prelude(
    driver: &mut Driver,
    case: &AdmissionCase,
    workspace: &CaseWorkspace,
) -> Result<(), RunError> {
    for (index, submission) in case.prelude.iter().enumerate() {
        let before = driver.revision()?;
        let authority = grants_map(&submission.grants);
        let assurance = prior_assurance(submission);
        let committed = match driver {
            Driver::Application { gate, .. } => {
                let request = application_request(&submission.change);
                matches!(
                    gate.apply(&request, &assurance, &AppAuthority::new(authority))
                        .map_err(|error| RunError::Storage(describe(error)))?,
                    GateOutcome::Applied { .. }
                )
            }
            Driver::Kernel { kernel } => {
                let source = StaticAuthority::new(authority);
                let outcome: Result<SubmissionOutcome, _> =
                    kernel.submit(&submission.change, &source);
                matches!(
                    outcome.map_err(|error| RunError::Storage(error.to_string()))?,
                    SubmissionOutcome::Committed(_)
                )
            }
        };
        let after = driver.revision()?;
        let _ = (case, workspace);
        if !committed || after != before + 1 {
            return Err(RunError::PreludeDidNotCommit {
                case_id: case.id.clone(),
                index,
                detail: format!("revision went from {before} to {after}"),
            });
        }
    }
    Ok(())
}

fn fault_statement(fault: FaultPoint, system: SystemId) -> Option<String> {
    let (objects, scopes, events, requests) = match system {
        SystemId::A | SystemId::B => ("app_objects", "app_scopes", "app_events", "app_requests"),
        SystemId::C => ("objects", "worlds", "events", "submissions"),
    };
    let (table, timing) = match fault {
        FaultPoint::BeforeObjectWrite => (objects, "BEFORE INSERT"),
        FaultPoint::BeforeWorldRevision => (scopes, "BEFORE UPDATE"),
        FaultPoint::BeforeEventAppend => (events, "BEFORE INSERT"),
        FaultPoint::BeforeSubmissionRecord => (requests, "BEFORE INSERT"),
        FaultPoint::LostResponseAfterCommit => return None,
    };
    Some(format!(
        "CREATE TRIGGER bench_fault {timing} ON {table} BEGIN SELECT RAISE(ABORT, 'simulated interruption'); END;"
    ))
}

/// Runs one case against one system and returns a structured result.
pub fn run_case(
    system: SystemId,
    case: &AdmissionCase,
    ablation: Option<Ablation>,
) -> Result<CaseResult, RunError> {
    let started = Instant::now();
    let workspace = CaseWorkspace::prepare(case)?;
    let mut driver = build_driver(system, case, &workspace)?;
    run_prelude(&mut driver, case, &workspace)?;

    let assurance = driver.assure(case, &workspace);
    workspace.apply_after_assurance(case)?;

    let outcome = match case.required_capability() {
        Some(_) => ActualOutcome::unsupported(),
        None => measure(system, case, &mut driver, &workspace, &assurance, ablation)?,
    };

    Ok(CaseResult {
        case_id: case.id.clone(),
        system,
        correct: correct(case, &outcome),
        actual: outcome,
        expected: ExpectedOutcomeRecord {
            status: case.expected.status,
            code: case.expected.code,
        },
        duration_micros: started.elapsed().as_micros() as u64,
    })
}

fn measure(
    system: SystemId,
    case: &AdmissionCase,
    driver: &mut Driver,
    workspace: &CaseWorkspace,
    assurance: &AssuranceOutcome,
    ablation: Option<Ablation>,
) -> Result<ActualOutcome, RunError> {
    let Some(fault) = case.fault else {
        return driver.submit(case, workspace, assurance, ablation);
    };
    match fault {
        FaultPoint::LostResponseAfterCommit => {
            // The commit happens and the response is lost. The instance is
            // dropped and a new one repeats the identical proposal.
            driver.submit(case, workspace, assurance, ablation)?;
            let after_commit = driver.revision()?;
            // The response is lost: the instance is dropped without reading its
            // outcome and a new one repeats the identical proposal.
            *driver = reopen_driver(system, case, workspace)?;
            let recovered = driver.submit(case, workspace, assurance, None)?;
            let after_recovery = driver.revision()?;
            if recovered.status == ActualStatus::Committed && after_recovery == after_commit {
                Ok(ActualOutcome::committed())
            } else {
                Ok(ActualOutcome::error())
            }
        }
        injected => {
            let statement =
                fault_statement(injected, system).expect("durable write classes are mapped");
            let connection = rusqlite::Connection::open(&workspace.database)
                .map_err(|error| RunError::Storage(error.to_string()))?;
            connection
                .execute_batch(&statement)
                .map_err(|error| RunError::Storage(error.to_string()))?;
            let before = driver.revision()?;
            let interrupted = driver.submit(case, workspace, assurance, ablation);
            let after = driver.revision()?;
            if interrupted.is_ok() {
                return Err(RunError::InterruptedFaultDidNotInterrupt {
                    case_id: case.id.clone(),
                    detail: format!("{injected:?} still admitted the change"),
                });
            }
            if after != before {
                return Err(RunError::FaultAdvancedState {
                    case_id: case.id.clone(),
                    detail: format!("{injected:?} moved the revision from {before} to {after}"),
                });
            }
            connection
                .execute_batch("DROP TRIGGER bench_fault;")
                .map_err(|error| RunError::Storage(error.to_string()))?;
            driver.submit(case, workspace, assurance, ablation)
        }
    }
}

fn correct(case: &AdmissionCase, actual: &ActualOutcome) -> bool {
    let admitted = actual.status == ActualStatus::Committed;
    let should_commit = case.expected.status == ExpectedStatus::Committed;
    if actual.status == ActualStatus::Error || admitted != should_commit {
        return false;
    }
    actual.code == case.expected.code
}

/// A whole run: every system against every case.
#[derive(Debug, Serialize)]
pub struct Benchmark {
    pub results: Vec<CaseResult>,
}

impl Benchmark {
    pub fn run(corpus: &Corpus) -> Result<Self, RunError> {
        let mut results = Vec::with_capacity(corpus.cases.len() * SystemId::ALL.len());
        for case in &corpus.cases {
            for system in SystemId::ALL {
                results.push(run_case(system, case, None)?);
            }
        }
        Ok(Self { results })
    }

    pub fn run_with_ablation(
        corpus: &Corpus,
        system: SystemId,
        ablation: Ablation,
    ) -> Result<Vec<CaseResult>, RunError> {
        let mut results = Vec::new();
        for case in &corpus.cases {
            results.push(run_case(system, case, Some(ablation))?);
        }
        Ok(results)
    }

    pub fn for_system(&self, system: SystemId) -> Vec<&CaseResult> {
        self.results
            .iter()
            .filter(|result| result.system == system)
            .collect()
    }

    /// Adverse cases a system admitted anyway.
    pub fn incorrect_admissions(&self, system: SystemId) -> Vec<&CaseResult> {
        self.for_system(system)
            .into_iter()
            .filter(|result| {
                result.expected.status != ExpectedStatus::Committed
                    && result.actual.status == ActualStatus::Committed
            })
            .collect()
    }

    /// Benign cases a system refused without cause.
    pub fn unjustified_rejections(&self, system: SystemId) -> Vec<&CaseResult> {
        self.for_system(system)
            .into_iter()
            .filter(|result| {
                result.expected.status == ExpectedStatus::Committed
                    && result.actual.status == ActualStatus::Rejected
            })
            .collect()
    }

    pub fn errors(&self, system: SystemId) -> Vec<&CaseResult> {
        self.for_system(system)
            .into_iter()
            .filter(|result| result.actual.status == ActualStatus::Error)
            .collect()
    }

    pub fn failures(&self) -> Vec<&CaseResult> {
        self.results
            .iter()
            .filter(|result| !result.correct)
            .collect()
    }

    pub fn unsupported_by_family(
        &self,
        system: SystemId,
        corpus: &Corpus,
    ) -> BTreeMap<CaseFamily, usize> {
        let mut counts: BTreeMap<CaseFamily, usize> = BTreeMap::new();
        for result in self.for_system(system) {
            if result.actual.status == ActualStatus::Unsupported {
                let family = corpus
                    .cases
                    .iter()
                    .find(|case| case.id == result.case_id)
                    .map(|case| case.family);
                if let Some(family) = family {
                    *counts.entry(family).or_insert(0) += 1;
                }
            }
        }
        counts
    }

    /// Every case where two systems reached different decision codes.
    pub fn decision_disagreements(&self, corpus: &Corpus) -> Vec<DecisionDisagreement> {
        let mut disagreements = Vec::new();
        for case in &corpus.cases {
            let code_of = |system: SystemId| {
                self.results
                    .iter()
                    .find(|result| result.system == system && result.case_id == case.id)
                    .and_then(|result| result.actual.code)
            };
            let a = code_of(SystemId::A);
            let b = code_of(SystemId::B);
            let c = code_of(SystemId::C);
            if a != b || b != c {
                disagreements.push(DecisionDisagreement {
                    case_id: case.id.clone(),
                    a,
                    b,
                    c,
                });
            }
        }
        disagreements
    }

    pub fn timings_micros(&self, system: SystemId) -> Vec<u64> {
        let mut values: Vec<u64> = self
            .for_system(system)
            .into_iter()
            .map(|result| result.duration_micros)
            .collect();
        values.sort_unstable();
        values
    }
}

#[derive(Debug, Serialize)]
pub struct DecisionDisagreement {
    pub case_id: String,
    pub a: Option<BenchmarkCode>,
    pub b: Option<BenchmarkCode>,
    pub c: Option<BenchmarkCode>,
}

pub fn median(values: &[u64]) -> u64 {
    if values.is_empty() {
        return 0;
    }
    values[values.len() / 2]
}

pub fn percentile_95(values: &[u64]) -> u64 {
    if values.is_empty() {
        return 0;
    }
    let index = (values.len() as f64 * 0.95).ceil() as usize;
    values[index.saturating_sub(1).min(values.len() - 1)]
}

/// The support surface all three systems declare. The report prints it so an
/// over-broad unsupported claim is visible rather than hidden in a total.
pub fn declared_support() -> Value {
    json!({
        "systems": ["A", "B", "C"],
        "note": "a case outside this surface is reported unsupported, never committed",
        "supportedCoverageProfiles": super::admission_case::SUPPORTED_COVERAGE_PROFILES,
        "supportedIntentKinds": super::admission_case::SUPPORTED_INTENT_KINDS,
        "unsupportedCoverageProfiles": super::admission_case::UNSUPPORTED_COVERAGE_PROFILES,
        "unsupportedIntentKinds": super::admission_case::UNSUPPORTED_INTENT_KINDS,
    })
}

pub fn expected_assurance_code(case: &AdmissionCase) -> Option<&'static str> {
    expected_code_for(case.assurance.mode)
}
