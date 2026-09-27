# The impact contract, for a consumer

Status: normative for any consumer of revised work  
Last verified: 2026-09-27

The impact engine was measured against a competent application and left the Kernel's shipped surface, as
decided in [ADR-003](ADR-003-measure-before-building.md). What was removed was the mechanism. What
survives is the set of rules the mechanism happened to enforce, written down here so that a consumer can
be held to them without depending on the Kernel.

This document is the source of truth. The engine in `tests/support/impact_core/` is the implementation
that was measured, kept only so the comparison can be repeated.
`the_contract_document_and_the_measured_codes_agree` in `tests/m3_comparison.rs` reads both and fails if
they part company, so a rule cannot be dropped or renamed here without the measurement disagreeing.

## What this is not

It is not an interface. There is no trait to implement and no type to construct. A consumer that decides
to follow these rules follows them; one that does not, does not, and the document does not stop it. The
enforcement that used to exist in the Kernel is gone, and saying otherwise would be false. What this buys
is that the rules are written down somewhere other than in a module that was measured and discarded.

## Rule 1: there is no validity flag

History, currency, business verdict, current authority and coverage are five separate results. A result can
be historically true and no longer usable without either being false.

A consumer that stores one boolean per object has collapsed five things. The collapse is invisible on a
fixture where they happen to agree, and it is the first thing that breaks on a real one.

## Rule 2: a recompute cannot promote a record

Recomputation re-derives a value. It does not grant coverage, widen a granted profile, or resolve an
object that was never evaluated. A record that said `partial` still says `partial` after a recompute that
agrees with it.

## Rule 3: profiles are granted by the consumer, never claimed by the producer

An evaluator may claim a weaker profile than the configuration grants. It may never claim a stronger one,
and a data manifest cannot award itself the closed profile. A refusal to run is a normal outcome, not an
error to be worked around.

## Rule 4: a recorded human decision is never recomputed

When the support a decision declared has moved, the correct response is an obligation naming a human
review and a change to nothing. Recomputing the decision is the failure this rule exists to prevent, and
it fails quietly, because the recomputed answer usually looks reasonable.

## Rule 5: a dependency is what a run consumed

Provenance explains. Only a consumed dependency forces re-evaluation. A set read records its declared
member set and re-resolves it, so a member added later is visible to the read rather than invisible to it.

## Rule 6: publication is a conservative compare-and-swap

Internal commit and external effect are different acts. A publish is refused when the world revision moved
since the revision was planned against. Finer validation is not assumed safe: it was not measured, so it is
not claimed.

## Rule 7: nothing here launches an external effect

No dispatcher, no retry, no outbox. An engine that revises work may produce an obligation; it may not act
on the world.

## The stable codes

A consumer may depend on these strings. They are the codes the measured engine emits, and the conformance
test compares this list against the engine's source, so a rename here without a change there fails the
build.

### Explanations

| Code | Means |
|---|---|
| `no_change_reaches_this_object` | nothing the object consumed moved, so it is reused untouched |
| `consumed_facets_unchanged` | a change reached it, and every facet its evaluators consumed still held |
| `recomputed_same` | it was re-evaluated and produced the same value |
| `recomputed_changed` | it was re-evaluated and produced a different value |
| `no_recorded_evaluation` | there is no record, so there is nothing to reuse |
| `no_evaluator_registered` | no evaluator exists for a node that needs one |
| `no_evaluator_for_changed_consumer` | a changed consumer has no evaluator, so its consumers are affected |
| `recorded_decision_support_moved` | a recorded decision's declared support moved, so a human must look |

### Obligations

| Code | Work owed |
|---|---|
| `recorded_decision_support_moved` | `HumanReview` |
| `no_recorded_evaluation` | `PureRecompute` |
| `no_evaluator_for_changed_consumer` | `MissingReference` |

## The reversal case, restated once

The comparison found exactly one measured advantage for the removed mechanism: a consumed facet is a
narrower cache key than a whole value, worth 2 evaluator runs on a closed 3-evaluator fixture. On a graph
of wide nodes that advantage would grow. Rule 5 is why that is true, and a consumer that wants it should
implement the facet and re-measure rather than restore the engine. The runner is
`cargo run --example incumbent_comparison`.
