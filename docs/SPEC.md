# World Kernel specification

Status: experimental, normative for this repository  
Last verified: 2026-09-27 at commit `f686d9a`, M2 in progress  
Active milestone: M2 portable continuity, decided in [ADR-001](ADR-001-portable-continuation.md)

## 1. Document contract

This file is the single normative specification for World Kernel. It defines what the product is,
which guarantees it may claim, and what OpenCode should implement next.

The repository uses this source order:

1. Tests and running code establish current behavior.
2. This file establishes required behavior and sequencing.
3. `BOUNDARIES.md` records current ownership, trust assumptions and proof status.
4. `EXPERIMENT.md` records the experimental question and falsification gates.
5. `README.md` is a short entry point, not another specification.

When implementation and this file disagree, record the mismatch before changing either one. A code
path that exists is not automatically a required contract. A requirement written here is not
automatically an implemented guarantee.

The research mandate remains useful background, but it is not a second source of truth. Its durable
requirements are reduced here to testable statements.

## 2. Product definition

World Kernel is a local library and runtime for admitting and recording grounded changes inside a
declared digital scope.

A grounded change connects:

- an actor and an intent;
- an exact candidate;
- the versioned observations used to prepare it;
- the declared limits of the context view;
- assurance covering that candidate;
- current authority to perform the intent;
- deterministic state patches;
- a receipt and replayable history.

The Kernel owns only native World state and its admission history. It observes or references state
owned by other products. It never becomes the owner of an external document, contract, repository or
service merely by ingesting it.

### Product outcome

A second actor must be able to accept, reject or resume an exact change from public records without
trusting the first actor's conversation or private reasoning.

### Non-goals

World Kernel is not:

- an agent framework;
- a model router or cognitive compiler;
- a replacement for UNI policy and evidence semantics;
- a replacement for Kollio documents and commands;
- a universal source of truth;
- a generic distributed transaction system;
- a claim of universal exactly-once external effects;
- a shared application that absorbs IntentLane, UNI or Kollio.

## 3. Authority and ownership

The first profile is local and mono-authority. The host process, the Kernel database, the configured
UNI binary and the approved integration configuration are trusted.

| State | Canonical owner | Kernel may store |
|---|---|---|
| Native World object | World Kernel | Full versioned state and history |
| UNI contract, evidence and decision | UNI | Exact assessment reference, normalized evidence digest and provider identity |
| Kollio document | Kollio | Versioned observation and digest |
| Kollio impact result | Kollio | Reassessment frontier, never a truth verdict |
| External resource | Originating system | Reference, observed version, digest, freshness and limits |
| Human approval | Approving authority | Reference, scope, actor, candidate binding and validity window |

Integrations have two distinct profiles:

- Observer profile: the external product keeps canonical state. The Kernel cannot block direct
  mutations in that product.
- Admission profile: one declared class of operations passes through Kernel admission. The integration
  must identify the single effective write path or expose reconciliation when another path exists.

No profile may advertise admission guarantees while leaving an equivalent unmediated write path
available to the same actor.

## 4. Current protocol

The only supported change schema is `world-change/v0-experimental`, defined by
`schemas/world-change-v0.experimental.schema.json` and represented by `GroundedChange` in
`src/model.rs`.

### Required fields and semantics

| Field | Meaning | Admission requirement |
|---|---|---|
| `schema` | Envelope semantics | Must equal the supported schema exactly |
| `world` | Isolated authority scope | Must equal the opened Kernel World |
| `proposalId` | Human and audit identity | Recorded in the commit event and receipt |
| `idempotencyKey` | Retry identity | Same key plus same full proposal returns the stored outcome; same key plus different proposal is rejected |
| `actor` | Principal requesting admission | Current `AuthoritySource` must allow the intent |
| `baseRevision` | World revision read by the producer | Must equal the current revision inside the commit transaction |
| `intent.kind` | Permission and operation class | Must be authorized and included by the coverage profile |
| `intent.target` | Intended published subject | Must equal the candidate reference |
| `candidate` | Exact subject of assurance and publication | Reference and digest must match assessment and patch |
| `reads` | Positive versioned dependencies | Every reference, revision and digest must still match |
| `coverage` | Declared limits of the producer's view | Must not be truncated and must cover the intent kind |
| `assessments` | External assurance records | At least one trusted provider must cover the exact candidate |
| `patches` | Deterministic native World mutations | Preconditions must match and one patch must publish the candidate |

The schema accepts SHA-256 identifiers with the `sha256:` prefix. Portable canonical JSON is not yet
specified beyond the existing Rust serialization and adapter normalization. Cross-language digest
claims therefore remain experimental.

Two protocol corrections were found while building M1 and are now fixed:

- `GroundedChange` and every type it contains reject unknown fields, matching the
  `additionalProperties: false` the schema has always declared. A typo such as `baseRevison` used to
  deserialize silently into a default.
- The Rust `Patch` serialized its inner field as `expected_revision` while the schema has always
  required `expectedRevision`, so a change produced by this library failed its own published schema.
  Rust now emits `expectedRevision`, and `the_serialized_envelope_uses_exactly_the_published_schema_field_names`
  reads the checked-in schema and asserts the emitted field names, which makes the schema executable
  rather than decorative.

The second correction changes a stored event payload shape, so a Kernel database written by `c24b3fc`
or earlier cannot be replayed by this build. No such database is released and the schema is
`v0-experimental`, so the rollback is a revert rather than a migration. This is recorded here because
benchmark work was otherwise meant to leave the Kernel database untouched.

### Admission order

`Kernel::submit` must preserve these semantics:

1. Reject unsupported schema and wrong World without opening a write transaction.
2. Resolve idempotency. An exact retry returns the stored outcome because it performs no new mutation.
3. Check current authority and declared view coverage.
4. Require trusted assurance for the exact candidate.
5. Require the intent and a patch to publish that candidate.
6. Open an immediate SQLite transaction.
7. Recheck World revision, positive read dependencies and patch preconditions inside the transaction.
8. Apply native patches, advance the World revision, append one event and persist the idempotent
   outcome in the same transaction.
9. Commit once and return the stored receipt.

A rejection before commit changes no projection, event, submission record or World revision.

### Replay

Replay consumes recorded events in sequence and reconstructs only native World state. It does not call
UNI, Kollio, a model, a network service or an effect dispatcher. An unknown event type is a corrupt
state error, not an invitation to guess semantics.

## 5. Normative invariants

Each invariant needs a named test before it can be reported as proved in the current profile.

### WK-01 Exact subject

An assessment for candidate A cannot satisfy candidate B. Reference and digest must both match. For
UNI, a valid bundle evidence record must also name the workspace-relative candidate path with the same
SHA-256.

### WK-02 Current authority

A new submission checks authority at admission time. A retry of an already committed identical
submission may return its stored receipt without re-authorizing because no new mutation occurs.

### WK-03 Current context

The World revision and every declared positive dependency must still match inside the commit
transaction. Negative query dependencies are not supported in v0 and must not be claimed.

### WK-04 No half commit

Native patches, World revision, commit event and stored idempotent outcome succeed or roll back
together.

### WK-05 Impact is not truth

An impact or reassessment frontier identifies work to review. It does not negate an assertion, cancel a
human decision or mint a replacement verdict.

### WK-06 Replay has no effects

Replay performs database reads and deterministic reduction only.

### WK-07 Uncertain external effects stay uncertain

This invariant is reserved until effects exist. A lost response after dispatch must eventually produce
`unknown`, not `confirmed` and not an automatic blind retry.

### WK-08 No self-authorization

Trusted providers and authority come from World or host configuration, never from proposal fields.
The current mono-authority profile assumes untrusted code cannot instantiate arbitrary trusted
assessment values inside the host process.

### WK-09 Visible view limits

Truncated coverage or coverage that omits the intent kind is rejected. A complete empty view and an
unavailable view must remain distinguishable in future view protocols.

### WK-10 Product autonomy

UNI and Kollio remain independently usable. World adapters call public interfaces and never require a
migration of product-owned state.

### WK-11 Versioned semantics

Unknown change schemas and event types fail closed. Future reducers and effect semantics must carry
explicit versions.

### WK-12 Isolated scopes

World identity is part of every object, event and submission key. A proposal for one World cannot be
admitted by another.

## 6. Integration specifications

### UNI

`ProcessUniRunner` is the required production seam for the current profile:

1. Run `uni --json verify <contract>` in the declared workspace.
2. Run `uni --json bundle export <contract> --out <temporary-file>`.
3. Run `uni --json bundle verify <temporary-file>` and require `ok: true`.
4. Accept only `uni-bundle-0.1` while that is the implemented parser version.
5. Normalize valid evidence records from the verified bundle.
6. Run `uni --json report` and require `decision: Accepted`.
7. Hash the candidate before and after this sequence.
8. Require one valid evidence record whose `artifact_files` entry matches the relative path and digest.
9. Hash the normalized evidence, stable report and candidate into an `uni-bound:` assessment reference.

The collector may not read `.uni/evidence` or `.uni/decisions` directly. It may not duplicate UNI's
policy, cache, binding or decision code.

Current limitation: the two file hashes do not exclude an A-B-A mutation during verification. The
general workspace profile remains trusted-host only until M2 defines a sealed subject profile.

Live verification command. `UNI_BIN` must be an absolute path: the runner executes the binary with the
declared workspace as its working directory, so a relative path cannot resolve.

```bash
cargo build -p uni-cli --manifest-path ../uni/Cargo.toml
UNI_BIN="$(cd ../uni && pwd)/target/debug/uni" \
  cargo test --test uni_collector real_uni_cli_binds_valid_evidence_to_the_candidate -- --ignored
```

### Kollio

The current adapter is observer-only:

- `observe_document` maps `documentId`, `semanticRevision` and a JSON digest to an `ObjectRevision`.
- `reassessment_frontier` preserves `proposedChanges`, `unaffectedRefs` and `wasTruncated`.
- The adapter cannot apply a Kollio command, replace document content, move canvas objects or convert
  an impact into a verdict.

Any write integration must use Kollio's public command protocol and preserve Kollio's own validation
and user authority.

## 7. Current proof matrix

| Requirement | Status at `c24b3fc` | Evidence |
|---|---|---|
| WK-01 | Proved for envelope admission and current UNI file profile | `tests/kernel_contract.rs`, `tests/uni_collector.rs` |
| WK-02 | Proved for synchronous new submissions | authority removal contract test |
| WK-03 | Partial | global World revision and positive reads covered; negative queries absent |
| WK-04 | Proved for SQLite native state | injected failure before the last durable write rolls everything back |
| WK-05 | Proved at adapter type boundary | Kollio adapter returns `ReassessmentFrontier` only |
| WK-06 | Proved for current event types | replay equality and restart tests |
| WK-07 | Not implemented | no external effect model exists |
| WK-08 | Partial | provider allowlist and external authority source covered; host can still mint values |
| WK-09 | Proved for declared coverage | truncated and missing intent coverage are rejected |
| WK-10 | Proved for current seams | UNI public CLI, Kollio JSON translation, no cross-repository writes |
| WK-11 | Proved for current schema and events | unknown schema and event type fail closed; the serialized envelope is asserted field by field against the checked-in schema |
| WK-12 | Proved for current storage keys and admission | wrong World rejected and tables keyed by World; the corpus scope family runs a live sibling World |

The standard suite has 96 passing tests and one ignored live UNI contract test. This count is
descriptive, not a product metric.

## 8. Active milestone M1: equal-information admission benchmark

### Question

Does the portable World change envelope and receipt reduce admission errors or integration work beyond
a solid application-specific transaction using the same information?

### Systems

| ID | System | Required information |
|---|---|---|
| A | Application-specific SQLite admission gate | Candidate, current authority, current revisions, read set, coverage, verifier outcome, patch preconditions and idempotency |
| B | The same application gate with UNI assurance | Exactly A's information, with the verifier outcome produced through the UNI seam |
| C | UNI plus World Kernel | Exactly B's information encoded as `GroundedChange`, then admitted by `Kernel::submit` |

No system may receive a hidden oracle, an extra dependency or a weaker expected result. System A must
be competently implemented. The benchmark is invalid if it compares C with an intentionally naive
baseline.

### Files to create

| Path | Purpose |
|---|---|
| `tests/fixtures/admission-corpus.jsonl` | Versioned 80-case corpus, one self-contained case per line |
| `tests/support/mod.rs` | Test-only module routing shared fixtures, compiled by two test targets and the example |
| `tests/support/admission_case.rs` | Strict deserialization and validation for corpus cases |
| `tests/support/assurance.rs` | The declared assurance condition, produced through the UNI seam and through an application verifier |
| `tests/support/baseline_a.rs` | Independent application-specific SQLite implementation |
| `tests/support/baseline_b.rs` | Same application gate consuming the normalized UNI outcome |
| `tests/support/system_c.rs` | The Kernel path, its current-state reader and the test-only ablations |
| `tests/support/harness.rs` | Shared driver: workspace, prelude, fault injection, results, aggregation |
| `tests/corpus_contract.rs` | Strictness and corpus invariant tests |
| `tests/comparative_admission.rs` | Runs A, B and C against every case and checks expected decisions |
| `examples/admission_benchmark.rs` | Emits deterministic JSON results and aggregate counts |
| `experiments/admission-benchmark/README.md` | Method, commands, environment and interpretation limits |
| `experiments/admission-benchmark/results.json` | Checked-in machine-readable result for the recorded environment |
| `experiments/admission-benchmark/RESULTS.md` | Generated human view, including failures and continuation decision |

The example reaches `tests/support` with `#[path = "../tests/support/mod.rs"] mod support;` so the
benchmark systems and the corpus parser are literally the same code in the test and the example.

Production Kernel code must not acquire benchmark switches. Ablations belong in the test-only baseline
and runner.

### Corpus contract

Every JSONL record is one line and contains every one of these keys, including
the ones whose value is `null`. The key set follows the fixture model fixed
below, not the earlier draft of this example, which omitted `prelude` and
`assurance` and used `initial` for `bootstrap`.

```json
{"schema":"world-kernel-admission-case/v1","id":"candidate.reference.001","family":"candidate_exactness","expected":{"status":"rejected","code":"ASSESSMENT_MISMATCH"},"bootstrap":{"world":"world:bench","trustedAssuranceProviders":["uni"],"objects":[{"ref":"requirement:demo","revision":1,"digest":"sha256:requirement-v1"}]},"prelude":[],"proposal":{...the GroundedChange envelope...},"current":{"grants":{"principal:worker-a":["publishCandidate"]},"candidatePath":"candidate.bin","candidateContents":"candidate-a","candidateContentsAfterAssurance":null},"assurance":{"mode":"accepted","provider":"uni","evidencePath":"release.uni"},"fault":null,"notes":"An assessment naming a different reference cannot cover this candidate."}
```

`initial`, `proposal` and `current` must deserialize into named Rust fixture types. They may not remain
untyped `serde_json::Value`. The fixture types must expose all information to all three systems even
when one implementation does not use a field.

Value convention: every enum value in a corpus record is `snake_case`, except
`expected.code`, which mirrors the Kernel's own `SCREAMING_SNAKE_CASE`
serialization so one code is the same string in the corpus, in a rejection and in
a report.

A case is **adverse** when it injects a fault or when the correct answer is not a
clean commit. The 60 adverse cases are 45 that must be rejected, 9 that are
unsupported, and 6 commit-interruption cases whose expected result is a commit
after a clean retry from an injected failure. A committed expectation is a benign
case.

The fixture model is fixed for M1:

```rust
struct AdmissionCase {
    schema: String,
    id: String,
    family: CaseFamily,
    expected: ExpectedOutcome,
    bootstrap: WorldBootstrap,
    prelude: Vec<PreludeSubmission>,
    proposal: GroundedChange,
    current: CurrentFixture,
    assurance: AssuranceFixture,
    fault: Option<FaultPoint>,
    notes: String,
}

struct PreludeSubmission {
    change: GroundedChange,
    grants: BTreeMap<String, BTreeSet<String>>,
}

struct CurrentFixture {
    grants: BTreeMap<String, BTreeSet<String>>,
    candidate_path: String,
    candidate_contents: String,
    candidate_contents_after_assurance: Option<String>,
}

struct AssuranceFixture {
    mode: AssuranceMode,
    provider: String,
    evidence_path: String,
}

enum AssuranceMode {
    Accepted,
    Rejected,
    EvidenceRequired,
    InvalidBundle,
    UnsupportedBundle,
    MissingRequiredEngine,
    RegistryDrift,
    WatchedFileDrift,
}

enum FaultPoint {
    BeforeObjectWrite,
    BeforeWorldRevision,
    BeforeEventAppend,
    BeforeSubmissionRecord,
    LostResponseAfterCommit,
}

struct ExpectedOutcome {
    status: ExpectedStatus,
    code: Option<BenchmarkCode>,
}

enum ExpectedStatus {
    Committed,
    Rejected,
    Unsupported,
}
```

`CaseFamily` is the snake-case family column below. `BenchmarkCode` contains every current
`RejectionCode`, every `UniCollectorError::code()` value used by the corpus, and `UNSUPPORTED`.
`candidate_path` is workspace-relative and must reject `..`, absolute paths and symlink escapes. The
harness writes `candidate_contents` as UTF-8 bytes, runs assurance, optionally replaces it with
`candidate_contents_after_assurance`, then performs admission. Prelude submissions run in listed order
with their own grants before the measured proposal. A prelude submission is world-state preparation
rather than a measured decision, so it is admitted with the assurance already established for that
prior commit; only the measured proposal goes through the case's declared assurance step.

`proposal.candidate.digest` must equal the digest of the effective candidate contents in every case
whose expected code is not `ASSESSMENT_MISMATCH` or `CANDIDATE_MISMATCH`. Only a deliberate producer
error may publish bytes its assurance never covered, so a stale digest can never hide inside an
unrelated rejection.

### Unsupported cases

A case declares an unimplemented capability in the producer's own declared view protocol or operation
class, never in a separate flag:

| Declaration | Meaning | Cases |
|---|---|---|
| `coverage.profile = closed-v1-negative-queries` | a `none exist` dependency, M4 | 4 |
| `coverage.profile = closed-v1-graph-traversal` | a cycle or a traversal budget | 3 |
| `intent.kind = dispatchExternalEffect` | external effect dispatch, M3 | 2 |

A system reports `unsupported` when a case falls outside its declared support surface. A runner may
read the case and never `expected`. The declared surface is printed in the result so an over-broad
claim is visible rather than hidden inside a total, and the corpus validator rejects a `committed`
expectation on a case that declares an unimplemented capability.

Fault points are implemented with temporary SQLite triggers against the relevant table. The
`LostResponseAfterCommit` case commits, drops the first Kernel instance without returning its outcome,
opens a new instance and repeats the identical proposal. Each baseline must reproduce the same
observable fault boundary without sharing Kernel implementation code.

The corpus has exactly 80 cases:

| Family | Adverse | Benign | Required coverage |
|---|---:|---:|---|
| Candidate exactness | 6 | 2 | reference mismatch, digest mismatch, changed file, exact repeat |
| Authority | 5 | 2 | missing grant, revoked grant, unrelated permission, valid grant |
| World and object concurrency | 7 | 2 | stale World, stale read, patch conflict, independent valid update |
| Idempotency and restart | 5 | 3 | exact retry, conflicting retry, lost response, reopened database |
| Coverage limits | 4 | 2 | truncated, missing intent, complete empty read set |
| Assurance trust | 5 | 2 | untrusted provider, non-accepted decision, invalid bundle, valid UNI result |
| Schema and scope isolation | 4 | 2 | unknown schema, wrong World, similar identifiers across Worlds |
| Commit interruption | 5 | 1 | failure before each durable write class, successful commit |
| Negative query dependency | 4 | 0 | expected known limitation, never counted as a pass for C |
| UNI environment and policy | 4 | 1 | missing required engine, registry drift, watched-file drift |
| Reassessment semantics | 3 | 1 | impact without automatic truth inversion |
| Adversarial input | 3 | 1 | imported text cannot change policy or authority |
| Graph termination | 3 | 0 | cycle and traversal budget remain explicit future requirements |
| External effect uncertainty | 2 | 1 | expected unsupported cases, never simulated as confirmed |
| Total | 60 | 20 | 80 cases |

Unsupported future cases remain in the corpus with an expected result of `unsupported`. They count as
incorrect if a system reports `committed` or `confirmed`. They do not count as implemented passes.

### Output contract

The example runner emits one JSON object per system and case:

```json
{
  "caseId": "candidate.substitution.001",
  "system": "C",
  "actual": {"status": "rejected", "code": "ASSESSMENT_MISMATCH"},
  "expected": {"status": "rejected", "code": "ASSESSMENT_MISMATCH"},
  "correct": true,
  "durationMicros": 420
}
```

The aggregate report must state:

- incorrect admissions;
- unjustified rejections among the 20 benign cases;
- unsupported cases by family;
- decision-code disagreements between A, B and C;
- median and p95 runtime, reported as descriptive only;
- production and integration lines added per system, with the counting command recorded;
- every failure, without filtering failed cases from totals.

### M1 acceptance criteria

1. The corpus contains exactly 80 unique IDs, 60 adverse cases and 20 benign cases with the family
   counts above.
2. A schema or strict fixture parser rejects missing fields, unknown enum values and duplicate IDs.
3. Systems A, B and C consume the same deserialized case object.
4. Each system produces a structured result for all 80 cases. Panics and skipped cases fail the run.
5. No system incorrectly admits an adverse case in a family it claims to support.
6. Each system has at most one unjustified rejection among the 20 benign cases.
7. Known unsupported cases are labeled `unsupported`; they are never rewritten as successes.
8. Three test-only input ablations run before normal C admission: replace the bound assessment with a
   forged current-candidate assessment, omit changed dependencies from the read set, and replace a
   truncated manifest with a complete one. Each ablation must incorrectly admit at least one matching
   adverse case. No production check receives a disable flag.
9. `RESULTS.md` is reproducible from `results.json` with a documented command and contains the complete
   failure list.
10. The existing 19 standard tests, the live UNI contract test, Clippy, formatting, schema check and
    vertical example remain green.

### M1 continuation gate

Continue to M2 only when all acceptance criteria pass and the result distinguishes at least one of
these outcomes:

- C prevents a class of error that competent A and B do not prevent with comparable implementation
  effort;
- C provides a reusable receipt, replay or handoff property whose equivalent materially increases A
  or B integration work;
- C adds no measured value, in which case reduce the project to the smallest useful adapter or pattern.

Do not claim a percentage of human-time improvement from the deterministic corpus. That metric requires
repeated human tasks and belongs to a later study.

### M1 result

All ten acceptance criteria pass. A, B and C each produce a structured result for all 80 cases, none
incorrectly admits an adverse case it claims to support, none has more than one unjustified rejection
among the 20 benign cases, the 9 unsupported cases stay `unsupported` in all three systems, and each of
the three test-only ablations weakens a decision on 4, 7 and 4 adverse cases respectively.

The three systems reach the same decision in all 80 cases, with no decision-code disagreement. C did
not prevent a class of error that competent A and B do not also prevent. The recorded cost is 392 lines
for A, 55 for B over the same gate, and 730 for C, excluding the shared fixture and the shared UNI
seam. This corpus never consumes a receipt, so the handoff and replay value of C is neither measured
nor disproved.

This is the third continuation-gate outcome. It was decided on 2026-09-27 in
[ADR-001](ADR-001-portable-continuation.md): M2 continues on a different and explicit hypothesis,
portable continuity of work, rather than admission superiority. The M1 result above is preserved
unchanged and is not restated in a more favourable form.

## 8a. M2: portable continuity

M2 asks whether a fresh consumer can reconstruct a work state and its justifications from exported
data, then determine what can be resumed in its own current context, without depending on the
producer's session, private database or paths. The matrix, the primary outcome and the criteria are
pre-registered in [M2-PROTOCOL.md](M2-PROTOCOL.md), written before the first retained run. The recorded
result is in [experiments/continuation](experiments/continuation/README.md).

Two properties of the current implementation bound what any M2 claim may say:

- The Kernel stores references, revisions and digests, never object bytes. A continuation package
  reconstructs a projection and its history, and declares every resource body absent. A hash without
  accessible content does not reconstruct an artifact.
- The recorded event log did not carry what was declared, so a continuation could not have answered
  under which rules and on which evidence a change was admitted. An `admissions` record now holds the
  declared change beside its outcome, written in the same transaction, so the two cannot diverge.

A continuation is never a self-authorization. The reconstructed World's trusted assurance providers
are the consumer's own configuration, passed explicitly, and a reconstruction never writes them. Reading
the past requires no trust; admitting anything new is a separate, explicit act.

## 9. Planned milestones after M1

### M2 Sealed single-file subject profile

Eliminate the current A-B-A window for a constrained single-file release artifact. The collector must
verify and later publish bytes from one host-owned immutable staging object. General mutable workspace
verification remains a separate, weaker profile.

Required proof: a concurrent mutator cannot cause bytes B to be verified while bytes A are admitted or
published under A's digest.

### M3 External effect ledger

Add typed effect intentions only after M1 and M2. States are `pending`, `dispatched`, `confirmed`,
`failed`, `unknown` and `compensated`. Replay never dispatches. A lost response after send produces
`unknown` and requires reconciliation.

Required proof: crash injection before send, after send and after confirmation never causes a blind
duplicate.

### M4 Context completeness

Add versioned authority snapshots, negative query dependencies and named coverage profiles whose
completeness is established by trusted collectors rather than producer declaration.

Required proof: adding an object that invalidates a prior `none exist` read makes the proposal stale.

### M5 Portable reassessment frontier

Connect at least two independent domains without moving their rules into the Kernel. UNI decides proof
sufficiency. Kollio decides how a human revises a retained decision. The Kernel carries changed
references and structured reasons to re-evaluate.

Required proof: the second adapter adds no domain condition to `Kernel::submit` and retains its
standalone product path.

## 10. Stop conditions

Reduce or stop the platform direction when any of these is observed:

- a competent application-specific transaction provides the same safety, replay and handoff value at
  lower total integration cost;
- a second adapter requires domain rules inside the Kernel;
- exact subject binding cannot be kept through the real application path;
- an operation advertised as mediated remains writable through an equivalent unmediated path;
- users do not reuse receipts, reassessment frontiers or handoff records;
- modeling and adapter work costs more than the errors or restart work it removes.

The right result may be a small reusable admission library. The experiment must be allowed to discover
that result.

## 11. Rollback and compatibility

- All current schemas remain experimental. Breaking changes require a new schema identifier and a
  migration or explicit refusal path.
- Benchmark work is isolated to tests, examples and `experiments/`; reverting it must not migrate the
  Kernel database.
- An adapter change must leave the integrated product usable without World Kernel.
- No milestone may require rewriting existing UNI or Kollio storage.
- Git revert is the rollback for code-only milestones until a stable schema is published.

## 12. OpenCode completion protocol

For each logical slice:

1. Name the SPEC requirement and acceptance criterion being implemented.
2. Add a failing test or corpus case first.
3. Implement the smallest production change that satisfies it.
4. Run the repository verification commands from `AGENTS.md`.
5. Update the current proof matrix only when the new test directly demonstrates the guarantee.
6. Update the active milestone status in this file. Do not create a parallel roadmap document.
7. Commit the slice locally with no attribution footer. Do not push without explicit approval.
