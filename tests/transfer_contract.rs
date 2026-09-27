//! The transfer contract, bound to the code it describes.
//!
//! `docs/TRANSFER-CONTRACT.md` is what a consumer reads instead of the planner, because
//! `docs/ADR-005-reduce-to-the-representation.md` moved the planner out of the shipped surface. This
//! file is the reason a status cannot be added, dropped or renamed on one side without the other
//! disagreeing.

/// The contract document and the moved implementation must name the same variants.
///
/// The vocabulary is the only thing a consumer has to depend on. A variant that exists in one place and
/// not the other is a lie to whoever reads the contract, and this test is what catches it.
#[test]
fn the_transfer_contract_and_the_moved_implementation_agree() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let contract = std::fs::read_to_string(root.join("docs/TRANSFER-CONTRACT.md"))
        .expect("the contract is checked in");
    let planner = std::fs::read_to_string(root.join("tests/support/transfer_core.rs"))
        .expect("the moved implementation is checked in");
    let representation = std::fs::read_to_string(root.join("src/experience.rs"))
        .expect("the representation is checked in");

    // The variant identifiers declared by `enum_name`, from its source.
    let declared = |source: &str, enum_name: &str| -> std::collections::BTreeSet<String> {
        let head = format!("pub enum {enum_name} {{");
        let start = source
            .find(&head)
            .unwrap_or_else(|| panic!("{enum_name} is declared"));
        let body = &source[start + head.len()..];
        let body = &body[..body
            .find("\n}")
            .expect("the enum body closes at column zero")];
        let names: std::collections::BTreeSet<String> = body
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#') && !line.starts_with("//"))
            .map(|line| {
                line.split(|c: char| c == '(' || c == ',' || c.is_whitespace())
                    .next()
                    .unwrap_or_default()
                    .to_owned()
            })
            .filter(|name| !name.is_empty())
            .collect();
        assert!(!names.is_empty(), "{enum_name} has variants to compare");
        names
    };

    // The first backticked word of every table row under the `### enum_name` heading.
    let documented = |contract: &str, enum_name: &str| -> std::collections::BTreeSet<String> {
        let heading = format!("### {enum_name}");
        let mut in_section = false;
        let mut names = std::collections::BTreeSet::new();
        for line in contract.lines() {
            if line.starts_with('#') {
                in_section = line.trim() == heading;
                continue;
            }
            if !in_section || !line.starts_with('|') {
                continue;
            }
            let Some(at) = line.find('`') else { continue };
            let rest = &line[at + 1..];
            let Some(end) = rest.find('`') else { continue };
            names.insert(rest[..end].to_owned());
        }
        assert!(!names.is_empty(), "{enum_name} has a table to compare");
        names
    };

    for (enum_name, source) in [
        ("TransferStatus", &planner),
        ("UnknownReason", &planner),
        ("InstantiationError", &planner),
        ("ApplicabilityCoverage", &representation),
    ] {
        let declared = declared(source, enum_name);
        let documented = documented(&contract, enum_name);
        let missing_from_docs: Vec<&String> = declared.difference(&documented).collect();
        let missing_from_source: Vec<&String> = documented.difference(&declared).collect();
        assert!(
            missing_from_docs.is_empty(),
            "{enum_name} declares variants the contract does not name: {missing_from_docs:?}"
        );
        assert!(
            missing_from_source.is_empty(),
            "{enum_name} is named in the contract but not declared: {missing_from_source:?}"
        );
    }
}
