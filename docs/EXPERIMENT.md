# Vertical-slice experiment

This file records the experiment and its falsification gates. Normative requirements and the active
implementation milestone live only in [SPEC.md](SPEC.md).

## Question

Can a second actor apply or reject an exact candidate from public records without trusting the first
actor's conversation, while preserving enough history to reconstruct the resulting World?

## Slice

The runnable example joins three independent pieces:

1. A Kollio document is observed as a versioned dependency. Kollio remains its owner.
2. A UNI stable report and UNI-verified evidence bundle are normalized into an assurance reference for
   an exact candidate.
3. World Kernel checks the current reads and authority, commits the candidate and emits a receipt.

Run it with:

```bash
cargo run --example vertical_slice
```

The contract tests then inject mutation during verification, candidate substitution after assurance,
authority removal, stale dependencies, incomplete context, idempotency conflicts and failure at the
last durable write. Every rejected or interrupted case must leave the World revision and projection
unchanged. A restart test also proves that a lost response after commit returns the original receipt
without advancing the World twice.

## Baselines

The equal-information benchmark is implemented and has been run. All three systems consume the same
deserialized case object, the same workspace bytes and the same current authority.

| System | Required implementation | Recorded |
|---|---|---|
| A | A solid application-specific transaction with the same checks | `tests/support/baseline_a.rs`, 392 lines |
| B | The same application using UNI for assurance | `tests/support/baseline_b.rs` over A, 55 lines |
| C | UNI plus the portable World change envelope and replayable receipt | `src/kernel.rs`, `src/model.rs`, 730 lines |

A is competent by construction: it implements the same checks with its own tables, its own transaction
order and its own authority list, and it requires a claim to cover the bytes its verifier actually
verified.

The result over all 80 cases is in
[`experiments/admission-benchmark/RESULTS.md`](../experiments/admission-benchmark/RESULTS.md). All
three systems reach the same decision in every case, with no decision-code disagreement. The
portable envelope prevented no class of error that a competent application transaction does not also
prevent on these 80 conditions.

That is a negative result about admission correctness, and it is deliberately recorded as one. The
corpus never consumes a receipt, so the replay and handoff value of C is untested rather than
disproved. No claim about human time follows from a deterministic corpus.

The three test-only ablations are what show the corpus is capable of detecting a missing check: a
forged assessment binding admits 4 exactness cases, dropping changed reads admits 7 concurrency and
reassessment cases, and completing a truncated coverage manifest admits 4 coverage cases. No
production check carries a disable flag.

## First falsification gates

Stop generalizing the Kernel if any of these occurs:

- a second adapter requires domain rules inside the Kernel;
- exact-subject binding cannot be made reliable without replacing UNI;
- a solid application-specific transaction provides the same handoff and replay value at lower total
  integration cost;
- users do not reuse receipts or reassessment frontiers after context changes.

None of these four gates fired. The third one, whether a competent application provides the same value
at lower cost, was measured twice and came back negative both times, for admission safety and then for
incremental revision. The fourth, whether users reuse receipts, was never reachable: no human ran the
system, so that gate is untested rather than passed.

## Completed hardening step

The trusted collector now runs UNI's complete public CLI path, verifies a versioned evidence bundle,
binds an exact candidate digest and rejects uncovered or substituted artifacts. SQLite fault injection
proves rollback at the last durable write, and process restart proves receipt recovery after a lost
response. Replay remains effect-free because this slice has no external effect dispatcher.

## What followed M1, in order

M1 concluded "no admission superiority, cost measured". That is a negative result and it was recorded as
one.

M2 asked whether work survives its producer. A fresh consumer can now reconstruct a work state and its
justifications from an exported package and decide what it may resume. The capability was demonstrated
for the Kernel only, because the two application baselines had no export of their own, so no cost
comparison exists. Result: [experiments/continuation](experiments/continuation/README.md).

M3 asked whether recorded work is revisable without being rebuilt, and whether the difference is
explainable. Five dimensions stayed separate, a consumed dependency forced re-evaluation, a recorded
human decision was never recomputed, and every result carried an explanation. 15 of 24 pre-registered
stories were covered, 4 partial, 5 not. Transfer to a second domain and human utility were never
measured. Result: [experiments/incremental](experiments/incremental/README.md).

Then the comparison, in [ADR-003](ADR-003-measure-before-building.md). A full recompute, a competent
application cache, the same application consuming UNI staleness, and the impact core were given the same
seven scenarios. All four agreed on every observable result. The application avoided the same work as the
core on six of the seven. On the seventh the core ran no evaluator where the application ran two, because
a consumed facet is a narrower cache key than a whole value, and that cost 1630 lines. Result:
[experiments/incumbent-comparison](experiments/incumbent-comparison/RESULTS.md).

The reduction was applied. `src/impact` left the shipped surface, the rules it enforced were written down
as rules a consumer can be held to in [IMPACT-CONTRACT.md](IMPACT-CONTRACT.md), and the engine stayed in
the repository so the comparison can be repeated.

## Why it stopped

Three milestones ran in sequence and produced the same sentence each time: capability established,
differential value not measured. The pattern is the finding. Every remaining increment was spent adding
capability, and capability was not the thing in doubt.

The equal-information comparison was the one measurement that could settle it, and it settled it against
the Kernel on the incremental scope. One clause of its pre-registered condition stayed false and is
recorded as false: the core does buy a consumed facet, worth 2 evaluator runs on a closed 3-evaluator
fixture. The reduction rests on the narrower ground that every other capability the core produced was
unconsumed by the fixture, so the decision was not to keep paying for capability nothing had measured.

That was the stopping point M1, M2 and M3 did not have. It did not hold: ADR-004 reversed the reduction
on the same day, because the reversal case ADR-003 itself required fires, and M4 resumed at tranche 1 only.
M5 then measured transferable experience against the same standard and produced the same sentence a fourth
time. Its gate scored condition 5 on complexity, as Amendment 1 of its protocol required, and the
competent baseline reached the same decisions and the same safety outcomes in 515 lines against the
Kernel's 1007, so the brief's reduction trigger fired rather than merely being available. The reduction
the trigger ordered is applied by [ADR-005](ADR-005-reduce-to-the-representation.md): the transfer planner
left the shipped surface, the rules it enforced are written down as rules a consumer can be held to in
[TRANSFER-CONTRACT.md](TRANSFER-CONTRACT.md), the representation `src/experience.rs` stayed in the crate,
and the measured planner stayed in the repository so the comparison can be repeated. The recorded transfer
artifact was not rescored, and its gate still reads three met, two not met, two unreachable; tranche 2 is
not funded, so M5 closes as a reduction on the same disc as the ones before it, not as an extension.

## What would resume it

Exactly one thing, and it is cheap. Re-run `cargo run --example incumbent_comparison` against a graph of
wide nodes with many unconsumed fields. The consumed-facet advantage is the only measured advantage the
removed engine had, and it grows with the number of unconsumed fields per node and with graph width. On
this fixture it was 2 evaluator runs against 1630 lines. If a wider graph makes that saving scale past the
line count, the reduction was wrong and the rules in `docs/IMPACT-CONTRACT.md` should be implemented
properly rather than documented. Until that measurement exists, the reduction stands and the cost of being
wrong is one example runner.

Nothing else resumes it. A second domain, a human path, an external effect ledger and the A-B-A
filesystem race are all unbuilt, unauthorized and not worth resuming on the evidence in this repository.
