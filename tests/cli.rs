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
        let result = json(dir.path(), &[operation], 3);
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
    let result = json(dir.path(), &["validate"], 3);
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
    let result = json(dir.path(), &["validate"], 3);
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
