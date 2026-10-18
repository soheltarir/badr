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
            if *operation == "deprecate" {
                event["decision_gap"] = "No replacement selected".into();
                event["existing_code_treatment"] = "Retain existing code".into();
            }
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
    let before_flight = fs::read_to_string(&path).unwrap();
    let no_outcome = flight.replace(
        "      Then its eligibility is evaluated under the new policy\n",
        "",
    );
    fs::write(&path, before_flight.clone() + &no_outcome).unwrap();
    check(decide(root, "BRR-ORD-001", &[]), 2);
    fs::write(&path, before_flight + flight).unwrap();
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

#[test]
fn rejects_proposed_adr_preserving_reasoning_and_allocation() {
    let dir = repo();
    let root = dir.path();
    ready_adr(root);
    let before = check(run(root, &["show", "0001"]), 0);
    let ledger = fs::read(root.join(".badr/evidence.jsonl")).unwrap();
    let result = check(
        run(
            root,
            &[
                "reject",
                "0001",
                "--confirm",
                "--actor",
                "Human",
                "--rationale",
                "Prefer another option",
            ],
        ),
        0,
    );
    assert_eq!(result["operation"], "reject");
    assert_eq!(result["result"], "rejected");
    assert_eq!(result["pending_commit_durability"], true);
    let shown = check(run(root, &["show", "0001"]), 0);
    assert_eq!(shown["records"][0]["status"], "rejected");
    assert_eq!(shown["records"][0]["directs_new_work"], false);
    assert_eq!(
        shown["records"][0]["decision_body"],
        before["records"][0]["decision_body"]
    );
    assert!(
        fs::read(root.join(".badr/evidence.jsonl"))
            .unwrap()
            .starts_with(&ledger)
    );
    assert!(
        fs::read_to_string(root.join(".badr/adrs/0001-decision.md"))
            .unwrap()
            .contains("Prefer another option")
    );
    check(run(root, &["validate"]), 0);
    assert_eq!(
        check(run(root, &["new", "adr", "next"]), 0)["affected_ids"],
        serde_json::json!(["0002"])
    );
}

#[test]
fn deprecation_withdraws_branch_direction_and_retains_exact_body_notes_and_receipts() {
    let dir = repo();
    let root = dir.path();
    let now: chrono::DateTime<chrono::Utc> = std::time::SystemTime::now().into();
    let text = format!(
        "---\nid: \"0001\"\nstatus: proposed\ndate: \"{}\"\n---\n{}",
        now.format("%Y-%m-%d"),
        include_str!("../src/templates/adr.md")
            .replace("{{title}}", "decision")
            .replace("TODO:", "Reviewed:")
    )
    .replace('\n', "\r\n");
    let path = root.join(".badr/adrs/0001-decision.md");
    let notes = "## Lifecycle notes\r\n\r\n### 2020-01-01 Preparation\r\nPrior note.\r\n";
    let input = root.join("input.md");
    fs::write(
        &input,
        text + "\r\n```md\r\n## Lifecycle notes\r\n```\r\n" + notes,
    )
    .unwrap();
    check(
        run(
            root,
            &[
                "new",
                "adr",
                "decision",
                "--content",
                input.to_str().unwrap(),
            ],
        ),
        0,
    );
    check(decide(root, "0001", &[]), 0);
    commit(root, "Approved direction on baseline");
    let baseline = git(root, &["rev-parse", "trunk"]);
    git(root, &["switch", "-c", "withdraw"]);
    fs::write(
        root.join("existing.rs"),
        "unchanged existing implementation\n",
    )
    .unwrap();
    let before = check(run(root, &["show", "0001"]), 0);
    let ledger = fs::read(root.join(".badr/evidence.jsonl")).unwrap();
    let result = check(
        run(
            root,
            &[
                "deprecate",
                "0001",
                "--confirm",
                "--actor",
                "Human",
                "--rationale",
                "Direction no longer suitable",
                "--decision-gap",
                "Storage direction awaits successor",
                "--existing-code-treatment",
                "Retain until migration is authorized",
            ],
        ),
        0,
    );
    assert_eq!(result["operation"], "deprecate");
    assert_eq!(result["result"], "deprecated");
    assert_eq!(result["pending_commit_durability"], true);
    let shown = check(run(root, &["show", "0001"]), 0);
    assert_eq!(shown["records"][0]["status"], "deprecated");
    assert_eq!(shown["records"][0]["directs_new_work"], false);
    assert_eq!(
        shown["records"][0]["decision_body"],
        before["records"][0]["decision_body"]
    );
    assert_eq!(
        check(run(root, &["list", "--directing"]), 0)["records"],
        serde_json::json!([])
    );
    let updated = fs::read_to_string(&path).unwrap();
    assert!(updated.contains(notes));
    assert!(updated.contains("Storage direction awaits successor"));
    assert!(updated.contains("Retain until migration is authorized"));
    let after = fs::read(root.join(".badr/evidence.jsonl")).unwrap();
    assert!(after.starts_with(&ledger));
    let receipt: Value =
        serde_json::from_str(String::from_utf8(after).unwrap().lines().last().unwrap()).unwrap();
    let approval: Value =
        serde_json::from_str(String::from_utf8(ledger).unwrap().lines().last().unwrap()).unwrap();
    assert_eq!(receipt["changes"][0]["approval_ref"], approval["event_id"]);
    assert_eq!(
        receipt["decision_gap"],
        "Storage direction awaits successor"
    );
    assert_eq!(
        receipt["existing_code_treatment"],
        "Retain until migration is authorized"
    );
    assert!(updated.contains(&format!(
        "date: \"{}\"",
        &receipt["at"].as_str().unwrap()[..10]
    )));
    assert_eq!(
        fs::read_to_string(root.join("existing.rs")).unwrap(),
        "unchanged existing implementation\n"
    );
    assert_eq!(git(root, &["rev-parse", "trunk"]), baseline);
    check(run(root, &["validate"]), 0);
    commit(root, "Withdraw direction");
    assert_eq!(
        check(run(root, &["validate"]), 0)["pending_commit_durability"],
        false
    );
    git(root, &["switch", "trunk"]);
    assert_eq!(
        check(run(root, &["list", "--directing"]), 0)["records"][0]["status"],
        "approved"
    );
}

fn withdraw(root: &Path, operation: &str, extra: &[&str]) -> Output {
    let mut args = vec![
        operation,
        "0001",
        "--confirm",
        "--actor",
        "Human",
        "--rationale",
        "Withdraw direction",
    ];
    if operation == "deprecate" {
        args.extend([
            "--decision-gap",
            "None",
            "--existing-code-treatment",
            "Retain existing code",
        ]);
    }
    args.extend_from_slice(extra);
    run(root, &args)
}

#[test]
fn withdrawals_refuse_missing_declarations_illegal_states_and_reopening_without_writes() {
    for operation in ["reject", "deprecate"] {
        let dir = repo();
        let root = dir.path();
        let path = ready_adr(root);
        if operation == "deprecate" {
            check(withdraw(root, operation, &[]), 2);
            check(decide(root, "0001", &[]), 0);
        } else {
            check(withdraw(root, "deprecate", &[]), 2);
        }
        let mut required = vec![
            "--confirm",
            "--actor",
            "Human",
            "--rationale",
            "Withdraw direction",
        ];
        if operation == "deprecate" {
            required.extend([
                "--decision-gap",
                "None",
                "--existing-code-treatment",
                "Retain existing code",
            ]);
        }
        let before = fs::read(&path).unwrap();
        let ledger = fs::read(root.join(".badr/evidence.jsonl")).unwrap();
        for flag in [
            "--confirm",
            "--actor",
            "--rationale",
            "--decision-gap",
            "--existing-code-treatment",
        ] {
            let Some(index) = required.iter().position(|s| *s == flag) else {
                continue;
            };
            let mut missing = required.clone();
            missing.drain(index..index + if flag == "--confirm" { 1 } else { 2 });
            let mut args = vec![operation, "0001"];
            args.extend(missing);
            check(run(root, &args), 2);
            if flag != "--confirm" {
                let mut blank = required.clone();
                blank[index + 1] = " \n ";
                let mut args = vec![operation, "0001"];
                args.extend(blank);
                check(run(root, &args), 2);
            }
            assert_eq!(fs::read(&path).unwrap(), before);
            assert_eq!(fs::read(root.join(".badr/evidence.jsonl")).unwrap(), ledger);
        }
        check(withdraw(root, operation, &[]), 0);
        let terminal = fs::read(&path).unwrap();
        let ledger = fs::read(root.join(".badr/evidence.jsonl")).unwrap();
        check(withdraw(root, "reject", &[]), 2);
        check(withdraw(root, "deprecate", &[]), 2);
        check(decide(root, "0001", &[]), 2);
        let input = root.join("reopen.md");
        fs::write(
            &input,
            String::from_utf8(terminal.clone()).unwrap().replace(
                &format!(
                    "status: {}",
                    if operation == "reject" {
                        "rejected"
                    } else {
                        "deprecated"
                    }
                ),
                "status: proposed",
            ),
        )
        .unwrap();
        check(
            run(
                root,
                &["edit", "0001", "--content", input.to_str().unwrap()],
            ),
            2,
        );
        fs::write(
            &input,
            String::from_utf8(terminal.clone())
                .unwrap()
                .replace("Reviewed:", "New reasoning:"),
        )
        .unwrap();
        check(
            run(
                root,
                &["edit", "0001", "--content", input.to_str().unwrap()],
            ),
            2,
        );
        assert_eq!(fs::read(&path).unwrap(), terminal);
        assert_eq!(fs::read(root.join(".badr/evidence.jsonl")).unwrap(), ledger);
        check(run(root, &["validate"]), 0);
    }
}

#[test]
fn withdrawals_report_partial_writes_and_preserve_precommit_replay() {
    for operation in ["reject", "deprecate"] {
        for (point, count) in [
            ("before_apply", 0),
            ("after_write_0", 1),
            ("after_write_1", 2),
            ("after_postvalidate", 2),
        ] {
            let dir = repo();
            let root = dir.path();
            ready_adr(root);
            if operation == "deprecate" {
                check(decide(root, "0001", &[]), 0);
            }
            let mut args = vec![
                operation,
                "0001",
                "--confirm",
                "--actor",
                "Human",
                "--rationale",
                "Withdraw direction",
            ];
            if operation == "deprecate" {
                args.extend([
                    "--decision-gap",
                    "None",
                    "--existing-code-treatment",
                    "Retain existing code",
                ]);
            }
            let result = check(
                Command::new(env!("CARGO_BIN_EXE_badr"))
                    .arg("--repo")
                    .arg(root)
                    .arg("--json")
                    .args(args)
                    .env("BADR_TEST_FAIL_AT", point)
                    .output()
                    .unwrap(),
                4,
            );
            let paths = [".badr/adrs/0001-decision.md", ".badr/evidence.jsonl"];
            assert_eq!(
                result["partial_write"]["completed"],
                serde_json::json!(paths[..count])
            );
            assert_eq!(
                result["partial_write"]["not_completed"],
                serde_json::json!(paths[count..])
            );
            assert_eq!(root.join(".badr.lock").exists(), count > 0);
            assert!(result["proposed_audit_commit_message"].is_null());
            check(run(root, &["validate"]), if count == 1 { 2 } else { 0 });
        }
    }
}

#[test]
fn withdrawal_quotes_caller_markdown_and_human_output_reports_audit_details() {
    let dir = repo();
    let root = dir.path();
    ready_adr(root);
    let output = Command::new(env!("CARGO_BIN_EXE_badr"))
        .arg("--repo")
        .arg(root)
        .args([
            "reject",
            "0001",
            "--confirm",
            "--actor",
            "Human",
            "--rationale",
            "Not selected\n\n## Lifecycle notes\n```\n### 2000-01-01",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("reject: rejected"));
    assert!(text.contains("Completed writes:"));
    assert!(text.contains("Pending audit commit: Some(true)"));
    assert!(text.contains("Reject 0001"));
    check(run(root, &["validate"]), 0);
}

#[test]
fn withdrawals_refuse_markdown_boundaries_that_would_change_exact_reasoning() {
    for operation in ["reject", "deprecate"] {
        for boundary in ["missing-newline", "open-fence"] {
            let dir = repo();
            let root = dir.path();
            let path = ready_adr(root);
            let text = fs::read_to_string(&path).unwrap();
            fs::write(
                &path,
                if boundary == "missing-newline" {
                    text.trim_end_matches('\n').to_owned()
                } else {
                    text + "\n```md\n"
                },
            )
            .unwrap();
            if operation == "deprecate" {
                check(decide(root, "0001", &[]), 0);
            }
            let before = fs::read(&path).unwrap();
            let ledger = fs::read(root.join(".badr/evidence.jsonl")).unwrap();
            let result = check(withdraw(root, operation, &[]), 2);
            assert_eq!(result["partial_write"]["completed"], serde_json::json!([]));
            assert_eq!(fs::read(&path).unwrap(), before);
            assert_eq!(fs::read(root.join(".badr/evidence.jsonl")).unwrap(), ledger);
            check(run(root, &["validate"]), 0);
        }
    }
}

#[test]
fn adr_deprecation_receipts_require_gap_and_existing_code_treatment() {
    let dir = repo();
    let root = dir.path();
    ready_adr(root);
    check(decide(root, "0001", &[]), 0);
    check(withdraw(root, "deprecate", &[]), 0);
    let path = root.join(".badr/evidence.jsonl");
    let original = fs::read_to_string(&path).unwrap();
    let events: Vec<Value> = original
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    for key in ["decision_gap", "existing_code_treatment"] {
        for value in [None, Some(serde_json::json!(" \n "))] {
            let mut changed = events.clone();
            let event = changed.last_mut().unwrap();
            if let Some(value) = value {
                event[key] = value;
            } else {
                event.as_object_mut().unwrap().remove(key);
            }
            fs::write(
                &path,
                changed
                    .iter()
                    .map(|event| event.to_string() + "\n")
                    .collect::<String>(),
            )
            .unwrap();
            check(run(root, &["validate"]), 2);
        }
    }
    fs::write(&path, original).unwrap();
    check(run(root, &["validate"]), 0);
}

fn lifecycle(root: &Path, operation: &str, id: &str) -> Output {
    run(
        root,
        &[
            operation,
            id,
            "--confirm",
            "--actor",
            "Human",
            "--rationale",
            "Explicit policy choice",
        ],
    )
}

fn approved_rule(root: &Path) {
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
    fs::write(
        &path,
        fs::read_to_string(&path)
            .unwrap()
            .replace("TODO describe", "Describe"),
    )
    .unwrap();
    check(lifecycle(root, "propose", "BRR-ORD-001"), 0);
    check(decide(root, "BRR-ORD-001", &[]), 0);
}

#[test]
fn activates_approved_rule_immediately_with_exact_receipt_and_no_implicit_replacement() {
    let dir = repo();
    let root = dir.path();
    approved_rule(root);
    let prior = fs::read_to_string(root.join(".badr/evidence.jsonl")).unwrap();
    let approval: Value = serde_json::from_str(prior.lines().last().unwrap()).unwrap();
    let before = std::time::SystemTime::now();
    let result = check(lifecycle(root, "activate", "BRR-ORD-001"), 0);
    let after = std::time::SystemTime::now();
    assert_eq!(result["result"], "effective");
    assert_eq!(result["affected_ids"], serde_json::json!(["BRR-ORD-001"]));
    assert_eq!(result["pending_commit_durability"], true);
    let ledger = fs::read_to_string(root.join(".badr/evidence.jsonl")).unwrap();
    assert!(ledger.starts_with(&prior));
    let receipt: Value = serde_json::from_str(ledger.lines().last().unwrap()).unwrap();
    let at = chrono::DateTime::parse_from_rfc3339(receipt["at"].as_str().unwrap()).unwrap();
    assert!(std::time::SystemTime::from(at) >= before);
    assert!(std::time::SystemTime::from(at) <= after);
    assert_eq!(receipt["changes"][0]["effective_from"], receipt["at"]);
    assert_eq!(receipt["changes"][0]["approval_ref"], approval["event_id"]);
    assert_eq!(
        receipt["changes"][0]["body_sha256"],
        approval["changes"][0]["body_sha256"]
    );
    assert_eq!(receipt["actor"], "Human");
    assert_eq!(receipt["confirmed"], true);
    let effective = check(run(root, &["list", "--effective"]), 0);
    assert_eq!(effective["records"].as_array().unwrap().len(), 1);
    assert_eq!(effective["records"][0]["effective_from"], receipt["at"]);
    assert!(
        check(run(root, &["list", "--pending-preview"]), 0)["records"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    check(run(root, &["validate"]), 0);
    commit(root, "Activate approved policy");
    check(run(root, &["validate"]), 0);
}

#[test]
fn withdraws_effective_rules_immediately_and_surfaces_gaps_without_revival() {
    for operation in ["deprecate", "retire"] {
        let dir = repo();
        let root = dir.path();
        approved_rule(root);
        check(lifecycle(root, "activate", "BRR-ORD-001"), 0);
        let original = check(run(root, &["show", "BRR-ORD-001"]), 0)["records"][0].clone();
        let prior = fs::read_to_string(root.join(".badr/evidence.jsonl")).unwrap();
        let result = check(lifecycle(root, operation, "BRR-ORD-001"), 0);
        let status = if operation == "retire" {
            "retired"
        } else {
            "deprecated"
        };
        assert_eq!(result["result"], status);
        assert!(
            result["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == "policy_gap")
        );
        let ledger = fs::read_to_string(root.join(".badr/evidence.jsonl")).unwrap();
        assert!(ledger.starts_with(&prior));
        let receipt: Value = serde_json::from_str(ledger.lines().last().unwrap()).unwrap();
        let shown = check(run(root, &["show", "BRR-ORD-001"]), 0);
        let withdrawn = &shown["records"][0];
        assert_eq!(withdrawn["status"], status);
        assert_eq!(withdrawn["effective_from"], original["effective_from"]);
        assert_eq!(withdrawn["effective_until"], receipt["at"]);
        assert_eq!(withdrawn["body"], original["body"]);
        assert_eq!(withdrawn["supersedes"], original["supersedes"]);
        assert_eq!(withdrawn["currently_effective"], false);
        assert!(
            shown["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == "policy_gap")
        );
        for filter in ["--effective", "--pending-preview"] {
            assert!(
                check(run(root, &["list", filter]), 0)["records"]
                    .as_array()
                    .unwrap()
                    .is_empty()
            );
        }
        for illegal in ["activate", "decide", "propose", "deprecate", "retire"] {
            check(lifecycle(root, illegal, "BRR-ORD-001"), 2);
        }
        assert_eq!(
            fs::read_to_string(root.join(".badr/evidence.jsonl")).unwrap(),
            ledger
        );
        commit(root, "Record full precommit lifecycle");
        check(run(root, &["validate"]), 0);
        let output = Command::new(env!("CARGO_BIN_EXE_badr"))
            .arg("--repo")
            .arg(root)
            .args(["show", "BRR-ORD-001"])
            .output()
            .unwrap();
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("policy_gap"));
        assert!(String::from_utf8_lossy(&output.stdout).contains(status));
    }
}

#[test]
fn brr_lifecycle_refuses_illegal_states_adr_targets_and_caller_supplied_times() {
    let dir = repo();
    let root = dir.path();
    check(run(root, &["new", "adr", "storage"]), 0);
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
    for state in ["draft", "proposed", "approved", "effective"] {
        let before = fs::read(&path).unwrap();
        let ledger = fs::read(root.join(".badr/evidence.jsonl")).unwrap();
        for operation in ["activate", "deprecate", "retire"] {
            for id in ["0001", "BRR-ORD-999"] {
                check(lifecycle(root, operation, id), 2);
            }
            if state == "draft"
                || state == "proposed"
                || state == "approved" && operation != "activate"
                || state == "effective" && operation == "activate"
            {
                check(lifecycle(root, operation, "BRR-ORD-001"), 2);
            }
            for flags in [
                vec!["--actor", "Human", "--rationale", "Choice"],
                vec!["--confirm", "--actor", " ", "--rationale", "Choice"],
                vec!["--confirm", "--actor", "Human", "--rationale", " "],
                vec!["--confirm"],
            ] {
                let mut args = vec![operation, "BRR-ORD-001"];
                args.extend(flags);
                check(run(root, &args), 2);
            }
            for (flag, time) in [
                ("--effective-from", "2000-01-01T00:00:00Z"),
                ("--effective-until", "2999-01-01T00:00:00Z"),
                ("--at", "2000-01-01T00:00:00Z"),
            ] {
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
                            "Choice",
                            flag,
                            time,
                        ],
                    ),
                    2,
                );
            }
        }
        assert_eq!(fs::read(&path).unwrap(), before);
        assert_eq!(fs::read(root.join(".badr/evidence.jsonl")).unwrap(), ledger);
        match state {
            "draft" => {
                fs::write(
                    &path,
                    fs::read_to_string(&path)
                        .unwrap()
                        .replace("TODO describe", "Describe"),
                )
                .unwrap();
                check(lifecycle(root, "propose", "BRR-ORD-001"), 0);
            }
            "proposed" => {
                check(decide(root, "BRR-ORD-001", &[]), 0);
            }
            "approved" => {
                check(lifecycle(root, "activate", "BRR-ORD-001"), 0);
            }
            _ => {}
        }
    }
}

#[test]
fn activated_and_withdrawn_bodies_cannot_edit_reset_or_change_applicability_directly() {
    let dir = repo();
    let root = dir.path();
    approved_rule(root);
    git(root, &["switch", "-c", "local-rule"]);
    check(lifecycle(root, "activate", "BRR-ORD-001"), 0);
    let path = root.join(".badr/brrs/domains/ordering/rules/BRR-ORD-001-cancellation.feature");
    let supplied = root.join("revision.feature");
    for state in ["effective", "retired"] {
        let original = fs::read_to_string(&path).unwrap();
        let ledger = fs::read(root.join(".badr/evidence.jsonl")).unwrap();
        fs::write(&supplied, original.clone() + "# changed meaning\n").unwrap();
        check(
            run(
                root,
                &[
                    "edit",
                    "BRR-ORD-001",
                    "--content",
                    supplied.to_str().unwrap(),
                ],
            ),
            2,
        );
        fs::write(
            &supplied,
            original.replace(&format!("status: {state}"), "status: proposed"),
        )
        .unwrap();
        check(
            run(
                root,
                &[
                    "edit",
                    "BRR-ORD-001",
                    "--content",
                    supplied.to_str().unwrap(),
                ],
            ),
            2,
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), original);
        for changed in [
            original.clone() + "# unauthorized\n",
            original.replace(&format!("status: {state}"), "status: approved"),
            original
                .lines()
                .map(|line| {
                    if line.starts_with("effective_from:") {
                        "effective_from: \"2000-01-01T00:00:00Z\""
                    } else {
                        line
                    }
                })
                .collect::<Vec<_>>()
                .join("\n"),
        ] {
            fs::write(&path, changed).unwrap();
            check(run(root, &["validate"]), 2);
            check(lifecycle(root, "retire", "BRR-ORD-001"), 2);
            assert_eq!(fs::read(root.join(".badr/evidence.jsonl")).unwrap(), ledger);
            fs::write(&path, &original).unwrap();
        }
        if state == "effective" {
            check(lifecycle(root, "retire", "BRR-ORD-001"), 0);
        }
    }
}

#[test]
fn lifecycle_partial_failures_report_exact_paths_and_retain_repair_lock() {
    for operation in ["activate", "deprecate", "retire"] {
        let dir = repo();
        let root = dir.path();
        approved_rule(root);
        if operation != "activate" {
            check(lifecycle(root, "activate", "BRR-ORD-001"), 0);
        }
        let ledger = fs::read(root.join(".badr/evidence.jsonl")).unwrap();
        fs::write(root.join("unrelated.txt"), "preserve me").unwrap();
        let out = Command::new(env!("CARGO_BIN_EXE_badr"))
            .arg("--repo")
            .arg(root)
            .arg("--json")
            .args([
                operation,
                "BRR-ORD-001",
                "--confirm",
                "--actor",
                "Human",
                "--rationale",
                "Choice",
            ])
            .env("BADR_TEST_FAIL_AT", "after_write_0")
            .output()
            .unwrap();
        let result = check(out, 4);
        assert_eq!(
            result["partial_write"]["completed"],
            serde_json::json!([
                ".badr/brrs/domains/ordering/rules/BRR-ORD-001-cancellation.feature"
            ])
        );
        assert_eq!(
            result["partial_write"]["not_completed"],
            serde_json::json!([".badr/evidence.jsonl"])
        );
        assert!(result["proposed_audit_commit_message"].is_null());
        assert!(root.join(".badr.lock").exists());
        assert_eq!(fs::read(root.join(".badr/evidence.jsonl")).unwrap(), ledger);
        assert_eq!(
            fs::read_to_string(root.join("unrelated.txt")).unwrap(),
            "preserve me"
        );
        check(lifecycle(root, operation, "BRR-ORD-001"), 4);
        check(run(root, &["validate"]), 2);
    }
}

#[test]
fn lifecycle_requires_complete_history_and_valid_whole_collection_without_writes() {
    let dir = repo();
    let root = dir.path();
    approved_rule(root);
    let config = fs::read(root.join(".badr/config.yaml")).unwrap();
    let path = root.join(".badr/brrs/domains/ordering/rules/BRR-ORD-001-cancellation.feature");
    for operation in ["activate", "deprecate", "retire"] {
        let record = fs::read(&path).unwrap();
        let ledger = fs::read(root.join(".badr/evidence.jsonl")).unwrap();
        fs::write(
            root.join(".badr/config.yaml"),
            "version: 1\nbaseline_branch: missing\n",
        )
        .unwrap();
        let result = check(lifecycle(root, operation, "BRR-ORD-001"), 3);
        assert!(
            result["partial_write"]["completed"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        fs::write(root.join(".badr/config.yaml"), &config).unwrap();
        fs::create_dir_all(root.join(".badr/adrs")).unwrap();
        fs::write(
            root.join(".badr/adrs/0001-invalid.md"),
            "invalid unrelated ADR",
        )
        .unwrap();
        check(lifecycle(root, operation, "BRR-ORD-001"), 2);
        fs::remove_file(root.join(".badr/adrs/0001-invalid.md")).unwrap();
        assert_eq!(fs::read(&path).unwrap(), record);
        assert_eq!(fs::read(root.join(".badr/evidence.jsonl")).unwrap(), ledger);
        assert!(!root.join(".badr.lock").exists());
        if operation == "activate" {
            check(lifecycle(root, "activate", "BRR-ORD-001"), 0);
        }
    }
}

#[test]
fn shared_deprecation_preserves_brr_calls_and_rejects_adr_only_declarations() {
    let dir = repo();
    let root = dir.path();
    approved_rule(root);
    check(lifecycle(root, "activate", "BRR-ORD-001"), 0);
    let path = root.join(".badr/brrs/domains/ordering/rules/BRR-ORD-001-cancellation.feature");
    let before = fs::read(&path).unwrap();
    let ledger = fs::read(root.join(".badr/evidence.jsonl")).unwrap();
    for extra in [
        vec!["--decision-gap", "None"],
        vec!["--existing-code-treatment", "Retain"],
        vec![
            "--decision-gap",
            "None",
            "--existing-code-treatment",
            "Retain",
        ],
    ] {
        let mut args = vec![
            "deprecate",
            "BRR-ORD-001",
            "--confirm",
            "--actor",
            "Human",
            "--rationale",
            "Withdraw policy",
        ];
        args.extend(extra);
        check(run(root, &args), 2);
        assert_eq!(fs::read(&path).unwrap(), before);
        assert_eq!(fs::read(root.join(".badr/evidence.jsonl")).unwrap(), ledger);
    }
    check(lifecycle(root, "deprecate", "BRR-ORD-001"), 0);
    check(run(root, &["validate"]), 0);
}

fn replacement(root: &Path, predecessors: &[&str], successors: &[&str], extra: &[&str]) -> Output {
    let mut args = vec![
        "supersede",
        "--complete",
        "--confirm",
        "--actor",
        "Human",
        "--rationale",
        "Complete replacement reviewed",
    ];
    for id in predecessors {
        args.extend(["--predecessor", id]);
    }
    for id in successors {
        args.extend(["--successor", id]);
    }
    args.extend_from_slice(extra);
    run(root, &args)
}
fn replacement_records(root: &Path, count: usize) {
    for n in 1..=count {
        check(run(root, &["new", "adr", &format!("choice-{n}")]), 0);
        let path = format!(".badr/adrs/{n:04}-choice-{n}.md");
        let input = root.join("input.md");
        fs::write(
            &input,
            fs::read_to_string(root.join(path))
                .unwrap()
                .replace("TODO:", "Reviewed:"),
        )
        .unwrap();
        check(
            run(
                root,
                &[
                    "edit",
                    &format!("{n:04}"),
                    "--content",
                    input.to_str().unwrap(),
                ],
            ),
            0,
        );
    }
}
#[test]
fn explicitly_replaces_an_approved_adr_and_preserves_reasoning_and_evidence() {
    let dir = repo();
    let root = dir.path();
    replacement_records(root, 2);
    check(decide(root, "0001", &[]), 0);
    check(decide(root, "0002", &[]), 0);
    let before = check(run(root, &["show", "0001"]), 0);
    let ledger = fs::read(root.join(".badr/evidence.jsonl")).unwrap();
    let result = check(replacement(root, &["0001"], &["0002"], &[]), 0);
    assert_eq!(result["operation"], "supersede");
    assert_eq!(result["result"], "superseded");
    assert_eq!(result["affected_ids"], serde_json::json!(["0001", "0002"]));
    let old = check(run(root, &["show", "0001"]), 0);
    let new = check(run(root, &["show", "0002"]), 0);
    assert_eq!(old["records"][0]["status"], "superseded");
    assert_eq!(old["records"][0]["directs_new_work"], false);
    assert_eq!(
        old["records"][0]["decision_body"],
        before["records"][0]["decision_body"]
    );
    assert_eq!(
        old["records"][0]["superseded_by"],
        serde_json::json!(["0002"])
    );
    assert_eq!(new["records"][0]["supersedes"], serde_json::json!(["0001"]));
    assert!(
        fs::read(root.join(".badr/evidence.jsonl"))
            .unwrap()
            .starts_with(&ledger)
    );
    check(run(root, &["validate"]), 0);
}

#[test]
fn replacement_coordinates_proposed_and_approved_successors_in_one_receipt() {
    let dir = repo();
    let root = dir.path();
    replacement_records(root, 3);
    check(decide(root, "0001", &[]), 0);
    check(decide(root, "0002", &[]), 0);
    let previous: Value = serde_json::from_str(
        fs::read_to_string(root.join(".badr/evidence.jsonl"))
            .unwrap()
            .lines()
            .last()
            .unwrap(),
    )
    .unwrap();
    check(
        replacement(
            root,
            &["0001"],
            &["0002", "0003"],
            &["--approve-successors", "--reviewed"],
        ),
        0,
    );
    let ledger = fs::read_to_string(root.join(".badr/evidence.jsonl")).unwrap();
    let receipt: Value = serde_json::from_str(ledger.lines().last().unwrap()).unwrap();
    assert_eq!(receipt["operation"], "decide");
    assert_eq!(receipt["reviewed"], true);
    assert_eq!(
        receipt["replacement"],
        serde_json::json!({"predecessors":["0001"],"successors":["0002","0003"],"complete":true})
    );
    assert_eq!(receipt["changes"][1]["approval_ref"], previous["event_id"]);
    assert_eq!(receipt["changes"][2]["approval_ref"], receipt["event_id"]);
    assert_eq!(
        check(run(root, &["list", "--directing"]), 0)["records"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    commit(root, "Complete replacement group");
    check(run(root, &["validate"]), 0);
}

#[test]
fn replacement_consolidates_deprecated_predecessors_retains_notes_and_never_revives() {
    let dir = repo();
    let root = dir.path();
    git(root, &["switch", "-c", "replacement"]);
    replacement_records(root, 3);
    for id in ["0001", "0002", "0003"] {
        check(decide(root, id, &[]), 0);
    }
    check(withdraw(root, "deprecate", &[]), 0);
    let previous = fs::read_to_string(root.join(".badr/adrs/0001-choice-1.md")).unwrap();
    check(replacement(root, &["0002", "0001"], &["0003"], &[]), 0);
    let changed = fs::read_to_string(root.join(".badr/adrs/0001-choice-1.md")).unwrap();
    let notes = previous.split_once("## Lifecycle notes").unwrap().1;
    assert!(
        changed
            .split_once("## Lifecycle notes")
            .unwrap()
            .1
            .starts_with(notes)
    );
    assert_eq!(
        check(run(root, &["show", "0003"]), 0)["records"][0]["supersedes"],
        serde_json::json!(["0001", "0002"])
    );
    // Neither side may escape completed replacement by resetting its approval.
    for n in 1..=3 {
        let input = root.join("input.md");
        fs::write(
            &input,
            fs::read_to_string(root.join(format!(".badr/adrs/{n:04}-choice-{n}.md")))
                .unwrap()
                .replace("Reviewed:", "Changed:"),
        )
        .unwrap();
        check(
            run(
                root,
                &[
                    "edit",
                    &format!("{n:04}"),
                    "--content",
                    input.to_str().unwrap(),
                ],
            ),
            2,
        );
        check(decide(root, &format!("{n:04}"), &[]), 2);
    }
    check(
        run(
            root,
            &[
                "deprecate",
                "0003",
                "--confirm",
                "--actor",
                "Human",
                "--rationale",
                "Withdraw successor",
                "--decision-gap",
                "Explicit gap",
                "--existing-code-treatment",
                "Retain",
            ],
        ),
        0,
    );
    assert_eq!(
        check(run(root, &["list", "--directing"]), 0)["records"],
        serde_json::json!([])
    );
    let old = check(run(root, &["show", "0001"]), 0);
    assert_eq!(old["records"][0]["status"], "superseded");
    assert_eq!(
        old["records"][0]["superseded_by"],
        serde_json::json!(["0003"])
    );
    let third = check(run(root, &["show", "0003"]), 0);
    assert_eq!(
        third["records"][0]["supersedes"],
        serde_json::json!(["0001", "0002"])
    );
    check(replacement(root, &["0003"], &["0001"], &[]), 2);
    check(run(root, &["validate"]), 0);
}

#[test]
fn replacement_refuses_incomplete_declarations_invalid_targets_and_unapproved_successors_without_writes()
 {
    let dir = repo();
    let root = dir.path();
    replacement_records(root, 2);
    check(decide(root, "0001", &[]), 0);
    let snapshot = fs::read(root.join(".badr/evidence.jsonl")).unwrap();
    for (old, new, extra) in [
        (vec!["0001"], vec!["0002"], vec![]),
        (vec!["0002"], vec!["0001"], vec![]),
        (vec!["0001"], vec!["9999"], vec![]),
        (vec!["0001"], vec!["00001"], vec![]),
        (vec!["0001"], vec!["BRR-ORD-001"], vec![]),
        (vec!["0001"], vec!["0001"], vec![]),
        (vec!["0001", "0001"], vec!["0002"], vec![]),
        (vec!["0001"], vec!["0002", "0002"], vec![]),
        (vec!["0001"], vec!["0002"], vec!["--approve-successors"]),
        (vec!["0001"], vec!["0002"], vec!["--reviewed"]),
    ] {
        check(replacement(root, &old, &new, &extra), 2);
    }
    for missing in [
        "--complete",
        "--confirm",
        "--actor",
        "--rationale",
        "--predecessor",
        "--successor",
        "--reviewed",
    ] {
        let mut args = vec![
            "supersede",
            "--predecessor",
            "0001",
            "--successor",
            "0002",
            "--complete",
            "--confirm",
            "--actor",
            "Human",
            "--rationale",
            "Replacement",
            "--approve-successors",
            "--reviewed",
        ];
        let index = args.iter().position(|s| *s == missing).unwrap();
        args.remove(index);
        if ["--actor", "--rationale", "--predecessor", "--successor"].contains(&missing) {
            args.remove(index);
        }
        check(run(root, &args), 2);
    }
    assert_eq!(
        fs::read(root.join(".badr/evidence.jsonl")).unwrap(),
        snapshot
    );
    assert!(!root.join(".badr.lock").exists());
    check(run(root, &["validate"]), 0);
    // Coordinated approval uses the same unresolved-gap checks as ordinary decide.
    let input = root.join("input.md");
    fs::write(
        &input,
        fs::read_to_string(root.join(".badr/adrs/0002-choice-2.md"))
            .unwrap()
            .replace("Reviewed:", "TODO:"),
    )
    .unwrap();
    check(
        run(
            root,
            &["edit", "0002", "--content", input.to_str().unwrap()],
        ),
        0,
    );
    check(
        replacement(
            root,
            &["0001"],
            &["0002"],
            &["--approve-successors", "--reviewed"],
        ),
        2,
    );
}

#[test]
fn replacement_faults_report_exact_group_progress_and_retain_repair_lock() {
    let paths = [
        ".badr/adrs/0001-choice-1.md",
        ".badr/adrs/0002-choice-2.md",
        ".badr/evidence.jsonl",
    ];
    for (point, count) in [
        ("before_apply", 0),
        ("after_write_0", 1),
        ("after_write_1", 2),
        ("after_write_2", 3),
        ("before_postvalidate", 3),
        ("after_postvalidate", 3),
    ] {
        let dir = repo();
        let root = dir.path();
        replacement_records(root, 2);
        check(decide(root, "0001", &[]), 0);
        let result = check(
            Command::new(env!("CARGO_BIN_EXE_badr"))
                .arg("--repo")
                .arg(root)
                .arg("--json")
                .args([
                    "supersede",
                    "--predecessor",
                    "0001",
                    "--successor",
                    "0002",
                    "--complete",
                    "--approve-successors",
                    "--reviewed",
                    "--confirm",
                    "--actor",
                    "Human",
                    "--rationale",
                    "Replacement",
                ])
                .env("BADR_TEST_FAIL_AT", point)
                .output()
                .unwrap(),
            4,
        );
        assert_eq!(
            result["partial_write"]["completed"],
            serde_json::json!(paths[..count])
        );
        assert_eq!(
            result["partial_write"]["not_completed"],
            serde_json::json!(paths[count..])
        );
        assert_eq!(result["success"], false);
        assert_eq!(root.join(".badr.lock").exists(), count > 0);
        if count > 0 {
            check(replacement(root, &["0001"], &["0002"], &[]), 4);
        }
    }
}

#[test]
fn replacement_approves_split_successors_and_retains_earlier_replacement_edges() {
    let dir = repo();
    let root = dir.path();
    replacement_records(root, 4);
    check(decide(root, "0001", &[]), 0);
    check(
        replacement(
            root,
            &["0001"],
            &["0002", "0003"],
            &["--approve-successors", "--reviewed"],
        ),
        0,
    );
    let second = fs::read_to_string(root.join(".badr/adrs/0002-choice-2.md")).unwrap();
    check(decide(root, "0004", &[]), 0);
    let output = Command::new(env!("CARGO_BIN_EXE_badr"))
        .arg("--repo")
        .arg(root)
        .args([
            "supersede",
            "--predecessor",
            "0002",
            "--successor",
            "0004",
            "--complete",
            "--confirm",
            "--actor",
            "Human",
            "--rationale",
            "Complete successor replacement",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("supersede: superseded\n"));
    assert!(stdout.contains("Completed writes:"));
    assert!(stdout.contains("Pending audit commit: Some(true)"));
    let after = check(run(root, &["show", "0002"]), 0);
    assert_eq!(
        after["records"][0]["supersedes"],
        serde_json::json!(["0001"])
    );
    assert_eq!(
        after["records"][0]["superseded_by"],
        serde_json::json!(["0004"])
    );
    let changed = fs::read_to_string(root.join(".badr/adrs/0002-choice-2.md")).unwrap();
    assert!(
        changed
            .split_once("## Lifecycle notes")
            .unwrap()
            .1
            .starts_with(second.split_once("## Lifecycle notes").unwrap().1)
    );
    let before = fs::read(root.join(".badr/evidence.jsonl")).unwrap();
    check(replacement(root, &["0004"], &["0001"], &[]), 2);
    assert_eq!(fs::read(root.join(".badr/evidence.jsonl")).unwrap(), before);
    commit(root, "Retain chained replacement history");
    check(run(root, &["validate"]), 0);
}
