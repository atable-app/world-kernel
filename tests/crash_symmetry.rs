//! Crash symmetry: the same observable interruption points for A2, B2 and C2.
//!
//! The M1 harness proved that a trigger-raised SQLite error rolls a commit back, but it observed that
//! from inside a process that then unwound normally. A process that disappears is a different
//! experience: no destructor runs, no error is returned to anyone, and the only evidence is what a later
//! process can read. This suite adds that second observation, for all three systems, at the same three
//! points: before the transaction, after a write but before commit, and after commit but before the
//! response.
//!
//! The child is this same test binary re-executed with an env var, so the interruption happens in a
//! process the test itself created. No user session or process is ever signalled. The child leaves
//! through `std::process::exit`, which skips every destructor including the SQLite connection's own
//! cleanup, and for the pre-commit point it dies *inside* the failed submit rather than after it.
//!
//! No production control receives a disable flag. The pre-commit point is produced by installing a
//! temporary trigger on the last durable write, exactly as M1 did, and then exiting rather than
//! unwinding.
//!
//! Scope limit, stated rather than hidden: C2 has a replay seam (`Kernel::replay`) and A2 and B2 do
//! not yet have one, because an application-level replay is part of the A2 baseline that M2 has not
//! built yet. The divergence assertions below therefore run for C2 only, and the absence is recorded
//! as a measured gap rather than papered over.

mod support;

use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    process::Command,
};

use support::{
    admission_case::digest_bytes,
    baseline_a::{AppAuthority, BaselineA, GateOutcome, PublishRequest},
};
use world_kernel::{
    Coverage, GroundedChange, Intent, Kernel, ObjectRevision, Patch, StaticAuthority,
    SubmissionOutcome, WorldBootstrap,
};

const WORLD: &str = "world:crash";
const ACTOR: &str = "principal:worker";
const INTENT: &str = "publishCandidate";
const CANDIDATE: &str = "artifact:release";
const REQUIREMENT: &str = "requirement:release";

const CHILD_ROLE: &str = "WK_CRASH_CHILD";
const CHILD_POINT: &str = "WK_CRASH_POINT";
const CHILD_DATABASE: &str = "WK_CRASH_DATABASE";
const CHILD_SYSTEM: &str = "WK_CRASH_SYSTEM";
const CHILD_KEY: &str = "WK_CRASH_KEY";
const CHILD_ASSURANCE: &str = "WK_CRASH_ASSURANCE";

fn candidate_digest() -> String {
    digest_bytes(b"release-candidate-a")
}

fn requirement_digest() -> String {
    digest_bytes(b"requirement-release-v1")
}

fn bootstrap_objects() -> Vec<ObjectRevision> {
    vec![ObjectRevision {
        reference: REQUIREMENT.to_owned(),
        revision: 1,
        digest: requirement_digest(),
    }]
}

fn trust() -> BTreeSet<String> {
    BTreeSet::from(["uni".to_owned()])
}

fn change(candidate_digest: &str, key: &str, base_revision: u64) -> GroundedChange {
    GroundedChange {
        schema: "world-change/v0-experimental".into(),
        world: WORLD.into(),
        proposal_id: format!("change:{key}"),
        idempotency_key: key.into(),
        actor: ACTOR.into(),
        base_revision,
        intent: Intent {
            kind: INTENT.into(),
            target: CANDIDATE.into(),
        },
        candidate: world_kernel::Candidate {
            reference: CANDIDATE.into(),
            digest: candidate_digest.into(),
        },
        reads: vec![ObjectRevision {
            reference: REQUIREMENT.into(),
            revision: 1,
            digest: requirement_digest(),
        }],
        coverage: Coverage {
            profile: "closed-v1".into(),
            complete_for: BTreeSet::from([INTENT.to_owned()]),
            truncated: false,
        },
        assessments: vec![world_kernel::AcceptedAssessment {
            provider: "uni".into(),
            subject: CANDIDATE.into(),
            subject_digest: candidate_digest.into(),
            assessment_ref: "uni-bound:crash-suite".into(),
        }],
        patches: vec![Patch::PutObject {
            reference: CANDIDATE.into(),
            expected_revision: None,
            digest: candidate_digest.into(),
        }],
    }
}

/// Runs the real application verifier over real candidate bytes.
///
/// The durability of a commit is what this suite measures, so assurance is kept
/// an honest input rather than a shortcut: the bytes on disk hash to exactly the
/// digest the proposal declares, and the gate's own binding check therefore
/// still has to pass.
fn assurance_for(
    workspace: &Path,
    bytes: &[u8],
    candidate_reference: &str,
) -> support::assurance::AssuranceOutcome {
    let path = workspace.join("candidate.bin");
    std::fs::write(&path, bytes).expect("the candidate bytes are written");
    support::assurance::app_verify(
        support::admission_case::AssuranceMode::Accepted,
        "uni",
        workspace,
        "candidate.bin",
        candidate_reference,
    )
}

fn candidate_bytes(declared: &str) -> Vec<u8> {
    if declared == candidate_digest() {
        b"release-candidate-a".to_vec()
    } else {
        b"release-candidate-b".to_vec()
    }
}

fn kernel_authority() -> StaticAuthority {
    StaticAuthority::new([(ACTOR.to_owned(), BTreeSet::from([INTENT.to_owned()]))])
}

/// The application request that system A2 and B2 receive, translated from the
/// same change so all three systems are fed the same declared information.
fn application_request(change: &GroundedChange) -> PublishRequest {
    support::harness::application_request(change)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InterruptionPoint {
    BeforeTransaction,
    AfterWriteBeforeCommit,
    AfterCommitBeforeResponse,
}

impl InterruptionPoint {
    const ALL: [InterruptionPoint; 3] = [
        Self::BeforeTransaction,
        Self::AfterWriteBeforeCommit,
        Self::AfterCommitBeforeResponse,
    ];

    fn as_str(&self) -> &'static str {
        match self {
            Self::BeforeTransaction => "beforeTransaction",
            Self::AfterWriteBeforeCommit => "afterWriteBeforeCommit",
            Self::AfterCommitBeforeResponse => "afterCommitBeforeResponse",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum System {
    A2,
    B2,
    C2,
}

impl System {
    const ALL: [System; 3] = [System::A2, System::B2, System::C2];

    fn label(&self) -> &'static str {
        match self {
            Self::A2 => "A2",
            Self::B2 => "B2",
            Self::C2 => "C2",
        }
    }

    fn parse(label: &str) -> System {
        match label {
            "A2" => Self::A2,
            "B2" => Self::B2,
            _ => Self::C2,
        }
    }

    fn is_application(&self) -> bool {
        matches!(self, Self::A2 | Self::B2)
    }
}

// ------------------------------------------------------------------ the child

/// Creates the world once, attempts the measured change, then disappears.
///
/// The child never returns. Every path leaves through `std::process::exit`, so
/// no destructor observes the failure and no response reaches anyone.
fn child_main() -> ! {
    let point = InterruptionPoint::ALL
        .iter()
        .copied()
        .find(|candidate| candidate.as_str() == std::env::var(CHILD_POINT).unwrap_or_default())
        .expect("the child knows its interruption point");
    let database =
        PathBuf::from(std::env::var(CHILD_DATABASE).expect("the child knows its storage"));
    let system = System::parse(&std::env::var(CHILD_SYSTEM).expect("the child knows its system"));
    let key = std::env::var(CHILD_KEY).expect("the child knows its proposal");
    let assurance =
        PathBuf::from(std::env::var(CHILD_ASSURANCE).expect("the child knows its verifier"));
    let proposal = change(&candidate_digest(), &key, 0);

    if system.is_application() {
        let mut gate = BaselineA::create(&database, WORLD, trust(), &bootstrap_objects())
            .expect("the child creates the application world once");
        if point == InterruptionPoint::BeforeTransaction {
            std::process::exit(17);
        }
        if point == InterruptionPoint::AfterWriteBeforeCommit {
            gate.install_interrupted_commit()
                .expect("the child can arm its last durable write");
        }
        let request = application_request(&proposal);
        let assessment =
            assurance_for(&assurance, &candidate_bytes(&candidate_digest()), CANDIDATE);
        let outcome = gate
            .apply(&request, &assessment, &AppAuthority::new(grants()))
            .expect_err("an armed commit cannot succeed");
        assert!(
            matches!(outcome, support::baseline_a::GateError::Storage(_)),
            "the child was interrupted by storage, not by policy: {outcome:?}"
        );
        // The commit was never durable. Die here rather than unwinding.
        std::process::exit(18);
    }

    let mut kernel = Kernel::create(
        &database,
        WorldBootstrap {
            world: WORLD.into(),
            trusted_assurance_providers: trust(),
            objects: bootstrap_objects(),
        },
    )
    .expect("the child creates the kernel world once");
    if point == InterruptionPoint::BeforeTransaction {
        std::process::exit(17);
    }
    if point == InterruptionPoint::AfterWriteBeforeCommit {
        arm_kernel_last_durable_write(&database);
    }
    let outcome = kernel.submit(&proposal, &kernel_authority());
    if point == InterruptionPoint::AfterWriteBeforeCommit {
        assert!(outcome.is_err(), "an armed kernel commit cannot succeed");
        // The commit was never durable. Die here rather than unwinding.
        std::process::exit(18);
    }
    assert!(matches!(outcome, Ok(SubmissionOutcome::Committed(_))));
    // The work is durable; the response is lost.
    std::process::exit(19);
}

/// Arms the last durable write of a Kernel commit. The trigger aborts inside
/// the transaction, so the process dies with the transaction open.
fn arm_kernel_last_durable_write(database: &Path) {
    rusqlite::Connection::open(database)
        .expect("the child reopens its own storage")
        .execute_batch(
            "CREATE TRIGGER kernel_interrupted_commit
             BEFORE INSERT ON submissions
             BEGIN
               SELECT RAISE(ABORT, 'simulated interruption before commit');
             END;",
        )
        .expect("the child can arm its last durable write");
}

/// Removes the interruption this suite injected.
///
/// The trigger is durable in the database, so the parent's retry is refused by
/// it until it is dropped. Recovering from an interrupted commit means the
/// interruption has ended, not that the retry is expected to succeed anyway.
fn clear_injected_interruption(database: &Path) {
    let connection = rusqlite::Connection::open(database).expect("the parent can reopen storage");
    for trigger in ["kernel_interrupted_commit", "app_interrupted_commit"] {
        connection
            .execute_batch(&format!("DROP TRIGGER IF EXISTS {trigger};"))
            .expect("the parent can drop the injected trigger");
    }
}

fn grants() -> std::collections::BTreeMap<String, BTreeSet<String>> {
    std::collections::BTreeMap::from([(ACTOR.to_owned(), BTreeSet::from([INTENT.to_owned()]))])
}

// ------------------------------------------------------- the parent's view

/// Reads and retries through the system's own seam, never through another's.
struct Handle {
    system: System,
    database: PathBuf,
    assurance: PathBuf,
}

#[derive(Debug, PartialEq, Eq)]
enum Decision {
    Committed,
    Refused(String),
}

impl Handle {
    fn new(system: System, database: &Path, assurance: &Path) -> Self {
        Self {
            system,
            database: database.to_path_buf(),
            assurance: assurance.to_path_buf(),
        }
    }

    fn revision(&self) -> u64 {
        match self.system {
            System::C2 => {
                Kernel::open(&self.database, WORLD)
                    .expect("the kernel storage stays openable")
                    .snapshot()
                    .expect("the snapshot is readable")
                    .revision
            }
            _ => BaselineA::reopened(&self.database, WORLD)
                .expect("the application storage stays openable")
                .revision()
                .expect("the application revision is readable"),
        }
    }

    fn candidate(&self) -> Option<(u64, String)> {
        match self.system {
            System::C2 => Kernel::open(&self.database, WORLD)
                .expect("the kernel storage stays openable")
                .snapshot()
                .expect("the snapshot is readable")
                .objects
                .get(CANDIDATE)
                .map(|state| (state.revision, state.digest.clone())),
            _ => BaselineA::reopened(&self.database, WORLD)
                .expect("the application storage stays openable")
                .object(CANDIDATE)
                .expect("the application object is readable"),
        }
    }

    fn submit(&self, change: &GroundedChange) -> Result<Decision, String> {
        match self.system {
            System::C2 => {
                let mut kernel = Kernel::open(&self.database, WORLD).map_err(|e| e.to_string())?;
                match kernel
                    .submit(change, &kernel_authority())
                    .map_err(|e| e.to_string())?
                {
                    SubmissionOutcome::Committed(_) => Ok(Decision::Committed),
                    SubmissionOutcome::Rejected(rejection) => Ok(Decision::Refused(
                        support::admission_case::BenchmarkCode::from(&rejection.code)
                            .as_str()
                            .to_owned(),
                    )),
                }
            }
            _ => {
                let mut gate =
                    BaselineA::reopened(&self.database, WORLD).map_err(|e| e.to_string())?;
                let request = application_request(change);
                let assessment = assurance_for(
                    &self.assurance,
                    &candidate_bytes(&change.candidate.digest),
                    CANDIDATE,
                );
                match gate
                    .apply(&request, &assessment, &AppAuthority::new(grants()))
                    .map_err(|e| e.to_string())?
                {
                    GateOutcome::Applied { .. } => Ok(Decision::Committed),
                    GateOutcome::Refused(code) => Ok(Decision::Refused(code.as_str().to_owned())),
                }
            }
        }
    }

    /// Replay is only available where a system implements it. Its absence is
    /// reported, not assumed.
    fn replay_matches_snapshot(&self) -> Option<bool> {
        match self.system {
            System::C2 => {
                let kernel = Kernel::open(&self.database, WORLD).ok()?;
                Some(kernel.replay().ok()? == kernel.snapshot().ok()?)
            }
            _ => None,
        }
    }
}

fn run_child(
    system: System,
    point: InterruptionPoint,
    database: &Path,
    assurance: &Path,
    key: &str,
) -> std::process::Output {
    Command::new(std::env::current_exe().expect("the test binary is re-executable"))
        .env(CHILD_ROLE, "1")
        .env(CHILD_POINT, point.as_str())
        .env(CHILD_DATABASE, database)
        .env(CHILD_SYSTEM, system.label())
        .env(CHILD_KEY, key)
        .env(CHILD_ASSURANCE, assurance)
        .arg("--exact")
        .arg("child_process_entry_point_is_a_no_op_when_not_re_executed")
        .output()
        .expect("the test can re-execute itself")
}

/// The parent half. When re-executed as the child, this test is a no-op so the
/// child's exact filter matches nothing but itself.
#[test]
fn child_process_entry_point_is_a_no_op_when_not_re_executed() {
    if std::env::var(CHILD_ROLE).as_deref() == Ok("1") {
        child_main();
    }
}

// ------------------------------------------------------------- the assertions

#[test]
fn a_process_that_disappears_leaves_the_recorded_state_untouched() {
    for system in System::ALL {
        for point in [
            InterruptionPoint::BeforeTransaction,
            InterruptionPoint::AfterWriteBeforeCommit,
        ] {
            let directory = tempfile::tempdir().expect("a temporary directory");
            let assurance = tempfile::tempdir().expect("a verifier workspace");
            let database = directory.path().join("world.db");
            let handle = Handle::new(system, &database, assurance.path());
            let key = format!("crash-{}", point.as_str());

            let output = run_child(system, point, &database, assurance.path(), &key);

            assert!(
                !output.status.success(),
                "{} at {} was expected to disappear",
                system.label(),
                point.as_str()
            );
            assert_eq!(
                handle.revision(),
                0,
                "{} at {} advanced the world across a process death",
                system.label(),
                point.as_str()
            );
            assert!(
                handle.candidate().is_none(),
                "{} at {} published the candidate across a process death",
                system.label(),
                point.as_str()
            );

            clear_injected_interruption(&database);

            // Retrying the same logical change produces exactly one transition.
            assert_eq!(
                handle.submit(&change(&candidate_digest(), &key, 0)),
                Ok(Decision::Committed),
                "{} cannot recover at {}",
                system.label(),
                point.as_str()
            );
            assert_eq!(handle.revision(), 1, "{}", system.label());
            assert_eq!(
                handle.candidate().map(|(revision, _)| revision),
                Some(1),
                "{} published a surprising object revision",
                system.label()
            );
            if let Some(matches) = handle.replay_matches_snapshot() {
                assert!(
                    matches,
                    "{} diverged between snapshot and replay",
                    system.label()
                );
            }

            // Repeating it once more is a stable response, not a second transition.
            assert_eq!(
                handle.submit(&change(&candidate_digest(), &key, 0)),
                Ok(Decision::Committed),
                "{} lost its idempotent response",
                system.label()
            );
            assert_eq!(
                handle.revision(),
                1,
                "{} double-applied one logical change",
                system.label()
            );
        }
    }
}

#[test]
fn a_lost_response_after_commit_is_recovered_by_repeating_the_identical_proposal() {
    for system in System::ALL {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let assurance = tempfile::tempdir().expect("a verifier workspace");
        let database = directory.path().join("world.db");
        let handle = Handle::new(system, &database, assurance.path());
        let key = "crash-after-commit-before-response";

        let output = run_child(
            system,
            InterruptionPoint::AfterCommitBeforeResponse,
            &database,
            assurance.path(),
            key,
        );

        assert!(!output.status.success());
        assert_eq!(
            handle.revision(),
            1,
            "{} lost a commit it had already made",
            system.label()
        );
        assert!(handle.candidate().is_some(), "{}", system.label());

        assert_eq!(
            handle.submit(&change(&candidate_digest(), key, 0)),
            Ok(Decision::Committed),
            "{} could not recover the stored outcome",
            system.label()
        );
        assert_eq!(
            handle.revision(),
            1,
            "{} advanced the world twice for one change",
            system.label()
        );
        if let Some(matches) = handle.replay_matches_snapshot() {
            assert!(matches, "{}", system.label());
        }
    }
}

#[test]
fn a_reused_identity_on_new_content_is_a_conflict_not_a_retry() {
    for system in System::ALL {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let assurance = tempfile::tempdir().expect("a verifier workspace");
        let database = directory.path().join("world.db");
        let handle = Handle::new(system, &database, assurance.path());
        let key = "crash-identity";

        let output = run_child(
            system,
            InterruptionPoint::AfterCommitBeforeResponse,
            &database,
            assurance.path(),
            key,
        );
        assert!(!output.status.success());
        assert_eq!(handle.revision(), 1, "{}", system.label());

        // The same key carrying different content, on the new revision.
        let conflicting = change(&digest_bytes(b"release-candidate-b"), key, 1);
        let decision = handle.submit(&conflicting);

        assert_eq!(
            decision,
            Ok(Decision::Refused("IDEMPOTENCY_CONFLICT".into())),
            "{} must treat a reused identity with new content as a conflict",
            system.label()
        );
        assert_eq!(
            handle.revision(),
            1,
            "{} changed state for a conflicting retry",
            system.label()
        );
    }
}

#[test]
fn the_child_really_dies_and_the_parent_really_observes() {
    // Guards the harness itself: without a real process death these tests would
    // pass for the wrong reason.
    let directory = tempfile::tempdir().expect("a temporary directory");
    let assurance = tempfile::tempdir().expect("a verifier workspace");
    let database = directory.path().join("world.db");
    let output = run_child(
        System::C2,
        InterruptionPoint::BeforeTransaction,
        &database,
        assurance.path(),
        "harness-self-check",
    );

    assert_eq!(output.status.code(), Some(17));
    assert!(Kernel::open(&database, WORLD).is_ok());
}
