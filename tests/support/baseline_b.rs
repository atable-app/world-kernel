//! System B: the same application gate, with the verifier outcome produced
//! through the UNI seam.
//!
//! B adds no admission logic of its own. It replaces system A's direct verifier
//! call with the production `UniCollector`, which hashes the candidate around
//! verification, asks UNI to export and validate its versioned evidence bundle,
//! and requires valid evidence naming the exact path and digest. That difference
//! is the whole of B.

use std::path::Path;

use world_kernel::adapters::uni::{CollectRequest, UniCollector};

use super::assurance::{AssuranceOutcome, FixtureUniRunner};

/// Runs assurance for system B through the production UNI collector.
pub fn assure(
    workspace: &Path,
    mode: super::admission_case::AssuranceMode,
    _provider: &str,
    candidate_relative: &str,
    contract_relative: &str,
    candidate_reference: &str,
) -> AssuranceOutcome {
    let runner = FixtureUniRunner::new(mode, candidate_relative, contract_relative);
    let collector = UniCollector::new(runner);
    let request = CollectRequest {
        workspace: workspace.to_path_buf(),
        contract: workspace.join(contract_relative),
        candidate_path: workspace.join(candidate_relative),
        candidate_ref: candidate_reference.to_owned(),
    };
    match collector.collect(&request) {
        Ok(collected) => AssuranceOutcome::Bound {
            provider: collected.assessment.provider,
            subject: collected.assessment.subject,
            subject_digest: collected.assessment.subject_digest,
        },
        Err(error) => AssuranceOutcome::Refused(refusal_code(error.code())),
    }
}

/// Every `UniCollectorError` code the corpus can provoke, including the ones a
/// system may refuse locally.
pub fn refusal_code(code: &str) -> &'static str {
    match code {
        "UNI_NOT_ACCEPTED" => "UNI_NOT_ACCEPTED",
        "CANDIDATE_NOT_COVERED_BY_UNI" => "CANDIDATE_NOT_COVERED_BY_UNI",
        "CANDIDATE_CHANGED_DURING_VERIFICATION" => "CANDIDATE_CHANGED_DURING_VERIFICATION",
        "UNI_COMMAND_FAILED" => "UNI_COMMAND_FAILED",
        "UNI_BUNDLE_INVALID" => "UNI_BUNDLE_INVALID",
        "UNI_BUNDLE_UNSUPPORTED" => "UNI_BUNDLE_UNSUPPORTED",
        other => panic!("system B has no benchmark code for the refusal {other}"),
    }
}
