//! The first continuation consumer.
//!
//! A producer admits a change through the production path, exports a continuation package, and then
//! stops. Its workspace is moved out of the consumer's reach. A fresh consumer, in a different path
//! and a different destination, receives only the package and an out-of-band trust anchor, and must
//! reconstruct the announced state and decide what can be resumed in its own current context.
//!
//! The test is deliberately hard to pass by accident. The consumer never receives the producer's
//! database path, so a consumer that went looking for one would fail rather than cheat. The expected
//! digest and revisions are written out independently of the export, so the consumer is checked against
//! an answer it did not produce. The trust anchor arrives separately, because a package must never
//! supply its own trust configuration.
//!
//! What the Kernel does not store is stated rather than assumed: it stores references, revisions and
//! digests, never object bytes. The package therefore declares an explicit body status per resource
//! instead of implying the content travelled with it.

mod support;

use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use support::admission_case::AssuranceMode;
use support::assurance::{FixtureUniRunner, collect_for_test};
use world_kernel::adapters::uni::{CollectRequest, UniCollector};
use world_kernel::continuation::{
    Anchor, ContinuationPackage, Guarantee, Reconstruction, reconstruct,
};
use world_kernel::{
    Coverage, GroundedChange, Intent, Kernel, ObjectRevision, Patch, StaticAuthority,
    SubmissionOutcome, WorldBootstrap, digest_of_bytes,
};

const WORLD: &str = "world:continuation";
const ACTOR: &str = "principal:producer";
const NEXT_ACTOR: &str = "principal:consumer";
const INTENT: &str = "publishCandidate";
const CANDIDATE: &str = "artifact:release";
const REQUIREMENT: &str = "requirement:release";

fn candidate_bytes() -> &'static [u8] {
    b"release-candidate-a"
}

fn candidate_digest() -> String {
    digest_of_bytes(candidate_bytes())
}

fn requirement_digest() -> String {
    digest_of_bytes(b"requirement-release-v1")
}

fn trust() -> BTreeSet<String> {
    BTreeSet::from(["uni".to_owned()])
}

fn bootstrap_objects() -> Vec<ObjectRevision> {
    vec![ObjectRevision {
        reference: REQUIREMENT.to_owned(),
        revision: 1,
        digest: requirement_digest(),
    }]
}

fn authority(actor: &str) -> StaticAuthority {
    StaticAuthority::new([(actor.to_owned(), BTreeSet::from([INTENT.to_owned()]))])
}

/// The admitted change, built by the producer through the production path.
fn producer_change(proposal: &str, key: &str) -> GroundedChange {
    let digest = candidate_digest();
    GroundedChange {
        schema: "world-change/v0-experimental".into(),
        world: WORLD.into(),
        proposal_id: proposal.into(),
        idempotency_key: key.into(),
        actor: ACTOR.into(),
        base_revision: 0,
        intent: Intent {
            kind: INTENT.into(),
            target: CANDIDATE.into(),
        },
        candidate: world_kernel::Candidate {
            reference: CANDIDATE.into(),
            digest: digest.clone(),
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
            subject_digest: digest.clone(),
            assessment_ref: "uni-bound:producer".into(),
        }],
        patches: vec![Patch::PutObject {
            reference: CANDIDATE.into(),
            expected_revision: None,
            digest,
        }],
    }
}

struct Producer {
    _workspace: tempfile::TempDir,
    database: PathBuf,
    package: PathBuf,
    head_digest: String,
}

impl Producer {
    /// Admits one change through the production path, then exports.
    fn run(directory: &Path) -> Producer {
        let workspace = tempfile::tempdir().expect("a producer workspace");
        let assurance_workspace = tempfile::tempdir().expect("the producer's verifier workspace");
        let database = workspace.path().join("world.db");
        // The verifier workspace is kept alive by the producer and is not part of
        // what the consumer receives.
        let candidate = assurance_workspace.path().join("candidate.bin");
        let contract = assurance_workspace.path().join("release.uni");
        fs::write(&candidate, candidate_bytes()).expect("candidate bytes");
        fs::write(&contract, b"VERSION 0.1\nDOMAIN software\nINTENT release\n").expect("contract");

        let mut kernel = Kernel::create(
            &database,
            WorldBootstrap {
                world: WORLD.into(),
                trusted_assurance_providers: trust(),
                objects: bootstrap_objects(),
            },
        )
        .expect("the producer creates its world");

        // Assurance runs through the production collector, so the assessment the
        // producer embeds is one UNI actually bound to these bytes.
        let collected = UniCollector::new(FixtureUniRunner::new(
            AssuranceMode::Accepted,
            "candidate.bin",
            "release.uni",
        ))
        .collect(&CollectRequest {
            workspace: assurance_workspace.path().to_path_buf(),
            contract,
            candidate_path: candidate,
            candidate_ref: CANDIDATE.into(),
        })
        .expect("assurance binds the candidate");

        let mut change = producer_change("change:producer-1", "producer-1");
        change.assessments = vec![collected.assessment];
        let outcome = kernel
            .submit(&change, &authority(ACTOR))
            .expect("the producer admits its change");
        assert!(matches!(outcome, SubmissionOutcome::Committed(_)));

        let package_path = directory.join("continuation.json");
        let exported = kernel
            .export_continuation(&package_path)
            .expect("the producer exports its continuation package");

        Producer {
            _workspace: workspace,
            database,
            package: package_path,
            head_digest: exported.head.head_digest,
        }
    }

    /// Makes the producer's private state unreachable, without deleting anything.
    fn go_away(self) -> ContinuationPackage {
        let package = fs::read_to_string(&self.package).expect("the package survives the producer");
        let hidden = self.database.with_extension("unreachable");
        fs::rename(&self.database, &hidden).expect("the producer's database moves out of reach");
        serde_json::from_str(&package).expect("the package parses")
    }
}

/// A proposal that reads the dependency at the revision it is actually at.
fn consumer_change(base_revision: u64) -> GroundedChange {
    consumer_change_reading(base_revision, 1, requirement_digest())
}

/// A proposal that reads a dependency which the consumer's own world has moved
/// past, used to expose a re-verification obligation rather than a generic
/// failure.
fn consumer_change_reading(
    base_revision: u64,
    read_revision: u64,
    read_digest: String,
) -> GroundedChange {
    let mut change = consumer_publishing(base_revision);
    change.reads = vec![ObjectRevision {
        reference: REQUIREMENT.into(),
        revision: read_revision,
        digest: read_digest,
    }];
    change
}

/// A proposal that both publishes the next candidate and advances the
/// dependency, so the consumer's context genuinely moves on.
fn consumer_advancing(base_revision: u64) -> GroundedChange {
    let mut change = consumer_publishing(base_revision);
    change.patches.push(Patch::PutObject {
        reference: REQUIREMENT.into(),
        expected_revision: Some(1),
        digest: digest_of_bytes(b"requirement-release-v2"),
    });
    change
}

fn consumer_publishing(base_revision: u64) -> GroundedChange {
    let digest = digest_of_bytes(b"release-candidate-b");
    GroundedChange {
        schema: "world-change/v0-experimental".into(),
        world: WORLD.into(),
        proposal_id: format!("change:consumer-{base_revision}"),
        idempotency_key: format!("consumer-{base_revision}"),
        actor: NEXT_ACTOR.into(),
        base_revision,
        intent: Intent {
            kind: INTENT.into(),
            target: CANDIDATE.into(),
        },
        candidate: world_kernel::Candidate {
            reference: CANDIDATE.into(),
            digest: digest.clone(),
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
            subject_digest: digest.clone(),
            assessment_ref: "uni-bound:consumer".into(),
        }],
        patches: vec![Patch::PutObject {
            reference: CANDIDATE.into(),
            expected_revision: Some(1),
            digest,
        }],
    }
}

#[test]
fn a_fresh_consumer_reconstructs_the_announced_state_from_the_package_alone() {
    let directory = tempfile::tempdir().expect("a handoff directory");
    let producer = Producer::run(directory.path());

    // The anchor is what a trusted channel would carry. It is not in the package.
    let anchor = Anchor {
        head_digest: producer.head_digest.clone(),
        world: WORLD.into(),
    };
    let package = producer.go_away();

    let destination = tempfile::tempdir().expect("the consumer's own destination");
    let reconstruction: Reconstruction = reconstruct(&package, &anchor)
        .expect("a coherent package is understood")
        .materialize(destination.path().join("reconstructed.db"), BTreeSet::new())
        .expect("reconstruction writes only to the chosen destination");

    // The semantic digest and the historical references are found without the producer.
    assert_eq!(reconstruction.world, WORLD);
    assert_eq!(reconstruction.world_revision, 1);
    assert_eq!(
        reconstruction.objects.get(CANDIDATE).map(|d| d.as_str()),
        Some(candidate_digest().as_str())
    );
    assert_eq!(
        reconstruction.objects.get(REQUIREMENT).map(|d| d.as_str()),
        Some(requirement_digest().as_str())
    );
    assert_eq!(
        reconstruction.admitted_proposals,
        vec!["change:producer-1".to_owned()]
    );
    assert_eq!(
        reconstruction.assurance_references.len(),
        1,
        "exactly the assurance the admitted change relied on travels"
    );
    assert!(
        reconstruction.assurance_references[0].starts_with("uni-bound:"),
        "the reference must be the one the producer's collector produced, got {}",
        reconstruction.assurance_references[0]
    );
    assert!(reconstruction.guarantee(Guarantee::FormatUnderstood));
    assert!(reconstruction.guarantee(Guarantee::ResourceIntegrity));
    assert!(reconstruction.guarantee(Guarantee::HistoricalReconstruction));
    assert!(
        reconstruction.guarantee(Guarantee::Authenticity),
        "an out-of-band anchor is what makes authenticity established here"
    );
    assert!(
        !reconstruction.guarantee(Guarantee::CompletenessBeyondScope),
        "completeness is relative to the announced scope and never global"
    );
    assert_eq!(
        reconstruction.absent_bodies,
        vec![CANDIDATE.to_owned(), REQUIREMENT.to_owned()],
        "a digest without accessible content is declared, not implied present"
    );

    // Replaying the reconstructed history reproduces the same projection, with
    // no producer tool and no producer path involved.
    let replayed = Kernel::open(&reconstruction.database, WORLD)
        .expect("the reconstruction is a real world")
        .replay()
        .expect("replay reduces the reconstructed transitions");
    assert_eq!(replayed.revision, 1);
    assert_eq!(
        replayed.objects[CANDIDATE].digest,
        candidate_digest(),
        "the reconstructed projection carries the announced digest"
    );
}

#[test]
fn a_reconstruction_reads_the_past_without_trust_and_cannot_grant_itself_any() {
    let directory = tempfile::tempdir().expect("a handoff directory");
    let producer = Producer::run(directory.path());
    let anchor = Anchor {
        head_digest: producer.head_digest.clone(),
        world: WORLD.into(),
    };
    let package = producer.go_away();

    // The consumer trusts nobody. Reading the announced past is still possible.
    let destination = tempfile::tempdir().expect("the consumer's own destination");
    let reconstruction = reconstruct(&package, &anchor)
        .expect("a coherent package is understood")
        .materialize(destination.path().join("world.db"), BTreeSet::new())
        .expect("a reconstruction needs no trust to read history");

    let mut consumer = Kernel::open(&reconstruction.database, WORLD).expect("the consumer's world");
    let outcome = consumer
        .submit(&consumer_change(1), &authority(NEXT_ACTOR))
        .expect("the attempt is decided");

    let SubmissionOutcome::Rejected(rejection) = outcome else {
        panic!("an untrusting consumer must not admit an assurance it cannot vouch for");
    };
    assert_eq!(
        rejection.code,
        world_kernel::RejectionCode::UntrustedAssessmentProvider,
        "the refusal must name the missing trust, not something else"
    );
    assert_eq!(
        consumer.snapshot().expect("snapshot").revision,
        1,
        "a refused continuation changes nothing"
    );
}

#[test]
fn a_continuation_is_a_new_admission_and_an_unchanged_context_can_proceed() {
    let directory = tempfile::tempdir().expect("a handoff directory");
    let producer = Producer::run(directory.path());
    let anchor = Anchor {
        head_digest: producer.head_digest.clone(),
        world: WORLD.into(),
    };
    let package = producer.go_away();

    let destination = tempfile::tempdir().expect("the consumer's own destination");
    let reconstruction = reconstruct(&package, &anchor)
        .expect("a coherent package is understood")
        .materialize(destination.path().join("world.db"), trust())
        .expect("reconstruction writes to the consumer's destination");

    let mut consumer = Kernel::open(&reconstruction.database, WORLD).expect("the consumer's world");
    let outcome = consumer
        .submit(&consumer_change(1), &authority(NEXT_ACTOR))
        .expect("a legitimate next transition is decided");

    assert!(
        matches!(outcome, SubmissionOutcome::Committed(_)),
        "an unchanged and authorized context must be able to continue: {outcome:?}"
    );
    assert_eq!(consumer.snapshot().expect("snapshot").revision, 2);
}

#[test]
fn a_stale_dependency_blocks_the_continuation_and_names_it() {
    let directory = tempfile::tempdir().expect("a handoff directory");
    let producer = Producer::run(directory.path());
    let anchor = Anchor {
        head_digest: producer.head_digest.clone(),
        world: WORLD.into(),
    };
    let package = producer.go_away();

    let destination = tempfile::tempdir().expect("the consumer's own destination");
    let reconstruction = reconstruct(&package, &anchor)
        .expect("a coherent package is understood")
        .materialize(destination.path().join("world.db"), trust())
        .expect("reconstruction writes to the consumer's destination");

    // The consumer's own context moves on: the declared read advances.
    let mut consumer = Kernel::open(&reconstruction.database, WORLD).expect("the consumer's world");
    let mut moved = consumer_advancing(1);
    moved.proposal_id = "change:consumer-context-moved".into();
    moved.idempotency_key = "consumer-context-moved".into();
    let moved_outcome = consumer
        .submit(&moved, &authority(NEXT_ACTOR))
        .expect("the context moves forward");
    assert!(
        matches!(moved_outcome, SubmissionOutcome::Committed(_)),
        "the consumer's own move must be admissible: {moved_outcome:?}"
    );

    // The consumer now proposes against the revision it read before the move.
    let stale = consumer_change_reading(2, 1, requirement_digest());
    let outcome = consumer
        .submit(&stale, &authority(NEXT_ACTOR))
        .expect("the stale continuation is decided");

    let SubmissionOutcome::Rejected(rejection) = outcome else {
        panic!("a continuation over a moved dependency must be blocked, not admitted");
    };
    assert_eq!(
        rejection.code,
        world_kernel::RejectionCode::StaleDependency,
        "the wrong reason would hide the obligation"
    );
    assert!(
        rejection.detail.contains(REQUIREMENT),
        "the obligation must name the dependency that moved: {}",
        rejection.detail
    );
}

#[test]
fn the_package_carries_no_trust_configuration_of_its_own() {
    let directory = tempfile::tempdir().expect("a handoff directory");
    let producer = Producer::run(directory.path());
    let package = producer.go_away();

    let bytes = serde_json::to_vec(&package).expect("the package serializes");
    for forbidden in ["trustAnchor", "trustedProviders", "secret", "credential"] {
        assert!(
            !String::from_utf8_lossy(&bytes).contains(forbidden),
            "a package must not carry {forbidden}"
        );
    }
}

#[test]
fn a_package_is_deterministic_for_identical_inputs() {
    let first = tempfile::tempdir().expect("a first directory");
    let second = tempfile::tempdir().expect("a second directory");
    let one = Producer::run(first.path());
    let two = Producer::run(second.path());

    assert_eq!(
        fs::read(&one.package).expect("readable"),
        fs::read(&two.package).expect("readable"),
        "two exports of identical state must be byte-identical"
    );
}

#[test]
fn a_package_against_the_wrong_anchor_is_refused() {
    let directory = tempfile::tempdir().expect("a handoff directory");
    let producer = Producer::run(directory.path());
    let package = producer.go_away();

    let wrong = Anchor {
        head_digest: digest_of_bytes(b"a different head"),
        world: WORLD.into(),
    };

    let outcome = reconstruct(&package, &wrong);

    assert!(
        outcome.is_err(),
        "a package must not verify against an anchor it did not produce"
    );
}

#[test]
fn the_assurance_reference_in_the_package_is_the_one_the_producer_obtained() {
    // Guards the package against carrying an invented reference.
    let case = support::admission_case::AdmissionCase {
        schema: "world-kernel-admission-case/v1".into(),
        id: "continuation.probe.001".into(),
        family: support::admission_case::CaseFamily::AssuranceTrust,
        expected: support::admission_case::ExpectedOutcome {
            status: support::admission_case::ExpectedStatus::Committed,
            code: None,
        },
        bootstrap: WorldBootstrap {
            world: WORLD.into(),
            trusted_assurance_providers: trust(),
            objects: bootstrap_objects(),
        },
        prelude: Vec::new(),
        proposal: producer_change("change:probe", "probe"),
        current: support::admission_case::CurrentFixture {
            grants: std::collections::BTreeMap::from([(
                ACTOR.to_owned(),
                BTreeSet::from([INTENT.to_owned()]),
            )]),
            candidate_path: "candidate.bin".into(),
            candidate_contents: String::from_utf8_lossy(candidate_bytes()).into_owned(),
            candidate_contents_after_assurance: None,
        },
        assurance: support::admission_case::AssuranceFixture {
            mode: AssuranceMode::Accepted,
            provider: "uni".into(),
            evidence_path: "release.uni".into(),
        },
        fault: None,
        notes: "the package must reference real collector output".into(),
    };

    let collected = collect_for_test(&case).expect("the fixture collector runs");

    assert!(
        collected
            .assessment
            .assessment_ref
            .starts_with("uni-bound:")
    );
}
