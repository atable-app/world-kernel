# The transfer contract, for a consumer

Status: normative for any consumer that reuses a past experience in a new context
Last verified: 2026-09-27

The transfer planner was measured against a competent application gate and moved out of the Kernel's
shipped surface by [ADR-005](ADR-005-reduce-to-the-representation.md). The code now lives in
`tests/support/transfer_core.rs`. The capsule representation it operated on, `src/experience.rs` and
`schemas/experience-capsule-v0.experimental.schema.json`, stays in the Kernel.

This document states the same rules from the consumer's side, for a consumer that implements its own
transfer gate rather than depending on this one, which is what the reduction says a consumer should do.
Where the two disagree, the moved implementation and its tests are what this repository actually does,
and this document is the claim about what a consumer may rely on.

`the_transfer_contract_and_the_moved_implementation_agree` in `tests/transfer_plan.rs` reads this
document and the moved implementation and fails if either names a variant the other does not, so a
status cannot be added, dropped or renamed on one side without the other disagreeing.

## What this is not

It is not an interface. There is no trait to implement and no function to call. A consumer that decides
to follow these rules follows them; one that does not, does not, and the document does not stop it. What
this buys is that the rules are written down somewhere a consumer can read without reading 684 lines of
test support.

## Rule 1: five statuses, never a boolean

A reuse decision is one of five outcomes. Collapsing them to a yes-or-no loses the two answers a
retrieval system most often needs: that a difference exists and a transformation covers it, and that the
answer is not known yet.

`DirectlyReusable` is about the **experience structure**. It never asserts that the source's evidence or
assurance became the target's.

## Rule 2: unknown is not absent, and not-observed is not unavailable

A difference report keeps four separate buckets: matched, different, unknown, unavailable. An unknown
key and an absent key are different findings, and a capability nobody checked and a capability known to
be missing are different findings. There is no `is_equivalent` convenience that folds them.

A consumer that reports one boolean, "compatible or not", has already lost the case this milestone
exists to catch: the superficially near-identical target where one condition that matters differs.

## Rule 3: a plan is bound to both revisions

A plan names the capsule revision and the target context revision it was computed against. If either
moves, the plan is stale. A stale plan is refused rather than reinterpreted, because reinterpreting it
is a decision made with the old context and labelled with the new one.

## Rule 4: a source assurance never becomes a target assurance

An assurance obligation is bound to the target, always. An instantiated candidate starts with an empty
assurance list, constructed empty at the construction site rather than enforced by a caller remembering
to clear it. Evidence gathered in the target is target evidence; it is never the source's evidence
under another name.

## Rule 5: declared coverage is a claim, not a grant

A capsule declares how complete its own conditions are. That declaration is read as a claim by whoever
consumes it. The planner honours `ClosedDeclared` only when the target's own context declares the same,
because a producer never awards itself a stronger profile than the consumer grants. The same reason the
impact engine refuses a self-awarded profile applies here.

## Rule 6: nothing here performs an external effect

Planning is read-only: it changes no state, calls nothing and creates no assurance. Instantiating
produces a value describing what was assembled. Neither step dispatches, publishes or retries
anything.

## The stable vocabulary

A consumer may depend on these names. The conformance test compares each table against the enum it
describes, so a rename on one side fails the build.

### TransferStatus

| Variant | Means |
|---|---|
| `DirectlyReusable` | the experience structure can be reused as it stands |
| `AdaptationRequired` | a known difference needs a transformation a declared adaptation covers |
| `AdditionalEvidenceRequired` | a relevant condition is not observed yet |
| `Incompatible` | a known target fact violates a required condition and no adaptation covers it |
| `InsufficientInformation` | the capsule does not declare enough to decide; a statement about the source |

### UnknownReason

| Variant | Means |
|---|---|
| `FactUnknown` | the target does not know this fact |
| `CapabilityNotObserved` | nobody has checked this capability in the target |
| `KeyAbsentFromDeclaration` | the capsule depends on a key it never declared |

### InstantiationError

| Variant | Means |
|---|---|
| `StalePlan` | the capsule or the target context moved since the plan was computed |
| `StatusRefusesCandidate` | the status does not permit a candidate to be created from this plan |

### ApplicabilityCoverage

| Variant | Means |
|---|---|
| `ClosedDeclared` | the domain guarantees all relevant inputs are captured |
| `PartialDeclared` | known conditions are captured; completeness is not guaranteed |
| `Opaque` | discovery only; direct reuse cannot be established from this capsule |

## Known surface this contract does not vouch for

`TransferOutcome`, `TransferAttempt` and the `ConditionKind` variants nothing constructs are declared in
the moved implementation and are never produced by it. They are carried across unchanged because the
move was not allowed to rewrite the measured implementation. A consumer should not treat their presence
as a guarantee that the planner exercises them.
