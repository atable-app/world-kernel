# M1 equal-information admission benchmark

This directory holds the recorded run of the benchmark described in
[`docs/SPEC.md`](../../docs/SPEC.md) section 8. It is a measurement, not a
product. Nothing here is imported by the library.

## Question

Does the portable World change envelope and receipt reduce admission errors or
integration work beyond a solid application-specific transaction using the same
information?

## Systems

| ID | System | Implementation |
|---|---|---|
| A | Application-specific SQLite admission gate | `tests/support/baseline_a.rs` |
| B | The same gate, with the verifier outcome produced through the UNI seam | `tests/support/baseline_b.rs` over `tests/support/baseline_a.rs` |
| C | UNI plus the World Kernel | `src/kernel.rs`, `src/model.rs`, `tests/support/system_c.rs` |

A is deliberately competent. It implements the same checks in application terms
with its own tables, its own transaction order and its own authority list, and
it requires a claim to cover the bytes its verifier actually verified. A
benchmark that beat a naive baseline would prove nothing, so the SPEC forbids
one and this benchmark has none.

A, B and C all receive the same deserialized `AdmissionCase`, the same workspace
bytes and the same current authority. The translation to the application request
in `harness.rs::application_request` carries every declared field across,
including the ones a given implementation never reads.

## The corpus

`tests/fixtures/admission-corpus.jsonl` is the source of truth: 80 self-contained
records, 60 adverse and 20 benign, versioned by
`schema: world-kernel-admission-case/v1`. The parser in
`tests/support/admission_case.rs` rejects a missing key, an unknown key, an
unknown enum value, a duplicate id and an unsafe candidate path, and
`tests/corpus_contract.rs` checks the counts, the family table, the prelude
admissibility and the digests.

Convention: every enum value in a corpus record is `snake_case`, except
`expected.code`, which mirrors the Kernel's own `SCREAMING_SNAKE_CASE`
serialization so a code is the same string in the corpus, in a receipt and in a
report.

A case is adverse when it injects a fault or when the correct answer is not a
clean commit. 60 adverse cases are 45 that must be rejected, 9 that are
unsupported, and 6 commit-interruption cases whose expected result is a commit
after a clean retry from an injected failure.

## How a case is run

1. write `current.candidateContents` and the assurance contract into a temp
   workspace;
2. open the world, plus a sibling world with the same name plus `-sibling` so a
   wrong world is a genuine scope test;
3. admit the `prelude` submissions in order, each with its own grants;
4. run the assurance step declared by `assurance.mode`;
5. replace the candidate with `current.candidateContentsAfterAssurance` when it
   is declared, which is the substitution the exactness family tests;
6. admit the measured proposal, injecting `fault` when one is declared.

A prelude submission is world-state preparation, not a measured decision, so it
is admitted with the assurance already established for that prior commit. Only
the measured proposal goes through the case's declared assurance step.

## How a system learns a case is unsupported

Nine cases declare a capability that v0 does not implement, in the producer's own
declared view protocol and operation class:

| Declaration | Meaning | Cases |
|---|---|---|
| `coverage.profile = closed-v1-negative-queries` | a `none exist` dependency (M4) | 4 |
| `coverage.profile = closed-v1-graph-traversal` | cycle or traversal budget | 3 |
| `intent.kind = dispatchExternalEffect` | external effect dispatch (M3) | 2 |

A system returns `unsupported` when a case falls outside its declared support
surface. The runners read the case, never `expected`. The declared surface is
printed in `results.json` and in `RESULTS.md` so an over-broad claim is visible
rather than hidden inside a total, and a `committed` expectation on an
unsupported declaration is rejected by the corpus validator.

## Fault injection

`fault` names a durable write class. The harness creates a temporary SQLite
trigger on the relevant table of the system's own storage, runs the measured
attempt, requires it to fail, requires the world revision not to have moved,
drops the trigger and retries. For `lost_response_after_commit` the attempt
commits, the instance is dropped without reading its outcome, a new instance is
opened over the same storage and the identical proposal is repeated.

## Ablations

Three test-only input ablations weaken the proposal before system C admits it.
No production check receives a disable flag; the ablated system receives a
different, weaker proposal.

| Ablation | What it removes | Adverse cases it wrongly admits |
|---|---|---|
| `forge_assessment` | the assessment binding, re-bound to the bytes on disk now | 4 in `candidate_exactness` |
| `drop_changed_reads` | every declared read that no longer matches | 7 in `world_object_concurrency` and `reassessment_semantics` |
| `complete_truncated_coverage` | the declared limits of the producer's view | 4 in `coverage_limits` |

## Commands

```bash
cargo run --example admission_benchmark                      # run and record
cargo run --example admission_benchmark -- --render-only    # RESULTS.md from results.json
cargo test --test comparative_admission                      # the acceptance criteria
cargo test --test corpus_contract                            # the corpus contract
```

Line counts reported per system are reproduced with the command recorded in
`results.json` under `aggregate.lineCountingCommand`:

```bash
wc -l tests/support/baseline_a.rs tests/support/baseline_b.rs src/kernel.rs src/model.rs tests/support/system_c.rs
```

`tests/support/harness.rs` and `tests/support/admission_case.rs` are shared
fixture cost and are excluded from every per-system figure. `src/adapters/uni.rs`
is the assurance seam shared by B and C and is excluded from both, so the
per-system numbers understate the real cost of C rather than flattering it.

## Recorded environment and limits

`results.json` records the operating system, architecture and whether debug
assertions were on, and the run exited non-zero if any acceptance criterion
failed. Durations depend on that environment, are recorded per case, and are
descriptive only. Every other field, including the continuation decision, is a
deterministic function of the corpus: `the_run_is_reproducible_apart_from_the_recorded_durations`
and `the_recorded_result_file_matches_a_fresh_run_apart_from_timings` in
`tests/comparative_admission.rs` prove it.

## Interpretation limits

- The corpus is deterministic and adversarial by construction. It measures
  whether a system catches the conditions it was built to catch. It is not a
  measure of human time, of integration time, or of how often these conditions
  occur in real work.
- Systems that declare a case unsupported receive no credit for it, and a
  system that declared everything unsupported would pass the correctness
  criteria while reporting 80 unsupported cases. The printed support surface and
  the per-family counts exist so that outcome is visible.
- A, B and C reaching the same decision says the envelope bought no admission
  correctness on these 80 conditions. It does not say the receipt, replay and
  handoff are worthless, because this corpus never consumes a receipt.
- No percentage of human-time improvement may be claimed from this corpus. That
  metric needs repeated human tasks and belongs to a later study.
