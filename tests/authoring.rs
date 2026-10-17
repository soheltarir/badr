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
#[test]
fn creates_proposed_adr_from_template_without_prompts() {
    let dir = repo();
    let root = dir.path();
    let result = check(run(root, &["new", "adr", "storage-choice"]), 0);
    assert_eq!(result["affected_ids"], serde_json::json!(["0001"]));
    assert_eq!(result["operation"], "new_adr");
    assert_eq!(result["pending_commit_durability"], true);
    let text = fs::read_to_string(root.join(".badr/adrs/0001-storage-choice.md")).unwrap();
    assert!(text.contains("status: proposed"));
    assert!(text.contains("CC0-1.0"));
    check(run(root, &["validate"]), 0);
    assert!(!root.join(".badr/brrs").exists());
    assert_eq!(git(root, &["rev-list", "--count", "HEAD"]), "1");
}

#[test]
fn registers_draft_brr_and_proposes_only_with_explicit_authorization() {
    let dir = repo();
    let root = dir.path();
    check(run(root, ADD), 0);
    let created = check(
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
    assert_eq!(created["affected_ids"], serde_json::json!(["BRR-ORD-001"]));
    let shown = check(run(root, &["show", "BRR-ORD-001"]), 0);
    assert_eq!(shown["records"][0]["status"], "draft");
    assert_eq!(
        shown["records"][0]["owners"],
        serde_json::json!(["Product"])
    );
    check(
        run(
            root,
            &[
                "propose",
                "BRR-ORD-001",
                "--actor",
                "Human",
                "--rationale",
                "Ready for review",
            ],
        ),
        2,
    );
    let proposed = check(
        run(
            root,
            &[
                "propose",
                "BRR-ORD-001",
                "--confirm",
                "--actor",
                "Human",
                "--rationale",
                "Ready for review",
            ],
        ),
        0,
    );
    assert_eq!(proposed["affected_ids"], serde_json::json!(["BRR-ORD-001"]));
    assert_eq!(
        check(run(root, &["show", "BRR-ORD-001"]), 0)["records"][0]["status"],
        "proposed"
    );
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
                "Again",
            ],
        ),
        2,
    );
    check(run(root, &["validate"]), 0);
    assert!(!root.join(".badr/adrs").exists());
}

fn commit(root: &Path, message: &str) {
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
            message,
        ],
    );
}
fn replace(root: &Path, path: &str, from: &str, to: &str) {
    let text = fs::read_to_string(root.join(path))
        .unwrap()
        .replace(from, to);
    fs::write(root.join(path), text).unwrap();
}
#[test]
fn supplied_content_is_preserved_and_must_match_identity_state_and_format() {
    let dir = repo();
    let root = dir.path();
    check(run(root, &["new", "adr", "template"]), 0);
    let adr = fs::read_to_string(root.join(".badr/adrs/0001-template.md"))
        .unwrap()
        .replace("0001", "0002")
        .replace('\n', "\r\n");
    fs::write(root.join("input.md"), &adr).unwrap();
    let input = root.join("input.md");
    check(
        run(
            root,
            &[
                "new",
                "adr",
                "supplied",
                "--content",
                input.to_str().unwrap(),
            ],
        ),
        0,
    );
    assert_eq!(
        fs::read_to_string(root.join(".badr/adrs/0002-supplied.md")).unwrap(),
        adr
    );
    let before = fs::read(root.join(".badr/evidence.jsonl")).unwrap();
    for text in [
        adr.clone(),
        adr.replace("0002", "0003").replace("proposed", "approved"),
        "invalid".into(),
    ] {
        fs::write(&input, text).unwrap();
        check(
            run(
                root,
                &[
                    "new",
                    "adr",
                    "refused",
                    "--content",
                    input.to_str().unwrap(),
                ],
            ),
            2,
        );
        assert_eq!(fs::read(root.join(".badr/evidence.jsonl")).unwrap(), before);
    }
    check(run(root, ADD), 0);
    check(
        run(
            root,
            &[
                "new", "brr", "template", "--domain", "ordering", "--owner", "Product",
            ],
        ),
        0,
    );
    let brr = fs::read_to_string(
        root.join(".badr/brrs/domains/ordering/rules/BRR-ORD-001-template.feature"),
    )
    .unwrap()
    .replace("BRR-ORD-001", "BRR-ORD-002")
    .replace('\n', "\r\n");
    fs::write(&input, &brr).unwrap();
    check(
        run(
            root,
            &[
                "new",
                "brr",
                "supplied",
                "--domain",
                "ordering",
                "--content",
                input.to_str().unwrap(),
            ],
        ),
        0,
    );
    let path = root.join(".badr/brrs/domains/ordering/rules/BRR-ORD-002-supplied.feature");
    assert_eq!(fs::read_to_string(&path).unwrap(), brr);
    let body = check(run(root, &["show", "BRR-ORD-002"]), 0)["records"][0]["body"].clone();
    check(
        run(
            root,
            &[
                "propose",
                "BRR-ORD-002",
                "--confirm",
                "--actor",
                "Human",
                "--rationale",
                "Ready",
            ],
        ),
        0,
    );
    assert_eq!(
        check(run(root, &["show", "BRR-ORD-002"]), 0)["records"][0]["body"],
        body
    );
    fs::write(
        &input,
        brr.replace("BRR-ORD-002", "BRR-ORD-003")
            .replace("status: draft", "status: proposed"),
    )
    .unwrap();
    check(
        run(
            root,
            &[
                "new",
                "brr",
                "refused",
                "--domain",
                "ordering",
                "--content",
                input.to_str().unwrap(),
            ],
        ),
        2,
    );
    check(
        run(
            root,
            &[
                "new", "brr", "missing", "--domain", "missing", "--owner", "Product",
            ],
        ),
        2,
    );
    check(
        run(
            root,
            &[
                "propose",
                "0001",
                "--confirm",
                "--actor",
                "Human",
                "--rationale",
                "Ready",
            ],
        ),
        2,
    );
}

#[test]
fn numeric_padding_and_independent_domain_sequences_do_not_release_deleted_ids() {
    let dir = repo();
    let root = dir.path();
    check(run(root, &["new", "adr", "reserved"]), 0);
    let path = ".badr/adrs/0001-reserved.md";
    replace(root, path, "0001", "0000099");
    fs::rename(root.join(path), root.join(".badr/adrs/0000099-reserved.md")).unwrap();
    replace(root, ".badr/evidence.jsonl", "0001", "0000099");
    check(run(root, &["validate"]), 0);
    let saved = fs::read(root.join(".badr/adrs/0000099-reserved.md")).unwrap();
    fs::remove_file(root.join(".badr/adrs/0000099-reserved.md")).unwrap();
    let invalid = check(run(root, &["new", "adr", "refused"]), 2);
    assert!(
        invalid["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "removed_record")
    );
    fs::write(root.join(".badr/adrs/0000099-reserved.md"), saved).unwrap();
    assert_eq!(
        check(run(root, &["new", "adr", "next"]), 0)["affected_ids"],
        serde_json::json!(["0100"])
    );
    check(run(root, ADD), 0);
    check(
        run(
            root,
            &[
                "domain",
                "add",
                "billing",
                "--prefix",
                "BIL",
                "--confirm",
                "--actor",
                "Human",
                "--rationale",
                "Register",
            ],
        ),
        0,
    );
    for (domain, id) in [
        ("ordering", "BRR-ORD-001"),
        ("ordering", "BRR-ORD-002"),
        ("billing", "BRR-BIL-001"),
    ] {
        assert_eq!(
            check(
                run(
                    root,
                    &[
                        "new", "brr", "rule", "--domain", domain, "--owner", "Product"
                    ]
                ),
                0
            )["affected_ids"],
            serde_json::json!([id])
        );
    }
}

#[test]
fn reserves_records_and_allocation_receipts_from_deleted_history_on_other_local_branches() {
    let dir = repo();
    let root = dir.path();
    commit(root, "configuration");
    git(root, &["switch", "-c", "other"]);
    check(run(root, &["new", "adr", "historical"]), 0);
    commit(root, "allocate historical identity");
    fs::remove_file(root.join(".badr/adrs/0001-historical.md")).unwrap();
    fs::remove_file(root.join(".badr/evidence.jsonl")).unwrap();
    commit(root, "delete record and receipt");
    git(root, &["switch", "trunk"]);
    assert_eq!(
        check(run(root, &["new", "adr", "next"]), 0)["affected_ids"],
        serde_json::json!(["0002"])
    );
    check(run(root, &["validate"]), 0);
}

#[test]
fn concurrent_local_allocation_is_serialized_and_retry_uses_next_id() {
    let dir = repo();
    let root = dir.path();
    let control = tempfile::tempdir().unwrap();
    let child = Command::new(env!("CARGO_BIN_EXE_badr"))
        .arg("--repo")
        .arg(root)
        .arg("--json")
        .args(["new", "adr", "first"])
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
    assert_eq!(
        check(run(root, &["new", "adr", "second"]), 4)["diagnostics"][0]["code"],
        "write_locked"
    );
    fs::write(control.path().join("resume"), "continue").unwrap();
    assert_eq!(
        check(child.wait_with_output().unwrap(), 0)["affected_ids"],
        serde_json::json!(["0001"])
    );
    assert_eq!(
        check(run(root, &["new", "adr", "second"]), 0)["affected_ids"],
        serde_json::json!(["0002"])
    );
}

#[test]
fn reports_partial_creation_and_proposal_at_every_write_boundary() {
    for kind in ["adr", "brr", "propose"] {
        let count = if kind == "brr" { 3 } else { 2 };
        let mut points = vec![("before_apply".to_owned(), 0)];
        for i in 0..count {
            points.push((format!("before_write_{i}"), i));
            points.push((format!("after_write_{i}"), i + 1));
        }
        points.extend([
            ("before_postvalidate".into(), count),
            ("after_postvalidate".into(), count),
        ]);
        for (point, completed) in points {
            let dir = repo();
            let root = dir.path();
            if kind != "adr" {
                check(run(root, ADD), 0);
            }
            let brr = [
                "new", "brr", "rule", "--domain", "ordering", "--owner", "Product",
            ];
            if kind == "propose" {
                check(run(root, &brr), 0);
            }
            let args: &[&str] = match kind {
                "adr" => &["new", "adr", "decision"],
                "brr" => &brr,
                _ => &[
                    "propose",
                    "BRR-ORD-001",
                    "--confirm",
                    "--actor",
                    "Human",
                    "--rationale",
                    "Ready",
                ],
            };
            let paths: &[&str] = match kind {
                "adr" => &[".badr/evidence.jsonl", ".badr/adrs/0001-decision.md"],
                "brr" => &[
                    ".badr/evidence.jsonl",
                    ".badr/brrs/domains/ordering/rules/BRR-ORD-001-rule.feature",
                    ".badr/brrs/registry.yaml",
                ],
                _ => &[
                    ".badr/brrs/domains/ordering/rules/BRR-ORD-001-rule.feature",
                    ".badr/evidence.jsonl",
                ],
            };
            let result = check(
                Command::new(env!("CARGO_BIN_EXE_badr"))
                    .arg("--repo")
                    .arg(root)
                    .arg("--json")
                    .args(args)
                    .env("BADR_TEST_FAIL_AT", &point)
                    .output()
                    .unwrap(),
                4,
            );
            assert_eq!(
                result["partial_write"]["completed"],
                serde_json::json!(paths[..completed]),
                "{kind}/{point}"
            );
            assert_eq!(
                result["partial_write"]["not_completed"],
                serde_json::json!(paths[completed..])
            );
            assert_eq!(result["proposed_audit_commit_message"], Value::Null);
            assert_eq!(root.join(".badr.lock").exists(), completed > 0);
            if completed > 0 {
                check(run(root, args), 4);
                if kind != "propose" {
                    let events = fs::read_to_string(root.join(".badr/evidence.jsonl")).unwrap();
                    assert!(events.lines().any(
                        |line| serde_json::from_str::<Value>(line).unwrap()["operation"]
                            == "allocate"
                    ));
                }
                if completed < count {
                    check(run(root, &["validate"]), 2);
                } else {
                    check(run(root, &["validate"]), 0);
                }
            }
        }
    }
}

#[test]
fn clone_collisions_are_rejected_without_renumbering_or_overwriting() {
    let dir = repo();
    let root = dir.path();
    commit(root, "configuration");
    let clone = tempfile::tempdir().unwrap();
    git(
        root,
        &[
            "clone",
            "--quiet",
            root.to_str().unwrap(),
            clone.path().to_str().unwrap(),
        ],
    );
    check(run(root, &["new", "adr", "first-clone"]), 0);
    check(run(clone.path(), &["new", "adr", "second-clone"]), 0);
    let first = fs::read(root.join(".badr/adrs/0001-first-clone.md")).unwrap();
    let second = fs::read(clone.path().join(".badr/adrs/0001-second-clone.md")).unwrap();
    fs::write(root.join(".badr/adrs/0001-second-clone.md"), &second).unwrap();
    let mut ledger = fs::read(root.join(".badr/evidence.jsonl")).unwrap();
    ledger.extend(fs::read(clone.path().join(".badr/evidence.jsonl")).unwrap());
    fs::write(root.join(".badr/evidence.jsonl"), &ledger).unwrap();
    let report = check(run(root, &["validate"]), 2);
    assert!(
        report["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "duplicate_identity" || d["code"] == "inconsistent_evidence")
    );
    check(run(root, &["new", "adr", "refused"]), 2);
    assert_eq!(
        fs::read(root.join(".badr/adrs/0001-first-clone.md")).unwrap(),
        first
    );
    assert_eq!(
        fs::read(root.join(".badr/adrs/0001-second-clone.md")).unwrap(),
        second
    );
    assert_eq!(fs::read(root.join(".badr/evidence.jsonl")).unwrap(), ledger);
}

#[test]
fn creation_refuses_invalid_collections_missing_history_and_unsafe_paths() {
    for case in ["invalid", "baseline", "shallow", "symlink", "stale_lock"] {
        let dir = repo();
        let root = dir.path();
        fs::write(root.join("unrelated"), "keep").unwrap();
        let outside = tempfile::tempdir().unwrap();
        match case {
            "invalid" => {
                fs::create_dir(root.join(".badr/adrs")).unwrap();
                fs::write(root.join(".badr/adrs/0001-invalid.md"), "invalid").unwrap();
            }
            "baseline" => {
                git(root, &["switch", "-c", "feature"]);
                git(root, &["branch", "-D", "trunk"]);
            }
            "shallow" => {
                fs::write(
                    root.join(".git/shallow"),
                    format!("{}\n", git(root, &["rev-parse", "HEAD"])),
                )
                .unwrap();
            }
            "symlink" => {
                std::os::unix::fs::symlink(outside.path(), root.join(".badr/adrs")).unwrap();
            }
            "stale_lock" => {
                fs::write(root.join(".badr.lock"), "pid=99999999\n").unwrap();
            }
            _ => unreachable!(),
        }
        check(
            run(root, &["new", "adr", "refused"]),
            match case {
                "baseline" | "shallow" => 3,
                "stale_lock" => 4,
                _ => 2,
            },
        );
        assert!(!root.join(".badr/evidence.jsonl").exists());
        assert!(fs::read_dir(outside.path()).unwrap().next().is_none());
        assert_eq!(fs::read_to_string(root.join("unrelated")).unwrap(), "keep");
    }
}

#[test]
fn reservations_include_receipt_only_history_and_large_numeric_identities() {
    let dir = repo();
    let root = dir.path();
    commit(root, "configuration");
    git(root, &["switch", "-c", "retained-allocation"]);
    check(run(root, &["new", "adr", "reserved"]), 0);
    let original = ".badr/adrs/0001-reserved.md";
    replace(root, original, "0001", "999999999999999999999999");
    fs::rename(
        root.join(original),
        root.join(".badr/adrs/999999999999999999999999-reserved.md"),
    )
    .unwrap();
    replace(
        root,
        ".badr/evidence.jsonl",
        "0001",
        "999999999999999999999999",
    );
    fs::remove_file(root.join(".badr/adrs/999999999999999999999999-reserved.md")).unwrap();
    commit(root, "retained allocation without record");
    git(root, &["switch", "trunk"]);
    assert_eq!(
        check(run(root, &["new", "adr", "next"]), 0)["affected_ids"],
        serde_json::json!(["1000000000000000000000000"])
    );
}

#[test]
fn human_output_includes_pending_audit_and_completed_writes() {
    let dir = repo();
    let root = dir.path();
    let output = Command::new(env!("CARGO_BIN_EXE_badr"))
        .arg("--repo")
        .arg(root)
        .args(["new", "adr", "storage"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("Completed writes:"));
    assert!(text.contains("Proposed audit commit message:"));
    assert!(text.contains("Allocate 0001"));
}

#[test]
fn rejected_and_withdrawn_records_keep_their_numeric_reservations() {
    for status in ["rejected", "deprecated"] {
        let dir = repo();
        let root = dir.path();
        check(run(root, &["new", "adr", "reserved"]), 0);
        let ledger_path = root.join(".badr/evidence.jsonl");
        let mut text = fs::read_to_string(&ledger_path).unwrap();
        let mut event: Value = serde_json::from_str(text.trim()).unwrap();
        let operations: &[(&str, &str, &str)] = if status == "rejected" {
            &[("reject", "proposed", "rejected")]
        } else {
            &[
                ("decide", "proposed", "approved"),
                ("deprecate", "approved", "deprecated"),
            ]
        };
        for (operation, from, to) in operations {
            let now: chrono::DateTime<chrono::Utc> = std::time::SystemTime::now().into();
            event["at"] = now
                .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true)
                .into();
            event["event_id"] = format!("fixture-{operation}").into();
            event["operation"] = (*operation).into();
            event["actor"] = "Human".into();
            event["rationale"] = "Lifecycle fixture".into();
            event["confirmed"] = true.into();
            event["changes"][0]["from"] = (*from).into();
            event["changes"][0]["to"] = (*to).into();
            if *operation == "decide" {
                event["reviewed"] = true.into();
                event["changes"][0]["approval_ref"] = "fixture-decide".into();
            }
            text.push_str(&serde_json::to_string(&event).unwrap());
            text.push('\n');
        }
        fs::write(&ledger_path, text).unwrap();
        replace(
            root,
            ".badr/adrs/0001-reserved.md",
            "status: proposed",
            &format!("status: {status}"),
        );
        check(run(root, &["validate"]), 0);
        assert_eq!(
            check(run(root, &["new", "adr", "next"]), 0)["affected_ids"],
            serde_json::json!(["0002"])
        );
    }
}

const APPROVE: &[&str] = &[
    "--confirm",
    "--reviewed",
    "--actor",
    "Human",
    "--rationale",
    "Reviewed and resolved applicable gaps",
];
fn decide(root: &Path, id: &str, extra: &[&str]) -> Output {
    let mut args = vec!["decide", id];
    args.extend_from_slice(APPROVE);
    args.extend_from_slice(extra);
    run(root, &args)
}
#[test]
fn approves_adr_with_exact_receipt_without_committing_or_changing_baseline() {
    use sha2::{Digest, Sha256};
    let dir = repo();
    let root = dir.path();
    let now: chrono::DateTime<chrono::Utc> = std::time::SystemTime::now().into();
    let date = now.format("%Y-%m-%d");
    let text = format!(
        "---\nid: \"0001\"\nstatus: proposed\ndate: \"{date}\"\n---\n{}",
        include_str!("../src/templates/adr.md")
            .replace("{{title}}", "storage-choice")
            .replace("TODO:", "Reviewed:")
    )
    .replace('\n', "\r\n");
    let notes = "## Lifecycle notes\r\n\r\n### 2020-01-01\r\n\r\nPrepared for review.\r\n";
    let input = root.join("input.md");
    fs::write(&input, text + notes).unwrap();
    check(
        run(
            root,
            &[
                "new",
                "adr",
                "storage-choice",
                "--content",
                input.to_str().unwrap(),
            ],
        ),
        0,
    );
    let path = root.join(".badr/adrs/0001-storage-choice.md");
    let body = check(run(root, &["show", "0001"]), 0)["records"][0]["decision_body"]
        .as_str()
        .unwrap()
        .to_owned();
    let baseline = git(root, &["rev-parse", "trunk"]);
    git(root, &["switch", "-c", "review"]);
    let earlier = fs::read(root.join(".badr/evidence.jsonl")).unwrap();
    let before = std::time::SystemTime::now();
    let result = check(decide(root, "0001", &[]), 0);
    assert_eq!(result["operation"], "decide");
    assert_eq!(result["result"], "approved");
    assert_eq!(result["baseline_commit"], baseline);
    assert_eq!(result["pending_commit_durability"], true);
    assert!(
        result["proposed_audit_commit_message"]
            .as_str()
            .unwrap()
            .contains("Approve 0001")
    );
    let shown = check(run(root, &["show", "0001"]), 0);
    assert_eq!(shown["records"][0]["status"], "approved");
    assert_eq!(shown["records"][0]["directs_new_work"], true);
    assert_eq!(shown["records"][0]["decision_body"], body);
    assert!(fs::read_to_string(&path).unwrap().ends_with(notes));
    let ledger = fs::read(root.join(".badr/evidence.jsonl")).unwrap();
    assert!(ledger.starts_with(&earlier));
    let receipt: Value = serde_json::from_slice(
        ledger
            .split(|b| *b == b'\n')
            .rfind(|s| !s.is_empty())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(receipt["reviewed"], true);
    assert_eq!(receipt["branch"], "review");
    assert_eq!(receipt["baseline_commit"], baseline);
    assert_eq!(receipt["changes"][0]["from"], "proposed");
    assert_eq!(receipt["changes"][0]["approval_ref"], receipt["event_id"]);
    assert_eq!(
        receipt["changes"][0]["body_sha256"],
        format!("{:x}", Sha256::digest(body.as_bytes()))
    );
    let at = chrono::DateTime::parse_from_rfc3339(receipt["at"].as_str().unwrap()).unwrap();
    assert!(std::time::SystemTime::from(at) >= before);
    assert_eq!(
        shown["records"][0]["date"],
        &receipt["at"].as_str().unwrap()[..10]
    );
    check(decide(root, "0001", &[]), 2);
    assert_eq!(fs::read(root.join(".badr/evidence.jsonl")).unwrap(), ledger);
    assert_eq!(git(root, &["rev-parse", "trunk"]), baseline);
    assert_eq!(git(root, &["rev-list", "--count", "HEAD"]), "1");
    check(run(root, &["validate"]), 0);
    commit(root, "Approve decision and retain receipts");
    assert_eq!(
        check(run(root, &["validate"]), 0)["pending_commit_durability"],
        false
    );
}

#[test]
fn approval_requires_resolved_review_and_preserves_inactive_brr_body() {
    use sha2::{Digest, Sha256};
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
    check(decide(root, "BRR-ORD-001", &[]), 2);
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
    let path = root.join(".badr/brrs/domains/ordering/rules/BRR-ORD-001-cancellation.feature");
    let before = fs::read(&path).unwrap();
    let ledger = fs::read(root.join(".badr/evidence.jsonl")).unwrap();
    // An explicit review declaration cannot resolve visible template gaps.
    check(decide(root, "BRR-ORD-001", &[]), 2);
    for args in [
        vec!["--confirm", "--actor", "Human", "--rationale", "Reviewed"],
        vec!["--reviewed", "--actor", "Human", "--rationale", "Reviewed"],
        vec![
            "--confirm",
            "--reviewed",
            "--actor",
            " ",
            "--rationale",
            "Reviewed",
        ],
        vec![
            "--confirm",
            "--reviewed",
            "--actor",
            "Human",
            "--rationale",
            " ",
        ],
        vec!["--confirm", "--reviewed"],
    ] {
        let mut command = vec!["decide", "BRR-ORD-001"];
        command.extend(args);
        check(run(root, &command), 2);
    }
    assert_eq!(fs::read(&path).unwrap(), before);
    assert_eq!(fs::read(root.join(".badr/evidence.jsonl")).unwrap(), ledger);
    let text = String::from_utf8(before)
        .unwrap()
        .replace("TODO describe", "Describe")
        .replace('\n', "\r\n");
    fs::write(&path, text).unwrap();
    let body = check(run(root, &["show", "BRR-ORD-001"]), 0)["records"][0]["body"]
        .as_str()
        .unwrap()
        .to_owned();
    check(decide(root, "BRR-ORD-001", &[]), 0);
    let shown = check(run(root, &["show", "BRR-ORD-001"]), 0);
    assert_eq!(shown["records"][0]["status"], "approved");
    assert_eq!(shown["records"][0]["currently_effective"], false);
    assert!(shown["records"][0].get("effective_from").is_none());
    assert!(shown["records"][0].get("effective_until").is_none());
    assert_eq!(shown["records"][0]["body"], body);
    let ledger = fs::read_to_string(root.join(".badr/evidence.jsonl")).unwrap();
    let receipt: Value = serde_json::from_str(ledger.lines().last().unwrap()).unwrap();
    assert_eq!(
        receipt["changes"][0]["body_sha256"],
        format!("{:x}", Sha256::digest(body.as_bytes()))
    );
    fs::write(
        &path,
        fs::read_to_string(&path).unwrap() + "# unauthorized edit\r\n",
    )
    .unwrap();
    check(decide(root, "BRR-ORD-001", &[]), 2);
    assert_eq!(
        fs::read_to_string(root.join(".badr/evidence.jsonl")).unwrap(),
        ledger
    );
    check(run(root, &["validate"]), 2);
}

#[test]
fn successor_approval_requires_observable_compatibility_treatment_without_replacement() {
    let dir = repo();
    let root = dir.path();
    check(run(root, ADD), 0);
    check(
        run(
            root,
            &[
                "new",
                "brr",
                "successor",
                "--domain",
                "ordering",
                "--owner",
                "Product",
            ],
        ),
        0,
    );
    let path = root.join(".badr/brrs/domains/ordering/rules/BRR-ORD-001-successor.feature");
    let text = fs::read_to_string(&path)
        .unwrap()
        .replace("TODO describe", "Describe")
        .replace("Feature:", "@successor\nFeature:");
    fs::write(&path, &text).unwrap();
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
    check(decide(root, "BRR-ORD-001", &[]), 2);
    let data = "    @compatibility:existing-data @treatment:grandfather\n    Scenario: Retain existing orders\n      Given an order already exists\n      When the new policy is introduced\n      Then its original cancellation rights remain\n";
    fs::write(&path, fs::read_to_string(&path).unwrap() + data).unwrap();
    check(decide(root, "BRR-ORD-001", &[]), 2);
    let flight = "    @compatibility:in-flight @treatment:re-evaluate\n    Scenario: Re-evaluate in-flight cancellations\n      Given a cancellation is in flight\n      When the new policy is introduced\n      Then its eligibility is evaluated under the new policy\n";
    fs::write(&path, fs::read_to_string(&path).unwrap() + flight).unwrap();
    check(decide(root, "BRR-ORD-001", &[]), 0);
    let shown = check(run(root, &["show", "BRR-ORD-001"]), 0);
    assert_eq!(shown["records"][0]["supersedes"], serde_json::json!([]));
    assert_eq!(shown["records"][0]["superseded_by"], serde_json::json!([]));
    assert_eq!(shown["records"][0]["currently_effective"], false);
    check(run(root, &["validate"]), 0);
}

fn ready_adr(root: &Path) -> std::path::PathBuf {
    check(run(root, &["new", "adr", "decision"]), 0);
    let path = root.join(".badr/adrs/0001-decision.md");
    fs::write(
        &path,
        fs::read_to_string(&path)
            .unwrap()
            .replace("TODO:", "Reviewed:"),
    )
    .unwrap();
    path
}

#[test]
fn approval_refuses_missing_evidence_and_unusable_baseline_without_writes() {
    for fault in [
        "evidence",
        "missing-baseline",
        "shallow",
        "invalid-relationship",
        "unresolved-gap",
        "illegal-status",
    ] {
        let dir = repo();
        let root = dir.path();
        let path = ready_adr(root);
        match fault {
            "evidence" => fs::remove_file(root.join(".badr/evidence.jsonl")).unwrap(),
            "missing-baseline" => {
                git(root, &["switch", "-c", "feature"]);
                git(root, &["branch", "-D", "trunk"]);
            }
            "shallow" => fs::write(
                root.join(".git/shallow"),
                git(root, &["rev-parse", "HEAD"]) + "\n",
            )
            .unwrap(),
            "invalid-relationship" => fs::write(
                &path,
                fs::read_to_string(&path).unwrap().replacen(
                    "status: proposed",
                    "related_adrs: [\"9999\"]\nstatus: proposed",
                    1,
                ),
            )
            .unwrap(),
            "unresolved-gap" => fs::write(
                &path,
                fs::read_to_string(&path).unwrap() + "\nTBD: resolve storage guarantees\n",
            )
            .unwrap(),
            _ => fs::write(
                &path,
                fs::read_to_string(&path)
                    .unwrap()
                    .replace("status: proposed", "status: rejected"),
            )
            .unwrap(),
        }
        let original = fs::read(&path).unwrap();
        let evidence = fs::read(root.join(".badr/evidence.jsonl")).ok();
        check(
            decide(root, "0001", &[]),
            if matches!(fault, "missing-baseline" | "shallow") {
                3
            } else {
                2
            },
        );
        assert_eq!(fs::read(&path).unwrap(), original, "{fault}");
        assert_eq!(fs::read(root.join(".badr/evidence.jsonl")).ok(), evidence);
        assert!(!root.join(".badr.lock").exists());
    }
}

#[test]
fn approval_reports_every_partial_write_boundary_and_retains_repair_lock() {
    for (point, count) in [
        ("before_apply", 0),
        ("before_write_0", 0),
        ("after_write_0", 1),
        ("before_write_1", 1),
        ("after_write_1", 2),
        ("before_postvalidate", 2),
        ("after_postvalidate", 2),
    ] {
        let dir = repo();
        let root = dir.path();
        ready_adr(root);
        let mut args = vec!["decide", "0001"];
        args.extend_from_slice(APPROVE);
        let out = Command::new(env!("CARGO_BIN_EXE_badr"))
            .arg("--repo")
            .arg(root)
            .arg("--json")
            .args(&args)
            .env("BADR_TEST_FAIL_AT", point)
            .output()
            .unwrap();
        let result = check(out, 4);
        let paths = [".badr/adrs/0001-decision.md", ".badr/evidence.jsonl"];
        assert_eq!(
            result["partial_write"]["completed"],
            serde_json::json!(paths[..count]),
            "{point}"
        );
        assert_eq!(
            result["partial_write"]["not_completed"],
            serde_json::json!(paths[count..])
        );
        assert_eq!(root.join(".badr.lock").exists(), count > 0);
        assert!(result["proposed_audit_commit_message"].is_null());
        if count > 0 {
            check(decide(root, "0001", &[]), 4);
        }
        check(run(root, &["validate"]), if count == 1 { 2 } else { 0 });
    }
}

#[test]
fn approval_detects_baseline_and_external_content_changes_before_publication() {
    for fault in ["baseline", "body"] {
        let dir = repo();
        let root = dir.path();
        let path = ready_adr(root);
        let control = tempfile::tempdir().unwrap();
        let mut args = vec!["decide", "0001"];
        args.extend_from_slice(APPROVE);
        let child = Command::new(env!("CARGO_BIN_EXE_badr"))
            .arg("--repo")
            .arg(root)
            .arg("--json")
            .args(&args)
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
        if fault == "baseline" {
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
                    "baseline moved",
                ],
            );
        } else {
            fs::write(
                &path,
                fs::read_to_string(&path).unwrap() + "\nExternal editor change\n",
            )
            .unwrap();
        }
        let expected = fs::read(&path).unwrap();
        let evidence = fs::read(root.join(".badr/evidence.jsonl")).unwrap();
        fs::write(control.path().join("resume"), "resume").unwrap();
        let result = check(child.wait_with_output().unwrap(), 4);
        assert_eq!(result["partial_write"]["completed"], serde_json::json!([]));
        assert_eq!(fs::read(&path).unwrap(), expected);
        assert_eq!(
            fs::read(root.join(".badr/evidence.jsonl")).unwrap(),
            evidence
        );
        assert!(!root.join(".badr.lock").exists());
    }
}
