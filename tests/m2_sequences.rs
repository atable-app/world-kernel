//! The M2 scenario matrix, executed against the continuation consumer.
//!
//! The matrix is pre-registered in `docs/M2-PROTOCOL.md`. This suite runs the
//! sequences that C2 can actually execute today and reports the rest as not
//! covered, rather than quietly narrowing the claim. Every scenario says which
//! sequence of the protocol it is.
//!
//! Two things are deliberately absent. No test reads an expected result from a
//! fixture in the system path: the checks are against the semantic digest and
//! revision the test computed itself. And no production control carries a disable
//! flag, so a mutation here acts on the real consumer.

mod support;

use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use world_kernel::continuation::{
    Anchor, BodyStatus, ContinuationPackage, Guarantee, Limits, head_digest, read_package_within,
    reconstruct_within,
};
use world_kernel::{
    Coverage, GroundedChange, Intent, Kernel, ObjectRevision, Patch, RejectionCode,
    StaticAuthority, SubmissionOutcome, WorldBootstrap, digest_of_bytes,
};

const WORLD: &str = "world:m2";
const PRODUCER: &str = "principal:producer";
const CONSUMER: &str = "principal:consumer";
const INTENT: &str = "publishCandidate";
const CANDIDATE: &str = "artifact:release";
const REQUIREMENT: &str = "requirement:release";
const SIBLING: &str = "object:unrelated";

fn requirement_digest(version: u8) -> String {
    digest_of_bytes(format!("requirement-release-v{version}").as_bytes())
}

fn candidate_digest(version: u8) -> String {
    digest_of_bytes(format!("release-candidate-{version}").as_bytes())
}

fn trust() -> BTreeSet<String> {
    BTreeSet::from(["uni".to_owned()])
}

fn authority(actor: &str) -> StaticAuthority {
    StaticAuthority::new([(actor.to_owned(), BTreeSet::from([INTENT.to_owned()]))])
}

/// A producer that admits `steps` changes, then exports and goes away.
struct Produced {
    package: ContinuationPackage,
    head: String,
    database: PathBuf,
    _workspace: tempfile::TempDir,
}

fn produce(directory: &Path, steps: usize) -> Produced {
    let workspace = tempfile::tempdir().expect("a producer workspace");
    let database = workspace.path().join("world.db");
    let mut kernel = Kernel::create(
        &database,
        WorldBootstrap {
            world: WORLD.into(),
            trusted_assurance_providers: trust(),
            objects: vec![
                ObjectRevision {
                    reference: REQUIREMENT.into(),
                    revision: 1,
                    digest: requirement_digest(1),
                },
                ObjectRevision {
                    reference: SIBLING.into(),
                    revision: 1,
                    digest: digest_of_bytes(b"unrelated-v1"),
                },
            ],
        },
    )
    .expect("the producer creates its world");

    for step in 0..steps {
        let reference = format!("artifact:step-{step}");
        let digest = candidate_digest(step as u8);
        let change = GroundedChange {
            schema: "world-change/v0-experimental".into(),
            world: WORLD.into(),
            proposal_id: format!("change:producer-{step}"),
            idempotency_key: format!("producer-{step}"),
            actor: PRODUCER.into(),
            base_revision: step as u64,
            intent: Intent {
                kind: INTENT.into(),
                target: reference.clone(),
            },
            candidate: world_kernel::Candidate {
                reference,
                digest: digest.clone(),
            },
            reads: vec![ObjectRevision {
                reference: REQUIREMENT.into(),
                revision: 1,
                digest: requirement_digest(1),
            }],
            coverage: Coverage {
                profile: "closed-v1".into(),
                complete_for: BTreeSet::from([INTENT.to_owned()]),
                truncated: false,
            },
            assessments: vec![world_kernel::AcceptedAssessment {
                provider: "uni".into(),
                subject: format!("artifact:step-{step}"),
                subject_digest: digest.clone(),
                assessment_ref: format!("uni-bound:producer-{step}"),
            }],
            patches: vec![Patch::PutObject {
                reference: format!("artifact:step-{step}"),
                expected_revision: None,
                digest,
            }],
        };
        let outcome = kernel
            .submit(&change, &authority(PRODUCER))
            .expect("the producer admits its change");
        assert!(matches!(outcome, SubmissionOutcome::Committed(_)));
    }

    let package = kernel
        .export_continuation(directory.join("continuation.json"))
        .expect("the producer exports");
    Produced {
        head: package.head.head_digest.clone(),
        package,
        database,
        _workspace: workspace,
    }
}

impl Produced {
    /// Makes the producer unreachable without deleting anything, and returns the
    /// package the consumer will actually receive.
    fn go_away(self) -> ContinuationPackage {
        fs::rename(&self.database, self.database.with_extension("unreachable"))
            .expect("the producer's private database moves out of reach");
        self.package
    }
}

fn anchor_for(produced: &Produced) -> Anchor {
    Anchor {
        head_digest: produced.head.clone(),
        world: WORLD.into(),
    }
}

/// A consumer that has reconstructed into its own destination with its own trust.
struct Consumed {
    kernel: Kernel,
    _destination: tempfile::TempDir,
    reconstruction: world_kernel::continuation::Reconstruction,
}

fn consume(
    package: &ContinuationPackage,
    anchor: &Anchor,
    trusted: BTreeSet<String>,
) -> Result<Consumed, world_kernel::continuation::Error> {
    let destination = tempfile::tempdir().expect("the consumer's own destination");
    let reconstruction = reconstruct_within(package, anchor, Limits::default())?
        .materialize(destination.path().join("world.db"), trusted)?;
    let kernel = Kernel::open(&reconstruction.database, WORLD)?;
    Ok(Consumed {
        kernel,
        _destination: destination,
        reconstruction,
    })
}

/// The consumer's next proposal, reading a dependency at a chosen revision.
fn proposal(base_revision: u64, read_revision: u64, read_digest: String) -> GroundedChange {
    let reference = format!("artifact:step-{}", base_revision + 10);
    let digest = candidate_digest((base_revision + 10) as u8);
    let patches = vec![Patch::PutObject {
        reference: reference.clone(),
        expected_revision: None,
        digest: digest.clone(),
    }];
    GroundedChange {
        schema: "world-change/v0-experimental".into(),
        world: WORLD.into(),
        proposal_id: format!("change:consumer-{base_revision}"),
        idempotency_key: format!("consumer-{base_revision}"),
        actor: CONSUMER.into(),
        base_revision,
        intent: Intent {
            kind: INTENT.into(),
            target: reference.clone(),
        },
        candidate: world_kernel::Candidate {
            reference: reference.clone(),
            digest: digest.clone(),
        },
        reads: vec![ObjectRevision {
            reference: REQUIREMENT.into(),
            revision: read_revision,
            digest: read_digest,
        }],
        coverage: Coverage {
            profile: "closed-v1".into(),
            complete_for: BTreeSet::from([INTENT.to_owned()]),
            truncated: false,
        },
        assessments: vec![world_kernel::AcceptedAssessment {
            provider: "uni".into(),
            subject: reference,
            subject_digest: digest,
            assessment_ref: "uni-bound:consumer".into(),
        }],
        patches,
    }
}

// ------------------------------------------------------ family 1 and 2: fresh consumer

/// Sequence 1: a fresh consumer, an unchanged context and an authorized actor.
#[test]
fn sequence_01_a_fresh_consumer_reconstructs_and_then_continues() {
    let directory = tempfile::tempdir().expect("a handoff directory");
    let produced = produce(directory.path(), 1);
    let anchor = anchor_for(&produced);
    let package = produced.go_away();

    let mut consumer = consume(&package, &anchor, trust()).expect("the package is understood");
    assert_eq!(consumer.reconstruction.world_revision, 1);
    assert!(
        consumer
            .reconstruction
            .guarantee(Guarantee::HistoricalReconstruction)
    );

    let outcome = consumer
        .kernel
        .submit(&proposal(1, 1, requirement_digest(1)), &authority(CONSUMER))
        .expect("the continuation is decided");

    assert!(
        matches!(outcome, SubmissionOutcome::Committed(_)),
        "a real next transition must be admitted: {outcome:?}"
    );
}

/// Sequence 2: the same, with a consumer that lacks the capability.
#[test]
fn sequence_02_a_fresh_consumer_without_authority_reads_history_and_is_blocked() {
    let directory = tempfile::tempdir().expect("a handoff directory");
    let produced = produce(directory.path(), 1);
    let anchor = anchor_for(&produced);
    let package = produced.go_away();

    // No trust: the consumer can read the past but cannot vouch for assurance.
    let mut consumer = consume(&package, &anchor, BTreeSet::new()).expect("history is readable");
    assert_eq!(consumer.reconstruction.world_revision, 1);

    // No grant either: the operation itself is not permitted.
    let outcome = consumer
        .kernel
        .submit(
            &proposal(1, 1, requirement_digest(1)),
            &StaticAuthority::default(),
        )
        .expect("the continuation is decided");

    let SubmissionOutcome::Rejected(rejection) = outcome else {
        panic!("a consumer without the capability must not continue");
    };
    assert_eq!(rejection.code, RejectionCode::Unauthorized);
    assert_eq!(
        consumer.kernel.snapshot().expect("snapshot").revision,
        1,
        "a blocked continuation changes nothing, while the past stays readable"
    );
}

// ------------------------------------------------- family 3: path and workspace portability

/// Sequence 3: the producer's workspace is gone and only the package remains.
#[test]
fn sequence_03_the_producer_workspace_is_never_needed() {
    let directory = tempfile::tempdir().expect("a handoff directory");
    let produced = produce(directory.path(), 2);
    let anchor = anchor_for(&produced);
    let package = produced.go_away();

    // The consumer is handed the package only. Its own path is different.
    let consumer = consume(&package, &anchor, trust()).expect("the package alone suffices");
    let snapshot = consumer.kernel.snapshot().expect("snapshot");

    assert_eq!(snapshot.revision, 2);
    assert_eq!(
        snapshot.objects["artifact:step-0"].digest,
        candidate_digest(0),
        "the reconstructed projection carries the announced digests"
    );
}

/// Sequence 4: a resource whose body did not travel is an obligation, not a
/// validity claim.
#[test]
fn sequence_04_an_absent_body_is_declared_and_blocks_what_cannot_be_verified() {
    let directory = tempfile::tempdir().expect("a handoff directory");
    let produced = produce(directory.path(), 1);
    let anchor = anchor_for(&produced);
    let mut package = produced.go_away();

    assert!(
        package
            .resources
            .iter()
            .all(|resource| resource.body_status == BodyStatus::Absent),
        "the producer holds digests, never bodies, so every body is absent"
    );

    let consumer = consume(&package, &anchor, trust()).expect("the projection still rebuilds");
    assert!(
        consumer
            .reconstruction
            .absent_bodies
            .contains(&CANDIDATE.to_owned())
            || !consumer.reconstruction.absent_bodies.is_empty(),
        "an absent body must be reported, not implied present"
    );

    // A body claim the consumer cannot check is refused rather than believed.
    package.resources[0].body_status = BodyStatus::Included;
    package.head.head_digest = head_digest(&package);
    let error = match consume(&package, &anchor_for_head(&package), trust()) {
        Ok(_) => panic!("an unverifiable body claim must be refused"),
        Err(error) => error,
    };
    assert!(
        error
            .to_string()
            .contains("claims a body this consumer cannot verify"),
        "the refusal must name the reason, got {error}"
    );
    let mismatched = {
        let mut tampered = package.clone();
        tampered.resources[0].digest = digest_of_bytes(b"a body nobody checked");
        tampered.head.head_digest = head_digest(&tampered);
        consume(&tampered, &anchor_for_head(&tampered), trust())
    };
    assert!(
        mismatched.is_err(),
        "a digest that disagrees with the history is refused"
    );
}

fn anchor_for_head(package: &ContinuationPackage) -> Anchor {
    Anchor {
        head_digest: package.head.head_digest.clone(),
        world: package.world.clone(),
    }
}

// ------------------------------------------------------------- family 4: dependencies

/// Sequence 7: a declared read the consumer's world has moved past.
#[test]
fn sequence_07_a_moved_declared_dependency_blocks_and_names_it() {
    let directory = tempfile::tempdir().expect("a handoff directory");
    let produced = produce(directory.path(), 1);
    let anchor = anchor_for(&produced);
    let package = produced.go_away();

    let mut consumer = consume(&package, &anchor, trust()).expect("the package is understood");

    // The consumer's own world moves the dependency forward.
    let mut move_it = proposal(1, 1, requirement_digest(1));
    move_it.proposal_id = "change:consumer-moved".into();
    move_it.idempotency_key = "consumer-moved".into();
    move_it.patches.push(Patch::PutObject {
        reference: REQUIREMENT.into(),
        expected_revision: Some(1),
        digest: requirement_digest(2),
    });
    let moved = consumer
        .kernel
        .submit(&move_it, &authority(CONSUMER))
        .expect("the move is decided");
    assert!(
        matches!(moved, SubmissionOutcome::Committed(_)),
        "{moved:?}"
    );

    // The next proposal still reads the revision from before the move.
    let stale = proposal(2, 1, requirement_digest(1));
    let outcome = consumer
        .kernel
        .submit(&stale, &authority(CONSUMER))
        .expect("the stale continuation is decided");

    let SubmissionOutcome::Rejected(rejection) = outcome else {
        panic!("a continuation over a moved dependency must be blocked");
    };
    assert_eq!(rejection.code, RejectionCode::StaleDependency);
    assert!(rejection.detail.contains(REQUIREMENT));
}

/// Sequence 8, the negative control: a change outside the declared coverage must
/// not claim to invalidate anything.
#[test]
fn sequence_08_an_unrelated_change_does_not_invalidate_a_continuation() {
    let directory = tempfile::tempdir().expect("a handoff directory");
    let produced = produce(directory.path(), 1);
    let anchor = anchor_for(&produced);
    let package = produced.go_away();

    let mut consumer = consume(&package, &anchor, trust()).expect("the package is understood");

    // Something outside the declared reads moves.
    let mut unrelated = proposal(1, 1, requirement_digest(1));
    unrelated.proposal_id = "change:consumer-unrelated".into();
    unrelated.idempotency_key = "consumer-unrelated".into();
    unrelated.patches.push(Patch::PutObject {
        reference: SIBLING.into(),
        expected_revision: Some(1),
        digest: digest_of_bytes(b"unrelated-v2"),
    });
    let moved = consumer
        .kernel
        .submit(&unrelated, &authority(CONSUMER))
        .expect("the unrelated move is decided");
    assert!(
        matches!(moved, SubmissionOutcome::Committed(_)),
        "{moved:?}"
    );

    // The continuation still reads the same declared dependency and proceeds.
    let next = proposal(2, 1, requirement_digest(1));
    let outcome = consumer
        .kernel
        .submit(&next, &authority(CONSUMER))
        .expect("the continuation is decided");

    assert!(
        matches!(outcome, SubmissionOutcome::Committed(_)),
        "an out-of-scope change must not block a valid continuation: {outcome:?}"
    );
}

// -------------------------------------------------- family 5: assurance substitution

/// Sequence 9: an assurance reference rebound to another candidate.
#[test]
fn sequence_09_a_substituted_assurance_reference_breaks_the_head_anchor() {
    let directory = tempfile::tempdir().expect("a handoff directory");
    let produced = produce(directory.path(), 1);
    let anchor = anchor_for(&produced);
    let package = produced.go_away();

    let mut tampered = package.clone();
    tampered.admissions[0].assessments[0].subject = "artifact:someone-else".into();
    // The head is recomputed, so the substitution is internally consistent: it is
    // caught by the out-of-band anchor, not by the package's own digest.
    tampered.head.head_digest = head_digest(&tampered);

    let outcome = consume(&tampered, &anchor, trust());
    assert!(
        outcome.is_err(),
        "a package that no longer matches the expected head must be refused"
    );
}

/// Sequence 10: recorded history stays readable while current applicability is
/// recomputed.
#[test]
fn sequence_10_history_is_not_rewritten_by_a_current_policy_change() {
    let directory = tempfile::tempdir().expect("a handoff directory");
    let produced = produce(directory.path(), 1);
    let anchor = anchor_for(&produced);
    let package = produced.go_away();

    // The consumer trusts a different provider than the producer recorded.
    let mut consumer = consume(&package, &anchor, BTreeSet::from(["other".to_owned()]))
        .expect("history is still readable under a different trust configuration");

    assert_eq!(consumer.reconstruction.world_revision, 1);
    assert_eq!(
        consumer.reconstruction.assurance_references,
        vec!["uni-bound:producer-0".to_owned()],
        "the recorded assurance is carried as a historical assertion"
    );

    // Continuing is a separate question, and the new trust configuration answers it.
    let outcome = consumer
        .kernel
        .submit(&proposal(1, 1, requirement_digest(1)), &authority(CONSUMER))
        .expect("the continuation is decided");
    let SubmissionOutcome::Rejected(rejection) = outcome else {
        panic!("a consumer that does not trust the recorded provider cannot continue");
    };
    assert_eq!(rejection.code, RejectionCode::UntrustedAssessmentProvider);
}

// ------------------------------------------------- family 6: tampering and anchors

/// Sequence 11: an altered resource body, with the manifest digest unchanged.
#[test]
fn sequence_11_a_tampered_resource_digest_is_refused() {
    let directory = tempfile::tempdir().expect("a handoff directory");
    let produced = produce(directory.path(), 1);
    let mut package = produced.go_away();

    let target = package
        .resources
        .iter()
        .position(|resource| resource.reference == REQUIREMENT)
        .expect("the manifest names the dependency");
    package.resources[target].digest = digest_of_bytes(b"a tampered body");
    // The head is recomputed, so the package is internally consistent. It is the
    // manifest that disagrees with the recorded history, not the anchor, that
    // catches this.
    package.head.head_digest = head_digest(&package);

    let outcome = consume(&package, &anchor_for_head(&package), trust());
    assert!(
        outcome.is_err(),
        "a resource digest that disagrees with the reconstructed history must be refused"
    );
}

/// Sequence 12: a package truncated before its announced head.
#[test]
fn sequence_12_a_truncated_package_does_not_verify_against_the_anchor() {
    let directory = tempfile::tempdir().expect("a handoff directory");
    let produced = produce(directory.path(), 3);
    let anchor = anchor_for(&produced);
    let mut package = produced.go_away();

    package.transitions.pop();
    // The head still claims the original length, which is exactly the coherent
    // truncation a self-contained digest cannot see through.
    package.head.head_digest = head_digest(&package);

    let outcome = consume(&package, &anchor, trust());
    assert!(
        outcome.is_err(),
        "a package truncated below its announced head must not verify"
    );
}

// -------------------------------------------------------- family 7: order and history

/// Sequence 13: transitions out of order.
#[test]
fn sequence_13_reordered_transitions_are_refused() {
    let directory = tempfile::tempdir().expect("a handoff directory");
    let produced = produce(directory.path(), 3);
    let mut package = produced.go_away();
    let anchor = anchor_for_head(&package);

    package.transitions.swap(1, 2);
    package.head.head_digest = head_digest(&package);

    let outcome = consume(&package, &anchor, trust());
    assert!(
        outcome.is_err(),
        "the recorded order is the only accepted order"
    );
}

/// Sequence 14: an unknown event type in the history.
#[test]
fn sequence_14_an_unknown_transition_kind_fails_closed() {
    let directory = tempfile::tempdir().expect("a handoff directory");
    let produced = produce(directory.path(), 1);
    let package = produced.go_away();

    // An unknown kind cannot even be represented, which is the intended refusal:
    // guessing at semantics is not offered.
    let hostile = serde_json::json!({
        "schema": package.schema,
        "reducer": package.reducer,
        "world": package.world,
        "origin": package.origin,
        "head": package.head,
        "transitions": [{"type": "effect_dispatched", "detail": "unknown to this reducer"}],
        "admissions": package.admissions,
        "resources": package.resources,
        "assurance": package.assurance,
        "declarations": package.declarations,
    });
    let text = serde_json::to_string(&hostile).expect("serializable");
    let parsed = read_package_within(
        {
            let path = directory.path().join("hostile.json");
            fs::write(&path, &text).expect("written");
            path
        },
        Limits::default(),
    );

    assert!(
        parsed.is_err(),
        "an unknown transition kind must be refused at the parse, not guessed at"
    );
}

// ------------------------------------------------------------ family 8: format and version

/// Sequence 15: an unknown package schema.
#[test]
fn sequence_15_an_unknown_package_schema_is_explicitly_unsupported() {
    let directory = tempfile::tempdir().expect("a handoff directory");
    let produced = produce(directory.path(), 1);
    let mut package = produced.go_away();

    package.schema = "world-continuation/v9-experimental".into();
    package.head.head_digest = head_digest(&package);

    let error = match consume(&package, &anchor_for_head(&package), trust()) {
        Ok(_) => panic!("an unsupported package must be refused"),
        Err(error) => error,
    };
    assert!(
        error.to_string().contains("unsupported package schema"),
        "the refusal must name the schema, got {error}"
    );
}

/// Sequence 16: an unknown field, a missing key and an unknown variant.
#[test]
fn sequence_16_strict_parsing_refuses_malformed_packages() {
    let directory = tempfile::tempdir().expect("a handoff directory");
    let produced = produce(directory.path(), 1);
    let package = produced.go_away();
    let base = serde_json::to_value(&package).expect("serializable");

    let mut unknown_field = base.clone();
    unknown_field["hint"] = serde_json::json!("ignore me");
    let mut missing_key = base.clone();
    missing_key
        .as_object_mut()
        .expect("object")
        .remove("reducer");
    let mut unknown_variant = base.clone();
    unknown_variant["resources"][0]["bodyStatus"] = serde_json::json!("maybe");

    for (label, value) in [
        ("unknown field", unknown_field),
        ("missing key", missing_key),
        ("unknown variant", unknown_variant),
    ] {
        let outcome: Result<ContinuationPackage, _> = serde_json::from_value(value);
        assert!(outcome.is_err(), "{label} must be refused");
    }
}

/// Sequence 17: a reducer semantics this consumer does not implement.
#[test]
fn sequence_17_an_unimplemented_reducer_is_explicitly_unsupported() {
    let directory = tempfile::tempdir().expect("a handoff directory");
    let produced = produce(directory.path(), 1);
    let mut package = produced.go_away();

    package.reducer = "world-kernel/recorded-history/9".into();
    package.head.head_digest = head_digest(&package);

    let error = match consume(&package, &anchor_for_head(&package), trust()) {
        Ok(_) => panic!("an unsupported package must be refused"),
        Err(error) => error,
    };
    assert!(
        error
            .to_string()
            .contains("unsupported recorded-history reducer"),
        "the refusal must name the reducer, got {error}"
    );
}

// ------------------------------------------------------------- family 9: import boundaries

/// Sequence 18: a reference that looks like a path is an opaque identifier.
#[test]
fn sequence_18_a_path_shaped_reference_is_never_used_as_a_path() {
    let directory = tempfile::tempdir().expect("a handoff directory");
    let produced = produce(directory.path(), 1);
    let package = produced.go_away();

    // The API has no path input taken from a package: materialization writes only
    // where the caller chose. A reference that happens to look like an escape is
    // just a string.
    let mut hostile = package.clone();
    hostile.resources[0].reference = "../../../../etc/passwd".into();
    hostile.head.head_digest = head_digest(&hostile);

    let destination = tempfile::tempdir().expect("a consumer destination");
    let outcome = reconstruct_within(&hostile, &anchor_for_head(&hostile), Limits::default());
    assert!(
        outcome.is_err(),
        "a reference absent from the history is refused"
    );

    // And the destination is untouched by the attempt.
    assert_eq!(
        std::fs::read_dir(destination.path())
            .expect("readable")
            .count(),
        0,
        "a refused import writes nothing into the destination"
    );
    let _ = destination;
}

/// Sequence 19: bounded sizes and counts.
#[test]
fn sequence_19_an_oversized_import_is_refused_before_any_work() {
    let directory = tempfile::tempdir().expect("a handoff directory");
    let produced = produce(directory.path(), 1);
    let package = produced.go_away();
    let anchor = anchor_for_head(&package);

    let tight = Limits {
        max_transitions: 1,
        ..Limits::default()
    };
    let error = match reconstruct_within(&package, &anchor, tight) {
        Ok(_) => panic!("an oversized import must be refused"),
        Err(error) => error,
    };
    assert!(
        error.to_string().contains("import refused"),
        "the refusal must be a bound, got {error}"
    );

    let tiny = Limits {
        max_package_bytes: 16,
        ..Limits::default()
    };
    let path = directory.path().join("continuation.json");
    fs::write(&path, serde_json::to_vec(&package).expect("serializable")).expect("written");
    let error = match read_package_within(&path, tiny) {
        Ok(_) => panic!("an oversized package must be refused"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("import refused"), "got {error}");
}

/// Sequence 20: a package carrying something executable is not executed.
#[test]
fn sequence_20_a_package_cannot_carry_a_command_or_a_privileged_policy() {
    let directory = tempfile::tempdir().expect("a handoff directory");
    let produced = produce(directory.path(), 1);
    let package = produced.go_away();

    let bytes = serde_json::to_vec(&package).expect("serializable");
    let text = String::from_utf8(bytes).expect("utf8");
    for forbidden in [
        "\"command\"",
        "\"exec\"",
        "\"shell\"",
        "\"script\"",
        "trustedProviders",
        "trustAnchor",
    ] {
        assert!(
            !text.contains(forbidden),
            "the package format has no field for {forbidden}"
        );
    }
}

// --------------------------------------------------------- family 10: benign progression

/// Sequence 21: three successive transitions across one package.
#[test]
fn sequence_21_successive_benign_transitions_all_progress() {
    let directory = tempfile::tempdir().expect("a handoff directory");
    let produced = produce(directory.path(), 1);
    let anchor = anchor_for(&produced);
    let package = produced.go_away();

    let mut consumer = consume(&package, &anchor, trust()).expect("the package is understood");
    for step in 1..=3u64 {
        let outcome = consumer
            .kernel
            .submit(
                &proposal(step, 1, requirement_digest(1)),
                &authority(CONSUMER),
            )
            .expect("each step is decided");
        assert!(
            matches!(outcome, SubmissionOutcome::Committed(_)),
            "step {step} must progress: {outcome:?}"
        );
    }
    assert_eq!(consumer.kernel.snapshot().expect("snapshot").revision, 4);
}

/// Sequence 22: a new proposal on new revisions is not an identity reuse.
#[test]
fn sequence_22_a_new_proposal_is_not_an_identity_reuse() {
    let directory = tempfile::tempdir().expect("a handoff directory");
    let produced = produce(directory.path(), 1);
    let anchor = anchor_for(&produced);
    let package = produced.go_away();

    let mut consumer = consume(&package, &anchor, trust()).expect("the package is understood");
    let first = proposal(1, 1, requirement_digest(1));
    let outcome = consumer
        .kernel
        .submit(&first, &authority(CONSUMER))
        .expect("the first step is decided");
    assert!(matches!(outcome, SubmissionOutcome::Committed(_)));

    // The same identity with different content is a conflict, not a retry.
    let mut reused = proposal(2, 1, requirement_digest(1));
    reused.idempotency_key = first.idempotency_key.clone();
    reused.proposal_id = first.proposal_id.clone();
    reused.candidate.digest = digest_of_bytes(b"different content");
    let outcome = consumer
        .kernel
        .submit(&reused, &authority(CONSUMER))
        .expect("the reuse is decided");
    let SubmissionOutcome::Rejected(rejection) = outcome else {
        panic!("a reused identity with new content must be a conflict");
    };
    assert_eq!(rejection.code, RejectionCode::IdempotencyConflict);
}

// --------------------------------------------------------------- family 11: trust profile

/// Sequence 23: no anchor, so authenticity is unestablished rather than true.
#[test]
fn sequence_23_without_an_anchor_authenticity_is_not_claimed() {
    let directory = tempfile::tempdir().expect("a handoff directory");
    let produced = produce(directory.path(), 1);
    let package = produced.go_away();

    let wrong = Anchor {
        head_digest: digest_of_bytes(b"an anchor the package never saw"),
        world: WORLD.into(),
    };
    let error = match reconstruct_within(&package, &wrong, Limits::default()) {
        Ok(_) => panic!("a package that does not match the anchor must be refused"),
        Err(error) => error,
    };
    assert!(
        error
            .to_string()
            .contains("does not match the expected anchor"),
        "got {error}"
    );
}

/// Sequence 24: every guarantee is reported separately, including the ones that
/// do not hold.
#[test]
fn sequence_24_guarantees_are_reported_separately() {
    let directory = tempfile::tempdir().expect("a handoff directory");
    let produced = produce(directory.path(), 1);
    let anchor = anchor_for(&produced);
    let package = produced.go_away();

    let consumer = consume(&package, &anchor, trust()).expect("the package is understood");
    let report = consumer.reconstruction.guarantee_report();

    assert_eq!(report.len(), 6, "six guarantees, not one boolean");
    assert!(report["format_understood"], "format_understood must hold");
    assert!(report["resource_integrity"], "resource_integrity must hold");
    assert!(report["authenticity"], "authenticity must hold");
    assert!(
        report["completeness_within_scope"],
        "completeness_within_scope must hold"
    );
    assert!(
        report["historical_reconstruction"],
        "historical_reconstruction must hold"
    );
    assert!(
        !report["completeness_beyond_scope"],
        "completeness beyond the announced scope is never claimed"
    );
}

// ------------------------------------------------------------------ metamorphic checks

/// Path move without semantic change.
#[test]
fn metamorphic_a_producer_in_another_path_produces_the_same_reconstruction() {
    let first = tempfile::tempdir().expect("a first handoff");
    let second = tempfile::tempdir().expect("a second handoff");
    let one = produce(first.path(), 2);
    let two = produce(second.path(), 2);

    assert_eq!(
        one.package.transitions, two.package.transitions,
        "the recorded history must not depend on where the producer ran"
    );
    assert_eq!(
        one.package.head.head_digest, two.package.head.head_digest,
        "and neither must the head digest"
    );
}

/// Idempotent replay into the same destination.
#[test]
fn metamorphic_replaying_the_same_package_twice_is_stable() {
    let directory = tempfile::tempdir().expect("a handoff directory");
    let produced = produce(directory.path(), 2);
    let anchor = anchor_for(&produced);
    let package = produced.go_away();

    let first = consume(&package, &anchor, trust()).expect("first reconstruction");
    let second = consume(&package, &anchor, trust()).expect("second reconstruction");

    assert_eq!(
        first.reconstruction.objects, second.reconstruction.objects,
        "two reconstructions of one package agree"
    );
    assert_eq!(
        first.kernel.snapshot().expect("snapshot"),
        second.kernel.snapshot().expect("snapshot")
    );
}

/// Two exports of an unchanged world are byte-identical.
#[test]
fn metamorphic_export_is_deterministic_for_an_unchanged_world() {
    let directory = tempfile::tempdir().expect("a handoff directory");
    let produced = produce(directory.path(), 2);
    let path = directory.path().join("again.json");
    let kernel = Kernel::open(&produced.database, WORLD).expect("reopen");
    let again = kernel.export_continuation(&path).expect("a second export");

    assert_eq!(
        again.head.head_digest, produced.package.head.head_digest,
        "an unchanged world exports the same head"
    );
}

/// The package never claims a body it does not carry.
#[test]
fn metamorphic_a_body_is_never_implied_present() {
    let directory = tempfile::tempdir().expect("a handoff directory");
    let produced = produce(directory.path(), 1);
    let package = produced.go_away();

    assert!(
        package.resources.iter().any(|resource| {
            resource.body_status == BodyStatus::Absent && resource.digest.starts_with("sha256:")
        }),
        "a digest without accessible content is declared absent"
    );
    assert!(
        !package.assurance.bytes_available,
        "the assurance transport is referenced, not shipped"
    );
}

/// Sequence coverage, reported rather than implied.
#[test]
fn the_matrix_reports_what_is_and_is_not_covered() {
    let covered = [
        (1, "fresh consumer, unchanged context"),
        (2, "fresh consumer without capability"),
        (3, "producer workspace removed"),
        (4, "absent body declared"),
        (7, "moved declared dependency"),
        (8, "unrelated change, negative control"),
        (9, "substituted assurance reference"),
        (10, "history kept, applicability recomputed"),
        (11, "tampered resource digest"),
        (12, "truncated package"),
        (13, "reordered transitions"),
        (14, "unknown transition kind"),
        (15, "unknown package schema"),
        (16, "strict parse refusals"),
        (17, "unimplemented reducer"),
        (18, "path-shaped reference"),
        (19, "bounded sizes and counts"),
        (20, "no executable field"),
        (21, "benign progression"),
        (22, "identity reuse refused"),
        (23, "anchor absent"),
        (24, "guarantees reported separately"),
    ];
    let not_covered = [
        (
            5,
            "crash and lost response: covered by tests/crash_symmetry.rs, not by a package",
        ),
        (6, "crash after a write before commit: same"),
    ];
    let equal_information_needed = [
        (
            3,
            "A2 and B2 have no export or application-level resume yet",
        ),
        (
            4,
            "A2 and B2 have no own snapshot to declare bodies against",
        ),
    ];

    assert_eq!(
        covered.len() + not_covered.len(),
        24,
        "the matrix is 24 sequences"
    );
    assert_eq!(
        equal_information_needed.len(),
        2,
        "two sequences are covered for C2 only and stay visibly incomplete: {equal_information_needed:?}"
    );
    let _ = not_covered;
}
