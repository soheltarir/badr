use serde_json::{Value, json};
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
        .args(args)
        .output()
        .unwrap()
}
fn report(root: &Path, args: &[&str], exit: i32) -> Value {
    let mut args = args.to_vec();
    args.push("--json");
    let out = run(root, &args);
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
    report(dir.path(), &["init", "--baseline", "trunk"], 0);
    fs::create_dir(dir.path().join(".badr/adrs")).unwrap();
    dir
}
fn body() -> &'static str {
    "# Storage\n\n## Context and Problem Statement\nWe need durable storage.\n\n## Decision Drivers\n* Simplicity\n\n## Considered Options\n* Files\n* Database\n\n## Decision Outcome\nChosen option: files, because Git provides history.\n\n### Consequences\n* Reviewable changes\n"
}
fn adr(id: &str, status: &str) -> String {
    format!(
        "---\nid: \"{id}\"\nstatus: {status}\ndate: \"2026-10-15\"\n---\n{}",
        body()
    )
}
fn write(root: &Path, name: &str, text: impl AsRef<[u8]>) {
    fs::write(root.join(".badr/adrs").join(name), text).unwrap();
}

#[test]
fn lists_filters_and_shows_adrs_with_honest_coverage() {
    let dir = repo();
    write(dir.path(), "0001-storage.md", adr("0001", "approved"));
    write(dir.path(), "0002-other.md", adr("0002", "proposed"));
    let result = report(
        dir.path(),
        &["list", "--type", "adr", "--status", "approved"],
        2,
    );
    assert_eq!(result["records"].as_array().unwrap().len(), 1);
    assert_eq!(result["records"][0]["id"], "0001");
    assert_eq!(result["records"][0]["directs_new_work"], true);
    assert_eq!(result["validation_coverage"]["collection"], "passed");
    assert_eq!(result["validation_coverage"]["evidence"], "incomplete");
    assert_eq!(result["validation_coverage"]["history"], "incomplete");
    let shown = report(dir.path(), &["show", "0001"], 2);
    assert_eq!(shown["records"][0]["decision_body"], body());
    assert_eq!(shown["records"][0]["body_start_line"], 6);
    assert_eq!(shown["records"][0]["related_adrs"], json!([]));
    let human = run(dir.path(), &["list", "--status", "approved"]);
    assert!(String::from_utf8_lossy(&human.stdout).contains("0001"));
    assert!(String::from_utf8_lossy(&human.stdout).contains("directs new work"));
}

#[test]
fn exact_decision_bytes_exclude_frontmatter_and_only_final_lifecycle_notes() {
    let dir = repo();
    let decision = body().replace("durable", "durable ☃").replace('\n', "\r\n");
    let mut text = adr("0001", "approved").replace(body(), &decision);
    text.push_str(
        "\r\n## Lifecycle notes\r\n\r\n### 2026-10-15 Approved\r\nReviewed and approved.\r\n",
    );
    write(dir.path(), "0001-storage.md", &text);
    let shown = report(dir.path(), &["show", "0001"], 2);
    assert_eq!(
        shown["records"][0]["decision_body"],
        format!("{decision}\r\n")
    );
    let before = fs::read(dir.path().join(".badr/adrs/0001-storage.md")).unwrap();
    let status = git(
        dir.path(),
        &["status", "--porcelain", "--untracked-files=all"],
    );
    for args in [
        vec!["validate"],
        vec!["list", "--directing"],
        vec!["show", "0001"],
    ] {
        let first = run(dir.path(), &[args.clone(), vec!["--json"]].concat());
        let second = run(dir.path(), &[args.clone(), vec!["--json"]].concat());
        assert_eq!(first.stdout, second.stdout);
        let result = report(dir.path(), &args, 2);
        assert_eq!(
            result["partial_write"],
            json!({"completed":[],"not_completed":[]})
        );
    }
    assert_eq!(
        fs::read(dir.path().join(".badr/adrs/0001-storage.md")).unwrap(),
        before
    );
    assert_eq!(
        git(
            dir.path(),
            &["status", "--porcelain", "--untracked-files=all"]
        ),
        status
    );
    assert!(!dir.path().join(".badr/evidence.jsonl").exists());
    assert!(!dir.path().join(".badr.lock").exists());
}

#[test]
fn rejects_invalid_metadata_without_rewriting_records() {
    let dir = repo();
    let valid = adr("0001", "proposed");
    let cases = [
        valid.replace("id: \"0001\"", "id: 0001"),
        valid.replace("id: \"0001\"", "id: \"001\""),
        valid.replace("id: \"0001\"", "id: \"0002\""),
        valid.replace("date: \"2026-10-15\"", "date: 2026-10-15"),
        valid.replace("2026-10-15", "2026-02-29"),
        valid.replace("2026-10-15", "2026-13-01"),
        valid.replace("status: proposed", "status: draft"),
        valid.replace("status: proposed", "status: accepted"),
        valid.replace("status: proposed", "status: [proposed]"),
        valid.replace("status: proposed", "status: proposed\nstatus: proposed"),
        valid.replace("status: proposed", "status: !unsafe proposed"),
        valid.replace("status: proposed", "status: !!str proposed"),
        valid.replace("status: proposed", "status: proposed\nunknown: true"),
        valid.replace("status: proposed", "status: proposed\nrelated_adrs: [0002]"),
        valid.replace("status: proposed", "status: proposed\nsupersedes: null"),
        valid.replace(
            "status: proposed",
            "status: proposed\nsuperseded_by: [false]",
        ),
        valid.replace(
            "status: proposed",
            "status: proposed\nrelated_business_rules: [bad-id]",
        ),
        valid.replace(
            "status: proposed",
            "status: proposed\ndecision-makers: ['']",
        ),
        valid.replace("status: proposed", "status: proposed\nid: \"0001\""),
    ];
    for text in cases {
        write(dir.path(), "0001-storage.md", &text);
        let result = report(dir.path(), &["validate"], 2);
        assert!(
            !result["diagnostics"].as_array().unwrap().is_empty(),
            "{text}"
        );
        assert_eq!(
            result["diagnostics"][0]["path"],
            ".badr/adrs/0001-storage.md"
        );
        assert_eq!(
            fs::read_to_string(dir.path().join(".badr/adrs/0001-storage.md")).unwrap(),
            text
        );
    }
    write(dir.path(), "0001-storage.md", [0xff, 0xfe]);
    assert_eq!(
        report(dir.path(), &["validate"], 2)["diagnostics"][0]["code"],
        "invalid_utf8"
    );
}

#[test]
fn reasoning_requires_real_sections_and_outcome_rationale() {
    let dir = repo();
    let valid = adr("0001", "proposed");
    for name in [
        "Context and Problem Statement",
        "Decision Drivers",
        "Considered Options",
        "Decision Outcome",
        "Consequences",
    ] {
        write(
            dir.path(),
            "0001-storage.md",
            valid.replace(name, "Other section"),
        );
        let result = report(dir.path(), &["validate"], 2);
        assert_eq!(result["diagnostics"][0]["code"], "missing_reasoning");
        assert!(result["diagnostics"][0]["body_line"].is_number());
        assert_eq!(result["diagnostics"][0]["stored_file_line"], 6);
    }
    for fence in ["```", "~~~~"] {
        let text = valid.replace(
            "## Decision Drivers\n* Simplicity",
            &format!("{fence}markdown\n## Decision Drivers\n* Simplicity\n{fence}"),
        );
        write(dir.path(), "0001-storage.md", text);
        report(dir.path(), &["validate"], 2);
    }
    write(
        dir.path(),
        "0001-storage.md",
        valid.replace("Chosen option: files, because Git provides history.\n", ""),
    );
    assert_eq!(
        report(dir.path(), &["validate"], 2)["diagnostics"][0]["code"],
        "empty_reasoning"
    );
}

#[test]
fn lifecycle_notes_must_be_final_dated_and_nonempty() {
    let dir = repo();
    let valid = adr("0001", "approved");
    let notes = "\n## Lifecycle notes\n\n### 2026-10-15 Approved\nReviewed.\n";
    for text in [
        format!("{valid}{notes}\n## Later decision\nMust not escape digest.\n"),
        valid.replace(
            "## Considered Options",
            &format!("{notes}\n## Considered Options"),
        ),
        format!("{valid}\n## Lifecycle notes\nUndated note.\n"),
        format!("{valid}{notes}{notes}"),
        format!("{valid}{}", notes.replace("2026-10-15", "2026-02-30")),
        format!("{valid}{}", notes.replace("Reviewed.", "")),
        format!("{valid}{}", notes.replace("### 2026", "#### 2026")),
    ] {
        write(dir.path(), "0001-storage.md", text);
        let result = report(dir.path(), &["validate"], 2);
        assert_eq!(result["diagnostics"][0]["code"], "invalid_lifecycle_notes");
        assert!(result["diagnostics"][0]["body_line"].as_u64().unwrap() > 1);
        assert_eq!(
            result["diagnostics"][0]["stored_file_line"]
                .as_u64()
                .unwrap(),
            result["diagnostics"][0]["body_line"].as_u64().unwrap() + 5
        );
    }
    let fenced = format!("{valid}\n~~~\n## Lifecycle notes\n### not-a-date\n~~~\n");
    write(dir.path(), "0001-storage.md", &fenced);
    assert_eq!(
        report(dir.path(), &["show", "0001"], 2)["records"][0]["decision_body"],
        fenced
            .split_once("---\n")
            .unwrap()
            .1
            .split_once("---\n")
            .unwrap()
            .1
    );
}

#[test]
fn adr_identity_is_numeric_global_and_directly_placed() {
    let dir = repo();
    write(dir.path(), "0001-storage.md", adr("0001", "proposed"));
    write(dir.path(), "00001-other.md", adr("00001", "proposed"));
    let collision = report(dir.path(), &["validate"], 2);
    assert!(
        collision["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "duplicate_identity")
    );
    fs::remove_file(dir.path().join(".badr/adrs/00001-other.md")).unwrap();
    for name in [
        "001-too-short.md",
        "0001-Uppercase.md",
        "0001-bad--title.md",
        "0001-title.txt",
    ] {
        write(dir.path(), name, adr("0001", "proposed"));
        report(dir.path(), &["validate"], 2);
        fs::remove_file(dir.path().join(".badr/adrs").join(name)).unwrap();
    }
    fs::create_dir(dir.path().join(".badr/adrs/nested")).unwrap();
    write(dir.path(), "nested/0002-hidden.md", adr("0002", "approved"));
    report(dir.path(), &["validate"], 2);
    report(dir.path(), &["show", "../0001-storage.md"], 2);
}

#[cfg(unix)]
#[test]
fn unsafe_adr_files_are_refused_without_reading_outside_collection() {
    use std::os::unix::fs::symlink;
    let dir = repo();
    let outside = tempfile::tempdir().unwrap();
    let target = outside.path().join("private.md");
    fs::write(&target, "private contents").unwrap();
    symlink(&target, dir.path().join(".badr/adrs/0001-secret.md")).unwrap();
    let out = run(dir.path(), &["validate", "--json"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(!String::from_utf8_lossy(&out.stdout).contains("private contents"));
    assert_eq!(fs::read_to_string(&target).unwrap(), "private contents");
}

#[test]
fn supports_empty_collection_all_statuses_and_strict_filter_arguments() {
    let dir = repo();
    assert_eq!(
        report(dir.path(), &["list", "--type", "adr"], 0)["records"],
        json!([])
    );
    report(dir.path(), &["show", "0001"], 2);
    for (i, status) in [
        "proposed",
        "approved",
        "rejected",
        "deprecated",
        "superseded",
    ]
    .iter()
    .enumerate()
    {
        let id = format!("{:04}", i + 1);
        write(dir.path(), &format!("{id}-example.md"), adr(&id, status));
    }
    assert_eq!(
        report(dir.path(), &["list", "--directing"], 2)["records"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    for status in [
        "proposed",
        "approved",
        "rejected",
        "deprecated",
        "superseded",
    ] {
        assert_eq!(
            report(dir.path(), &["list", "--status", status], 2)["records"][0]["status"],
            status
        );
    }
    report(dir.path(), &["list", "--status", "accepted"], 2);
    report(dir.path(), &["list", "--type", "unknown"], 2);
}

#[test]
fn consequences_belong_to_outcome_and_rationale_may_have_a_subheading() {
    let dir = repo();
    let valid = adr("0001", "proposed");
    let misplaced = valid
        .replace("### Consequences\n* Reviewable changes\n", "")
        .replace(
            "## Considered Options",
            "### Consequences\n* Reviewable changes\n\n## Considered Options",
        );
    write(dir.path(), "0001-storage.md", misplaced);
    report(dir.path(), &["validate"], 2);
    write(
        dir.path(),
        "0001-storage.md",
        valid.replace(
            "Chosen option: files, because Git provides history.",
            "### Rationale\nFiles let us use Git for history.",
        ),
    );
    report(dir.path(), &["validate"], 2);
}

#[test]
fn lifecycle_notes_need_visible_note_content() {
    let dir = repo();
    write(
        dir.path(),
        "0001-storage.md",
        format!(
            "{}\n## Lifecycle notes\n\n### 2026-10-15\n<!-- placeholder -->\n",
            adr("0001", "approved")
        ),
    );
    assert_eq!(
        report(dir.path(), &["validate"], 2)["diagnostics"][0]["code"],
        "invalid_lifecycle_notes"
    );
}

#[test]
fn metadata_diagnostics_locate_the_actual_field() {
    let dir = repo();
    for field in [
        "decision-makers: ['']",
        "related_adrs: ['bad']",
        "supersedes: ['bad']",
        "superseded_by: ['bad']",
        "related_business_rules: ['bad']",
    ] {
        write(
            dir.path(),
            "0001-storage.md",
            adr("0001", "proposed").replacen(
                "date: \"2026-10-15\"\n",
                &format!("date: \"2026-10-15\"\n# comment\n{field}\n"),
                1,
            ),
        );
        let diagnostic = &report(dir.path(), &["validate"], 2)["diagnostics"][0];
        assert_eq!(diagnostic["stored_file_line"], 6, "{field}");
        assert_eq!(diagnostic["body_line"], Value::Null);
        assert_eq!(diagnostic["record_id"], "0001");
    }
}

#[cfg(unix)]
#[test]
fn inaccessible_adr_collection_cannot_claim_evidence_or_history_passed() {
    let dir = repo();
    let outside = tempfile::tempdir().unwrap();
    fs::remove_dir(dir.path().join(".badr/adrs")).unwrap();
    std::os::unix::fs::symlink(outside.path(), dir.path().join(".badr/adrs")).unwrap();
    let result = report(dir.path(), &["validate"], 2);
    assert_eq!(result["validation_coverage"]["collection"], "incomplete");
    assert_eq!(result["validation_coverage"]["evidence"], "incomplete");
    assert_eq!(result["validation_coverage"]["history"], "incomplete");
    assert_eq!(result["diagnostics"][0]["path"], ".badr/adrs");
}
