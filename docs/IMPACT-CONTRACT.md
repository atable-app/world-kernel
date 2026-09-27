# The impact contract, for a consumer

Status: normative for any consumer of revised work  
Last verified: 2026-09-27

The impact engine was measured against a competent application and reduced away by
[ADR-003](ADR-003-measure-before-building.md), then restored by
[ADR-004](ADR-004-the-facet-advantage-scales.md) once the reduction's own reversal case fired. The engine
is in `src/impact` and it is part of the Kernel again.

This document states the same rules from the consumer's side, for a consumer that implements its own
revision rather than depending on this one. Where the two disagree, the engine and its tests are what the
Kernel actually does, and this document is the claim about what a consumer may rely on.

`the_contract_document_and_the_measured_codes_agree` in `tests/m3_comparison.rs` reads this document and
the engine's source and fails if either names a code the other does not, so a rule cannot be dropped or
renamed here without the implementation disagreeing.

## What this is not

It is not an interface. There is no trait to implement and no type to construct. A consumer that decides
to follow these rules follows them; one that does not, does not, and the document does not stop it. What
this buys is that the rules are written down somewhere a consumer can read them without reading the
engine.

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

## Rule 5 is the one that earned the engine back

A consumed facet is a narrower cache key than a whole value. On a node carrying fields nobody reads, a
whole-value key re-runs every consumer and a facet key re-runs none. Measured across fan-out widths: 3
consumers avoids 3 evaluations, 1 000 avoids 1 000, 10 000 avoids 10 000, and padding the node from one
unconsumed field to eight changes nothing.

That single mechanic is why the engine is back. A consumer implementing Rule 5 has to implement it: once
a run has declared which part of a node it consumed, a whole-value key is the wrong key. The runner is
`cargo run --example incumbent_comparison`, and the measurement is `the_facet_advantage_scales_with_the_number_of_consumers`.
