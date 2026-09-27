//! The assurance outcome the corpus declares, produced two ways.
//!
//! System B and C drive the production `UniCollector` over a fixture
//! `UniCommandRunner`. System A calls the same evaluation directly, because an
//! application-specific gate calls its own verifier. Both must reach the same
//! decision about the same assurance information, otherwise the benchmark would
//! be measuring a deliberately weaker baseline.

use std::{fs, path::Path};

use serde_json::{Value, json};
use world_kernel::AcceptedAssessment;
use world_kernel::adapters::uni::{
    CollectRequest, UniCollector, UniCollectorError, UniCommandRunner,
};

use super::admission_case::{AdmissionCase, AssuranceMode, digest_bytes};

/// What the assurance step concluded, in the vocabulary every system shares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssuranceOutcome {
    /// Assurance bound the candidate and accepts it.
    Bound {
        provider: String,
        subject: String,
        subject_digest: String,
    },
    /// Assurance refused, with the code the assurance seam reports.
    Refused(&'static str),
}

impl AssuranceOutcome {
    pub fn refusal_code(&self) -> Option<&'static str> {
        match self {
            Self::Refused(code) => Some(code),
            Self::Bound { .. } => None,
        }
    }
}

/// The code each declared mode is expected to produce.
pub fn expected_code_for(mode: AssuranceMode) -> Option<&'static str> {
    match mode {
        AssuranceMode::Accepted => None,
        AssuranceMode::Rejected | AssuranceMode::RegistryDrift => Some("UNI_NOT_ACCEPTED"),
        AssuranceMode::EvidenceRequired => Some("CANDIDATE_NOT_COVERED_BY_UNI"),
        AssuranceMode::InvalidBundle => Some("UNI_BUNDLE_INVALID"),
        AssuranceMode::UnsupportedBundle => Some("UNI_BUNDLE_UNSUPPORTED"),
        AssuranceMode::MissingRequiredEngine => Some("UNI_COMMAND_FAILED"),
        AssuranceMode::WatchedFileDrift => Some("CANDIDATE_CHANGED_DURING_VERIFICATION"),
    }
}

fn hex(digest: &str) -> &str {
    digest.strip_prefix("sha256:").unwrap_or(digest)
}

fn read_digest(path: &Path) -> Result<String, &'static str> {
    fs::read(path)
        .map(|bytes| digest_bytes(&bytes))
        .map_err(|_| "IO_ERROR")
}

/// The application-side verifier used by system A.
///
/// It applies the same discipline as the production collector: hash the
/// candidate before and after the verification, and require the verifier's own
/// evidence to name that exact path and digest. That is what makes A a
/// competent baseline rather than a naive one.
pub fn app_verify(
    mode: AssuranceMode,
    provider: &str,
    workspace: &Path,
    candidate_relative: &str,
    candidate_reference: &str,
) -> AssuranceOutcome {
    let candidate = workspace.join(candidate_relative);
    let Ok(before) = read_digest(&candidate) else {
        return AssuranceOutcome::Refused("CANDIDATE_NOT_COVERED_BY_UNI");
    };

    if let Some(code) = match mode {
        AssuranceMode::InvalidBundle => Some("UNI_BUNDLE_INVALID"),
        AssuranceMode::UnsupportedBundle => Some("UNI_BUNDLE_UNSUPPORTED"),
        AssuranceMode::MissingRequiredEngine => Some("UNI_COMMAND_FAILED"),
        AssuranceMode::WatchedFileDrift => {
            if fs::write(&candidate, b"candidate-b").is_err() {
                Some("IO_ERROR")
            } else {
                Some("CANDIDATE_CHANGED_DURING_VERIFICATION")
            }
        }
        _ => None,
    } {
        return AssuranceOutcome::Refused(code);
    }

    let Ok(after) = read_digest(&candidate) else {
        return AssuranceOutcome::Refused("IO_ERROR");
    };
    if before != after {
        return AssuranceOutcome::Refused("CANDIDATE_CHANGED_DURING_VERIFICATION");
    }

    if mode == AssuranceMode::EvidenceRequired {
        return AssuranceOutcome::Refused("CANDIDATE_NOT_COVERED_BY_UNI");
    }
    if !matches!(mode, AssuranceMode::Accepted) {
        return AssuranceOutcome::Refused("UNI_NOT_ACCEPTED");
    }
    AssuranceOutcome::Bound {
        provider: provider.to_owned(),
        subject: candidate_reference.to_owned(),
        subject_digest: before,
    }
}

/// The collected assessment, used by the corpus honesty checks.
pub struct CollectedForTest {
    pub assessment: AcceptedAssessment,
}

/// Runs the production collector over a case's declared workspace so a test can
/// check that the corpus embeds what its own assurance step produces.
pub fn collect_for_test(case: &AdmissionCase) -> Option<CollectedForTest> {
    if case.assurance.mode != AssuranceMode::Accepted {
        return None;
    }
    let directory = tempfile::tempdir().ok()?;
    let candidate = directory.path().join(&case.current.candidate_path);
    let contract = directory.path().join(&case.assurance.evidence_path);
    std::fs::write(&candidate, case.current.candidate_contents.as_bytes()).ok()?;
    std::fs::write(&contract, b"VERSION 0.1").ok()?;
    let runner = FixtureUniRunner::new(
        case.assurance.mode,
        &case.current.candidate_path,
        &case.assurance.evidence_path,
    );
    let collected = UniCollector::new(runner)
        .collect(&CollectRequest {
            workspace: directory.path().to_path_buf(),
            contract,
            candidate_path: candidate,
            candidate_ref: case.proposal.candidate.reference.clone(),
        })
        .ok()?;
    let _ = collected.candidate.digest;
    Some(CollectedForTest {
        assessment: collected.assessment,
    })
}

/// A `UniCommandRunner` producing the declared condition through UNI's own
/// versioned bundle transport.
pub struct FixtureUniRunner {
    pub mode: AssuranceMode,
    pub candidate_relative: String,
    pub contract: String,
}

impl FixtureUniRunner {
    pub fn new(mode: AssuranceMode, candidate_relative: &str, contract: &str) -> Self {
        Self {
            mode,
            candidate_relative: candidate_relative.to_owned(),
            contract: contract.to_owned(),
        }
    }

    fn valid_evidence(&self, workspace: &Path) -> Value {
        let digest = read_digest(&workspace.join(&self.candidate_relative)).unwrap_or_default();
        json!({
            "bundleVersion": "uni-bundle-0.1",
            "evidence": [{
                "claim_id": "release",
                "state": "Valid",
                "artifact_files": {self.candidate_relative.clone(): hex(&digest).to_owned()},
            }],
        })
    }
}

impl UniCommandRunner for FixtureUniRunner {
    fn verify(&self, workspace: &Path, _contract: &Path) -> Result<Value, UniCollectorError> {
        match self.mode {
            AssuranceMode::InvalidBundle => Err(UniCollectorError::BundleInvalid),
            AssuranceMode::UnsupportedBundle => Err(UniCollectorError::UnsupportedBundle(
                "uni-bundle-0.2".into(),
            )),
            AssuranceMode::MissingRequiredEngine => Err(UniCollectorError::CommandFailed(
                "verify exited with status 127: verifier 'candidate.ready' is not installed".into(),
            )),
            AssuranceMode::WatchedFileDrift => {
                // The watched file changes while the verifier runs. The collector
                // is left to detect it by comparing its own two hashes.
                fs::write(workspace.join(&self.candidate_relative), b"candidate-b")?;
                Ok(self.valid_evidence(workspace))
            }
            AssuranceMode::EvidenceRequired => Ok(json!({
                "bundleVersion": "uni-bundle-0.1",
                "evidence": [{
                    "claim_id": "release",
                    "state": "Stale",
                    "artifact_files": {"other.bin": "not-the-candidate"},
                }],
            })),
            _ => Ok(self.valid_evidence(workspace)),
        }
    }

    fn report(&self, _workspace: &Path) -> Result<Value, UniCollectorError> {
        let (decision, reason) = match self.mode {
            AssuranceMode::Rejected => ("EvidenceRequired", "one claim is missing"),
            AssuranceMode::RegistryDrift => (
                "RegistryDrift",
                "verifier registry no longer matches the contract",
            ),
            _ => ("Accepted", "all required claims verified"),
        };
        Ok(json!({
            "intent": {"id": "release"},
            "decision": decision,
            "reason": reason,
            "summary": {"claims_total": 1, "claims_verified": 1},
            "claims": [{"claim_id": "release", "state": "Valid"}],
            "assurance": "A2",
            "contract": self.contract,
        }))
    }
}
