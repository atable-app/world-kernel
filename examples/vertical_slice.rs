use std::{collections::BTreeSet, fs, path::Path};

use serde_json::{Value, json};
use world_kernel::{
    Coverage, GroundedChange, Intent, Kernel, Patch, StaticAuthority, WorldBootstrap,
    adapters::{
        kollio,
        uni::{
            CollectRequest, UniCollector, UniCollectorError, UniCommandRunner, candidate_from_file,
        },
    },
};

struct DemoUniRunner;

impl UniCommandRunner for DemoUniRunner {
    fn verify(&self, workspace: &Path, _contract: &Path) -> Result<Value, UniCollectorError> {
        let digest = candidate_from_file(workspace.join("candidate.bin"), "unused")?
            .digest
            .trim_start_matches("sha256:")
            .to_owned();
        Ok(json!({
            "evidence": [{
                "claim_id": "release-tests",
                "state": "Valid",
                "artifact_files": {"candidate.bin": digest}
            }]
        }))
    }

    fn report(&self, _workspace: &Path) -> Result<Value, UniCollectorError> {
        Ok(json!({
            "intent": {"id": "release"},
            "decision": "Accepted",
            "reason": "all required claims verified",
            "summary": {"claims_total": 1, "claims_verified": 1},
            "claims": [{"claim_id": "release-tests", "state": "Valid"}],
            "assurance": "A2"
        }))
    }
}

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
    let workspace = tempfile::tempdir()?;
    let candidate_path = workspace.path().join("candidate.bin");
    let contract_path = workspace.path().join("release.uni");
    fs::write(&candidate_path, b"release-candidate-a")?;
    fs::write(&contract_path, b"VERSION 0.1")?;
    let collected = UniCollector::new(DemoUniRunner).collect(&CollectRequest {
        workspace: workspace.path().to_path_buf(),
        contract: contract_path,
        candidate_path,
        candidate_ref: "artifact:release".into(),
    })?;
    let candidate = collected.candidate;
    let assessment = collected.assessment;

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
