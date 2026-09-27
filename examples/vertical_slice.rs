use std::collections::BTreeSet;

use serde_json::json;
use world_kernel::{
    Candidate, Coverage, GroundedChange, Intent, Kernel, Patch, StaticAuthority, WorldBootstrap,
    adapters::{kollio, uni},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let kollio_document = json!({
        "schemaVersion": 7,
        "documentId": "decision:launch",
        "revision": 12,
        "semanticRevision": 5,
        "content": {},
        "relationships": {},
        "decisions": {},
        "presentation": {}
    });
    let observed_document = kollio::observe_document(&kollio_document)?;
    let candidate = Candidate {
        reference: "artifact:release".into(),
        digest: "sha256:release-candidate-a".into(),
    };
    let uni_report = json!({
        "intent": {"id": "release"},
        "decision": "Accepted",
        "reason": "all required claims verified",
        "summary": {"claims_total": 1, "claims_verified": 1},
        "claims": [{"claim_id": "release-tests", "state": "Valid"}],
        "assurance": "A2"
    });
    let assessment = uni::assessment_from_report(&uni_report, &candidate)?;

    let mut kernel = Kernel::create(
        ":memory:",
        WorldBootstrap {
            world: "world:release-demo".into(),
            trusted_assurance_providers: BTreeSet::from(["uni".into()]),
            objects: vec![observed_document.object.clone()],
        },
    )?;
    let change = GroundedChange {
        schema: "world-change/v0-experimental".into(),
        world: "world:release-demo".into(),
        proposal_id: "change:release-1".into(),
        idempotency_key: "release-1".into(),
        actor: "principal:worker-a".into(),
        base_revision: 0,
        intent: Intent {
            kind: "publishCandidate".into(),
            target: candidate.reference.clone(),
        },
        candidate: candidate.clone(),
        reads: vec![observed_document.object],
        coverage: Coverage {
            profile: "release-demo-closed-v1".into(),
            complete_for: BTreeSet::from(["publishCandidate".into()]),
            truncated: false,
        },
        assessments: vec![assessment],
        patches: vec![Patch::PutObject {
            reference: candidate.reference,
            expected_revision: None,
            digest: candidate.digest,
        }],
    };
    let authority = StaticAuthority::new([(
        "principal:worker-a".into(),
        BTreeSet::from(["publishCandidate".into()]),
    )]);

    let outcome = kernel.submit(&change, &authority)?;
    println!("{}", serde_json::to_string_pretty(&outcome)?);
    assert_eq!(kernel.snapshot()?, kernel.replay()?);
    Ok(())
}
