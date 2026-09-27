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

The third gate is now the live one. It is a product decision, and it is open.

## Completed hardening step

The trusted collector now runs UNI's complete public CLI path, verifies a versioned evidence bundle,
binds an exact candidate digest and rejects uncovered or substituted artifacts. SQLite fault injection
proves rollback at the last durable write, and process restart proves receipt recovery after a lost
response. Replay remains effect-free because this slice has no external effect dispatcher.

## Next experiment

The M1 continuation decision comes first: either fund M2 and measure the immutable single-file subject
profile, or reduce the project to the smallest useful adapter or pattern. The recorded result gives no
correctness argument for the envelope, so that choice has to be made on the receipt and replay value
rather than on admission errors.

If M2 proceeds, the next measurement is an A-B-A filesystem race experiment. After that, an external
effect dispatcher is justified only if the first two results show the cost is worth paying; do not add
one before then.
