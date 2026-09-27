# M3 incremental revision experiment

The recorded run of the experiment defined in [`docs/M3-PROTOCOL.md`](../../docs/M3-PROTOCOL.md)
and decided in [`docs/ADR-002-revisable-work.md`](../../docs/ADR-002-revisable-work.md).

## Question

When the conditions change, does recorded work become revisable without being rebuilt, and can the
difference be explained?

The engine answers four questions about a changed scope: what changed in the observed scope, what can
still be reused and on what basis, what is now necessary, and why each object is concerned or was kept.

## What exists

| Piece | Where |
|---|---|
| The model: nodes, versions, captured reads, query reads, the five dimensions, obligations, explanations | `src/impact/model.rs` |
| The engine: plan, re-evaluate, stop propagation, publish, plus the independent oracle | `src/impact/engine.rs` |
| The closed arithmetic fixture and its evaluators | `tests/support/impact_fixture.rs` |
| The equivalence property and the arithmetic mutations | `tests/incremental_revision.rs` |
| The confirmation matrix and the mutation checks | `tests/m3_confirmation.rs` |
| The recorded result and its generated human view | `results.json`, `RESULTS.md` |

## Commands

```bash
cargo run --example m3_revision                    # record
cargo run --example m3_revision -- --render-only   # RESULTS.md from results.json
cargo test --test incremental_revision              # the property and the mutations
cargo test --test m3_confirmation                  # the story matrix
```

## The oracle is structurally independent

`full_recompute` receives a snapshot, a target list, the evaluators and the trust configuration. It has no
parameter through which an `EvaluationRecord`, an index or an invalidation function could arrive, so it
cannot call the incremental strategy even by accident. It rebuilds every requested node and iterates until
nothing moves, which is what a clean build does. The essential small cases also carry manual expectations,
so a mistake common to an evaluator cannot make both routes equally wrong.

## Five dimensions, and no validity flag

`HistoryState`, `Currency`, `business_verdict`, `Authority` and `Coverage` are separate results. A test that
passed on v1 stays historically passed; after its subject changes, its result may no longer suffice without
either being false. `recomputed_same` keeps a negative business verdict: identical is not satisfied, and the
verdict is carried through from the domain rather than computed here.

## Profiles are granted, not self-declared

An evaluator may claim a weaker profile than the consumer permits. Claiming a stronger one is refused with
`ProfileNotGranted`. A recompute never promotes a record's coverage either: a guarantee is regained by
capturing more, not by recomputing. A declaration in the data cannot award itself the closed profile, and
`completeness_beyond_scope` is never claimed.

## A dependency is what was consumed, not what was mentioned

Every run records the facet it consumed, the binding it used, the comparator version that fingerprinted it,
and whether the value was present or an observed absence. A change to a consumed field that leaves the
produced value identical is `recomputed_same`, and its consumers are not run. A set read records the
*declared* member set and re-resolves it, so a member that appears is visible to the read; re-resolving only
the members that existed last time would make a new member invisible, which was a real defect found while
building this.

A change to an object nothing consumed leaves its targets `unchanged_in_scope`, which is a different
statement from `reused_after_check`. The first says no change could reach it; the second says a change
reached it and its consumed values held.

## A decision is not a computation

A recorded decision is never recomputed. When the support it declares moved, the engine produces a
`HumanReview` obligation and reports `NeedsHumanReview`. It does not substitute a new decision, and the
recorded wording, author and date are not rewritten. This is asserted against the snapshot itself, not only
against the engine's output.

## Obligations, not automatic re-run of everything

An obligation names a target and revision, a reason code, a work type, preconditions, the evidence that
would close it and the authority required. The dedup identity covers the target, the reason, the work type
and the profile, so repeating a plan does not duplicate a task and an incompatible context does not reuse
the old identity. An agent's "done" is not in the acceptance criteria of any obligation.

## Publication is a conservative compare-and-swap

`assert_publishable` compares the plan's target revision with the world's current revision and refuses on any
difference. Finer per-read-set validation is not implemented and is not assumed safe.

## Measured work

Evaluation counts are reported next to the reference full recompute, so an avoided evaluation and a pass that
merely re-ran everything are never confused. The ratio is never converted into tokens, euros or unobserved
human time.

## What this session did not establish

- **H3-Transfert.** No second domain, so no transfer cost and no claim that the abstraction is useful.
- **H3-Utilite.** No consented observation. Human utility is not measured, and automated tests do not
  establish it.
- **No R3, A3, B3 comparison.** The equal-information comparison the mandate asks for is not run, so
  differential value is unmeasured and the fixture cannot speak to it.
- **No IntentLane path, no Kollio or Sarah scenario.** Not inspected, so no capability is claimed there.
- **Five of twenty-four stories not covered and four partial**, listed in `RESULTS.md` with the reason. The
  uncovered ones are a conditional branch, time-triggered expiry, contradiction arbitration, a durable store
  for the engine itself, and the second domain.

## The fixture is not a product

The arithmetic fixture is invented for tests. It is small enough to check by hand, which is the point, and
useless as evidence about real work. No reliability rate follows from a hand-designed suite.
