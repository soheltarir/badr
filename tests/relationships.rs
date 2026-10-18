use serde_json::Value;
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};
use tempfile::TempDir;
fn git(root: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap().trim().into()
}
fn run(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_badr"))
        .arg("--repo")
        .arg(root)
        .arg("--json")
        .args(args)
        .output()
        .unwrap()
}
fn check(out: Output, exit: i32) -> Value {
    assert_eq!(
        out.status.code(),
        Some(exit),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    assert!(out.stderr.is_empty());
    serde_json::from_slice(&out.stdout).unwrap()
}
fn repo() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-b", "trunk"]);
    git(
        dir.path(),
        &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "commit",
            "--allow-empty",
            "-m",
            "baseline",
        ],
    );
    check(run(dir.path(), &["init", "--baseline", "trunk"]), 0);
    dir
}
const ADD: &[&str] = &[
    "domain",
    "add",
    "ordering",
    "--prefix",
    "ORD",
    "--confirm",
    "--actor",
    "Human",
    "--rationale",
    "Establish ordering vocabulary",
];
const ADR: &str = ".badr/adrs/0001-storage-choice.md";

fn relate(root: &Path, operation: &str, source: &str, target: &str) -> Output {
    run(
        root,
        &[
            operation,
            source,
            target,
            "--confirm",
            "--actor",
            "Human",
            "--rationale",
            "Document connection",
        ],
    )
}

#[test]
fn directed_adr_links_preserve_content_and_can_be_removed() {
    let dir = repo();
    let root = dir.path();
    check(run(root, &["new", "adr", "storage-choice"]), 0);
    check(run(root, &["new", "adr", "other-choice"]), 0);
    let before = check(run(root, &["show", "0001"]), 0);
    let ledger = fs::read(root.join(".badr/evidence.jsonl")).unwrap();
    let linked = check(relate(root, "link", "0001", "0002"), 0);
    assert_eq!(linked["affected_ids"], serde_json::json!(["0001"]));
    assert_eq!(linked["pending_commit_durability"], true);
    let after = check(run(root, &["show", "0001"]), 0);
    assert_eq!(
        after["records"][0]["related_adrs"],
        serde_json::json!(["0002"])
    );
    assert_eq!(
        after["records"][0]["decision_body"],
        before["records"][0]["decision_body"]
    );
    assert_eq!(after["records"][0]["status"], "proposed");
    assert_eq!(
        check(run(root, &["show", "0002"]), 0)["records"][0]["related_adrs"],
        serde_json::json!([])
    );
    assert!(
        fs::read_to_string(root.join(ADR))
            .unwrap()
            .contains("- '0002'")
    );
    assert!(
        fs::read(root.join(".badr/evidence.jsonl"))
            .unwrap()
            .starts_with(&ledger)
    );
    check(relate(root, "unlink", "0001", "0002"), 0);
    assert_eq!(
        check(run(root, &["show", "0001"]), 0)["records"][0]["related_adrs"],
        serde_json::json!([])
    );
    check(run(root, &["validate"]), 0);
}

#[test]
fn cross_type_links_and_cycles_traverse_only_explicit_outgoing_edges() {
    let dir = repo();
    let root = dir.path();
    check(run(root, ADD), 0);
    check(
        run(
            root,
            &[
                "new",
                "brr",
                "cancellation",
                "--domain",
                "ordering",
                "--owner",
                "Product",
            ],
        ),
        0,
    );
    for title in ["storage-choice", "other-choice", "unrelated"] {
        check(run(root, &["new", "adr", title]), 0);
    }
    check(relate(root, "link", "BRR-ORD-001", "0001"), 0);
    check(relate(root, "link", "0001", "0002"), 0);
    check(relate(root, "link", "0002", "BRR-ORD-001"), 0);
    let result = check(run(root, &["related", "0001"]), 0);
    assert_eq!(
        result["records"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["0001", "0002", "BRR-ORD-001"]
    );
    assert_eq!(check(run(root, &["related", "0001"]), 0), result);
    assert_eq!(
        check(run(root, &["related", "0003"]), 0)["records"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let human = Command::new(env!("CARGO_BIN_EXE_badr"))
        .arg("--repo")
        .arg(root)
        .args(["related", "0001"])
        .output()
        .unwrap();
    assert!(human.status.success());
    let human = String::from_utf8(human.stdout).unwrap();
    assert!(human.contains("0001 -> 0002"));
    assert!(human.contains("0002 -> BRR-ORD-001"));
    assert!(human.contains("BRR-ORD-001 -> 0001"));
    check(relate(root, "unlink", "0002", "BRR-ORD-001"), 0);
    assert_eq!(
        check(run(root, &["related", "0001"]), 0)["records"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    check(run(root, &["related", "9999"]), 2);
}

#[test]
fn absent_targets_are_unverified_missing_targets_are_errors_and_neither_writes() {
    let dir = repo();
    let root = dir.path();
    check(run(root, &["new", "adr", "storage-choice"]), 0);
    let original = fs::read(root.join(ADR)).unwrap();
    let ledger = fs::read(root.join(".badr/evidence.jsonl")).unwrap();
    let absent = check(relate(root, "link", "0001", "BRR-ORD-001"), 3);
    assert!(
        absent["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "unverified_reference")
    );
    check(relate(root, "link", "0001", "0002"), 2);
    assert_eq!(fs::read(root.join(ADR)).unwrap(), original);
    assert_eq!(fs::read(root.join(".badr/evidence.jsonl")).unwrap(), ledger);
    check(run(root, ADD), 0);
    check(relate(root, "link", "0001", "BRR-ORD-001"), 2);
    check(run(root, &["validate"]), 0);
}

#[test]
fn authorization_illegal_fields_aliases_and_unrelated_invalidity_are_refused() {
    let dir = repo();
    let root = dir.path();
    for title in ["storage-choice", "other-choice"] {
        check(run(root, &["new", "adr", title]), 0);
    }
    let original = fs::read(root.join(ADR)).unwrap();
    let ledger = fs::read(root.join(".badr/evidence.jsonl")).unwrap();
    for args in [
        vec!["link", "0001", "0002"],
        vec![
            "link",
            "0001",
            "0002",
            "--actor",
            "Human",
            "--rationale",
            "Connection",
        ],
        vec![
            "link",
            "0001",
            "0002",
            "--confirm",
            "--actor",
            " ",
            "--rationale",
            "Connection",
        ],
        vec![
            "link",
            "0001",
            "0002",
            "--confirm",
            "--actor",
            "Human",
            "--rationale",
            " ",
        ],
        vec![
            "link",
            "0001",
            "0002",
            "--supersedes",
            "--confirm",
            "--actor",
            "Human",
            "--rationale",
            "Connection",
        ],
        vec![
            "unlink",
            "0001",
            "0002",
            "--superseded-by",
            "--confirm",
            "--actor",
            "Human",
            "--rationale",
            "Connection",
        ],
    ] {
        check(run(root, &args), 2);
    }
    check(relate(root, "link", "0001", "00002"), 2);
    check(relate(root, "unlink", "0001", "0002"), 2);
    fs::write(
        root.join(".badr/adrs/0002-other-choice.md"),
        "invalid unrelated content",
    )
    .unwrap();
    check(relate(root, "link", "0001", "0002"), 2);
    assert_eq!(fs::read(root.join(ADR)).unwrap(), original);
    assert_eq!(fs::read(root.join(".badr/evidence.jsonl")).unwrap(), ledger);
}

#[test]
fn frozen_decisions_keep_approval_digests_and_append_only_notes() {
    let dir = repo();
    let root = dir.path();
    for title in ["storage-choice", "other-choice"] {
        check(run(root, &["new", "adr", title]), 0);
    }
    let text = fs::read_to_string(root.join(ADR))
        .unwrap()
        .replace("TODO:", "Reviewed:");
    fs::write(root.join(ADR), text).unwrap();
    check(
        run(
            root,
            &[
                "decide",
                "0001",
                "--confirm",
                "--reviewed",
                "--actor",
                "Human",
                "--rationale",
                "Reviewed decision",
            ],
        ),
        0,
    );
    let before = check(run(root, &["show", "0001"]), 0);
    git(root, &["add", ".badr"]);
    git(
        root,
        &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "commit",
            "-m",
            "Approved decision",
        ],
    );
    let ledger = fs::read(root.join(".badr/evidence.jsonl")).unwrap();
    check(relate(root, "link", "0001", "0002"), 0);
    let linked = fs::read_to_string(root.join(ADR)).unwrap();
    let notes = linked.split("## Lifecycle notes").nth(1).unwrap();
    check(relate(root, "link", "0001", "0002"), 2);
    check(relate(root, "unlink", "0001", "0002"), 0);
    let text = fs::read_to_string(root.join(ADR)).unwrap();
    assert!(
        text.split("## Lifecycle notes")
            .nth(1)
            .unwrap()
            .starts_with(notes)
    );
    assert!(
        fs::read(root.join(".badr/evidence.jsonl"))
            .unwrap()
            .starts_with(&ledger)
    );
    let after = check(run(root, &["show", "0001"]), 0);
    assert_eq!(
        before["records"][0]["decision_body"],
        after["records"][0]["decision_body"]
    );
    assert_eq!(after["records"][0]["status"], "approved");
    check(run(root, &["validate"]), 0);
}

#[test]
fn partial_link_publication_reports_paths_and_retains_repair_lock() {
    let dir = repo();
    let root = dir.path();
    for title in ["storage-choice", "other-choice"] {
        check(run(root, &["new", "adr", title]), 0);
    }
    let ledger = fs::read(root.join(".badr/evidence.jsonl")).unwrap();
    let result = check(
        Command::new(env!("CARGO_BIN_EXE_badr"))
            .arg("--repo")
            .arg(root)
            .arg("--json")
            .args([
                "link",
                "0001",
                "0002",
                "--confirm",
                "--actor",
                "Human",
                "--rationale",
                "Connection",
            ])
            .env("BADR_TEST_FAIL_AT", "after_write_0")
            .output()
            .unwrap(),
        4,
    );
    assert_eq!(result["affected_ids"], serde_json::json!(["0001"]));
    assert_eq!(
        result["partial_write"]["completed"],
        serde_json::json!([ADR])
    );
    assert_eq!(
        result["partial_write"]["not_completed"],
        serde_json::json!([".badr/evidence.jsonl"])
    );
    assert!(root.join(".badr.lock").exists());
    assert_eq!(fs::read(root.join(".badr/evidence.jsonl")).unwrap(), ledger);
    check(relate(root, "unlink", "0001", "0002"), 4);
}

#[test]
fn frozen_brr_links_preserve_rule_and_cannot_link_business_rules() {
    let dir = repo();
    let root = dir.path();
    check(run(root, ADD), 0);
    check(
        run(
            root,
            &[
                "new",
                "brr",
                "cancellation",
                "--domain",
                "ordering",
                "--owner",
                "Product",
            ],
        ),
        0,
    );
    check(run(root, &["new", "adr", "storage-choice"]), 0);
    let path = root.join(".badr/brrs/domains/ordering/rules/BRR-ORD-001-cancellation.feature");
    let text = fs::read_to_string(&path)
        .unwrap()
        .replace("TODO", "Reviewed");
    fs::write(&path, text).unwrap();
    check(
        run(
            root,
            &[
                "propose",
                "BRR-ORD-001",
                "--confirm",
                "--actor",
                "Human",
                "--rationale",
                "Ready",
            ],
        ),
        0,
    );
    check(
        run(
            root,
            &[
                "decide",
                "BRR-ORD-001",
                "--confirm",
                "--reviewed",
                "--actor",
                "Human",
                "--rationale",
                "Reviewed rule",
            ],
        ),
        0,
    );
    let before = check(run(root, &["show", "BRR-ORD-001"]), 0);
    check(relate(root, "link", "BRR-ORD-001", "0001"), 0);
    check(relate(root, "unlink", "BRR-ORD-001", "0001"), 0);
    let after = check(run(root, &["show", "BRR-ORD-001"]), 0);
    assert_eq!(before["records"][0], after["records"][0]);
    check(relate(root, "link", "BRR-ORD-001", "BRR-ORD-001"), 2);
    for operation in ["activate", "deprecate"] {
        check(
            run(
                root,
                &[
                    operation,
                    "BRR-ORD-001",
                    "--confirm",
                    "--actor",
                    "Human",
                    "--rationale",
                    "Apply reviewed lifecycle choice",
                ],
            ),
            0,
        );
        let before = check(run(root, &["show", "BRR-ORD-001"]), 0);
        check(relate(root, "link", "BRR-ORD-001", "0001"), 0);
        check(relate(root, "unlink", "BRR-ORD-001", "0001"), 0);
        let after = check(run(root, &["show", "BRR-ORD-001"]), 0);
        assert_eq!(before["records"][0], after["records"][0]);
    }
    check(run(root, &["validate"]), 0);
}
