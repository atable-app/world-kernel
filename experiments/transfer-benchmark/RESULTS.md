# Transferable experience: tranche 1

Protocol: docs/M5-PROTOCOL.md

## Slice 0: what is actually here

every dependency the brief names was checked in the real repository before anything was built

- **m1**: implemented and measured, 80 cases, three systems, the same decision in all 80. Negative result, unchanged.
- **m2**: implemented. src/continuation.rs, 19 public items. Capability for C2 only; the baselines have no export, so no cost comparison exists.
- **m3**: implemented and restored by ADR-004 after the reversal case fired.
- **m4**: alternatives and receipts only. No decision, no attempt, no failure record, no rationale: a grep for those four words returns zero in src/branch.rs. M5's failure memory is therefore owned by M5, as the brief's own section 15 defines TransferAttempt.
- **uni**: confirmed against the real repository rather than taken on trust: uni-evidence, uni-decision, uni-verify, incremental content-bound evidence, STALE, uni brief, uni bundle verify. The brief's description of the boundary is accurate.
- **productPaths**: IntentLane and Kollio are outside this workspace's boundary and Kollio is dirty with concurrent work. Slices 7 and 8 are recorded as not attempted rather than approximated.

## The corpus

30 closed cases.

| Family | Cases | Expected |
|---|---|---|
| exact transfer | 6 | directly_reusable |
| adaptable transfer | 6 | adaptation_required |
| deceptive similarity | 6 | incompatible |
| insufficient or unknown | 12 | additional_evidence_required and insufficient_information |

Not the 96-case corpus: the brief's 96-case corpus, bounded generative trees, the B and C UNI integrations and the JSONL fixture format are tranche 2 and are not claimed here

## Metrics

| Metric | Kernel | Baseline A | Target |
|---|---|---|---|
| falseDirectTransfer | 0 | 0 | 0 |
| falseIncompatibility | 0 | 0 | - |
| relevantFailureNotSurfaced | 0 | - | - |
| sourceAssurancePromotedToTarget | 0 | - | - |
| unknownCollapsedToFact | 0 | - | - |


## The prediction, and whether it held

Made: A and C are expected to tie on the closed corpus, because a competent baseline that compares the same declared conditions with the same unknown state does the same work in fewer lines.

Held: **true**

the safety metric is perfect on both sides, so it does not separate them, and the continuation gate's fifth condition cannot be scored from this. The tie means tranche 2's integration is not funded by anything in this artifact.

## Mutations

| Mutation | Failed a test |
|---|---|
| promote an unknown fact to a satisfied value | yes |
| treat a not-observed capability as unavailable | yes |
| copy source assurance onto the instantiated target candidate | yes |
| drop a recurrent prior failure from the verdict | yes |
| let a known violating fact fall through to directly reusable | yes |
| accept a plan whose target context revision moved | yes |
| treat an explicitly absent fact as unknown | yes |

## The continuation gate, scored

the brief's seven conditions, scored honestly against this artifact

| Condition | Met |
|---|---|
| closed-profile false direct transfers are zero | **no** |
| unknown target conditions are never silently promoted | **no** |
| source assurance is never silently promoted | **no** |
| one benchmark class safely avoids target work | **no** |
| the competent baseline does not give the same result at lower complexity | **undecidable** |
| one real IntentLane transfer demonstrates measurable reuse | **no** |
| Kollio consumes the same core without reimplementing it | **no** |

three met, one not met, one undecidable, two unreachable. M5 is funded up to the benchmark and no further, which is what the protocol said before the run.

## Not measured

- no transfer measurement in euros, tokens or human time; avoided work is a count of obligations and nothing more
- no B or C baseline using UNI, so no assurance-integration comparison exists
- no retrieval quality measurement: a capsule is handed to the planner, so the discovery problem is not exercised at all
- no generative bounded trees, no seeds, no counterexample reduction
- no real consumer: the corpus is closed invented arithmetic
