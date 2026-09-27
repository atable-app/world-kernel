//! The continuation package: an export that lets a fresh consumer reconstruct a
//! World and decide what can be resumed, without the producer.
//!
//! Scope of what this can honestly promise, established before the format was
//! designed:
//!
//! - The Kernel stores object references, revisions and digests. It never stores
//!   object bytes. A package therefore reconstructs a projection and its history,
//!   and declares every resource body as absent. A hash without accessible content
//!   does not reconstruct an artifact.
//! - A package never carries a trust configuration. A consumer is given an
//!   expected head out of band and compares it; a digest protects the binding to a
//!   reference, while a chain of digests alone cannot detect a coherent rewrite or
//!   truncation.
//! - Guarantees are reported separately rather than collapsed into one verdict:
//!   understanding the format, resource integrity, authenticity, completeness
//!   relative to the announced scope, historical reconstruction, and the current
//!   applicability of a new continuation are distinct outcomes.
//! - Replay consumes recorded transitions only. It never calls a model, a verifier
//!   or an effect, and it writes only to the destination the caller chooses.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use crate::model::{ObjectState, digest_of_bytes};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const PACKAGE_SCHEMA: &str = "world-continuation/v0-experimental";
pub const RECORDED_HISTORY_REDUCER: &str = "world-kernel/recorded-history/0";

/// Bounds applied to every import.
///
/// A package is untrusted input. Sizes and counts are bounded before any work is
/// done, and no field of a package is ever used as a filesystem path: a reference
/// is an opaque identifier, and the consumer only ever writes to the destination it
/// was given.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub max_package_bytes: usize,
    pub max_transitions: usize,
    pub max_admissions: usize,
    pub max_resources: usize,
    pub max_identifier_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_package_bytes: 8 * 1024 * 1024,
            max_transitions: 100_000,
            max_admissions: 100_000,
            max_resources: 100_000,
            max_identifier_bytes: 4_096,
        }
    }
}

impl Limits {
    fn check_identifier(&self, label: &'static str, value: &str) -> Result<(), Error> {
        if value.len() > self.max_identifier_bytes {
            return Err(Error::LimitExceeded {
                limit: format!("{label} longer than {} bytes", self.max_identifier_bytes),
            });
        }
        Ok(())
    }

    fn check_counts(&self, package: &ContinuationPackage) -> Result<(), Error> {
        for (count, limit, label) in [
            (
                package.transitions.len(),
                self.max_transitions,
                "transition count",
            ),
            (
                package.admissions.len(),
                self.max_admissions,
                "admission count",
            ),
            (
                package.resources.len(),
                self.max_resources,
                "resource count",
            ),
        ] {
            if count > limit {
                return Err(Error::LimitExceeded {
                    limit: format!("{label} above {limit}"),
                });
            }
        }
        Ok(())
    }
}

/// One recorded transition, typed so an unknown kind fails closed rather than
/// being guessed at.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RecordedTransition {
    Genesis {
        world: String,
        objects: BTreeMap<String, ObjectState>,
    },
    ChangeCommitted {
        proposal_id: String,
        candidate_digest: String,
        patches: Vec<crate::Patch>,
    },
}

/// What a producer recorded about one admitted proposal, including the declared
/// rules and the evidence it relied on. The recorded events alone do not carry
/// this: an event names a proposal and a candidate, not the reads it depended on
/// or the assurance behind it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecordedAdmission {
    pub proposal_id: String,
    pub idempotency_key: String,
    pub actor: String,
    pub base_revision: u64,
    pub intent: crate::Intent,
    pub candidate: crate::Candidate,
    pub reads: Vec<crate::ObjectRevision>,
    pub coverage: crate::Coverage,
    pub assessments: Vec<crate::AcceptedAssessment>,
    pub patches: Vec<crate::Patch>,
    pub outcome: crate::SubmissionOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Origin {
    pub kind: String,
    pub world_revision: u64,
    pub transition_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Head {
    pub world_revision: u64,
    pub event_sequence: u64,
    pub head_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResourceRecord {
    pub reference: String,
    pub revision: u64,
    pub digest: String,
    /// `absent` whenever the producer held a digest but not the bytes. A consumer
    /// must treat an absent body as an explicit obligation, never as a valid
    /// artifact.
    pub body_status: BodyStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BodyStatus {
    Absent,
    Included,
}

/// References to the assurance the admitted changes relied on, without the
/// evaluation logic. Verification here is reference reading, not re-running the
/// producer's verifiers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssuranceTransport {
    pub kind: String,
    pub verification: String,
    pub bytes_available: bool,
    pub references: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContinuationPackage {
    pub schema: String,
    pub reducer: String,
    pub world: String,
    pub origin: Origin,
    pub head: Head,
    pub transitions: Vec<RecordedTransition>,
    pub admissions: Vec<RecordedAdmission>,
    pub resources: Vec<ResourceRecord>,
    pub assurance: AssuranceTransport,
    /// Declarations a consumer must not have to guess, kept as text so the
    /// package states its own limits.
    pub declarations: Vec<String>,
}

/// The out-of-band anchor. It arrives over a trusted channel and never inside the
/// package, so a package can never vouch for itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Anchor {
    pub head_digest: String,
    pub world: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Guarantee {
    /// The package parsed under a format this consumer implements.
    FormatUnderstood,
    /// Every resource digest is well formed and agrees with the reconstruction.
    ResourceIntegrity,
    /// The out-of-band anchor matched. Without one, this is unestablished rather
    /// than true.
    Authenticity,
    /// The sequence is closed at the announced head, relative to that scope.
    CompletenessWithinScope,
    /// The projection was rebuilt from recorded transitions.
    HistoricalReconstruction,
    /// Never claimed: completeness is relative to a manifest and a head, so a
    /// hidden uninstrumented source cannot be detected by a package.
    CompletenessBeyondScope,
}

impl Guarantee {
    pub fn label(&self) -> &'static str {
        match self {
            Self::FormatUnderstood => "format_understood",
            Self::ResourceIntegrity => "resource_integrity",
            Self::Authenticity => "authenticity",
            Self::CompletenessWithinScope => "completeness_within_scope",
            Self::HistoricalReconstruction => "historical_reconstruction",
            Self::CompletenessBeyondScope => "completeness_beyond_scope",
        }
    }
}

/// A package that parsed and agreed with its anchor, before anything is written.
#[derive(Debug, Clone)]
pub struct Understood {
    world: String,
    head_digest: String,
    head_revision: u64,
    objects: BTreeMap<String, String>,
    admitted_proposals: Vec<String>,
    assurance_references: Vec<String>,
    absent_bodies: Vec<String>,
    transitions: Vec<RecordedTransition>,
}

#[derive(Debug, Clone)]
pub struct Reconstruction {
    pub world: String,
    pub world_revision: u64,
    pub head_digest: String,
    pub objects: BTreeMap<String, String>,
    pub admitted_proposals: Vec<String>,
    pub assurance_references: Vec<String>,
    pub absent_bodies: Vec<String>,
    pub guarantees: BTreeSet<Guarantee>,
    pub database: PathBuf,
}

impl Reconstruction {
    pub fn guarantee(&self, guarantee: Guarantee) -> bool {
        self.guarantees.contains(&guarantee)
    }

    /// Reports every guarantee separately, including the ones that do not hold.
    pub fn guarantee_report(&self) -> BTreeMap<&'static str, bool> {
        [
            Guarantee::FormatUnderstood,
            Guarantee::ResourceIntegrity,
            Guarantee::Authenticity,
            Guarantee::CompletenessWithinScope,
            Guarantee::HistoricalReconstruction,
            Guarantee::CompletenessBeyondScope,
        ]
        .into_iter()
        .map(|guarantee| (guarantee.label(), self.guarantee(guarantee)))
        .collect()
    }
}

/// Reads a package, checks it against an anchor and reduces its transitions.
///
/// This writes nothing. Replay here is deterministic reduction over recorded
/// payloads; it does not call a verifier, a model or an effect.
pub fn reconstruct(package: &ContinuationPackage, anchor: &Anchor) -> Result<Understood, Error> {
    reconstruct_within(package, anchor, Limits::default())
}

pub fn reconstruct_within(
    package: &ContinuationPackage,
    anchor: &Anchor,
    limits: Limits,
) -> Result<Understood, Error> {
    limits.check_counts(package)?;
    for (label, value) in [
        ("world", package.world.as_str()),
        ("schema", package.schema.as_str()),
        ("reducer", package.reducer.as_str()),
    ] {
        limits.check_identifier(label, value)?;
    }
    if package.schema != PACKAGE_SCHEMA {
        return Err(Error::UnsupportedSchema(package.schema.clone()));
    }
    if package.reducer != RECORDED_HISTORY_REDUCER {
        return Err(Error::UnsupportedReducer(package.reducer.clone()));
    }
    if package.world != anchor.world {
        return Err(Error::WorldMismatch {
            package: package.world.clone(),
            anchor: anchor.world.clone(),
        });
    }
    let recomputed = head_digest(package);
    if recomputed != anchor.head_digest {
        return Err(Error::AnchorMismatch {
            expected: anchor.head_digest.clone(),
            found: recomputed,
        });
    }

    let projection = reduce(&package.transitions, &package.world)?;
    for resource in &package.resources {
        let recorded = projection.objects.get(&resource.reference).ok_or_else(|| {
            Error::ResourceNotInHistory {
                reference: resource.reference.clone(),
            }
        })?;
        if recorded.digest != resource.digest || recorded.revision != resource.revision {
            return Err(Error::ResourceIntegrity {
                reference: resource.reference.clone(),
            });
        }
        if !resource.digest.starts_with("sha256:") {
            return Err(Error::ResourceIntegrity {
                reference: resource.reference.clone(),
            });
        }
        // A body claim the consumer cannot check is not accepted on trust. This
        // format carries no body bytes, so an `included` status is a declaration
        // with nothing behind it and is refused rather than believed.
        if resource.body_status == BodyStatus::Included {
            return Err(Error::UnverifiableBodyClaim {
                reference: resource.reference.clone(),
            });
        }
    }

    let mut admitted_proposals: Vec<String> = package
        .admissions
        .iter()
        .map(|admission| admission.proposal_id.clone())
        .collect();
    admitted_proposals.sort();
    let mut assurance_references: Vec<String> = package
        .admissions
        .iter()
        .flat_map(|admission| {
            admission
                .assessments
                .iter()
                .map(|assessment| assessment.assessment_ref.clone())
        })
        .collect();
    assurance_references.sort();
    assurance_references.dedup();
    let absent_bodies: Vec<String> = package
        .resources
        .iter()
        .filter(|resource| resource.body_status == BodyStatus::Absent)
        .map(|resource| resource.reference.clone())
        .collect();

    Ok(Understood {
        world: package.world.clone(),
        head_digest: recomputed,
        head_revision: package.head.world_revision,
        objects: projection
            .objects
            .iter()
            .map(|(name, state)| (name.clone(), state.digest.clone()))
            .collect(),
        admitted_proposals,
        assurance_references,
        absent_bodies,
        transitions: package.transitions.clone(),
    })
}

impl Understood {
    /// Writes the reconstruction into an explicitly chosen, isolated destination.
    ///
    /// The destination is a new World. Its trusted assurance providers are the
    /// consumer's own configuration, supplied here as an explicit argument, and
    /// never taken from the package: a reconstruction must not bootstrap its own
    /// authority, and a package can never vouch for itself. Reading the past does
    /// not require them; admitting anything new does.
    ///
    /// The live World and any external application are never touched.
    pub fn materialize(
        self,
        destination: impl AsRef<Path>,
        trusted_assurance_providers: BTreeSet<String>,
    ) -> Result<Reconstruction, Error> {
        let path = destination.as_ref().to_path_buf();
        let mut kernel = crate::Kernel::create(
            &path,
            crate::WorldBootstrap {
                world: self.world.clone(),
                trusted_assurance_providers,
                objects: Vec::new(),
            },
        )?;
        kernel.import_history(&self.transitions)?;

        let snapshot = kernel.snapshot()?;
        if snapshot.revision != self.head_revision {
            return Err(Error::ReconstructionDiverged {
                expected: self.head_revision,
                found: snapshot.revision,
            });
        }
        if kernel.replay()? != snapshot {
            return Err(Error::ReplayDiverged(self.world.clone()));
        }

        let mut guarantees = BTreeSet::from([
            Guarantee::FormatUnderstood,
            Guarantee::ResourceIntegrity,
            // The anchor arrived out of band and matched. Where no anchor is
            // supplied, callers must not report this.
            Guarantee::Authenticity,
            Guarantee::CompletenessWithinScope,
            Guarantee::HistoricalReconstruction,
        ]);
        guarantees.remove(&Guarantee::CompletenessBeyondScope);

        Ok(Reconstruction {
            world: self.world,
            world_revision: self.head_revision,
            head_digest: self.head_digest,
            objects: self.objects,
            admitted_proposals: self.admitted_proposals,
            assurance_references: self.assurance_references,
            absent_bodies: self.absent_bodies,
            guarantees,
            database: path,
        })
    }
}

/// Reduces recorded transitions to a projection. The same reduction runs when the
/// destination replays its own history, so the two cannot drift.
pub(crate) fn reduce(
    transitions: &[RecordedTransition],
    world: &str,
) -> Result<crate::WorldSnapshot, Error> {
    let mut revision = 0u64;
    let mut objects: BTreeMap<String, ObjectState> = BTreeMap::new();
    let mut started = false;
    for transition in transitions {
        match transition {
            RecordedTransition::Genesis {
                world: genesis_world,
                objects: genesis_objects,
            } => {
                if started {
                    return Err(Error::HistoryOrder("genesis is not first".into()));
                }
                if genesis_world != world {
                    return Err(Error::HistoryOrder(format!(
                        "genesis names {genesis_world}, the package names {world}"
                    )));
                }
                objects = genesis_objects.clone();
                started = true;
            }
            RecordedTransition::ChangeCommitted { patches, .. } => {
                if !started {
                    return Err(Error::HistoryOrder(
                        "a change appears before genesis".into(),
                    ));
                }
                revision += 1;
                for patch in patches {
                    let crate::Patch::PutObject {
                        reference,
                        expected_revision,
                        digest,
                    } = patch;
                    objects.insert(
                        reference.clone(),
                        ObjectState {
                            revision: expected_revision.map_or(1, |value| value + 1),
                            digest: digest.clone(),
                        },
                    );
                }
            }
        }
    }
    if !started {
        return Err(Error::HistoryOrder("no genesis transition".into()));
    }
    Ok(crate::WorldSnapshot {
        world: world.to_owned(),
        revision,
        objects,
    })
}

/// The digest a consumer checks against its out-of-band anchor.
///
/// It covers the world, the recorded history, the recorded admissions, the
/// resource manifest and the assurance references. It excludes the head itself,
/// which carries this value, and it carries no trust configuration.
pub fn head_digest(package: &ContinuationPackage) -> String {
    let material = serde_json::json!({
        "schema": package.schema,
        "reducer": package.reducer,
        "world": package.world,
        "origin": package.origin,
        "transitions": package.transitions,
        "admissions": package.admissions,
        "resources": package.resources,
        "assurance": package.assurance,
    });
    digest_of_bytes(&serde_json::to_vec(&material).unwrap_or_default())
}

/// Reads a package from disk with the same strictness as the change envelope: an
/// unknown key, a missing key or an unknown variant is a refusal.
/// Reads a package from disk, refusing an oversized file before parsing it.
pub fn read_package(path: impl AsRef<Path>) -> Result<ContinuationPackage, Error> {
    read_package_within(path, Limits::default())
}

pub fn read_package_within(
    path: impl AsRef<Path>,
    limits: Limits,
) -> Result<ContinuationPackage, Error> {
    let bytes = fs::read(path.as_ref()).map_err(|error| Error::Io {
        path: path.as_ref().display().to_string(),
        detail: error.to_string(),
    })?;
    if bytes.len() > limits.max_package_bytes {
        return Err(Error::LimitExceeded {
            limit: format!(
                "package of {} bytes above {}",
                bytes.len(),
                limits.max_package_bytes
            ),
        });
    }
    let text = String::from_utf8(bytes).map_err(|error| Error::Parse(error.to_string()))?;
    let package: ContinuationPackage =
        serde_json::from_str(&text).map_err(|error| Error::Parse(error.to_string()))?;
    limits.check_counts(&package)?;
    Ok(package)
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("unsupported package schema: {0}")]
    UnsupportedSchema(String),
    #[error("unsupported recorded-history reducer: {0}")]
    UnsupportedReducer(String),
    #[error("package names world {package} while the anchor names {anchor}")]
    WorldMismatch { package: String, anchor: String },
    #[error("package head {found} does not match the expected anchor {expected}")]
    AnchorMismatch { expected: String, found: String },
    #[error("resource {reference} disagrees with the reconstructed history")]
    ResourceIntegrity { reference: String },
    #[error("resource {reference} is claimed but absent from the history")]
    ResourceNotInHistory { reference: String },
    #[error("recorded history is not usable: {0}")]
    HistoryOrder(String),
    #[error("reconstruction reached revision {found} instead of {expected}")]
    ReconstructionDiverged { expected: u64, found: u64 },
    #[error("replay of the reconstruction disagrees with its own projection: {0}")]
    ReplayDiverged(String),
    #[error("package is not parsable: {0}")]
    Parse(String),
    #[error("import refused: {limit}")]
    LimitExceeded { limit: String },
    #[error("resource {reference} claims a body this consumer cannot verify")]
    UnverifiableBodyClaim { reference: String },
    #[error("cannot read {path}: {detail}")]
    Io { path: String, detail: String },
    #[error(transparent)]
    Kernel(#[from] crate::KernelError),
}
