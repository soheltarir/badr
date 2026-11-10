use serde_json::Value;
use std::{fs, path::Path, process::Command};
use tempfile::TempDir;

fn git(root: &Path, args: &[&str]) {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn run(root: &Path, args: &[&str]) -> (i32, Value) {
    let out = Command::new(env!("CARGO_BIN_EXE_badr"))
        .arg("--repo")
        .arg(root)
        .arg("--json")
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    (
        out.status.code().unwrap(),
        serde_json::from_slice(&out.stdout).unwrap(),
    )
}

fn repo(user: Option<&str>) -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    git(root, &["init", "-b", "trunk"]);
    git(
        root,
        &[
            "-c",
            "user.name=Committer",
            "-c",
            "user.email=c@example.invalid",
            "commit",
            "--allow-empty",
            "-m",
            "baseline",
        ],
    );
    if let Some(user) = user {
        git(root, &["config", "user.name", user]);
    }
    assert_eq!(run(root, &["init", "--baseline", "trunk"]).0, 0);
    dir
}

fn ledger(root: &Path) -> Value {
    let text = fs::read_to_string(root.join(".badr/evidence.jsonl")).unwrap();
    serde_json::from_str(text.lines().last().unwrap()).unwrap()
}

fn domain(root: &Path, extra: &[&str]) -> (i32, Value) {
    let mut args = vec!["domain", "add", "ordering", "--prefix", "ORD"];
    args.extend(extra);
    run(root, &args)
}

#[test]
fn omitted_actor_defaults_to_git_user_and_is_marked() {
    let dir = repo(Some("Git Person"));
    let root = dir.path();
    let (code, _) = domain(root, &["--confirm", "--rationale", "Establish vocabulary"]);
    assert_eq!(code, 0);
    let event = ledger(root);
    assert_eq!(event["actor"], "Git Person");
    assert_eq!(event["actor_defaulted"], true);
    assert_eq!(event["confirmed"], true);
    assert_eq!(run(root, &["validate"]).0, 0);
}

#[test]
fn omitted_actor_without_git_user_is_refused_before_publication() {
    let dir = repo(None);
    let root = dir.path();
    let before = fs::read_to_string(root.join(".badr/evidence.jsonl")).unwrap_or_default();
    let (code, report) = domain(root, &["--confirm", "--rationale", "Establish vocabulary"]);
    assert_ne!(code, 0);
    assert!(
        report["diagnostics"][0]["message"]
            .as_str()
            .unwrap()
            .contains("user.name")
    );
    assert_eq!(
        fs::read_to_string(root.join(".badr/evidence.jsonl")).unwrap_or_default(),
        before
    );
    assert!(!root.join(".badr/brrs/domains/ordering").exists());
}

#[test]
fn explicit_actor_is_recorded_as_given_and_not_marked() {
    let dir = repo(Some("Git Person"));
    let root = dir.path();
    let (code, _) = domain(
        root,
        &["--confirm", "--actor", "Human", "--rationale", "Establish"],
    );
    assert_eq!(code, 0);
    let event = ledger(root);
    assert_eq!(event["actor"], "Human");
    assert!(event.get("actor_defaulted").is_none());
}

#[test]
fn defaulting_does_not_waive_confirmation_or_rationale() {
    let dir = repo(Some("Git Person"));
    let root = dir.path();
    assert_ne!(domain(root, &["--rationale", "Why"]).0, 0);
    assert_ne!(domain(root, &["--confirm"]).0, 0);
    assert!(!root.join(".badr/brrs/domains/ordering").exists());
}
