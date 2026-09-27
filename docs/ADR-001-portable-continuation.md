# ADR 001: M2 continues on portable continuity, not on admission superiority

Date: 2026-09-27
Status: accepted for one bounded experiment
Supersedes: nothing
Relates to: [SPEC.md](SPEC.md) sections 2, 8 and 10, [EXPERIMENT.md](EXPERIMENT.md)

## Context

M1 ran an equal-information admission benchmark over 80 cases. All ten acceptance criteria passed. The
recorded result is negative for the original hypothesis: systems A, B and C reached the same decision
in all 80 cases, with zero decision-code disagreement, and C prevented no class of error that a
competent application-specific transaction does not also prevent. Recorded cost was 392 lines for A,
55 for B over the same gate, and 730 for C, excluding the shared fixture and the shared assurance seam.

That result stands. It is not restated here in a more favourable form. The three test-only ablations
showed the corpus is capable of detecting a missing check; they do not show a value unique to the
Kernel.

What M1 could not measure is the product outcome stated in SPEC section 2: that a second actor can
accept, reject or resume an exact change from public records without trusting the first actor's
conversation or private reasoning. The corpus never consumes a receipt, so receipt, handoff and replay
value is untested rather than disproved.

## Decision

Continue on a different and explicit hypothesis: portable, verifiable, revisable continuity of work,
rather than admission superiority over a competent application.

The M2 question:

> Can a fresh consumer reconstruct a work state and its justifications from exported data, then
> determine what can be resumed in its own current context, without depending on the producer's
> session, private database or paths?

M2 is a bounded experiment. M3, incremental cross-domain invalidation, is a horizon and is not
implemented to make M2 pass.

## What is reused rather than reinvented

The local UNI implementation at `c1d4c1f`, binary `uni 0.9.4`, already provides the transport and
invalidation semantics the continuation half needs. M2 links to them rather than copying them:

| Existing UNI surface | What it already does | Who keeps it |
|---|---|---|
| `uni bundle export` / `bundle verify` | JSONL header plus one record per artifact, each carrying `sha256` over its canonical body, with cross-checks that reject evidence gathered under another registry or contract | UNI |
| Evidence states and 7 invalidation dimensions | `contract_hash`, `artifact_hash`, verifier `fingerprint`, `registry_hash`, `commit_sha` plus `platform`, `policy_hash`, and time | UNI |
| `.uni/events.jsonl` journal | Append-only event stream with rotation, `uni events`, byte-deterministic OTLP export | UNI |
| `uni brief` | The work order: per-claim obligation, required or critical status, resolved registry command, watched files | UNI |

UNI states plainly that importing a bundle never injects proofs into a live cache: verification only
reports, and transport is not authority. M2 preserves that property. The Kernel does not re-implement
claim evaluation, evidence validity or policy; it records references and versions to those artifacts
and refuses to interpret them as its own authority.

The Kernel keeps responsibility for its own versioned states, admitted transitions and application
preconditions. Applications keep their business models and their interfaces. A continuation package is
a link between existing pieces and introduces no second source of truth and no second authority.

## A constraint discovered before designing

The Kernel stores object references, revisions and digests. It never stores object bytes. Therefore a
Kernel continuation package can reconstruct the projection and its history, and it cannot reconstruct
an artifact body it was never given.

This is not a defect to fix inside M2. It is the boundary the package must declare: a hash without
accessible content does not reconstruct an artifact, so the package carries an explicit body status
rather than implying the bytes are there. Any M2 claim about reconstructing a work state is bounded to
what the Kernel actually recorded. A consumer that needs artifact bytes must receive them from the
producer's export or declare the dependency absent and refuse to resume what it cannot verify.

## Three operations, kept separate

1. **Historical read.** What state was recorded, which change was admitted, at which revisions, under
   which rules and on which evidence. A receipt is a historical assertion by its issuer. Its internal
   consistency does not by itself prove its authenticity, the quality of its tests, or the truth of its
   observations.
2. **State replay.** Reconstruct a projection from an explicitly accepted origin or snapshot and the
   recorded transitions that follow. Replay consumes recorded outputs. It does not call the model, the
   tools or the effects that produced them. Replay writes only to an explicitly chosen isolated
   destination, and never modifies the live World or an external application. A historical
   reconstruction does not need the reader's current permissions in order to describe the past, while
   access to the data remains subject to the reader's permissions.
3. **Continuation evaluation.** Compare the candidate or the next obligations against the consumer's
   own current context: revisions read, artifacts, relevant assurance, capabilities, policies and
   authority. An old authorization does not transfer automatically. A continuation produces a new
   proposal and, where applicable, a new admission. A change of model does not automatically revoke an
   independent proof over unchanged bytes, but any real dependency on the evaluator, its environment
   or its version must be represented.

## Consequences

Accepted:

- A continuation package format becomes a production surface of the Kernel, so it needs a version, a
  strict parser and a named-schema rejection path like `world-change/v0-experimental`.
- Guarantee results must be structured, not a single boolean: format understanding, resource
  integrity, authenticity per the trust profile actually available, completeness relative to the
  announced scope, historical reconstruction, and current applicability are distinct outcomes.
- Import must be bounded in size and count, treat paths as untrusted, forbid destination escape and
  symlink escape, and never execute a command or load a privileged policy from a package. Secrets and
  private justifications are not exported by default.
- A hash chain alone cannot detect a complete rewrite or a coherent truncation without an expected head
  or anchor held out of band. Only integrity relative to a digest received over a trusted channel is
  claimed unless a separate authenticity mechanism is actually tested.
- A fresh consumer is a real interoperability requirement, so at least one consumer outside this
  repository's language must read the experimental format. A minimal TypeScript or Node reader that does
  not import the Kernel's Rust library is an interoperability test, not proof of adoption.

Rejected:

- M2 as a hardening milestone. M2 strengthens nothing that M1 showed to be worth strengthening. Sealed
  single-file subjects remain M2 in the old ordering and are not started here.
- Reusing A and B unchanged. An equal-information comparison must give the baselines the functions the
  comparison needs, including their own log, snapshot, export and application-level resume.
- A new signing infrastructure. Existing mechanisms are reused where they satisfy the requirement, and
  where only digest-relative integrity is tested, that is stated exactly.

## Status and honesty condition

This ADR records a product decision to run one bounded experiment. It does not record an outcome.

If a small UNI adaptation plus an application log covers the same uses at the same or lower cost, the
recommendation will be to reduce, keeping the learnings, the corpus and the useful formats, and not to
continue out of attachment to the name World Kernel. If performance is equivalent but cost was not
measured, the conclusion will be "capability established, differential value not measured", never
"product success".
