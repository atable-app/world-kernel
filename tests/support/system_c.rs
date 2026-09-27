//! System C: UNI plus the World Kernel.
//!
//! The case's `GroundedChange` is submitted to the production `Kernel` with a
//! current authority source. No Kernel code is reimplemented here; the only
//! harness code is the ablations, which mutate the proposal before admission and
//! exist to show that each admission check changes a decision.

use world_kernel::{GroundedChange, SubmissionOutcome};

use super::admission_case::{BenchmarkCode, digest_bytes};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ablation {
    /// Replace the embedded assurance with one freshly bound to the bytes the
    /// workspace holds now, as a system that re-binds at admission time would.
    ForgeAssessment,
    /// Omit from the read set every declared dependency that no longer matches.
    DropChangedReads,
    /// Replace a truncated or incomplete view with a complete one.
    CompleteTruncatedCoverage,
}

impl Ablation {
    pub const ALL: [Ablation; 3] = [
        Ablation::ForgeAssessment,
        Ablation::DropChangedReads,
        Ablation::CompleteTruncatedCoverage,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            Self::ForgeAssessment => "forge_assessment",
            Self::DropChangedReads => "drop_changed_reads",
            Self::CompleteTruncatedCoverage => "complete_truncated_coverage",
        }
    }
}

/// The current state the harness can observe, used only to build ablations and
/// never to decide a case.
#[derive(Debug, Clone)]
pub struct CurrentState {
    pub object_digests: Vec<(String, u64, String)>,
    pub candidate_digest: String,
}

impl CurrentState {
    pub fn digest_of(&self, reference: &str) -> Option<&(String, u64, String)> {
        self.object_digests
            .iter()
            .find(|(name, _, _)| name == reference)
    }
}

/// Applies a test-only input ablation. No production check is disabled: the
/// proposal handed to the Kernel is a different, weaker proposal.
pub fn ablate(change: &GroundedChange, ablation: Ablation, state: &CurrentState) -> GroundedChange {
    let mut change = change.clone();
    match ablation {
        Ablation::ForgeAssessment => {
            let digest = state.candidate_digest.clone();
            for assessment in &mut change.assessments {
                assessment.subject = change.candidate.reference.clone();
                assessment.subject_digest = digest.clone();
            }
        }
        Ablation::DropChangedReads => {
            change.reads.retain(|read| {
                state
                    .digest_of(&read.reference)
                    .is_some_and(|(name, revision, digest)| {
                        name == &read.reference
                            && *revision == read.revision
                            && *digest == read.digest
                    })
            });
        }
        Ablation::CompleteTruncatedCoverage => {
            change.coverage.truncated = false;
            change
                .coverage
                .complete_for
                .insert(change.intent.kind.clone());
        }
    }
    change
}

pub fn outcome_to_benchmark(outcome: &SubmissionOutcome) -> (bool, Option<BenchmarkCode>) {
    match outcome {
        SubmissionOutcome::Committed(_) => (true, None),
        SubmissionOutcome::Rejected(rejection) => {
            (false, Some(BenchmarkCode::from(&rejection.code)))
        }
    }
}

/// The digest the workspace holds, for the ablation that must re-bind the
/// candidate to what is on disk.
pub fn workspace_digest(contents: &str) -> String {
    digest_bytes(contents.as_bytes())
}
