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
fn registers_domain_with_governed_evidence_and_pending_audit() {
    let dir = repo();
    let root = dir.path();
    fs::write(root.join("unrelated.txt"), "keep me").unwrap();
    let result = check(run(root, ADD), 0);
    assert_eq!(result["operation"], "domain_add");
    assert_eq!(result["validation_coverage"]["history"], "passed");
    assert_eq!(result["pending_commit_durability"], true);
    assert!(
        result["proposed_audit_commit_message"]
            .as_str()
            .unwrap()
            .contains("ordering")
    );
    assert_eq!(
        result["partial_write"]["completed"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    let registry: Value =
        yaml_serde::from_str(&fs::read_to_string(root.join(".badr/brrs/registry.yaml")).unwrap())
            .unwrap();
    assert_eq!(registry["domains"]["ordering"], "ORD");
    assert_eq!(registry["rules"], serde_json::json!({}));
    assert!(
        root.join(".badr/brrs/domains/ordering/glossary.md")
            .is_file()
    );
    assert!(!root.join(".badr/brrs/domains/ordering/rules").exists());
    let event: Value = serde_json::from_str(
        fs::read_to_string(root.join(".badr/evidence.jsonl"))
            .unwrap()
            .trim(),
    )
    .unwrap();
    assert_eq!(event["operation"], "domain_add");
    assert_eq!(event["actor"], "Human");
    assert_eq!(event["confirmed"], true);
    assert_eq!(event["domain"]["slug"], "ordering");
    assert_eq!(event["baseline_commit"], git(root, &["rev-parse", "trunk"]));
    check(run(root, &["validate"]), 0);
    assert_eq!(
        fs::read_to_string(root.join("unrelated.txt")).unwrap(),
        "keep me"
    );
    assert!(!root.join(".badr.lock").exists());
    assert_eq!(git(root, &["rev-list", "--count", "HEAD"]), "1");
}

#[test]
fn rejects_missing_authorization_duplicates_and_unrelated_invalidity() {
    let dir = repo();
    let root = dir.path();
    check(
        run(
            root,
            &[
                "domain",
                "add",
                "ordering",
                "--prefix",
                "ORD",
                "--actor",
                "Human",
                "--rationale",
                "Reviewed",
            ],
        ),
        2,
    );
    assert!(!root.join(".badr/brrs").exists());
    check(run(root, ADD), 0);
    let original = fs::read(root.join(".badr/evidence.jsonl")).unwrap();
    check(run(root, ADD), 2);
    check(
        run(
            root,
            &[
                "domain",
                "add",
                "billing",
                "--prefix",
                "ORD",
                "--confirm",
                "--actor",
                "Human",
                "--rationale",
                "Reviewed",
            ],
        ),
        2,
    );
    assert_eq!(
        fs::read(root.join(".badr/evidence.jsonl")).unwrap(),
        original
    );
    fs::create_dir(root.join(".badr/adrs")).unwrap();
    fs::write(root.join(".badr/adrs/0001-invalid.md"), "invalid").unwrap();
    check(
        run(
            root,
            &[
                "domain",
                "add",
                "billing",
                "--prefix",
                "BILL",
                "--confirm",
                "--actor",
                "Human",
                "--rationale",
                "Reviewed",
            ],
        ),
        2,
    );
    assert!(!root.join(".badr/brrs/domains/billing").exists());
}

#[test]
fn reports_every_write_boundary_and_retains_known_failure_lock() {
    let paths = [
        ".badr/brrs/domains/ordering/glossary.md",
        ".badr/brrs/registry.yaml",
        ".badr/evidence.jsonl",
    ];
    for (point, count) in [
        ("before_write_0", 0),
        ("after_write_0", 1),
        ("before_write_1", 1),
        ("after_write_1", 2),
        ("before_write_2", 2),
        ("after_write_2", 3),
        ("before_postvalidate", 3),
        ("after_postvalidate", 3),
    ] {
        let dir = repo();
        let root = dir.path();
        let out = Command::new(env!("CARGO_BIN_EXE_badr"))
            .arg("--repo")
            .arg(root)
            .arg("--json")
            .args(ADD)
            .env("BADR_TEST_FAIL_AT", point)
            .output()
            .unwrap();
        let result = check(out, 4);
        assert_eq!(
            result["partial_write"]["completed"],
            serde_json::json!(paths[..count]),
            "{point}"
        );
        assert_eq!(
            result["partial_write"]["not_completed"],
            serde_json::json!(paths[count..]),
            "{point}"
        );
        if count > 0 {
            assert!(root.join(".badr.lock").is_file());
            let blocked = check(run(root, ADD), 4);
            assert_eq!(blocked["diagnostics"][0]["code"], "write_locked");
        } else {
            assert!(!root.join(".badr.lock").exists());
        }
        if count == 3 {
            check(run(root, &["validate"]), 0);
        }
    }
}

fn paused(root: &Path, point: &str, control: &Path) -> std::process::Child {
    let child = Command::new(env!("CARGO_BIN_EXE_badr"))
        .arg("--repo")
        .arg(root)
        .arg("--json")
        .args(ADD)
        .env("BADR_TEST_PAUSE_AT", point)
        .env("BADR_TEST_CONTROL", control)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let start = std::time::Instant::now();
    while !control.join("ready").exists() {
        assert!(
            start.elapsed().as_secs() < 15,
            "writer never reached {point}"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    child
}
fn resume(child: std::process::Child, control: &Path, exit: i32) -> Value {
    fs::write(control.join("resume"), "continue").unwrap();
    check(child.wait_with_output().unwrap(), exit)
}
#[test]
fn refuses_external_edits_baseline_changes_and_concurrent_writers() {
    for mutation in ["external", "baseline", "concurrent", "lock", "symlink"] {
        let dir = repo();
        let root = dir.path();
        let control = tempfile::tempdir().unwrap();
        let writer = paused(root, "before_apply", control.path());
        match mutation {
            "external" => fs::write(
                root.join(".badr/config.yaml"),
                "version: 1\nbaseline_branch: trunk\n# external edit\n",
            )
            .unwrap(),
            "baseline" => {
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
                        "move baseline",
                    ],
                );
            }
            "concurrent" => {
                check(run(root, ADD), 4);
            }
            "lock" => {
                fs::remove_file(root.join(".badr.lock")).unwrap();
                fs::write(root.join(".badr.lock"), "replacement lock").unwrap();
            }
            "symlink" => {
                std::os::unix::fs::symlink(control.path(), root.join(".badr/brrs")).unwrap();
            }
            _ => unreachable!(),
        }
        let result = resume(
            writer,
            control.path(),
            match mutation {
                "concurrent" => 0,
                "symlink" => 2,
                _ => 4,
            },
        );
        if mutation != "concurrent" {
            assert_eq!(result["partial_write"]["completed"], serde_json::json!([]));
        }
        assert!(!control.path().join("domains").exists());
        if mutation == "lock" {
            assert_eq!(
                fs::read_to_string(root.join(".badr.lock")).unwrap(),
                "replacement lock"
            );
        }
    }
}

#[test]
fn detects_external_edits_after_each_publication_without_overwriting_them() {
    for point in [
        "after_write_0",
        "after_write_1",
        "after_write_2",
        "before_postvalidate",
        "after_postvalidate",
    ] {
        let dir = repo();
        let root = dir.path();
        let control = tempfile::tempdir().unwrap();
        let writer = paused(root, point, control.path());
        let path = root.join(".badr/brrs/domains/ordering/glossary.md");
        fs::write(&path, "external glossary edit").unwrap();
        resume(writer, control.path(), 4);
        assert_eq!(fs::read_to_string(path).unwrap(), "external glossary edit");
        assert!(root.join(".badr.lock").is_file());
        check(run(root, ADD), 4);
    }
}

#[test]
fn refuses_active_and_stale_locks_missing_history_and_unsafe_storage() {
    for problem in [
        "active",
        "stale",
        "missing",
        "shallow",
        "unrelated",
        "symlink",
        "path",
    ] {
        let dir = repo();
        let root = dir.path();
        match problem {
            "active" | "stale" => fs::write(
                root.join(".badr.lock"),
                if problem == "active" {
                    "pid=1"
                } else {
                    "pid=999999999"
                },
            )
            .unwrap(),
            "missing" => fs::write(
                root.join(".badr/config.yaml"),
                "version: 1\nbaseline_branch: missing\n",
            )
            .unwrap(),
            "shallow" => {
                fs::write(
                    root.join(".git/shallow"),
                    format!("{}\n", git(root, &["rev-parse", "HEAD"])),
                )
                .unwrap();
            }
            "unrelated" => {
                fs::create_dir_all(root.join(".badr/brrs/domains/ordering")).unwrap();
                fs::write(root.join(".badr/brrs/domains/ordering/glossary.md"), "keep").unwrap();
            }
            "symlink" => {
                std::os::unix::fs::symlink(dir.path(), root.join(".badr/brrs")).unwrap();
            }
            "path" => {
                check(
                    run(
                        root,
                        &[
                            "domain",
                            "add",
                            "../escape",
                            "--prefix",
                            "ORD",
                            "--confirm",
                            "--actor",
                            "Human",
                            "--rationale",
                            "Reviewed",
                        ],
                    ),
                    2,
                );
                continue;
            }
            _ => unreachable!(),
        }
        check(
            run(root, ADD),
            match problem {
                "active" | "stale" => 4,
                "missing" | "shallow" => 3,
                _ => 2,
            },
        );
        assert!(!root.join(".badr/evidence.jsonl").exists());
    }
}

#[test]
fn retains_domain_identity_in_evidence_and_history() {
    let dir = repo();
    let root = dir.path();
    check(run(root, ADD), 0);
    let out = check(
        run(
            root,
            &[
                "domain",
                "add",
                "billing",
                "--prefix",
                "BILL",
                "--confirm",
                "--actor",
                "Human",
                "--rationale",
                "Reviewed",
            ],
        ),
        0,
    );
    assert_eq!(
        out["partial_write"]["completed"].as_array().unwrap().len(),
        3
    );
    check(run(root, &["validate"]), 0);
    fs::write(
        root.join(".badr/brrs/registry.yaml"),
        "version: 1\ndomains: {ordering: XYZ, billing: BILL}\nrules: {}\n",
    )
    .unwrap();
    let report = check(run(root, &["validate"]), 2);
    assert!(
        report["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "domain_identity_changed")
    );
}

#[test]
fn cannot_reassign_a_prefix_reserved_by_a_deleted_historical_registry() {
    let dir = repo();
    let root = dir.path();
    fs::create_dir_all(root.join(".badr/brrs/domains/old")).unwrap();
    fs::write(root.join(".badr/brrs/domains/old/glossary.md"), "# Old\n").unwrap();
    fs::write(
        root.join(".badr/brrs/registry.yaml"),
        "version: 1\ndomains: {old: ORD}\nrules: {}\n",
    )
    .unwrap();
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
            "reserve old prefix",
        ],
    );
    fs::remove_dir_all(root.join(".badr/brrs")).unwrap();
    check(run(root, &["validate"]), 0);
    let report = check(run(root, ADD), 2);
    assert!(
        report["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "domain_identity_changed")
    );
    assert!(!root.join(".badr/evidence.jsonl").exists());
}

#[test]
fn historical_registration_cannot_disappear_in_an_intermediate_commit() {
    let dir = repo();
    let root = dir.path();
    check(run(root, ADD), 0);
    let registry = fs::read(root.join(".badr/brrs/registry.yaml")).unwrap();
    let commit = || {
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
                "snapshot",
            ],
        );
    };
    commit();
    fs::write(
        root.join(".badr/brrs/registry.yaml"),
        "version: 1\ndomains: {}\nrules: {}\n",
    )
    .unwrap();
    commit();
    fs::write(root.join(".badr/brrs/registry.yaml"), registry).unwrap();
    let report = check(run(root, &["validate"]), 2);
    assert!(
        report["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "removed_domain")
    );
}

#[test]
fn postwrite_validation_failure_is_operational_even_after_all_writes() {
    let dir = repo();
    let root = dir.path();
    let control = tempfile::tempdir().unwrap();
    let writer = paused(root, "validate_written", control.path());
    fs::create_dir(root.join(".badr/adrs")).unwrap();
    fs::write(root.join(".badr/adrs/0001-invalid.md"), "invalid").unwrap();
    let result = resume(writer, control.path(), 4);
    assert_eq!(
        result["partial_write"]["completed"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert_eq!(
        result["partial_write"]["not_completed"],
        serde_json::json!([])
    );
    assert!(
        result["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "manual_repair_required")
    );
    assert!(root.join(".badr.lock").exists());
    check(run(root, ADD), 4);
}

#[test]
fn same_bytes_in_an_unexpected_replacement_are_not_accepted() {
    let dir = repo();
    let root = dir.path();
    let control = tempfile::tempdir().unwrap();
    let writer = paused(root, "before_apply", control.path());
    let path = root.join(".badr/config.yaml");
    let old = fs::read(&path).unwrap();
    fs::rename(&path, root.join("old-config.yaml")).unwrap();
    fs::write(path, old).unwrap();
    let result = resume(writer, control.path(), 4);
    assert_eq!(result["partial_write"]["completed"], serde_json::json!([]));
    assert!(!root.join(".badr/brrs").exists());
}

#[test]
fn preserves_existing_records_and_reports_each_once() {
    let dir = repo();
    let root = dir.path();
    fs::create_dir(root.join(".badr/adrs")).unwrap();
    fs::write(root.join(".badr/adrs/0001-storage.md"), "---\nid: \"0001\"\nstatus: proposed\ndate: \"2020-01-01\"\n---\n# Storage\n\n## Context and Problem Statement\nNeed durable storage.\n\n## Decision Drivers\nSimplicity.\n\n## Considered Options\nFiles or database.\n\n## Decision Outcome\nFiles because Git provides history.\n\n### Consequences\nReviewable changes.\n").unwrap();
    let event = serde_json::json!({"version":1,"event_id":"existing","operation":"allocate","at":"2020-01-01T00:00:00Z","branch":"trunk","baseline_commit":git(root, &["rev-parse", "trunk"]),"changes":[{"id":"0001","path":".badr/adrs/0001-storage.md","from":null,"to":"proposed","body_sha256":"4504aae16439932a5613e7099c28c399399de749dc50ac6416d3132a52b3b167","date":"2020-01-01","notes_sha256":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"}]});
    let ledger = format!("{event}\n");
    fs::write(root.join(".badr/evidence.jsonl"), &ledger).unwrap();
    check(run(root, &["validate"]), 0);
    let before = fs::read(root.join(".badr/adrs/0001-storage.md")).unwrap();
    let report = check(run(root, ADD), 0);
    assert_eq!(report["records"].as_array().unwrap().len(), 1);
    assert_eq!(
        fs::read(root.join(".badr/adrs/0001-storage.md")).unwrap(),
        before
    );
    assert!(
        fs::read_to_string(root.join(".badr/evidence.jsonl"))
            .unwrap()
            .starts_with(&ledger)
    );
}
