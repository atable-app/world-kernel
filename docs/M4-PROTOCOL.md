# M4 protocol, tranche 1

Date: 2026-09-27
Status: pre-registered, written before the tranche-1 results were retained
Relates to: [ADR-004](ADR-004-the-facet-advantage-scales.md), the branch brief of 2026-09-27

This file fixes the stories tranche 1 must cover and the way each one is decided, before any result is
kept. It is not the full 32-story matrix from the brief. That matrix is not authorised by ADR-004 and is
not claimed. What is authorised is the audit, this protocol, the Alpha/Beta fixture, an independent oracle,
isolated snapshots, and the proof that a combination of 110 is blocked while a composition of 100 is
admissible after a decision.

## The fixture

Invented for tests. It is not Sarah, not IntentLane, not a real budget and not a real decision. All values
are abstract integers and no model is called.

```text
S0
  capacity = 100
  load_a   = 40
  load_b   = 40
  rule     load_a + load_b <= capacity

Alpha, from S0:  load_a = 55   -> 95, satisfies
Beta,  from S0:  load_b = 55   -> 95, satisfies
Beta', from S0:  load_b = 45   -> 85, satisfies

Alpha then Beta  -> load_a = 55, load_b = 55 -> 110, violates capacity = 100
Alpha then Beta' -> load_a = 55, load_b = 45 -> 100, satisfies exactly
```

The two changes touch distinct fields. There is no value conflict in the structural sense. There is a rule
violation in the combination, which is the point: a merge can be well-formed and still be wrong.

## The primary red test

1. Create Alpha and Beta from S0. Each is correct in its own right.
2. Adopt Alpha into the target by the authorised path. The target is then 95.
3. Propose Beta's change, computed from its own base S0, into that target.
4. Require: a candidate at 110, an explicit obligation, and **no change to the target**.
5. Require: the Alpha and Beta histories remain readable, and the blocking reason names the rule and the
   versions actually examined.

Then immediately, because a test that blocks every adoption is not a test:

6. Add Beta' at `load_b = 45`. The composition is exactly 100 and must be admissible after the required
   decision and assurance.

## Stories covered by this tranche

| ID | Story | Central assertion |
|---|---|---|
| M4-01 | Fork then change an inherited object | source and sibling branches unchanged |
| M4-02 | The target advances after a branch is created | no floating read; re-grounding is explicit |
| M4-11 | Alpha 55 + Beta 55, capacity 100 | combination at 110 blocked although each branch is valid alone |
| M4-16 | A favourable result under an assumed capacity of 120 | no promotion into the real target at 100 |
| M4-15 | Partial adoption then a second adoption | the excluded part is not treated as already merged |
| M4-25 | Fork, compare, replay, archive | zero unauthorised external calls |

M4-02, M4-15 and M4-25 are included only where tranche 1 can actually decide them. A story that cannot be
decided by tranche 1 is listed as not covered rather than approximated.

## How each story is decided

- The expectations live in the test harness. No engine code reads an expected value, a case id, or a
  property named `expected`.
- The oracle materialises the small snapshots' values, applies the requested selection under an explicit
  fixture contract, and recomputes every constraint from scratch. It calls neither the composition planner
  nor the incremental engine nor their conflict resolution.
- A candidate built by the Kernel is compared against the oracle separately from the engine that
  evaluates it. Two evaluators agreeing about a wrongly built candidate prove nothing about the build.
- Adoption of a candidate is compared against a from-scratch evaluation of that same candidate.

## The properties asserted

For a successful adoption:

- the resulting target equals the approved candidate;
- the source branch is unchanged;
- the provenance names the exact selected units;
- no unselected input entered the target silently;
- earlier decisions are still readable;
- no external call was made.

## Mutations that must each fail a test

- drop the check on the composed candidate;
- omit the branch context from the reuse key;
- promote an assumption into an observation;
- treat a partial selection as a full merge;
- import the source's assurance as current;
- accept an approval that no longer matches the candidate.

No production disable flag is added for any of these.

## Explicitly not claimed by this tranche

- the full 32-story matrix;
- portable export and import of branch history, which is conditioned on M2 and not attempted here;
- any IntentLane or Kollio path;
- human utility, which needs a person and cannot be measured by test count;
- the 32-story matrix's generative bounded trees, which arrive with the runner in a later tranche;
- that a branch isolation test proves network or plugin isolation. It does not.

## Continuation

If the primary red test and the admissible composition both hold, tranche 1 is complete and tranche 2,
which is the assumption context, the branch-keyed cache and the read sets, is the next decision. If the
blocked combination can only be achieved by building a second impact engine, the answer recorded in ADR-004
applies: reduce rather than proceed.
