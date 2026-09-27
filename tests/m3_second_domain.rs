//! M3-24: a second domain through an adapter after the core freezes.
//!
//! This is also the H3-Transfert measurement. The hypothesis is that the same core applies to a
//! second domain without domain rules multiplying inside it, and "without multiplying" has to mean
//! something countable. Two things are counted here: the core is scanned for the vocabulary of every
//! domain any fixture uses, and a pass over the second domain is checked against a full recompute of
//! the second domain rather than against an answer anyone wrote down.
//!
//! The freeze itself is measured outside this file, by `src/impact` being byte-identical across the
//! change that introduced `tests/support/second_domain.rs`. A test that pinned that digest would
//! fail on the next legitimate change to the core, which would teach whoever made it to ignore the
//! test rather than to read it.

mod support;

use std::fs;

use support::second_domain::{
    self, ASSIGNED, DEFAULT_MESSAGES, DELIVERY, MSG_TWO, Message, NOTICE, PENDING,
};
use world_kernel::impact::{Disposition, Snapshot};

fn base() -> Snapshot {
    second_domain::messages_world(1, "eu", DEFAULT_MESSAGES)
}

fn routes(first: &'static str, second: &'static str, third: &'static str) -> [Message; 3] {
    [
        Message::new(first, "first"),
        Message::new(second, "second"),
        Message::new(third, "third"),
    ]
}

#[test]
fn the_second_domain_reaches_the_same_answer_as_a_full_recompute_of_itself() {
    let base = base();
    let recorded = second_domain::records_at(&base);

    let cases = [
        (
            "a message claims another region",
            second_domain::messages_world(2, "eu", routes("eu", "eu", "eu")),
        ),
        (
            "the table's primary moves",
            second_domain::messages_world(3, "us", DEFAULT_MESSAGES),
        ),
        (
            "a preview nobody reads changes and the message gains a version",
            second_domain::messages_world(
                4,
                "eu",
                [
                    Message::new("eu", "first"),
                    Message::new("us", "second").at_version(2),
                    Message::new("eu", "third"),
                ],
            ),
        ),
        (
            "a route and the primary move together",
            second_domain::messages_world(5, "us", routes("us", "eu", "us")),
        ),
    ];

    for (label, target) in cases {
        let (revised, observable) = second_domain::pass(&base, &target, &recorded);
        assert_eq!(
            revised.findings.len(),
            4,
            "{label}: every target in the second domain has a finding"
        );
        assert_eq!(
            revised.base_revision, 1,
            "{label}: the plan is anchored at the revision it was planned against"
        );
        // The oracle agreement is asserted inside `pass`, so a disagreement between an incremental
        // answer and a full recompute of the same snapshot fails here rather than silently.
        assert!(
            !observable.outputs.is_empty(),
            "{label}: the second domain produced something to compare"
        );
    }
}

#[test]
fn a_route_change_reaches_the_chain_that_consumes_a_route_and_stops_where_the_value_stops() {
    let base = base();
    let recorded = second_domain::records_at(&base);
    let target = second_domain::messages_world(2, "eu", routes("eu", "eu", "eu"));

    let (revised, _) = second_domain::pass(&base, &target, &recorded);

    assert_eq!(
        revised.evaluations, 2,
        "assigning and delivering consume the region; the set declaration does not, and the notice \
         does not, because the number delivered did not move"
    );
    assert_eq!(
        revised.findings[PENDING].disposition,
        Disposition::UnchangedInScope,
        "the query resolved to the same members at the same versions, so the set is not re-declared"
    );
    assert_eq!(
        revised.findings[ASSIGNED].disposition,
        Disposition::RecomputedChanged,
        "assigning read the region that moved, and its own output moved with it"
    );
    assert_eq!(
        revised.findings[DELIVERY].disposition,
        Disposition::RecomputedSame,
        "delivering read what assigning produced and produced the same count"
    );
    assert_eq!(
        revised.findings[NOTICE].disposition,
        Disposition::ReusedAfterCheck,
        "the count delivered is three either way, so noticing is reached, checked against what it \
         consumed, and reused rather than re-run"
    );
}

#[test]
fn a_change_no_consumer_reads_re_evaluates_only_the_node_that_declared_the_set() {
    let base = base();
    let recorded = second_domain::records_at(&base);

    let untouched = second_domain::messages_world(2, "eu", DEFAULT_MESSAGES);
    let (nothing, nothing_seen) = second_domain::pass(&base, &untouched, &recorded);

    let preview_moved = second_domain::messages_world(
        3,
        "eu",
        [
            Message::new("eu", "first"),
            Message::new("us", "second").at_version(2),
            Message::new("eu", "third"),
        ],
    );
    let (revised, seen) = second_domain::pass(&base, &preview_moved, &recorded);

    assert_eq!(
        nothing.evaluations, 0,
        "a snapshot whose content did not move costs nothing"
    );
    assert_eq!(
        revised.evaluations, 1,
        "the message's content moved, so the set that named it is re-resolved; nothing reads the \
         field that changed, so no consumer follows"
    );
    assert_eq!(
        revised.findings[PENDING].disposition,
        Disposition::RecomputedSame,
        "the declaration is re-checked and finds the same members"
    );
    for id in [ASSIGNED, DELIVERY, NOTICE] {
        assert_eq!(
            revised.findings[id].disposition,
            Disposition::UnchangedInScope,
            "{id} never reads {MSG_TWO}'s preview"
        );
    }
    assert_eq!(
        seen, nothing_seen,
        "a field no evaluator reads changes nothing anyone can observe"
    );
}

#[test]
fn the_core_carries_no_vocabulary_from_any_domain() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));

    // One token list per fixture that exists in this repository. A token that reaches `src/impact`
    // means a domain rule travelled into the core, which is the failure H3-Transfert is about.
    let domain_tokens = [
        // the original arithmetic fixture
        "world:impact-demo",
        "world:fanout",
        "cost_a",
        "cost_b",
        // M3-11, the conditional read
        "world:conditional",
        "source_a",
        "source_b",
        "bystander",
        "routed_consumer",
        // M3-15, expiry on explicit time
        "world:timed",
        "freshness",
        "archived",
        // M3-18, a contradiction in a collection
        "world:claims",
        "claimant",
        "every_claim",
        "claim_alice",
        // M3-24, the second domain
        "world:messages",
        "routing_table",
        "message_one",
        "awaiting",
        "preview",
    ];

    for file in ["model.rs", "engine.rs", "mod.rs"] {
        let text = fs::read_to_string(root.join("src/impact").join(file))
            .expect("the core source is readable");
        let lower = text.to_lowercase();
        for token in domain_tokens {
            assert!(
                !lower.contains(&token.to_lowercase()),
                "{file} names `{token}`: a domain rule has reached the core"
            );
        }
    }
}

#[test]
fn the_second_domain_is_written_against_the_same_public_surface_as_the_first() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let adapter =
        fs::read_to_string(root.join("tests/support/second_domain.rs")).expect("it is checked in");

    assert!(
        adapter.contains("use world_kernel::impact::{"),
        "the second domain imports the core rather than a wrapper of its own"
    );
    assert!(
        !adapter
            .replace("world_kernel::impact", "")
            .contains("world_kernel::"),
        "every mention of the crate is the impact module, so there is no second door into it"
    );
    for entry_point in [
        "Snapshot",
        "Evaluator",
        "ReadJournal",
        "Query",
        "revise",
        "full_recompute",
        "TrustConfiguration",
        "Limits",
    ] {
        assert!(
            adapter.contains(entry_point),
            "the second domain reaches the core through {entry_point}, the same door the first ones use"
        );
    }
}
