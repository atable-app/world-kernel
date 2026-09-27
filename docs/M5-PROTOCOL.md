# M5 protocol, tranche 1

Date: 2026-09-27
Status: pre-registered, written before the tranche-1 results were retained
Relates to: [ADR-004](ADR-004-the-facet-advantage-scales.md), the transferable-experience brief of 2026-09-27

## Slice 0: the real state, verified rather than assumed

The brief requires this before anything is implemented, so it is recorded first, with the commands that
produced it.

| Dependency the brief names | Real state in this repository |
|---|---|
| M1 admission benchmark | Implemented and measured. 80 cases, three equal-information systems, the same decision in all 80. Negative result, unchanged. |
| M2 portable continuation | Implemented. `src/continuation.rs`, 19 public items, versioned package with an `admissions` table. Capability for C2 only: the two application baselines have no export of their own, so no cost comparison exists. |
| M3 selective re-evaluation | Implemented and restored. `src/impact`, 15 engine items and 32 model items. Reduced away by ADR-003, restored by ADR-004 after the reversal case fired. |
| M4 alternatives, **attempts, decisions, failures and rationale** | Only the first exists. `src/branch.rs` has `Branch`, `Proposal`, `Prepared`, `Gate` and `Receipt`. It has **no** decision, no attempt, no failure record and no rationale: a grep for those four words returns zero. |
| UNI as the assurance layer | Confirmed against the real repository, not taken on trust. `uni-evidence`, `uni-decision`, `uni-verify`, incremental content-bound evidence, `STALE` on commit or watched-file change, `uni brief` for deterministic handoff, `uni bundle verify` for offline audit. The brief's description of the boundary is accurate. |

Two consequences, and neither is a reason to stop:

1. **M5's failure memory cannot lean on M4.** The brief says M4 supplies attempts, decisions, failures and
   rationale. In this repository it supplies alternatives and receipts only. So `TransferAttempt` is owned
   by M5, as the brief's own section 15 defines it, and M5 tranche 1 depends on M4 only for what exists: a
   `Proposal` and a `Branch` that a new candidate can be created from.
2. **The Kollio and IntentLane slices are not reachable from here.** Neither repository is inside this
   workspace's boundary, Kollio is dirty with concurrent user work, and the standing instruction is not to
   edit either. Slice 7 and slice 8 of the brief are therefore recorded as not attempted rather than
   approximated.

## The continuation gate the brief sets, scored against reality

M5 is funded beyond the benchmark only if all seven hold. None holds today, and two of them cannot be
scored at all yet:

| Condition | State |
|---|---|
| closed-profile false direct transfers are zero | not met, nothing is built |
| unknown target conditions are never silently promoted | not met |
| source assurance is never silently promoted | not met |
| one benchmark class safely avoids target work | not met |
| the competent baseline does not give the same result at lower complexity | **unmeasurable**, no baseline exists yet |
| one real IntentLane transfer demonstrates measurable reuse | not met, no reachable path |
| Kollio consumes the same core without reimplementing it | not met, no reachable path |

The gate is therefore not passed, and the only honest reading is that M5 is funded **up to the benchmark**
and no further. That is the scope of this tranche.

### Amendment 1, 2026-09-27: condition 5 is scored on complexity

The table above is left as it was written, because pre-registering a gate and then editing it after the
run is the failure the pre-registration exists to prevent. This is an additive amendment and it changes
the score of exactly one condition.

**What was recorded.** The run scored condition 5 as `undecidable`, on the ground that *"the baseline ties
on the corpus. Whether it is lower complexity is not answerable from a line count alone."*

**Why that is not a reason to withhold the score.** The objection is a real one in general: a line count is
a poor proxy for complexity, and a project can be short and incomprehensible. It does not apply here,
because the brief nominates the same measure twice. `core_loc` is one of the brief's own secondary metrics
in section 24, and the reduction trigger in section 2 is written in terms of complexity: the Kernel *"must
be reduced accordingly"* if the competent baseline provides the same safety and reuse with materially lower
complexity. The brief therefore supplies both the metric and the trigger, and both are in the same units.

**The rule, stated before the numbers are applied to it.**

> Condition 5 is not met when the recorded baseline reaches the same corpus decisions and the same safety
> outcomes as the Kernel on **fewer** counted lines.

Three clauses, each checkable against the artifact: the same decisions, which the recorded tie establishes;
the same safety outcomes, which the metrics table establishes with `falseDirectTransfer` 0 = 0 and
`falseIncompatibility` 0 = 0; and fewer counted lines.

**On the word "materially".** The brief writes *materially* and this amendment does not define a threshold,
because the measured ratio does not need one. `kernelSurface` is 1007 and `baselineA` is 515, a ratio of
**1.96**. Any threshold at or below 1.96 makes the condition fail, and 1.96 is not a margin anyone would
choose as *clearly* lower. The recorded note also says the baseline's count is *"an over-estimate"*, because
it shares its fixture file with the oracle, which makes the real ratio larger and the conclusion stronger.
A test asserts the ratio is under 2.0 so that the verdict cannot be made to depend on where a threshold is
put.

**What the amendment does not do.** It does not claim the baseline is better. On this corpus nothing
separated the two systems on any measured outcome, which is the finding and not a footnote to it: **the
corpus found no case in which the extra 492 lines bought anything.** The 6 `falseIncompatibility` belong to
a different and deliberately weaker baseline, `baselineWithoutDeclaredParameters`, and not to A, which
scores 0 like the Kernel.

### Amendment 2, 2026-09-27: the reduction the trigger called for is applied

Amendment 1's condition 5 came out not met, so the brief's reduction fired.
[ADR-005](ADR-005-reduce-to-the-representation.md) applies it: `src/transfer.rs` moves to
`tests/support/transfer_core.rs` and `pub mod transfer` leaves `src/lib.rs`. The shipped M5 surface is
`src/experience.rs` alone, 323 lines, against a recorded `kernelSurface` of 1007 and a `baselineA` of 515.
The representation ships and the decision procedure does not, because the measurement was about the
procedure.

**Where this document's path now points.** Below, "How each is decided" says the oracle calls nothing in
`world_kernel::transfer`. That text is left as written, because it records what was true when the protocol
was pre-registered and when the corpus ran. The module is now `tests/support/transfer_core.rs`, and the
claim underneath it is unchanged: `tests/support/transfer_fixture.rs` calls nothing in `transfer_core`
when it computes an expectation, so a bug in the planner and a matching bug in the expectation still
cannot cancel out.

**The recorded score is not re-run.** `experiments/transfer-benchmark/` is untouched by the reduction.
`kernelSurface` still reads 1007 because the example counts the mechanism where it now lives rather than
where it used to live, `baselineA` still reads 515, the ratio is still 1.96, and condition 5 is still not
met. A recorded score belongs to the run that produced it, and a reduction does not get to rescore the gate
that ordered it.

**What is still not funded.** Tranche 2, the 32-story matrix, the IntentLane transfer and Kollio consuming
the same core. Moving code into test support is not integration.

**The reversal case** is stated with its commands in ADR-005: a consumer outside `tests/` that reads a
capsule and reaches a wrong decision where the planner reached the right one, or a wider corpus that
separates A from C. Neither is deferred behind a milestone, because ADR-003 wrote a reversal case, called
it cheap, and then deferred it.

## What this tranche is

The smallest closed fixture where superficial similarity would cause an incorrect transfer, plus the strict
contracts that make the error impossible to express by accident, plus the competent baseline, because a
capability measured without one is the exact mistake ADR-004 records.

Included:

- a strict `ExperienceCapsule` and `ContextSnapshot` that cannot collapse unknown into absent;
- a deterministic `ContextDelta` that is a comparison against declared conditions, not a textual diff;
- a `TransferPlan` with the five statuses and **no boolean**;
- instantiation that creates a new identity and promotes no assurance;
- `TransferAttempt` and prior-failure surfacing;
- baseline A, retrieval plus an application gate, with no Kernel dependency;
- a closed corpus across the brief's four families, with an independent oracle.

Excluded, and not approximated: the 96-case corpus, bounded generative trees, the B and C baselines as UNI
integrations, the JSONL fixture format, the two product integrations, and any transfer measurement in money
or human time.

## The five facts a target can be in

The point of the whole milestone, and the reason the types are shaped the way they are. A fact is
established, explicitly absent, unknown, or not yet observed. A capability is available, unavailable,
unsupported, or not observed. Collapsing these into `None` is what makes a retrieval system dangerous, so
the type system refuses to.

## The primary red test, from the brief's section 16

Source: `capacity = 100`, `load = 80`, `minimum_reserve = 10`, intervention "reserve 10 before admission",
observed source result accepted with no overflow.

Target B: `capacity = 100`, `load = 80`, `minimum_reserve = 30`.

Required: `adaptation_required`. The difference `minimum_reserve: 10 -> 30` named. The source intervention
not reusable as-is. An adaptation obligation to reserve at least the target's minimum. A recomputation
obligation and a target-specific assurance obligation. The source history untouched.

A retrieval baseline that filters on `capacity` and `load` alone finds this experience with high confidence,
because capacity and load are identical. That similarity is the trap and the test is the trap.

## The other four red tests

| Test | Required |
|---|---|
| target C, `load = 70`, `minimum_reserve = 10` | `directly_reusable`, and instantiation copies no assurance |
| deceptive similarity: same framework, different runtime, `storage.local` unavailable | `incompatible`, or `adaptation_required` only if a declared server adaptation exists |
| `capability.app_intent_schema_X` never observed in the target | `additional_evidence_required`, with an observation obligation. **Not** `directly_reusable` and **not** `incompatible` |
| a prior transfer failed under `single_writer = false` and the target also establishes it | the failure is surfaced, and the failed intervention is not proposed as direct reuse |

## How each is decided

- Expectations live in the harness. No engine code reads an expected value, a case id, or a field named
  `expected`.
- The oracle is a separate program over plain values. It receives the declared conditions, the target and
  the declared adaptations, and computes the status and obligations directly. It calls nothing in
  `world_kernel::transfer` or `world_kernel::experience`.
- A plan's status is asserted together with its obligations, because a right status for the wrong reason is
  still a wrong plan.
- Baseline A is written to be competent, not to lose. It does metadata filtering and a deterministic
  comparison, and it has its own explicit unknown state. If it ties with the Kernel, that is recorded as a
  tie and the Kernel's value is the line count, not the outcome.

## What is expected, stated before the run

Recorded here so it cannot be read as a surprise afterwards: **A and C are expected to tie on the closed
corpus.** A competent baseline that compares the same declared conditions with the same unknown state is
doing the same work in fewer lines. The interesting measurement is therefore not the outcome and not the
safety metric, both of which should be perfect on both sides on a closed fixture. It is whether the Kernel
earns its lines anywhere, and if it does not, tranche 2's integration is not funded.

## Mutations that must each fail a test

- promote an unknown target fact to a satisfied condition;
- treat a not-observed capability as unavailable, which turns a missing observation into a false refusal;
- copy a source assurance ref onto an instantiated target candidate;
- let a prior failure be silently dropped when its condition matches;
- let a known violating fact fall through to `directly_reusable`;
- accept a plan whose target context revision moved.

No production disable flag is added for any of these.
