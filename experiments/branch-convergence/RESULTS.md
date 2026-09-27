# Branch convergence: tranche 1

Protocol: docs/M4-PROTOCOL.md · Decision: docs/ADR-004-the-facet-advantage-scales.md

Design consequence: Alpha and Beta change different fields, so there is no value conflict to detect. A system that only compared values would merge them cleanly and produce a target that breaks its own rule. The gate is therefore a constraint check on the reconstructed candidate.

## The primary test

`a_combination_of_two_valid_branches_is_blocked_and_the_target_does_not_move`

Alpha and Beta are each sound at 95. Their composition is 110 against a capacity of 100. The candidate is built, the rule is checked on it, the target does not move, and the refusal names the rule and the versions examined.

Counterweight: a_revised_branch_composing_exactly_to_the_limit_is_admissible, because a test that blocks every adoption is not a test

## Stories

| ID | Story | State | Where |
|---|---|---|---|
| M4-01 | fork then change an inherited object | covered | `tests/branch_convergence.rs, forking_and_changing_leaves_the_source_and_its_sibling_untouched` |
| M4-02 | the target advances after a branch is created | covered | `tests/branch_convergence.rs, a_target_that_moved_makes_the_proposal_stale_rather_than_rebased` |
| M4-11 | Alpha 55 + Beta 55, capacity 100 | covered | `tests/branch_convergence.rs, a_combination_of_two_valid_branches_is_blocked_and_the_target_does_not_move` |
| M4-16 | a favourable result under an assumed capacity of 120 | notCovered | `tranche 1 has no assumption context; the scenario is pre-registered for tranche 2 and is not approximated here` |
| M4-15 | partial adoption then a second adoption | partial | `the receipt names the exact selection and the excluded part stays in the source; the second adoption over the same unit is not exercised` |
| M4-25 | fork, compare, replay, archive with zero external calls | partial | `archive is implemented and fork is covered; there is no replay, no comparison surface and no external-call counter in tranche 1` |

## Mutations that had to fail

| Mutation | Test it broke |
|---|---|
| drop the constraint check on the composed candidate | `a_combination_of_two_valid_branches_is_blocked_and_the_target_does_not_move` |
| treat a partial selection as a full merge | `a_partial_adoption_does_not_make_the_excluded_part_look_merged` |
| accept a proposal whose target revision moved | `commit refused the stale proposal` |
| guess an unknown base instead of refusing | `an_unknown_base_is_refused_rather_than_guessed` |

## Absent by design

- no multi-parent merge; the base is a single known ancestor and an unknown one is diagnosed
- no automatic re-grounding; a moved target yields a stale proposal
- no automatic merge of any kind
- no safe_to_merge boolean, because structure, constraints, assurance, authority and currency are five questions
- no second impact engine; the constraint check reuses what already exists

## Not authorised by ADR-004

- the other 26 stories of the 32-story matrix, which ADR-004 does not authorise
- portable export and import of branch history, conditioned on M2 and not attempted
- any IntentLane or Kollio path
- human utility, which needs a person and cannot be measured by a test count
- bounded generative trees, which arrive with the runner in a later tranche
- that a branch isolation test proves network or plugin isolation; it does not

## Not measured

- no cost per evaluation, so no claim in euros, tokens or human time
- no replanning cost from the conservative compare-and-swap, which is retained deliberately
- no equal-information comparison against a competent branching application, which the M1 and M3 protocol would both require before any claim of superiority
- no real consumer: the fixture is invented arithmetic
