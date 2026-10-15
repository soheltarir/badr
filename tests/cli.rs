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
    String::from_utf8(out.stdout).unwrap().trim().to_owned()
}
fn repo(committed: bool) -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-b", "trunk"]);
    if committed {
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
    }
    dir
}
fn run(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_badr"))
        .arg("--repo")
        .arg(root)
        .args(args)
        .output()
        .unwrap()
}
fn json(root: &Path, args: &[&str], code: i32) -> Value {
    let mut args = args.to_vec();
    args.push("--json");
    let out = run(root, &args);
    assert_eq!(
        out.status.code(),
        Some(code),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        out.stderr.is_empty(),
        "JSON diagnostics belong in the report"
    );
    serde_json::from_slice(&out.stdout).unwrap()
}

#[test]
fn initializes_explicit_non_main_baseline_without_unrelated_writes() {
    let dir = repo(true);
    fs::write(dir.path().join("notes.txt"), "keep me").unwrap();
    let before = git(dir.path(), &["rev-parse", "HEAD"]);
    let result = json(dir.path(), &["init", "--baseline", "trunk"], 0);
    assert_eq!(result["operation"], "init");
    assert_eq!(result["result"], "created");
    assert_eq!(result["baseline_commit"], before);
    assert_eq!(result["pending_commit_durability"], true);
    assert_eq!(
        fs::read_to_string(dir.path().join(".badr/config.yaml")).unwrap(),
        "version: 1\nbaseline_branch: trunk\n"
    );
    assert_eq!(fs::read_dir(dir.path().join(".badr")).unwrap().count(), 1);
    assert_eq!(
        fs::read_to_string(dir.path().join("notes.txt")).unwrap(),
        "keep me"
    );
    assert_eq!(git(dir.path(), &["rev-parse", "HEAD"]), before);
    assert!(!dir.path().join("AGENTS.md").exists());
}

#[test]
fn empty_inspection_is_deterministic_read_only_and_reports_coverage() {
    let dir = repo(true);
    json(dir.path(), &["init", "--baseline", "trunk"], 0);
    let before = git(
        dir.path(),
        &["status", "--porcelain", "--untracked-files=all"],
    );
    for operation in ["list", "validate"] {
        let result = json(dir.path(), &[operation], 0);
        assert_eq!(result["records"], serde_json::json!([]));
        assert_eq!(
            result["validation_coverage"],
            serde_json::json!({"configuration":"passed","collection":"passed","evidence":"passed","history":"passed"})
        );
        assert_eq!(
            run(dir.path(), &[operation, "--json"]).stdout,
            run(dir.path(), &[operation, "--json"]).stdout
        );
        let human = run(dir.path(), &[operation]);
        assert!(human.status.success());
        assert!(human.stderr.is_empty());
    }
    assert_eq!(
        git(
            dir.path(),
            &["status", "--porcelain", "--untracked-files=all"]
        ),
        before
    );
    git(dir.path(), &["add", ".badr/config.yaml"]);
    git(
        dir.path(),
        &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "commit",
            "-m",
            "config",
        ],
    );
    assert_eq!(
        json(dir.path(), &["validate"], 0)["pending_commit_durability"],
        false
    );
    assert_eq!(
        json(dir.path(), &["init", "--baseline", "trunk"], 0)["pending_commit_durability"],
        false
    );
}

#[test]
fn repeat_init_preserves_matching_bytes_and_refuses_conflicts() {
    let dir = repo(true);
    json(dir.path(), &["init", "--baseline", "trunk"], 0);
    let config = dir.path().join(".badr/config.yaml");
    let custom = "# preserve this comment\nbaseline_branch: 'trunk'\nversion: 1\n";
    fs::write(&config, custom).unwrap();
    assert_eq!(
        json(dir.path(), &["init", "--baseline", "trunk"], 0)["result"],
        "unchanged"
    );
    json(dir.path(), &["init", "--baseline", "main"], 2);
    assert_eq!(fs::read_to_string(config).unwrap(), custom);
}

#[test]
fn missing_and_unborn_baselines_allow_only_initial_configuration() {
    for committed in [false, true] {
        let dir = repo(committed);
        let branch = if committed { "missing" } else { "trunk" };
        let result = json(dir.path(), &["init", "--baseline", branch], 3);
        assert_eq!(result["success"], false);
        assert!(result["baseline_commit"].is_null());
        assert_eq!(result["diagnostics"][0]["code"], "baseline_unavailable");
        assert!(
            result["diagnostics"][0]["message"]
                .as_str()
                .unwrap()
                .contains("Create and commit")
        );
        assert!(dir.path().join(".badr/config.yaml").is_file());
        for operation in ["list", "validate"] {
            assert_eq!(
                json(dir.path(), &[operation], 3)["validation_coverage"]["history"],
                "incomplete"
            );
        }
    }
}

#[test]
fn arguments_non_repository_and_existing_data_are_not_overwritten() {
    let dir = tempfile::tempdir().unwrap();
    json(dir.path(), &["init", "--baseline", "main"], 2);
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
    let dir = repo(true);
    for args in [
        vec!["init"],
        vec!["bogus"],
        vec!["init", "--baseline", "../escape"],
        vec!["init", "--baseline", "@{-1}"],
    ] {
        json(dir.path(), &args, 2);
        assert!(!dir.path().join(".badr").exists());
    }
    fs::create_dir(dir.path().join(".badr")).unwrap();
    fs::write(dir.path().join(".badr/keep"), "unrelated").unwrap();
    json(dir.path(), &["init", "--baseline", "trunk"], 2);
    assert!(!dir.path().join(".badr/config.yaml").exists());
    assert_eq!(
        fs::read_to_string(dir.path().join(".badr/keep")).unwrap(),
        "unrelated"
    );
}

#[test]
fn malformed_configuration_is_never_blessed_or_overwritten() {
    let dir = repo(true);
    json(dir.path(), &["init", "--baseline", "trunk"], 0);
    for text in [
        "version: 2\nbaseline_branch: trunk\n",
        "version: 1\nversion: 1\nbaseline_branch: trunk\n",
        "version: 1\nbaseline_branch: trunk\nunknown: true\n",
        "version: 1\nbaseline_branch: !unsafe trunk\n",
        "version: 1\nbaseline_branch: [trunk]\n",
        "version: 1\nbaseline_branch: '@{-1}'\n",
    ] {
        fs::write(dir.path().join(".badr/config.yaml"), text).unwrap();
        json(dir.path(), &["validate"], 2);
        json(dir.path(), &["init", "--baseline", "trunk"], 2);
        assert_eq!(
            fs::read_to_string(dir.path().join(".badr/config.yaml")).unwrap(),
            text
        );
    }
}

#[test]
fn populated_collections_and_historical_records_never_claim_complete_success() {
    let dir = repo(true);
    json(dir.path(), &["init", "--baseline", "trunk"], 0);
    fs::create_dir_all(dir.path().join(".badr/adrs")).unwrap();
    fs::write(dir.path().join(".badr/adrs/0001-example.md"), "unvalidated").unwrap();
    for operation in ["list", "validate"] {
        let result = json(dir.path(), &[operation], 2);
        assert_eq!(result["validation_coverage"]["collection"], "incomplete");
        assert_eq!(
            result["diagnostics"][0]["path"],
            ".badr/adrs/0001-example.md"
        );
    }
    git(dir.path(), &["add", ".badr"]);
    git(
        dir.path(),
        &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "commit",
            "-m",
            "historical record",
        ],
    );
    fs::remove_file(dir.path().join(".badr/adrs/0001-example.md")).unwrap();
    let result = json(dir.path(), &["validate"], 2);
    assert_eq!(result["validation_coverage"]["collection"], "passed");
    assert_eq!(result["validation_coverage"]["history"], "incomplete");
}

#[cfg(unix)]
#[test]
fn symlinks_are_refused_without_touching_their_targets() {
    use std::os::unix::fs::symlink;
    let dir = repo(true);
    let outside = tempfile::tempdir().unwrap();
    symlink(outside.path(), dir.path().join(".badr")).unwrap();
    json(dir.path(), &["init", "--baseline", "trunk"], 2);
    assert_eq!(fs::read_dir(outside.path()).unwrap().count(), 0);
    fs::remove_file(dir.path().join(".badr")).unwrap();
    json(dir.path(), &["init", "--baseline", "trunk"], 0);
    symlink(outside.path(), dir.path().join(".badr/adrs")).unwrap();
    json(dir.path(), &["validate"], 2);
}

#[test]
fn existing_writer_lock_is_an_operational_failure_and_is_preserved() {
    let dir = repo(true);
    fs::write(dir.path().join(".badr.lock"), "another writer").unwrap();
    let result = json(dir.path(), &["init", "--baseline", "trunk"], 4);
    assert_eq!(result["diagnostics"][0]["code"], "write_locked");
    assert_eq!(
        fs::read_to_string(dir.path().join(".badr.lock")).unwrap(),
        "another writer"
    );
    assert!(!dir.path().join(".badr").exists());
}

#[test]
fn filesystem_operational_failures_are_distinct_from_invalid_configuration() {
    let dir = repo(true);
    fs::create_dir(dir.path().join(".badr")).unwrap();
    // A path that cannot be used as a configuration file is invalid input.
    fs::create_dir(dir.path().join(".badr/config.yaml")).unwrap();
    json(dir.path(), &["validate"], 2);
    // A missing target directory is an operational failure, without writes.
    json(
        &dir.path().join("missing"),
        &["init", "--baseline", "trunk"],
        4,
    );
}

#[test]
fn shallow_history_is_reported_as_incomplete() {
    let source = repo(true);
    let clone = tempfile::tempdir().unwrap();
    let url = format!("file://{}", source.path().display());
    git(clone.path(), &["clone", "--depth", "1", &url, "."]);
    let result = json(clone.path(), &["init", "--baseline", "trunk"], 3);
    assert_eq!(result["validation_coverage"]["history"], "incomplete");
    assert!(result["baseline_commit"].is_string());
}

#[test]
fn human_diagnostics_use_stderr_and_json_argument_errors_are_structured() {
    let dir = repo(true);
    let out = run(dir.path(), &["init"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("--baseline"));
    assert!(!String::from_utf8_lossy(&out.stdout).contains("Usage:"));
    let result = json(dir.path(), &["init"], 2);
    assert_eq!(result["operation"], "arguments");
    assert_eq!(result["schema_version"], 1);
    assert_eq!(
        result["partial_write"],
        serde_json::json!({"completed":[],"not_completed":[]})
    );
    for flag in ["--help", "--version"] {
        assert!(run(dir.path(), &[flag]).status.success());
    }
}

#[test]
fn failure_after_configuration_publication_reports_the_completed_write() {
    let dir = repo(true);
    // Git can still resolve commits, but status cannot read this corrupt index.
    fs::write(dir.path().join(".git/index"), "corrupt index").unwrap();
    let result = json(dir.path(), &["init", "--baseline", "trunk"], 4);
    assert_eq!(result["success"], false);
    assert_eq!(
        result["partial_write"]["completed"],
        serde_json::json!([".badr/config.yaml"])
    );
    assert!(dir.path().join(".badr/config.yaml").is_file());
    assert!(!dir.path().join(".badr.lock").exists());
}

#[cfg(unix)]
#[test]
fn inspection_does_not_execute_repository_fsmonitor_hooks() {
    use std::os::unix::fs::PermissionsExt;
    let dir = repo(true);
    fs::write(dir.path().join("tracked.txt"), "tracked").unwrap();
    git(dir.path(), &["add", "tracked.txt"]);
    let hook = dir.path().join(".git/fsmonitor-hook");
    fs::write(&hook, "#!/bin/sh\ntouch hook-was-run\nprintf '\\0'\n").unwrap();
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
    git(
        dir.path(),
        &["config", "core.fsmonitor", hook.to_str().unwrap()],
    );
    json(dir.path(), &["init", "--baseline", "trunk"], 0);
    json(dir.path(), &["validate"], 0);
    assert!(!dir.path().join("hook-was-run").exists());
}

#[test]
fn repository_names_ending_in_whitespace_do_not_redirect_writes() {
    let parent = tempfile::tempdir().unwrap();
    let sibling = parent.path().join("repo");
    let target = parent.path().join("repo ");
    for path in [&sibling, &target] {
        fs::create_dir(path).unwrap();
        git(path, &["init", "-b", "trunk"]);
    }
    json(&target, &["init", "--baseline", "trunk"], 3);
    assert!(target.join(".badr/config.yaml").exists());
    assert!(!sibling.join(".badr").exists());
}

#[test]
fn merged_branch_history_cannot_hide_added_then_deleted_records() {
    let dir = repo(true);
    json(dir.path(), &["init", "--baseline", "trunk"], 0);
    git(dir.path(), &["add", ".badr"]);
    git(
        dir.path(),
        &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "commit",
            "-m",
            "config",
        ],
    );
    git(dir.path(), &["checkout", "-b", "side"]);
    fs::create_dir(dir.path().join(".badr/adrs")).unwrap();
    fs::write(
        dir.path().join(".badr/adrs/0001-example.md"),
        "historical record",
    )
    .unwrap();
    git(dir.path(), &["add", ".badr"]);
    git(
        dir.path(),
        &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "commit",
            "-m",
            "add record",
        ],
    );
    git(dir.path(), &["rm", ".badr/adrs/0001-example.md"]);
    git(
        dir.path(),
        &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "commit",
            "-m",
            "delete record",
        ],
    );
    git(dir.path(), &["checkout", "trunk"]);
    git(
        dir.path(),
        &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "merge",
            "--no-ff",
            "side",
            "-m",
            "merge side",
        ],
    );
    let result = json(dir.path(), &["validate"], 2);
    assert_eq!(result["validation_coverage"]["history"], "incomplete");
}

#[test]
fn literal_at_prefixed_baseline_names_are_supported() {
    let dir = repo(true);
    git(dir.path(), &["branch", "@team"]);
    let result = json(dir.path(), &["init", "--baseline", "@team"], 0);
    assert_eq!(
        result["baseline_commit"],
        git(dir.path(), &["rev-parse", "refs/heads/@team"])
    );
}

fn brr_repo() -> TempDir {
    let dir = repo(true);
    json(dir.path(), &["init", "--baseline", "trunk"], 0);
    fs::create_dir_all(dir.path().join(".badr/brrs/domains/ordering/rules")).unwrap();
    fs::write(
        dir.path().join(".badr/brrs/domains/ordering/glossary.md"),
        "# Ordering\n",
    )
    .unwrap();
    fs::write(dir.path().join(".badr/brrs/registry.yaml"), "version: 1\ndomains:\n  ordering: ORD\nrules:\n  BRR-ORD-001: domains/ordering/rules/BRR-ORD-001-cancellation.feature\n").unwrap();
    fs::write(dir.path().join(BRR_PATH), brr("proposed", "")).unwrap();
    dir
}
const BRR_PATH: &str = ".badr/brrs/domains/ordering/rules/BRR-ORD-001-cancellation.feature";
fn brr(status: &str, metadata: &str) -> String {
    format!(
        "---\nid: BRR-ORD-001\ndomain: ordering\nstatus: {status}\nowners: [Product, Engineering]\n{metadata}---\n@brr:BRR-ORD-001 @domain:ordering\nFeature: Cancellation\n  Rule: Customers can cancel before fulfilment\n    Scenario: Cancel early\n      Given an unfulfilled order\n      When its customer cancels\n      Then it is cancelled\n"
    )
}

#[test]
fn inspect_registered_brr_and_show_exact_body_without_writes() {
    let dir = brr_repo();
    let before = fs::read(dir.path().join(BRR_PATH)).unwrap();
    let result = json(
        dir.path(),
        &[
            "list", "--type", "brr", "--domain", "ordering", "--status", "proposed",
        ],
        2,
    );
    assert_eq!(result["validation_coverage"]["collection"], "passed");
    assert_eq!(result["validation_coverage"]["history"], "incomplete");
    assert_eq!(result["records"][0]["id"], "BRR-ORD-001");
    assert_eq!(result["records"][0]["selection"], "pending_preview");
    assert_eq!(result["records"][0]["currently_effective"], false);
    let shown = json(dir.path(), &["show", "BRR-ORD-001"], 2);
    assert_eq!(
        shown["records"][0]["body"],
        brr("proposed", "")
            .split_once("---\n@brr")
            .map(|(_, body)| format!("@brr{body}"))
            .unwrap()
    );
    assert_eq!(
        json(dir.path(), &["list", "--effective"], 2)["records"],
        serde_json::json!([])
    );
    assert_eq!(
        json(dir.path(), &["list", "--pending-preview"], 2)["records"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let human = run(dir.path(), &["list", "--pending-preview"]);
    assert!(String::from_utf8_lossy(&human.stdout).contains("pending_preview"));
    assert_eq!(fs::read(dir.path().join(BRR_PATH)).unwrap(), before);
    assert_eq!(
        json(dir.path(), &["validate"], 2)["partial_write"],
        serde_json::json!({"completed":[],"not_completed":[]})
    );
}

#[test]
fn brr_structure_outlines_and_source_lines_are_checked() {
    let dir = brr_repo();
    let original = brr("proposed", "");
    let cases = [
        original.replace("Feature: Cancellation", "Feature: One\nFeature: Two"),
        original.replace("  Rule:", "  Rule: Extra\n  Rule:"),
        original.replace("Customers can cancel before fulfilment", ""),
        original.replace("    Scenario: Cancel early\n      Given an unfulfilled order\n      When its customer cancels\n      Then it is cancelled\n", ""),
        original.replace("    Scenario: Cancel early", "    Scenario:"),
        original.replace("    Scenario: Cancel early", "    Scenario Outline: Cancel <time>"),
        original.replace("    Scenario: Cancel early", "    Scenario Outline: Cancel <time>") + "      Examples:\n        | time |\n",
        original.replace("  Rule: Customers can cancel before fulfilment\n", ""),
        original.replace("@brr:BRR-ORD-001", "@brr:BRR-ORD-002"),
        original.replace("@domain:ordering", "@domain:shipping"),
    ];
    for text in cases {
        fs::write(dir.path().join(BRR_PATH), &text).unwrap();
        let result = json(dir.path(), &["validate"], 2);
        assert_eq!(result["diagnostics"][0]["path"], BRR_PATH, "{text}");
        assert_eq!(fs::read_to_string(dir.path().join(BRR_PATH)).unwrap(), text);
    }
    let outline = original.replace(
        "    Scenario: Cancel early",
        "    Scenario Outline: Cancel <time>",
    ) + "      Examples:\n        | time |\n        | early |\n";
    fs::write(dir.path().join(BRR_PATH), outline.replace('\n', "\r\n")).unwrap();
    let result = json(dir.path(), &["show", "BRR-ORD-001"], 2);
    assert_eq!(result["validation_coverage"]["collection"], "passed");
    assert!(
        result["records"][0]["body"]
            .as_str()
            .unwrap()
            .contains("\r\n")
    );
    let no_rule = original.replace("  Rule: Customers can cancel before fulfilment\n", "");
    fs::write(dir.path().join(BRR_PATH), no_rule).unwrap();
    let message = json(dir.path(), &["validate"], 2)["diagnostics"][0]["message"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(
        message.contains("body-relative line 2, stored-file line 8"),
        "{message}"
    );
}

#[test]
fn brr_metadata_and_lifecycle_validation_reject_malformed_values() {
    let dir = brr_repo();
    let original = brr("proposed", "");
    let cases = [
        original.replace("id: BRR-ORD-001\n", ""),
        original.replace("domain: ordering\n", ""),
        original.replace("status: proposed\n", ""),
        original.replace("owners: [Product, Engineering]\n", ""),
        original.replace("id: BRR-ORD-001", "id: BRR-ord-001"),
        original.replace("id: BRR-ORD-001", "id: BRR-ORD-01"),
        original.replace("domain: ordering", "domain: Ordering"),
        original.replace("owners: [Product, Engineering]", "owners: []"),
        original.replace("owners: [Product, Engineering]", "owners: ['   ']"),
        original.replace("owners: [Product, Engineering]", "owners: [42]"),
        original.replace("owners: [Product, Engineering]", "owners: Product"),
        brr("unknown", ""),
        brr("proposed", "status: proposed\n"),
        brr("proposed", "unknown: value\n"),
        brr("proposed", "description: !unsafe value\n"),
        brr("proposed", "description: null\n"),
        brr("proposed", "description: [invalid]\n"),
        brr("proposed", "related_architecture_decisions: [1]\n"),
        brr("proposed", "related_architecture_decisions: ['wrong']\n"),
        brr("proposed", "supersedes: BRR-ORD-002\n"),
        brr("proposed", "superseded_by: [BRR-ORD-x]\n"),
        brr("proposed", "effective_from: '2020-01-01T00:00:00Z'\n"),
        brr("draft", "effective_until: '2020-01-01T00:00:00Z'\n"),
        brr("approved", "effective_from: '2020-01-01T00:00:00Z'\n"),
        brr("effective", ""),
        brr("effective", "effective_from: '2020-02-30T00:00:00Z'\n"),
        brr("effective", "effective_from: '2020-01-01T00:00:00'\n"),
        brr("effective", "effective_from: '2020-01-01T00:00Z'\n"),
        brr("effective", "effective_from: '2099-01-01T00:00:00Z'\n"),
        brr("retired", "effective_until: '2020-01-01T00:00:00Z'\n"),
        brr(
            "retired",
            "effective_from: '2020-01-02T00:00:00Z'\neffective_until: '2020-01-01T00:00:00Z'\n",
        ),
    ];
    for text in cases {
        fs::write(dir.path().join(BRR_PATH), &text).unwrap();
        json(dir.path(), &["validate"], 2);
        assert_eq!(fs::read_to_string(dir.path().join(BRR_PATH)).unwrap(), text);
    }
    fs::write(
        dir.path().join(BRR_PATH),
        b"---\nid: \xff\n---\nFeature: Invalid\n",
    )
    .unwrap();
    json(dir.path(), &["validate"], 2);
    for status in [
        "draft",
        "proposed",
        "approved",
        "effective",
        "deprecated",
        "superseded",
        "retired",
    ] {
        let times = match status {
            "effective" => "effective_from: '2020-01-01T05:30:00+05:30'\n",
            "deprecated" | "superseded" | "retired" => {
                "effective_from: '2020-01-01T05:30:00+05:30'\neffective_until: '2020-01-01T00:00:00Z'\n"
            }
            _ => "",
        };
        fs::write(dir.path().join(BRR_PATH), brr(status, &format!("description: A rule\nrelated_architecture_decisions: ['0001']\nsupersedes: [BRR-ORD-002]\nsuperseded_by: []\n{times}"))).unwrap();
        let result = json(dir.path(), &["list", "--status", status], 2);
        assert_eq!(result["validation_coverage"]["collection"], "passed");
        assert_eq!(
            result["records"][0]["currently_effective"],
            status == "effective"
        );
        assert_eq!(
            json(dir.path(), &["list", "--effective"], 2)["records"]
                .as_array()
                .unwrap()
                .len(),
            usize::from(status == "effective")
        );
    }
}

#[test]
fn brr_registry_rejects_collisions_and_unregistered_or_misplaced_files() {
    let dir = brr_repo();
    let registry_path = dir.path().join(".badr/brrs/registry.yaml");
    let original = fs::read_to_string(&registry_path).unwrap();
    let cases = [
        original.replace("version: 1", "version: 2"),
        original.replace("version: 1", "version: 1\nversion: 1"),
        original.clone() + "unknown: true\n",
        original.replace("ordering: ORD", "ordering: !unsafe ORD"),
        original.replace("ordering: ORD", "ordering: ord"),
        original.replace("ordering: ORD", "ordering: ORD\n  shipping: ORD"),
        original.replace("ordering: ORD", "ordering: ORD\n  ordering: SHP"),
        original.replace("ordering: ORD", "Ordering: ORD"),
        original.replace("ordering: ORD", "ordering: 123"),
        original.replace("BRR-ORD-001: domains", "BRR-ORD-001: domains/ordering/rules/BRR-ORD-001-cancellation.feature\n  BRR-ORD-001: domains"),
        original.clone() + "  BRR-ORD-0001: domains/ordering/rules/BRR-ORD-0001-other.feature\n",
        original.clone() + "  BRR-ORD-002: domains/ordering/rules/BRR-ORD-001-cancellation.feature\n",
        original.replace("domains/ordering/rules/BRR-ORD-001-cancellation.feature", "../outside.feature"),
        original.replace("domains/ordering/rules/BRR-ORD-001-cancellation.feature", "/tmp/outside.feature"),
        original.replace("domains/ordering/rules/BRR-ORD-001-cancellation.feature", "domains/ordering/rules/./BRR-ORD-001-cancellation.feature"),
        original.replace("domains/ordering/rules/BRR-ORD-001-cancellation.feature", "domains/ordering/rules/missing.feature"),
        original.replace("BRR-ORD-001:", "BRR-ORD-002:"),
    ];
    for text in cases {
        fs::write(&registry_path, &text).unwrap();
        json(dir.path(), &["validate"], 2);
        assert_eq!(fs::read_to_string(&registry_path).unwrap(), text);
    }
    fs::write(&registry_path, &original).unwrap();
    let glossary = dir.path().join(".badr/brrs/domains/ordering/glossary.md");
    fs::remove_file(&glossary).unwrap();
    json(dir.path(), &["validate"], 2);
    fs::write(glossary, "# Ordering").unwrap();
    fs::remove_file(&registry_path).unwrap();
    json(dir.path(), &["validate"], 2);
    fs::write(&registry_path, &original).unwrap();
    let misplaced = ".badr/brrs/BRR-ORD-001-cancellation.feature";
    fs::rename(dir.path().join(BRR_PATH), dir.path().join(misplaced)).unwrap();
    fs::write(
        &registry_path,
        original.replace(
            "domains/ordering/rules/BRR-ORD-001-cancellation.feature",
            "BRR-ORD-001-cancellation.feature",
        ),
    )
    .unwrap();
    json(dir.path(), &["validate"], 2);
    fs::rename(dir.path().join(misplaced), dir.path().join(BRR_PATH)).unwrap();
    fs::write(&registry_path, original).unwrap();
    fs::write(
        dir.path()
            .join(".badr/brrs/domains/ordering/rules/BRR-ORD-001-other.feature"),
        brr("proposed", ""),
    )
    .unwrap();
    json(dir.path(), &["validate"], 2);
}

#[test]
fn empty_registry_is_valid_but_wrong_managed_file_types_are_not() {
    let dir = repo(true);
    json(dir.path(), &["init", "--baseline", "trunk"], 0);
    fs::create_dir(dir.path().join(".badr/brrs")).unwrap();
    fs::write(
        dir.path().join(".badr/brrs/registry.yaml"),
        "version: 1\ndomains: {}\nrules: {}\n",
    )
    .unwrap();
    assert_eq!(
        json(dir.path(), &["validate"], 0)["validation_coverage"]["collection"],
        "passed"
    );
    fs::write(dir.path().join("unrelated.feature"), "not imported").unwrap();
    json(dir.path(), &["list"], 0);
    fs::remove_file(dir.path().join(".badr/brrs/registry.yaml")).unwrap();
    fs::remove_dir(dir.path().join(".badr/brrs")).unwrap();
    fs::write(dir.path().join(".badr/brrs"), "wrong root type").unwrap();
    json(dir.path(), &["validate"], 2);
}

#[cfg(unix)]
#[test]
fn brr_managed_paths_refuse_symlinks_and_special_files() {
    use std::os::unix::fs::symlink;
    let dir = brr_repo();
    let outside = tempfile::tempdir().unwrap();
    let outside_file = outside.path().join("outside.feature");
    fs::write(&outside_file, brr("proposed", "")).unwrap();
    let path = dir.path().join(BRR_PATH);
    fs::remove_file(&path).unwrap();
    symlink(&outside_file, &path).unwrap();
    json(dir.path(), &["validate"], 2);
    assert_eq!(
        fs::read_to_string(&outside_file).unwrap(),
        brr("proposed", "")
    );
    fs::remove_file(&path).unwrap();
    let rules = dir.path().join(".badr/brrs/domains/ordering/rules");
    fs::remove_dir(&rules).unwrap();
    symlink(outside.path(), &rules).unwrap();
    json(dir.path(), &["validate"], 2);
    fs::remove_file(&rules).unwrap();
    fs::create_dir(&rules).unwrap();
    let fifo = rules.join("BRR-ORD-001-cancellation.feature");
    assert!(
        Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap()
            .success()
    );
    json(dir.path(), &["validate"], 2);
}

#[test]
fn registry_paths_must_reference_features_and_filenames_must_match_identity() {
    let dir = brr_repo();
    let path = dir.path().join(BRR_PATH);
    for name in [
        "BRR-ORD-001-Cancellation.feature",
        "BRR-ORD-002-cancellation.feature",
        "BRR-ORD-001-.feature",
    ] {
        let renamed = path.with_file_name(name);
        fs::rename(&path, &renamed).unwrap();
        let result = json(dir.path(), &["validate"], 2);
        assert!(
            result["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["message"].as_str().unwrap().contains("Filename"))
        );
        fs::rename(renamed, &path).unwrap();
    }
    fs::remove_file(&path).unwrap();
    fs::write(dir.path().join(".badr/brrs/registry.yaml"), "version: 1\ndomains:\n  ordering: ORD\nrules:\n  BRR-ORD-001: domains/ordering/glossary.md\n").unwrap();
    json(dir.path(), &["validate"], 2);
}

#[test]
fn body_diagnostics_and_json_order_are_deterministic() {
    let dir = brr_repo();
    let path = dir.path().join(BRR_PATH);
    fs::write(
        &path,
        brr("proposed", "").replace("Feature: Cancellation", "not a feature"),
    )
    .unwrap();
    let first = run(dir.path(), &["validate", "--json"]);
    let result: Value = serde_json::from_slice(&first.stdout).unwrap();
    assert!(
        result["diagnostics"][0]["message"]
            .as_str()
            .unwrap()
            .contains("body-relative line")
    );
    assert_eq!(
        run(dir.path(), &["validate", "--json"]).stdout,
        first.stdout
    );
    for text in [
        "Feature: Missing frontmatter\n",
        "---\nid: BRR-ORD-001\n",
        "---\n---\nFeature: Missing metadata\n",
    ] {
        fs::write(&path, text).unwrap();
        json(dir.path(), &["validate"], 2);
    }
    fs::write(&path, brr("proposed", "").replace("      Given an unfulfilled order", "      Given an unfulfilled order\n      \"\"\"\n      Feature: text inside an argument\n      Rule: text inside an argument\n      # language: nonexistent\n      \"\"\"")).unwrap();
    assert_eq!(
        json(dir.path(), &["validate"], 2)["validation_coverage"]["collection"],
        "passed"
    );
    fs::write(
        &path,
        brr("proposed", "")
            .replace("@brr:BRR-ORD-001", "# language: fr\n@brr:BRR-ORD-001")
            .replace("Feature: Cancellation", "Fonctionnalité: Annulation")
            .replace(
                "Rule: Customers can cancel before fulfilment",
                "Règle: Annuler avant expédition",
            )
            .replace("Scenario: Cancel early", "Plan du scénario: Annuler <time>")
            .replace("Given an unfulfilled order", "Soit une commande")
            .replace("When its customer cancels", "Quand le client annule")
            .replace("Then it is cancelled", "Alors elle est annulée")
            + "      Exemples:\n        | time |\n        | tôt |\n",
    )
    .unwrap();
    assert_eq!(
        json(dir.path(), &["validate"], 2)["validation_coverage"]["collection"],
        "passed"
    );
}

#[cfg(unix)]
#[test]
fn unreadable_brr_is_an_operational_failure() {
    use std::os::unix::fs::PermissionsExt;
    let dir = brr_repo();
    let path = dir.path().join(BRR_PATH);
    fs::set_permissions(&path, fs::Permissions::from_mode(0o0)).unwrap();
    let output = run(dir.path(), &["validate", "--json"]);
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    assert_eq!(
        output.status.code(),
        Some(4),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[test]
fn applicability_times_require_quoted_yaml_scalars() {
    let dir = brr_repo();
    let path = dir.path().join(BRR_PATH);
    for metadata in [
        "effective_from: 2020-01-01T00:00:00Z\n",
        "'effective_from': 2020-01-01T00:00:00Z\n",
        "effective_from: >-\n  2020-01-01T00:00:00Z\n",
        "supersedes: &empty []\nsuperseded_by: *empty\neffective_from: 2020-01-01T00:00:00Z\n",
        "effective_from: '2020-01-01T00:00:00Z'\neffective_until: 2020-01-02T00:00:00Z\n",
    ] {
        let status = if metadata.contains("effective_until") {
            "retired"
        } else {
            "effective"
        };
        fs::write(&path, brr(status, metadata)).unwrap();
        json(dir.path(), &["validate"], 2);
    }
    for metadata in [
        "effective_from: \"2020-01-01T00:00:00Z\"\n",
        "effective_from: \"2020\\x2d01-01T00:00:00Z\"\n",
        "effective_from: &activation '2020-01-01T00:00:00Z'\n",
        "description: &activation '2020-01-01T00:00:00Z'\neffective_from: *activation\n",
    ] {
        fs::write(&path, brr("effective", metadata)).unwrap();
        assert_eq!(
            json(dir.path(), &["validate"], 2)["validation_coverage"]["collection"],
            "passed"
        );
    }
    fs::write(&path, brr("effective", "effective_from: '2020-01-01T00:00:00Z'\n").replace("id: BRR-ORD-001\ndomain: ordering\nstatus: effective\nowners: [Product, Engineering]\neffective_from: '2020-01-01T00:00:00Z'\n", "{id: BRR-ORD-001, domain: ordering, status: effective, owners: [Product], effective_from: '2020-01-01T00:00:00Z'}\n")).unwrap();
    assert_eq!(
        json(dir.path(), &["validate"], 2)["validation_coverage"]["collection"],
        "passed"
    );
}

#[test]
fn mixed_brr_and_adr_collections_keep_type_status_and_authority_filters() {
    let dir = brr_repo();
    fs::create_dir(dir.path().join(".badr/adrs")).unwrap();
    let path = dir.path().join(".badr/adrs/0001-storage.md");
    fs::write(&path, "---\nid: \"0001\"\nstatus: approved\ndate: \"2026-10-15\"\n---\n# Storage\n\n## Context and Problem Statement\nNeed storage.\n\n## Decision Drivers\nSimplicity.\n\n## Considered Options\nFiles or database.\n\n## Decision Outcome\nFiles because Git gives history.\n\n### Consequences\nReviewable changes.\n").unwrap();
    let all = json(dir.path(), &["list"], 2);
    assert_eq!(all["records"].as_array().unwrap().len(), 2);
    assert_eq!(all["validation_coverage"]["collection"], "passed");
    for (kind, id) in [("brr", "BRR-ORD-001"), ("adr", "0001")] {
        let selected = json(dir.path(), &["list", "--type", kind], 2);
        assert_eq!(selected["records"].as_array().unwrap().len(), 1);
        assert_eq!(selected["records"][0]["id"], id);
        assert_eq!(json(dir.path(), &["show", id], 2)["records"][0]["id"], id);
    }
    assert_eq!(
        json(dir.path(), &["list", "--status", "approved"], 2)["records"][0]["id"],
        "0001"
    );
    assert_eq!(
        json(dir.path(), &["list", "--domain", "ordering"], 2)["records"][0]["id"],
        "BRR-ORD-001"
    );
    assert_eq!(
        json(dir.path(), &["list", "--pending-preview"], 2)["records"][0]["id"],
        "BRR-ORD-001"
    );
    assert_eq!(
        json(dir.path(), &["list", "--directing"], 2)["records"][0]["id"],
        "0001"
    );
    assert_eq!(
        json(dir.path(), &["list", "--effective"], 2)["records"],
        serde_json::json!([])
    );
    fs::write(&path, "malformed ADR").unwrap();
    json(dir.path(), &["list", "--type", "brr"], 2);
}
