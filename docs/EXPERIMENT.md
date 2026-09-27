# Vertical-slice experiment

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

The eventual benchmark must compare equal-information systems:

| System | Required implementation |
|---|---|
| A | A solid application-specific transaction with the same checks |
| B | The same application using UNI for assurance |
| C | UNI plus the portable World change envelope and replayable receipt |

This repository currently establishes C's minimal mechanics and a conformance suite. It does not yet
claim that C beats A or B. The comparative task study, 80-case corpus and 30 percent human-time target
from the research brief have not been run.

## First falsification gates

Stop generalizing the Kernel if any of these occurs:

- a second adapter requires domain rules inside the Kernel;
- exact-subject binding cannot be made reliable without replacing UNI;
- a solid application-specific transaction provides the same handoff and replay value at lower total
  integration cost;
- users do not reuse receipts or reassessment frontiers after context changes.

## Completed hardening step

The trusted collector now runs UNI's complete public CLI path, verifies a versioned evidence bundle,
binds an exact candidate digest and rejects uncovered or substituted artifacts. SQLite fault injection
proves rollback at the last durable write, and process restart proves receipt recovery after a lost
response. Replay remains effect-free because this slice has no external effect dispatcher.

## Next experiment

Run the same cases against an application-specific transaction baseline, then add an immutable
candidate snapshot or descriptor-based locking experiment to measure and eliminate the remaining A-B-A
filesystem race. Do not add an external effect dispatcher until those two results justify the extra
state machine.
