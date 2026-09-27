//! M4 tranche 1: two correct branches, one incorrect composition.
//!
//! The stories and the way each is decided are fixed in `docs/M4-PROTOCOL.md` before these results were
//! retained. The expectations live here, in the harness. No code under test reads an expected value, a
//! case id, or a property named `expected`.

mod support;

use support::branch_fixture as fixture;
use world_kernel::branch::{Branch, Proposal, Requested, commit, prepare};
use world_kernel::impact::Snapshot;

fn proposal(source: &str, target: &Snapshot, units: &[&str]) -> Proposal {
    Proposal {
        source: source.to_owned(),
        target: "main".to_owned(),
        base: fixture::s0(),
        target_revision: target.revision,
        selection: Requested {
            units: units.iter().map(|unit| (*unit).to_owned()).collect(),
        },
    }
}

/// The primary red test from the protocol.
///
/// Alpha and Beta are each correct from their common base. Their combination is wrong. The target must
/// not move, and the reason must name the rule and the versions actually examined.
#[test]
fn a_combination_of_two_valid_branches_is_blocked_and_the_target_does_not_move() {
    let base = fixture::s0();
    let alpha = fixture::branch_with("alpha", &base, fixture::LOAD_A, 55);
    let beta = fixture::branch_with("beta", &base, fixture::LOAD_B, 55);
    let branches = fixture::store(vec![alpha.clone(), beta.clone()]);

    // Each branch is individually sound. Checked before the combination, not assumed.
    for branch in [&alpha, &beta] {
        let view = fixture::values_of(&branch.head);
        let oracle = fixture::oracle(&view, &view, &fixture::values_of(&base), true, true);
        assert!(
            oracle.satisfies,
            "{} alone must satisfy the rule",
            branch.id
        );
    }

    // The target adopts Alpha by the authorised path. It is then 95.
    let mut target = base.clone();
    let adopted_alpha = commit(
        &prepare(
            &branches,
            &proposal("alpha", &target, &[fixture::LOAD_A]),
            &target,
        )
        .expect("Alpha prepares"),
        &mut target,
        &fixture::rules(),
    )
    .expect("Alpha is admissible on its own");
    assert_eq!(fixture::values_of(&target), (100, 55, 40));

    // Now propose Beta's change, computed from its own base S0, into that target.
    let blocked = prepare(
        &branches,
        &proposal("beta", &target, &[fixture::LOAD_B]),
        &target,
    )
    .expect("Beta prepares; the problem is the composition, not the preparation");

    // The oracle says what the composition is, computed by a separate program.
    let expected = fixture::oracle(
        &fixture::values_of(&target),
        &fixture::values_of(&beta.head),
        &fixture::values_of(&base),
        false,
        true,
    );
    assert!(
        !expected.satisfies,
        "the oracle must also find the composition wrong, or the test proves nothing"
    );

    // The candidate really is 110. A gate that blocked a wrong candidate would prove nothing either.
    assert_eq!(fixture::values_of(&blocked.candidate), (100, 55, 55));

    let before = target.clone();
    let refusal = commit(&blocked, &mut target, &fixture::rules())
        .expect_err("a composition of 110 must be refused");
    assert_eq!(
        target, before,
        "the target must not move when a composition is refused"
    );
    assert_eq!(adopted_alpha.applied, vec![fixture::LOAD_A.to_owned()]);

    // The reason names the rule and the versions examined.
    let world_kernel::branch::CommitError::Blocked { violations } = refusal else {
        panic!("the refusal must be a constraint, not something else")
    };
    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0].rule, "load_sum_within_capacity");
    assert!(
        violations[0].detail.contains("110") && violations[0].detail.contains("100"),
        "the reason must carry the numbers that failed: {}",
        violations[0].detail
    );
    assert!(
        violations[0]
            .versions
            .iter()
            .any(|(id, _)| id == fixture::LOAD_B),
        "the reason must name the versions it looked at"
    );

    // Both histories are still readable after the refusal.
    assert!(branches.contains_key("alpha") && branches.contains_key("beta"));
    assert_eq!(fixture::values_of(&branches["beta"].head), (100, 40, 55));
}

/// The protocol refuses to accept a test that blocks everything. This is the other half.
#[test]
fn a_revised_branch_composing_exactly_to_the_limit_is_admissible() {
    let base = fixture::s0();
    let alpha = fixture::branch_with("alpha", &base, fixture::LOAD_A, 55);
    let beta_prime = fixture::branch_with("beta", &base, fixture::LOAD_B, 45);
    let branches = fixture::store(vec![alpha.clone(), beta_prime.clone()]);

    let mut target = base.clone();
    commit(
        &prepare(
            &branches,
            &proposal("alpha", &target, &[fixture::LOAD_A]),
            &target,
        )
        .expect("Alpha prepares"),
        &mut target,
        &fixture::rules(),
    )
    .expect("Alpha is admissible");

    let expected = fixture::oracle(
        &fixture::values_of(&target),
        &fixture::values_of(&beta_prime.head),
        &fixture::values_of(&base),
        false,
        true,
    );
    assert!(expected.satisfies, "45 composes to exactly 100");
    assert_eq!((expected.load_a, expected.load_b), (55, 45));

    let prepared = prepare(
        &branches,
        &proposal("beta", &target, &[fixture::LOAD_B]),
        &target,
    )
    .expect("Beta' prepares");
    let receipt = commit(&prepared, &mut target, &fixture::rules())
        .expect("a composition that exactly meets the limit is admissible");
    assert_eq!(fixture::values_of(&target), (100, 55, 45));
    assert_eq!(receipt.applied, vec![fixture::LOAD_B.to_owned()]);
}

/// M4-01: forking and changing an inherited object leaves the source and its sibling alone.
#[test]
fn forking_and_changing_leaves_the_source_and_its_sibling_untouched() {
    let base = fixture::s0();
    let alpha = fixture::branch_with("alpha", &base, fixture::LOAD_A, 55);
    let beta = fixture::branch_with("beta", &base, fixture::LOAD_B, 55);
    let before_alpha = fixture::values_of(&alpha.head);
    let before_beta = fixture::values_of(&beta.head);
    let before_base = fixture::values_of(&base);

    // A third branch, forked from Beta's head, changes what Alpha did not.
    let mut gamma = Branch::fork("gamma", &beta.head);
    gamma.head = gamma.head.clone().with_node(
        fixture::LOAD_A,
        world_kernel::impact::NodeNature::Observed,
        world_kernel::impact::NodeVersion::new(
            2,
            "planning/2",
            serde_json::json!({"value": 10}),
            "planning",
        ),
    );

    assert_eq!(
        fixture::values_of(&alpha.head),
        before_alpha,
        "alpha is untouched"
    );
    assert_eq!(
        fixture::values_of(&beta.head),
        before_beta,
        "beta is untouched"
    );
    assert_eq!(
        fixture::values_of(&base),
        before_base,
        "the base is untouched"
    );
    assert_eq!(fixture::values_of(&gamma.head), (100, 10, 55));
    assert_eq!(
        gamma.base.revision, beta.head.revision,
        "gamma is anchored on beta's head"
    );
}

/// M4-15: a partial selection is not a merge. The receipt names what was taken, and the excluded part is
/// still not in the target.
#[test]
fn a_partial_adoption_does_not_make_the_excluded_part_look_merged() {
    let base = fixture::s0();
    // One branch changes both loads, so the selection has something to exclude.
    let both = {
        let mut branch = fixture::branch_with("both", &base, fixture::LOAD_A, 50);
        branch.head = branch.head.clone().with_node(
            fixture::LOAD_B,
            world_kernel::impact::NodeNature::Observed,
            world_kernel::impact::NodeVersion::new(
                2,
                "planning/2",
                serde_json::json!({"value": 45}),
                "planning",
            ),
        );
        branch
    };
    let branches = fixture::store(vec![both.clone()]);
    let mut target = base.clone();

    let prepared = prepare(
        &branches,
        &proposal("both", &target, &[fixture::LOAD_A]),
        &target,
    )
    .expect("a selection of one unit prepares");
    let receipt = commit(&prepared, &mut target, &fixture::rules()).expect("and is admissible");

    assert_eq!(receipt.applied, vec![fixture::LOAD_A.to_owned()]);
    assert_eq!(
        fixture::values_of(&target),
        (100, 50, 40),
        "load_b was not selected"
    );
    assert_eq!(
        fixture::values_of(&branches["both"].head),
        (100, 50, 45),
        "the source is unchanged and still holds the excluded part"
    );
}

/// M4-02: a target that moved since the proposal was prepared is a stale proposal, not a silent rebase.
#[test]
fn a_target_that_moved_makes_the_proposal_stale_rather_than_rebased() {
    let base = fixture::s0();
    let alpha = fixture::branch_with("alpha", &base, fixture::LOAD_A, 55);
    let branches = fixture::store(vec![alpha]);

    let mut target = base.clone();
    let stale = proposal("alpha", &target, &[fixture::LOAD_A]);

    // The target advances for an unrelated reason.
    target.revision += 1;

    let refusal = prepare(&branches, &stale, &target)
        .expect_err("a proposal made against an older target is refused");
    assert!(
        refusal.to_string().contains("prepared at target revision"),
        "the reason must say the proposal is stale: {refusal}"
    );
    assert_eq!(fixture::values_of(&target), (100, 40, 40), "nothing moved");
}

/// An unknown base is diagnosed, never guessed. A guessed base yields a plausible candidate from the
/// wrong history, which is worse than a refusal.
#[test]
fn an_unknown_base_is_refused_rather_than_guessed() {
    let base = fixture::s0();
    let alpha = fixture::branch_with("alpha", &base, fixture::LOAD_A, 55);
    let branches = fixture::store(vec![alpha]);
    let target = base.clone();

    let mut wrong = proposal("alpha", &target, &[fixture::LOAD_A]);
    wrong.base.revision = 7;
    let refusal = prepare(&branches, &wrong, &target).expect_err("a mismatched base is refused");
    assert!(refusal.to_string().contains("forked from"), "{refusal}");

    // A unit the branch never touched is not silently imported under a false provenance.
    let untouched = prepare(
        &branches,
        &proposal("alpha", &target, &[fixture::LOAD_B]),
        &target,
    )
    .expect_err("a unit the source did not change is refused");
    assert!(
        untouched.to_string().contains("not a change"),
        "{untouched}"
    );
}

/// The gate answers the constraint question and only that one. There is no boolean that also claims
/// assurance, authority and currency were fine.
#[test]
fn the_gate_answers_only_the_constraint_question() {
    let base = fixture::s0();
    let heavy = fixture::branch_with_both("heavy", &base, 55, 55);
    let branches = fixture::store(vec![heavy]);
    let target = base.clone();

    let prepared = prepare(
        &branches,
        &proposal("heavy", &target, &[fixture::LOAD_A, fixture::LOAD_B]),
        &target,
    )
    .expect("both units prepare");
    assert_eq!(fixture::values_of(&prepared.candidate), (100, 55, 55));
    let gate = world_kernel::branch::check(&prepared, &fixture::rules());

    assert!(gate.is_blocked(), "55 + 55 = 110 exceeds 100");
    assert!(
        gate.obligations.is_empty(),
        "no obligation was owed; the rule simply failed"
    );

    // The verdict is relative to the target's own rules, not to a notion of safety the Kernel holds.
    // With no rules supplied, the same candidate is not blocked, and the Kernel says nothing about it.
    let unjudged = world_kernel::branch::check(&prepared, &[]);
    assert!(
        !unjudged.is_blocked(),
        "a Kernel with no rules must not invent a verdict"
    );
    assert!(unjudged.constraint_violations.is_empty());
}
