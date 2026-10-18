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

#[test]
fn supplied_proposal_edit_preserves_exact_bytes_and_evidence() {
    let dir = repo();
    let root = dir.path();
    check(run(root, &["new", "adr", "storage-choice"]), 0);
    let before = fs::read(root.join(".badr/evidence.jsonl")).unwrap();
    let text = fs::read_to_string(root.join(ADR)).unwrap() + "\nA caller supplied clarification.\n";
    let input = dir.path().join("input.md");
    fs::write(&input, &text).unwrap();
    let result = check(
        run(
            root,
            &["edit", "0001", "--content", input.to_str().unwrap()],
        ),
        0,
    );
    assert_eq!(result["result"], "edited");
    assert_eq!(result["affected_ids"], serde_json::json!(["0001"]));
    assert_eq!(
        result["partial_write"]["completed"],
        serde_json::json!([ADR])
    );
    assert_eq!(result["pending_commit_durability"], true);
    assert_eq!(fs::read_to_string(root.join(ADR)).unwrap(), text);
    assert_eq!(fs::read(root.join(".badr/evidence.jsonl")).unwrap(), before);
    check(run(root, &["validate"]), 0);
}

fn editor(script: &str) -> (TempDir, String) {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("editor");
    fs::write(&path, format!("#!/bin/sh\n{script}\n")).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    let name = path.to_str().unwrap().to_owned();
    (dir, name)
}

#[test]
fn explicit_editor_changes_only_scratch_then_publishes_valid_content() {
    let dir = repo();
    let root = dir.path();
    check(run(root, &["new", "adr", "storage-choice"]), 0);
    let (_editor_dir, executable) = editor(
        "printf '\\nEditor clarification.\\n' >> \"$1\"\nprintf 'editor output must not corrupt JSON'\n",
    );
    let result = check(run(root, &["edit", "0001", "--editor", &executable]), 0);
    assert_eq!(result["result"], "edited");
    assert!(
        fs::read_to_string(root.join(ADR))
            .unwrap()
            .ends_with("Editor clarification.\n")
    );
    check(run(root, &["validate"]), 0);
}

fn edit_content(root: &Path, id: &str, text: &str, exit: i32) -> Value {
    let input = tempfile::NamedTempFile::new().unwrap();
    fs::write(input.path(), text).unwrap();
    check(
        run(
            root,
            &["edit", id, "--content", input.path().to_str().unwrap()],
        ),
        exit,
    )
}

#[test]
fn supplied_brr_edits_and_direct_pending_edits_are_valid() {
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
    let path = root.join(".badr/brrs/domains/ordering/rules/BRR-ORD-001-cancellation.feature");
    let text = fs::read_to_string(&path)
        .unwrap()
        .replace("Product", "Engineering")
        + "\n# Clarified rule\n";
    edit_content(root, "BRR-ORD-001", &text, 0);
    assert_eq!(fs::read_to_string(&path).unwrap(), text);
    fs::write(&path, format!("{text}\n# External draft change\n")).unwrap();
    check(run(root, &["validate"]), 0);
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
    let proposed = fs::read_to_string(&path).unwrap() + "\n# External proposal change\n";
    fs::write(&path, &proposed).unwrap();
    check(run(root, &["validate"]), 0);
    edit_content(
        root,
        "BRR-ORD-001",
        &(proposed + "\n# Supplied proposal change\n"),
        0,
    );
    check(run(root, &["new", "adr", "storage-choice"]), 0);
    let adr = fs::read_to_string(root.join(ADR)).unwrap() + "\nExternal reasoning edit.\n";
    fs::write(root.join(ADR), &adr).unwrap();
    check(run(root, &["validate"]), 0);
}

#[test]
fn editor_failure_cancellation_noop_and_invalid_result_preserve_managed_files() {
    let dir = repo();
    let root = dir.path();
    check(run(root, &["new", "adr", "storage-choice"]), 0);
    let original = fs::read(root.join(ADR)).unwrap();
    let ledger = fs::read(root.join(".badr/evidence.jsonl")).unwrap();
    for (script, exit) in [
        ("exit 0", 0),
        ("printf 'bad' > \"$1\"; exit 1", 4),
        ("printf 'cancelled change' > \"$1\"; exit 130", 2),
        ("printf 'bad' > \"$1\"", 2),
        ("rm \"$1\"; ln -s /dev/null \"$1\"", 4),
    ] {
        let (_dir, executable) = editor(script);
        let result = check(run(root, &["edit", "0001", "--editor", &executable]), exit);
        assert_eq!(result["partial_write"]["completed"], serde_json::json!([]));
        if exit == 0 {
            assert_eq!(result["result"], "unchanged");
            assert_eq!(result["affected_ids"], serde_json::json!([]));
            assert!(result["proposed_audit_commit_message"].is_null());
        }
        assert_eq!(fs::read(root.join(ADR)).unwrap(), original);
        assert_eq!(fs::read(root.join(".badr/evidence.jsonl")).unwrap(), ledger);
        assert!(!root.join(".badr.lock").exists());
    }
    edit_content(root, "0001", std::str::from_utf8(&original).unwrap(), 0);
}

#[test]
fn supplied_identity_status_date_relationship_and_notes_tampering_is_refused() {
    let dir = repo();
    let root = dir.path();
    check(run(root, &["new", "adr", "storage-choice"]), 0);
    let text = fs::read_to_string(root.join(ADR)).unwrap();
    for changed in [
        "malformed replacement".to_owned(),
        text.replace("0001", "0002"),
        text.replace("status: proposed", "status: approved"),
        text.replace("status: proposed", "status: rejected"),
        text.replace("date:", "related_adrs: [\"0002\"]\ndate:"),
        text.replace("date:", "supersedes: [\"0002\"]\ndate:"),
        text.replace("date:", "unknown: true\ndate:"),
        text.replace("date:", "id: \"0001\"\ndate:"),
        format!("{text}\n## Lifecycle notes\n### 2026-10-17\nUnauthorized note.\n"),
    ] {
        edit_content(root, "0001", &changed, 2);
        assert_eq!(fs::read_to_string(root.join(ADR)).unwrap(), text);
    }
    let date: String = serde_json::from_slice::<Value>(&run(root, &["show", "0001"]).stdout)
        .unwrap()["records"][0]["date"]
        .as_str()
        .unwrap()
        .into();
    edit_content(root, "0001", &text.replace(&date, "2000-01-01"), 2);
    edit_content(root, "9999", &text, 2);
    check(run(root, &["validate"]), 0);
}

#[test]
fn brr_identity_domain_status_and_applicability_are_preserved() {
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
    let path = root.join(".badr/brrs/domains/ordering/rules/BRR-ORD-001-cancellation.feature");
    let text = fs::read_to_string(&path).unwrap();
    for changed in [
        text.replace("BRR-ORD-001", "BRR-ORD-002"),
        text.replace("ordering", "billing"),
        text.replace("status: draft", "status: proposed"),
        text.replace("status: draft", "status: effective"),
        text.replace(
            "status: draft",
            "status: draft\neffective_from: \"2020-01-01T00:00:00Z\"",
        ),
        text.replace(
            "owners:",
            "related_architecture_decisions: [\"0001\"]\nowners:",
        ),
    ] {
        edit_content(root, "BRR-ORD-001", &changed, 2);
        assert_eq!(fs::read_to_string(&path).unwrap(), text);
    }
}

// Retained authorization fixtures exercise frozen states, including unimplemented rejection.
fn freeze_adr(root: &Path, rejected: bool) {
    let ledger_path = root.join(".badr/evidence.jsonl");
    let ledger = fs::read_to_string(&ledger_path).unwrap();
    let mut event: Value = serde_json::from_str(ledger.lines().last().unwrap()).unwrap();
    let status = if rejected { "rejected" } else { "approved" };
    event["operation"] = if rejected { "reject" } else { "decide" }.into();
    event["event_id"] = "authorization-fixture".into();
    event["actor"] = "Human".into();
    event["rationale"] = "Reviewed".into();
    event["confirmed"] = true.into();
    event["changes"][0]["from"] = "proposed".into();
    event["changes"][0]["to"] = status.into();
    if !rejected {
        event["reviewed"] = true.into();
        event["changes"][0]["approval_ref"] = "authorization-fixture".into();
    }
    fs::write(ledger_path, format!("{ledger}{event}\n")).unwrap();
    let text = fs::read_to_string(root.join(ADR))
        .unwrap()
        .replace("status: proposed", &format!("status: {status}"));
    fs::write(root.join(ADR), text).unwrap();
    check(run(root, &["validate"]), 0);
}

#[test]
fn frozen_bodies_cannot_change_or_reopen_but_descriptive_owners_can() {
    for rejected in [false, true] {
        let dir = repo();
        let root = dir.path();
        check(run(root, &["new", "adr", "storage-choice"]), 0);
        freeze_adr(root, rejected);
        let text = fs::read_to_string(root.join(ADR)).unwrap();
        edit_content(root, "0001", &(text.clone() + "\nChanged reasoning.\n"), 2);
        let status = if rejected { "rejected" } else { "approved" };
        edit_content(
            root,
            "0001",
            &text.replace(&format!("status: {status}"), "status: proposed"),
            2,
        );
        let edited = text.replace("date:", "decision-makers: [Engineering]\ndate:");
        edit_content(root, "0001", &edited, 0);
        assert_eq!(fs::read_to_string(root.join(ADR)).unwrap(), edited);
        let (_dir, executable) = editor("exit 0");
        assert_eq!(
            check(run(root, &["edit", "0001", "--editor", &executable]), 0)["result"],
            "unchanged"
        );
        check(run(root, &["validate"]), 0);
        fs::write(
            root.join(ADR),
            format!("{edited}\nUnauthorized external change.\n"),
        )
        .unwrap();
        edit_content(root, "0001", &edited, 2);
    }
}

#[test]
fn explicit_approval_freezes_edited_content_without_blocking_metadata_or_noops() {
    let dir = repo();
    let root = dir.path();
    check(run(root, &["new", "adr", "storage-choice"]), 0);
    let text = fs::read_to_string(root.join(ADR))
        .unwrap()
        .replace("TODO:", "Reviewed:");
    assert_eq!(edit_content(root, "0001", &text, 0)["result"], "edited");
    let approved = check(
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
                "Reviewed storage decision",
            ],
        ),
        0,
    );
    assert_eq!(approved["result"], "approved");
    let evidence = fs::read(root.join(".badr/evidence.jsonl")).unwrap();
    let text = fs::read_to_string(root.join(ADR)).unwrap();
    let unchanged = edit_content(root, "0001", &text, 0);
    assert_eq!(unchanged["result"], "unchanged");
    assert!(unchanged["proposed_audit_commit_message"].is_null());
    edit_content(root, "0001", &(text.clone() + "\nChanged reasoning.\n"), 2);
    assert_eq!(fs::read_to_string(root.join(ADR)).unwrap(), text);
    let metadata = text.replace("decision-makers: []", "decision-makers: [Engineering]");
    assert_eq!(edit_content(root, "0001", &metadata, 0)["result"], "edited");
    assert_eq!(
        fs::read(root.join(".badr/evidence.jsonl")).unwrap(),
        evidence
    );
    check(run(root, &["validate"]), 0);
}

#[test]
fn unrelated_changes_survive_and_unrelated_invalidity_prevents_editor_launch() {
    let dir = repo();
    let root = dir.path();
    check(run(root, &["new", "adr", "storage-choice"]), 0);
    check(run(root, &["new", "adr", "other-choice"]), 0);
    let other = root.join(".badr/adrs/0002-other-choice.md");
    let other_text = fs::read_to_string(&other).unwrap() + "\nUncommitted unrelated reasoning.\n";
    fs::write(&other, &other_text).unwrap();
    fs::write(root.join("unrelated.txt"), "user work\n").unwrap();
    git(root, &["add", "unrelated.txt"]);
    let index = git(root, &["ls-files", "--stage"]);
    let text = fs::read_to_string(root.join(ADR)).unwrap() + "\nTarget clarification.\n";
    edit_content(root, "0001", &text, 0);
    assert_eq!(fs::read_to_string(&other).unwrap(), other_text);
    assert_eq!(git(root, &["ls-files", "--stage"]), index);
    assert_eq!(
        fs::read_to_string(root.join("unrelated.txt")).unwrap(),
        "user work\n"
    );
    fs::write(&other, "broken unrelated record").unwrap();
    let (_dir, executable) = editor("exit 99");
    check(run(root, &["edit", "0001", "--editor", &executable]), 2);
    assert_eq!(fs::read_to_string(root.join(ADR)).unwrap(), text);
    assert_eq!(
        fs::read_to_string(&other).unwrap(),
        "broken unrelated record"
    );
}

#[test]
fn editor_requires_explicit_external_executable_and_an_exclusive_input_choice() {
    let dir = repo();
    let root = dir.path();
    check(run(root, &["new", "adr", "storage-choice"]), 0);
    check(run(root, &["edit", "0001"]), 2);
    check(run(root, &["edit", "0001", "--editor", "vi"]), 2);
    let (_dir, executable) = editor("exit 99");
    let local = root.join("editor");
    fs::copy(&executable, &local).unwrap();
    check(
        run(root, &["edit", "0001", "--editor", local.to_str().unwrap()]),
        2,
    );
    let link = root.join("external-editor-link");
    std::os::unix::fs::symlink(&local, &link).unwrap();
    check(
        run(root, &["edit", "0001", "--editor", link.to_str().unwrap()]),
        2,
    );
    check(
        run(
            root,
            &[
                "edit",
                "0001",
                "--editor",
                &executable,
                "--content",
                root.join(ADR).to_str().unwrap(),
            ],
        ),
        2,
    );
    let invalid_utf8 = root.join("invalid-input");
    fs::write(&invalid_utf8, [0xff]).unwrap();
    check(
        run(
            root,
            &["edit", "0001", "--content", invalid_utf8.to_str().unwrap()],
        ),
        2,
    );
    check(
        run(
            root,
            &["edit", "0001", "--content", "/nonexistent-badr-input"],
        ),
        4,
    );
    check(
        run(
            root,
            &["edit", "0001", "--editor", "/nonexistent-badr-editor"],
        ),
        4,
    );
    check(run(root, &["validate"]), 0);
}

#[test]
fn reports_failures_before_and_after_publication_without_rewriting_evidence() {
    for (point, published) in [
        ("before_apply", false),
        ("before_write_0", false),
        ("after_write_0", true),
        ("validate_written", true),
        ("after_postvalidate", true),
    ] {
        let dir = repo();
        let root = dir.path();
        check(run(root, &["new", "adr", "storage-choice"]), 0);
        let original = fs::read_to_string(root.join(ADR)).unwrap();
        let ledger = fs::read(root.join(".badr/evidence.jsonl")).unwrap();
        let text = original.clone() + "\nEdited reasoning.\n";
        let input = tempfile::NamedTempFile::new().unwrap();
        fs::write(input.path(), &text).unwrap();
        let result = check(
            Command::new(env!("CARGO_BIN_EXE_badr"))
                .arg("--repo")
                .arg(root)
                .arg("--json")
                .args(["edit", "0001", "--content", input.path().to_str().unwrap()])
                .env("BADR_TEST_FAIL_AT", point)
                .output()
                .unwrap(),
            4,
        );
        assert_eq!(result["affected_ids"], serde_json::json!(["0001"]));
        assert_eq!(
            result["partial_write"]["completed"],
            if published {
                serde_json::json!([ADR])
            } else {
                serde_json::json!([])
            }
        );
        assert_eq!(
            result["partial_write"]["not_completed"],
            if published {
                serde_json::json!([])
            } else {
                serde_json::json!([ADR])
            }
        );
        assert_eq!(root.join(".badr.lock").exists(), published);
        assert_eq!(
            fs::read_to_string(root.join(ADR)).unwrap(),
            if published { text } else { original }
        );
        assert_eq!(fs::read(root.join(".badr/evidence.jsonl")).unwrap(), ledger);
        assert!(result["proposed_audit_commit_message"].is_null());
    }
}

#[test]
fn external_changes_during_planning_are_preserved_and_block_publication() {
    let dir = repo();
    let root = dir.path();
    check(run(root, &["new", "adr", "storage-choice"]), 0);
    let original = fs::read_to_string(root.join(ADR)).unwrap();
    let input = tempfile::NamedTempFile::new().unwrap();
    fs::write(input.path(), original.clone() + "\nSupplied edit.\n").unwrap();
    let control = tempfile::tempdir().unwrap();
    let child = Command::new(env!("CARGO_BIN_EXE_badr"))
        .arg("--repo")
        .arg(root)
        .arg("--json")
        .args(["edit", "0001", "--content", input.path().to_str().unwrap()])
        .env("BADR_TEST_PAUSE_AT", "before_apply")
        .env("BADR_TEST_CONTROL", control.path())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let start = std::time::Instant::now();
    while !control.path().join("ready").exists() {
        assert!(start.elapsed().as_secs() < 15);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let external = original + "\nExternal user edit.\n";
    fs::write(root.join(ADR), &external).unwrap();
    fs::write(control.path().join("resume"), "continue").unwrap();
    let result = check(child.wait_with_output().unwrap(), 4);
    assert_eq!(result["partial_write"]["completed"], serde_json::json!([]));
    assert_eq!(fs::read_to_string(root.join(ADR)).unwrap(), external);
    assert!(!root.join(".badr.lock").exists());
}

fn decide(root: &Path, id: &str) {
    if id == "0001" {
        let text = fs::read_to_string(root.join(ADR))
            .unwrap()
            .replace("TODO:", "Reviewed:");
        fs::write(root.join(ADR), text).unwrap();
    }
    check(
        run(
            root,
            &[
                "decide",
                id,
                "--confirm",
                "--reviewed",
                "--actor",
                "Human",
                "--rationale",
                "Reviewed",
            ],
        ),
        0,
    );
}
fn reset(root: &Path, id: &str, text: &str, exit: i32) -> Value {
    let input = tempfile::NamedTempFile::new().unwrap();
    fs::write(input.path(), text).unwrap();
    check(
        run(
            root,
            &[
                "edit",
                id,
                "--content",
                input.path().to_str().unwrap(),
                "--confirm",
                "--actor",
                "Human",
                "--rationale",
                "Revise branch-local reasoning",
            ],
        ),
        exit,
    )
}

#[test]
fn branch_local_reset_retains_receipts_and_requires_reapproval() {
    let dir = repo();
    let root = dir.path();
    git(root, &["switch", "-c", "feature"]);
    check(run(root, &["new", "adr", "storage-choice"]), 0);
    decide(root, "0001");
    let ledger = fs::read_to_string(root.join(".badr/evidence.jsonl")).unwrap();
    let prior: Value = serde_json::from_str(ledger.lines().last().unwrap()).unwrap();
    commit_records(root);
    let text = fs::read_to_string(root.join(ADR)).unwrap();
    assert_eq!(reset(root, "0001", &text, 0)["result"], "unchanged");
    assert_eq!(
        fs::read_to_string(root.join(".badr/evidence.jsonl")).unwrap(),
        ledger
    );
    let changed = text + "\nRevised reasoning.\n";
    let result = reset(root, "0001", &changed, 0);
    assert_eq!(result["pending_commit_durability"], true);
    assert!(
        fs::read_to_string(root.join(ADR))
            .unwrap()
            .contains("status: proposed")
    );
    let after = fs::read_to_string(root.join(".badr/evidence.jsonl")).unwrap();
    assert!(after.starts_with(&ledger));
    let event: Value = serde_json::from_str(after.lines().last().unwrap()).unwrap();
    assert_eq!(event["operation"], "edit");
    assert_eq!(event["changes"][0]["approval_ref"], prior["event_id"]);
    assert_eq!(event["confirmed"], true);
    commit_records(root);
    check(run(root, &["validate"]), 0);
    decide(root, "0001");
    let reapproved = fs::read_to_string(root.join(".badr/evidence.jsonl")).unwrap();
    assert!(reapproved.starts_with(&after));
    check(run(root, &["validate"]), 0);
}

fn commit_records(root: &Path) {
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
            "Records",
        ],
    );
}
fn approved_feature() -> TempDir {
    let dir = repo();
    git(dir.path(), &["switch", "-c", "feature"]);
    check(run(dir.path(), &["new", "adr", "storage-choice"]), 0);
    decide(dir.path(), "0001");
    dir
}
fn register(root: &Path, id: &str, exit: i32) -> Value {
    check(
        run(
            root,
            &[
                "edit",
                id,
                "--register-external",
                "--confirm",
                "--actor",
                "Human",
                "--rationale",
                "Register external revision",
            ],
        ),
        exit,
    )
}

#[test]
fn external_registration_resolves_only_target_body_and_preserves_prior_receipts() {
    let dir = approved_feature();
    let root = dir.path();
    register(root, "0001", 2); // No digest mismatch to register.
    let original = fs::read_to_string(root.join(ADR)).unwrap();
    let ledger = fs::read_to_string(root.join(".badr/evidence.jsonl")).unwrap();
    let changed = original.clone() + "\nExternal reasoning change.\n";
    fs::write(root.join(ADR), &changed).unwrap();
    check(run(root, &["validate"]), 2);
    assert_eq!(fs::read_to_string(root.join(ADR)).unwrap(), changed);
    // Ordinary edit cannot implicitly register the externally changed body.
    reset(root, "0001", &changed, 2);
    check(run(root, &["edit", "0001", "--register-external"]), 2);
    for corrupt in [
        changed.replace("status: approved", "status: proposed"),
        changed.replace("date:", "related_adrs: [\"9999\"]\ndate:"),
        format!("{changed}\n## Lifecycle notes\n### 2020-01-01\nUnexpected note.\n"),
    ] {
        fs::write(root.join(ADR), &corrupt).unwrap();
        register(root, "0001", 2);
        assert_eq!(fs::read_to_string(root.join(ADR)).unwrap(), corrupt);
        assert_eq!(
            fs::read_to_string(root.join(".badr/evidence.jsonl")).unwrap(),
            ledger
        );
    }
    fs::write(root.join(ADR), &changed).unwrap();
    fs::write(root.join(".badr/unrelated"), "Unrecognized storage").unwrap();
    register(root, "0001", 2);
    fs::remove_file(root.join(".badr/unrelated")).unwrap();
    let result = register(root, "0001", 0);
    assert_eq!(result["validation_coverage"]["history"], "passed");
    assert!(
        fs::read_to_string(root.join(".badr/evidence.jsonl"))
            .unwrap()
            .starts_with(&ledger)
    );
    assert_eq!(
        fs::read_to_string(root.join(ADR)).unwrap(),
        changed.replace("status: approved", "status: proposed")
    );
    check(run(root, &["validate"]), 0);
    decide(root, "0001");
}

#[test]
fn reset_requires_named_feature_branch_complete_history_and_authorization() {
    let dir = approved_feature();
    let root = dir.path();
    let text = fs::read_to_string(root.join(ADR)).unwrap();
    let changed = text.clone() + "\nChange.\n";
    edit_content(root, "0001", &changed, 2);
    git(root, &["switch", "--detach"]);
    reset(root, "0001", &changed, 2);
    git(root, &["switch", "trunk"]);
    reset(root, "0001", &changed, 2);
    git(root, &["switch", "feature"]);
    git(root, &["branch", "-m", "renamed"]);
    let git_dir = git(root, &["rev-parse", "--absolute-git-dir"]);
    fs::write(
        Path::new(&git_dir).join("shallow"),
        format!("{}\n", git(root, &["rev-parse", "HEAD"])),
    )
    .unwrap();
    reset(root, "0001", &changed, 3);
    fs::remove_file(Path::new(&git_dir).join("shallow")).unwrap();
    let (_editor, executable) = editor("exit 0");
    check(
        run(
            root,
            &[
                "edit",
                "0001",
                "--editor",
                &executable,
                "--confirm",
                "--actor",
                "Human",
                "--rationale",
                "Check no-op",
            ],
        ),
        0,
    );
    assert_eq!(fs::read_to_string(root.join(ADR)).unwrap(), text);
    let (_editor, executable) = editor("printf '\\nRevised reasoning.\\n' >> \"$1\"");
    check(
        run(
            root,
            &[
                "edit",
                "0001",
                "--editor",
                &executable,
                "--confirm",
                "--actor",
                "Human",
                "--rationale",
                "Revise",
            ],
        ),
        0,
    );
    check(run(root, &["validate"]), 0);
}

#[test]
fn earlier_baseline_proposal_blocks_reset_after_branch_rename_and_reconfiguration() {
    let dir = repo();
    let root = dir.path();
    let initial = git(root, &["rev-parse", "HEAD"]);
    check(run(root, &["new", "adr", "storage-choice"]), 0);
    commit_records(root); // Earlier proposal makes the identity durable.
    git(root, &["switch", "-c", "feature"]);
    decide(root, "0001");
    let changed = fs::read_to_string(root.join(ADR)).unwrap() + "\nChange.\n";
    reset(root, "0001", &changed, 2);
    git(root, &["branch", "-m", "renamed"]);
    reset(root, "0001", &changed, 2);
    git(root, &["branch", "older", &initial]);
    fs::write(
        root.join(".badr/config.yaml"),
        "version: 1\nbaseline_branch: older\n",
    )
    .unwrap();
    git(root, &["branch", "-D", "trunk"]);
    reset(root, "0001", &changed, 2);
    fs::write(root.join(ADR), &changed).unwrap();
    register(root, "0001", 2);
}

#[test]
fn brr_reset_and_external_registration_keep_body_bytes_and_inactive_state() {
    let dir = repo();
    let root = dir.path();
    git(root, &["switch", "-c", "feature"]);
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
    let path = ".badr/brrs/domains/ordering/rules/BRR-ORD-001-cancellation.feature";
    let text = fs::read_to_string(root.join(path))
        .unwrap()
        .replace("TODO", "Reviewed");
    fs::write(root.join(path), text).unwrap();
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
    decide(root, "BRR-ORD-001");
    let text = fs::read_to_string(root.join(path)).unwrap() + "\n# Changed rule\r\n";
    reset(root, "BRR-ORD-001", &text, 0);
    assert_eq!(
        fs::read_to_string(root.join(path)).unwrap(),
        text.replace("status: approved", "status: proposed")
    );
    decide(root, "BRR-ORD-001");
    let text = fs::read_to_string(root.join(path)).unwrap() + "\n# External change\n";
    fs::write(root.join(path), text).unwrap();
    register(root, "BRR-ORD-001", 0);
    check(run(root, &["validate"]), 0);
    decide(root, "BRR-ORD-001");
}

#[test]
fn reset_partial_publication_reports_record_and_retained_manual_repair_lock() {
    let dir = approved_feature();
    let root = dir.path();
    let original = fs::read_to_string(root.join(ADR)).unwrap();
    let ledger = fs::read(root.join(".badr/evidence.jsonl")).unwrap();
    let input = tempfile::NamedTempFile::new().unwrap();
    fs::write(input.path(), original + "\nRevised reasoning.\n").unwrap();
    let result = check(
        Command::new(env!("CARGO_BIN_EXE_badr"))
            .arg("--repo")
            .arg(root)
            .arg("--json")
            .args([
                "edit",
                "0001",
                "--content",
                input.path().to_str().unwrap(),
                "--confirm",
                "--actor",
                "Human",
                "--rationale",
                "Revise",
            ])
            .env("BADR_TEST_FAIL_AT", "after_write_0")
            .output()
            .unwrap(),
        4,
    );
    assert_eq!(
        result["partial_write"]["completed"],
        serde_json::json!([ADR])
    );
    assert_eq!(
        result["partial_write"]["not_completed"],
        serde_json::json!([".badr/evidence.jsonl"])
    );
    assert_eq!(fs::read(root.join(".badr/evidence.jsonl")).unwrap(), ledger);
    assert!(root.join(".badr.lock").exists());
    assert!(
        fs::read_to_string(root.join(ADR))
            .unwrap()
            .contains("status: proposed")
    );
    check(run(root, &["new", "adr", "blocked"]), 4);
}

#[test]
fn deleted_baseline_identity_remains_ineligible() {
    let dir = repo();
    let root = dir.path();
    check(run(root, &["new", "adr", "storage-choice"]), 0);
    decide(root, "0001");
    commit_records(root);
    // A later baseline tree omits the ID, but history still reserves and freezes it.
    git(root, &["rm", ADR]);
    git(
        root,
        &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "commit",
            "-m",
            "Delete record",
        ],
    );
    git(root, &["switch", "-c", "feature"]);
    git(root, &["restore", "--source=HEAD~1", "--", ADR]);
    let original = fs::read_to_string(root.join(ADR)).unwrap();
    reset(root, "0001", &(original.clone() + "\nChange.\n"), 2);
    assert_eq!(fs::read_to_string(root.join(ADR)).unwrap(), original);
}

#[test]
fn reset_rechecks_baseline_and_checkout_before_publication() {
    for moved in ["baseline", "checkout"] {
        let dir = approved_feature();
        let root = dir.path();
        let original = fs::read_to_string(root.join(ADR)).unwrap();
        let input = tempfile::NamedTempFile::new().unwrap();
        fs::write(input.path(), original.clone() + "\nChange.\n").unwrap();
        let control = tempfile::tempdir().unwrap();
        let writer = Command::new(env!("CARGO_BIN_EXE_badr"))
            .arg("--repo")
            .arg(root)
            .arg("--json")
            .args([
                "edit",
                "0001",
                "--content",
                input.path().to_str().unwrap(),
                "--confirm",
                "--actor",
                "Human",
                "--rationale",
                "Revise",
            ])
            .env("BADR_TEST_PAUSE_AT", "before_apply")
            .env("BADR_TEST_CONTROL", control.path())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let start = std::time::Instant::now();
        while !control.path().join("ready").exists() {
            assert!(start.elapsed().as_secs() < 15);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        if moved == "baseline" {
            git(
                root,
                &[
                    "-c",
                    "user.name=Test",
                    "-c",
                    "user.email=test@example.invalid",
                    "commit",
                    "--allow-empty",
                    "-m",
                    "Move baseline",
                ],
            );
            git(root, &["branch", "-f", "trunk", "HEAD"]);
        } else {
            git(root, &["switch", "trunk"]);
        }
        fs::write(control.path().join("resume"), "continue").unwrap();
        let result = check(writer.wait_with_output().unwrap(), 4);
        assert_eq!(result["partial_write"]["completed"], serde_json::json!([]));
        assert_eq!(fs::read_to_string(root.join(ADR)).unwrap(), original);
    }
}

#[test]
fn external_registration_does_not_exempt_another_approved_body() {
    let dir = approved_feature();
    let root = dir.path();
    check(run(root, &["new", "adr", "other-choice"]), 0);
    let other = root.join(".badr/adrs/0002-other-choice.md");
    let text = fs::read_to_string(&other)
        .unwrap()
        .replace("TODO:", "Reviewed:");
    fs::write(&other, text).unwrap();
    decide(root, "0002");
    let ledger = fs::read(root.join(".badr/evidence.jsonl")).unwrap();
    for path in [root.join(ADR), other] {
        let text = fs::read_to_string(&path).unwrap() + "\nExternal change.\n";
        fs::write(path, text).unwrap();
    }
    let result = register(root, "0001", 2);
    assert!(
        result["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "body_digest_mismatch" && d["record_id"] == "0002")
    );
    assert_eq!(fs::read(root.join(".badr/evidence.jsonl")).unwrap(), ledger);
}
