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
fn report(root: &Path, exit: i32) -> Value {
    let out = run(root, &["validate", "--json"]);
    assert_eq!(
        out.status.code(),
        Some(exit),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    assert!(out.stderr.is_empty());
    serde_json::from_slice(&out.stdout).unwrap()
}
fn commit(root: &Path) {
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
            "records",
        ],
    );
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
    assert!(
        run(dir.path(), &["init", "--baseline", "trunk"])
            .status
            .success()
    );
    fs::create_dir(dir.path().join(".badr/adrs")).unwrap();
    dir
}
const ADR: &str = ".badr/adrs/0001-storage.md";
const BODY: &str = "# Storage\n\n## Context and Problem Statement\nNeed durable storage.\n\n## Decision Drivers\nSimplicity.\n\n## Considered Options\nFiles or database.\n\n## Decision Outcome\nFiles because Git provides history.\n\n### Consequences\nReviewable changes.\n";
fn write_adr(root: &Path, status: &str, body: &str, notes: &str, links: &str) {
    fs::write(
        root.join(ADR),
        format!(
            "---\nid: \"0001\"\nstatus: {status}\ndate: \"2020-01-01\"\n{links}---\n{body}{notes}"
        ),
    )
    .unwrap();
}
fn digest(bytes: &[u8]) -> String {
    // Independent fixture oracle: system SHA-256, not BADR's hashing implementation.
    use std::io::Write;
    let mut child = Command::new("shasum")
        .args(["-a", "256"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(bytes).unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    String::from_utf8(out.stdout)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .into()
}
fn change(status: &str, from: Option<&str>, approval: Option<&str>) -> Value {
    json!({"id":"0001","path":ADR,"from":from,"to":status,"body_sha256":digest(BODY.as_bytes()),"approval_ref":approval,"date":"2020-01-01","notes_sha256":digest(b""),"related":[],"supersedes":[],"superseded_by":[]})
}
fn event(root: &Path, id: &str, operation: &str, changes: Vec<Value>) -> Value {
    json!({"version":1,"event_id":id,"operation":operation,"at":"2020-01-01T00:00:00Z","branch":git(root,&["symbolic-ref","--short","HEAD"]),"baseline_commit":git(root,&["rev-parse","trunk"]),"actor":"Human","rationale":"Reviewed change","confirmed":true,"reviewed":operation == "decide","changes":changes})
}
fn ledger(root: &Path, events: &[Value]) {
    fs::write(
        root.join(".badr/evidence.jsonl"),
        events
            .iter()
            .map(|e| e.to_string() + "\n")
            .collect::<String>(),
    )
    .unwrap();
}
fn codes(result: &Value) -> Vec<&str> {
    result["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["code"].as_str().unwrap())
        .collect()
}

#[test]
fn valid_precommit_approval_chain_matches_exact_body_without_writes() {
    let dir = repo();
    let root = dir.path();
    write_adr(root, "approved", BODY, "", "");
    ledger(
        root,
        &[
            event(
                root,
                "allocate-1",
                "allocate",
                vec![change("proposed", None, None)],
            ),
            event(
                root,
                "approve-1",
                "decide",
                vec![change("approved", Some("proposed"), Some("approve-1"))],
            ),
        ],
    );
    let before = fs::read(root.join(ADR)).unwrap();
    let evidence = fs::read(root.join(".badr/evidence.jsonl")).unwrap();
    let result = report(root, 0);
    assert_eq!(result["validation_coverage"]["evidence"], "passed");
    assert_eq!(result["validation_coverage"]["history"], "passed");
    assert_eq!(
        result["partial_write"],
        json!({"completed":[],"not_completed":[]})
    );
    assert_eq!(fs::read(root.join(ADR)).unwrap(), before);
    assert_eq!(
        fs::read(root.join(".badr/evidence.jsonl")).unwrap(),
        evidence
    );
    write_adr(
        root,
        "approved",
        &(BODY.to_owned() + "<!-- edit -->\n"),
        "",
        "",
    );
    assert!(codes(&report(root, 2)).contains(&"body_digest_mismatch"));
}

#[test]
fn retained_evidence_and_notes_are_append_only() {
    let dir = repo();
    let root = dir.path();
    let notes = "## Lifecycle notes\n\n### 2020-01-01 Approved\nHuman approved this decision.\n";
    write_adr(root, "approved", BODY, notes, "");
    let allocate = event(
        root,
        "allocate-1",
        "allocate",
        vec![change("proposed", None, None)],
    );
    let mut approved = change("approved", Some("proposed"), Some("approve-1"));
    approved["notes_sha256"] = json!(digest(notes.as_bytes()));
    let approval = event(root, "approve-1", "decide", vec![approved.clone()]);
    ledger(root, &[allocate.clone(), approval.clone()]);
    commit(root);
    report(root, 0);
    let mut link = approved;
    link["from"] = json!("approved");
    link["related"] = json!(["0002"]);
    let mut linked = event(root, "link-1", "link", vec![link]);
    linked["changes"][0]["notes_sha256"] = json!(digest(b""));
    write_adr(root, "approved", BODY, "", "related_adrs: [\"0002\"]\n");
    ledger(root, &[allocate.clone(), approval.clone(), linked]);
    assert!(codes(&report(root, 2)).contains(&"lifecycle_notes_rewritten"));
    write_adr(root, "approved", BODY, notes, "");
    let mut replacement = approval;
    replacement["rationale"] = json!("Rewritten receipt");
    ledger(root, &[allocate, replacement]);
    assert!(codes(&report(root, 2)).contains(&"evidence_rewritten"));
}

#[test]
fn missing_evidence_manual_status_and_removed_records_are_errors() {
    let dir = repo();
    let root = dir.path();
    write_adr(root, "proposed", BODY, "", "");
    assert!(codes(&report(root, 2)).contains(&"missing_evidence"));
    ledger(
        root,
        &[event(
            root,
            "allocate-1",
            "allocate",
            vec![change("proposed", None, None)],
        )],
    );
    report(root, 0);
    write_adr(root, "approved", BODY, "", "");
    assert!(codes(&report(root, 2)).contains(&"record_evidence_mismatch"));
    fs::remove_file(root.join(ADR)).unwrap();
    assert!(codes(&report(root, 2)).contains(&"removed_record"));
}

#[test]
fn supporting_collection_references_are_unverified_only_when_absent() {
    let dir = repo();
    let root = dir.path();
    write_adr(
        root,
        "proposed",
        BODY,
        "",
        "related_business_rules: [BRR-ORD-001]\n",
    );
    let mut linked = change("proposed", Some("proposed"), None);
    linked["related"] = json!(["BRR-ORD-001"]);
    ledger(
        root,
        &[
            event(
                root,
                "alloc",
                "allocate",
                vec![change("proposed", None, None)],
            ),
            event(root, "link", "link", vec![linked]),
        ],
    );
    assert!(codes(&report(root, 3)).contains(&"unverified_reference"));
    fs::create_dir(root.join(".badr/brrs")).unwrap();
    fs::write(
        root.join(".badr/brrs/registry.yaml"),
        "version: 1\ndomains: {}\nrules: {}\n",
    )
    .unwrap();
    assert!(codes(&report(root, 2)).contains(&"missing_reference"));
}

#[test]
fn adr_group_approval_and_replacement_requires_complete_reciprocal_sets() {
    let dir = repo();
    let root = dir.path();
    write_adr(root, "superseded", BODY, "", "superseded_by: [\"0002\"]\n");
    let second = ".badr/adrs/0002-successor.md";
    fs::write(root.join(second),format!("---\nid: \"0002\"\nstatus: approved\ndate: \"2020-01-01\"\nsupersedes: [\"0001\"]\n---\n{BODY}")).unwrap();
    let mut alloc2 = change("proposed", None, None);
    alloc2["id"] = json!("0002");
    alloc2["path"] = json!(second);
    let mut old = change("superseded", Some("approved"), Some("approve-1"));
    old["superseded_by"] = json!(["0002"]);
    let mut new = alloc2.clone();
    new["from"] = json!("proposed");
    new["to"] = json!("approved");
    new["approval_ref"] = json!("replace");
    new["supersedes"] = json!(["0001"]);
    let mut group = event(root, "replace", "decide", vec![old, new]);
    group["replacement"] = json!({"predecessors":["0001"],"successors":["0002"],"complete":true});
    let prefix = vec![
        event(
            root,
            "alloc1",
            "allocate",
            vec![change("proposed", None, None)],
        ),
        event(
            root,
            "approve-1",
            "decide",
            vec![change("approved", Some("proposed"), Some("approve-1"))],
        ),
        event(root, "alloc2", "allocate", vec![alloc2]),
    ];
    let mut all = prefix.clone();
    all.push(group.clone());
    ledger(root, &all);
    report(root, 0);
    for bad in [
        json!({"predecessors":["0001"],"successors":["0002"],"complete":false}),
        json!({"predecessors":["0001"],"successors":["0002","0003"],"complete":true}),
    ] {
        let mut invalid = group.clone();
        invalid["replacement"] = bad;
        let mut all = prefix.clone();
        all.push(invalid);
        ledger(root, &all);
        report(root, 2);
    }
    let mut invalid = group;
    invalid["changes"][1]["supersedes"] = json!([]);
    let mut all = prefix;
    all.push(invalid);
    ledger(root, &all);
    report(root, 2);
}

#[test]
fn reapproval_retains_receipts_and_only_the_active_revision_is_frozen() {
    let dir = repo();
    let root = dir.path();
    git(root, &["checkout", "-b", "feature"]);
    let old = event(
        root,
        "approve-old",
        "decide",
        vec![change("approved", Some("proposed"), Some("approve-old"))],
    );
    let modified = BODY.replace("Simplicity.", "Simplicity and privacy.");
    let mut reset = change("proposed", Some("approved"), Some("approve-old"));
    reset["body_sha256"] = json!(digest(modified.as_bytes()));
    let mut reapproved = reset.clone();
    reapproved["from"] = json!("proposed");
    reapproved["to"] = json!("approved");
    reapproved["approval_ref"] = json!("approve-new");
    let events = vec![
        event(
            root,
            "allocate",
            "allocate",
            vec![change("proposed", None, None)],
        ),
        old,
        event(root, "reset", "edit", vec![reset]),
        event(root, "approve-new", "decide", vec![reapproved]),
    ];
    write_adr(root, "approved", &modified, "", "");
    ledger(root, &events);
    report(root, 0);
    let mut bad = events.clone();
    bad[3]["changes"][0]["approval_ref"] = json!("approve-old");
    ledger(root, &bad);
    report(root, 2);
    ledger(root, &events);
    write_adr(root, "approved", BODY, "", "");
    assert!(codes(&report(root, 2)).contains(&"body_digest_mismatch"));
    let mut bad = events.clone();
    bad[2]["branch"] = json!("trunk");
    ledger(root, &bad);
    assert!(codes(&report(root, 2)).contains(&"ineligible_reset"));
    ledger(root, &events[..2]);
    write_adr(root, "approved", BODY, "", "");
    git(root, &["checkout", "trunk"]);
    commit(root);
    git(root, &["checkout", "feature"]);
    git(root, &["merge", "--ff-only", "trunk"]);
    let mut bad = events;
    let baseline = git(root, &["rev-parse", "trunk"]);
    bad[2]["baseline_commit"] = json!(baseline);
    bad[3]["baseline_commit"] = bad[2]["baseline_commit"].clone();
    ledger(root, &bad);
    write_adr(root, "approved", &modified, "", "");
    assert!(codes(&report(root, 2)).contains(&"ineligible_reset"));
}

#[test]
fn malformed_events_and_repeated_approval_cannot_bless_content() {
    let dir = repo();
    let root = dir.path();
    write_adr(root, "approved", BODY, "", "");
    let valid = vec![
        event(
            root,
            "alloc",
            "allocate",
            vec![change("proposed", None, None)],
        ),
        event(
            root,
            "approve",
            "decide",
            vec![change("approved", Some("proposed"), Some("approve"))],
        ),
    ];
    for (key, value) in [
        ("version", json!(2)),
        ("event_id", json!("alloc")),
        ("operation", json!("unknown")),
        ("actor", json!("")),
        ("rationale", json!(null)),
        ("confirmed", json!(false)),
        ("at", json!("2099-01-01T00:00:00Z")),
        ("branch", json!("@{-1}")),
    ] {
        let mut bad = valid.clone();
        bad[1][key] = value;
        ledger(root, &bad);
        report(root, 2);
    }
    for (key, value) in [
        ("from", json!("draft")),
        ("body_sha256", json!("not-a-digest")),
        ("approval_ref", json!("absent")),
        ("path", json!(".badr/adrs/../outside.md")),
        ("to", json!("effective")),
        ("unknown", json!(true)),
    ] {
        let mut bad = valid.clone();
        bad[1]["changes"][0][key] = value;
        ledger(root, &bad);
        report(root, 2);
    }
    let mut repeated = valid.clone();
    repeated.push(event(
        root,
        "approve-again",
        "decide",
        vec![change("approved", Some("approved"), Some("approve-again"))],
    ));
    ledger(root, &repeated);
    report(root, 2);
    for text in [
        "{\"version\":1,\"version\":1}\n",
        "{}\n",
        "\n",
        "not json\n",
    ] {
        fs::write(root.join(".badr/evidence.jsonl"), text).unwrap();
        assert!(codes(&report(root, 2)).contains(&"invalid_evidence"));
    }
    ledger(root, &valid);
    let bytes = fs::read(root.join(".badr/evidence.jsonl")).unwrap();
    fs::write(root.join(".badr/evidence.jsonl"), &bytes[..bytes.len() - 1]).unwrap();
    report(root, 2);
    fs::write(root.join(".badr/evidence.jsonl"), b"\xff\n").unwrap();
    report(root, 2);
}

#[test]
fn absent_or_shallow_history_keeps_local_evidence_results_without_claiming_success() {
    let dir = repo();
    let root = dir.path();
    write_adr(root, "approved", BODY, "", "");
    let valid = vec![
        event(
            root,
            "alloc",
            "allocate",
            vec![change("proposed", None, None)],
        ),
        event(
            root,
            "approve",
            "decide",
            vec![change("approved", Some("proposed"), Some("approve"))],
        ),
    ];
    ledger(root, &valid);
    commit(root);
    let clone = tempfile::tempdir().unwrap();
    git(
        clone.path(),
        &[
            "clone",
            "--depth",
            "1",
            &format!("file://{}", root.display()),
            ".",
        ],
    );
    let result = report(clone.path(), 3);
    assert_eq!(result["validation_coverage"]["evidence"], "passed");
    assert_eq!(result["validation_coverage"]["history"], "incomplete");
    let mut unavailable = valid;
    unavailable[0]["baseline_commit"] = json!("f".repeat(40));
    ledger(root, &unavailable);
    assert!(codes(&report(root, 3)).contains(&"history_incomplete"));
}

const BRR_PATH: &str = ".badr/brrs/domains/ordering/rules/BRR-ORD-001-cancellation.feature";
const BRR_BODY: &str = "@brr:BRR-ORD-001 @domain:ordering\nFeature: Cancellation\n  Rule: Cancel before fulfilment\n    Scenario: Early cancellation\n      Given an unfulfilled order\n      When cancelled\n      Then cancellation succeeds\n";
fn brr_change(status: &str, from: Option<&str>, approval: Option<&str>) -> Value {
    json!({"id":"BRR-ORD-001","path":BRR_PATH,"from":from,"to":status,"body_sha256":digest(BRR_BODY.as_bytes()),"approval_ref":approval,"related":[],"supersedes":[],"superseded_by":[]})
}
fn write_brr(root: &Path, status: &str, times: &str, body: &str) {
    fs::create_dir_all(root.join(".badr/brrs/domains/ordering/rules")).unwrap();
    fs::write(root.join(".badr/brrs/registry.yaml"),"version: 1\ndomains:\n  ordering: ORD\nrules:\n  BRR-ORD-001: domains/ordering/rules/BRR-ORD-001-cancellation.feature\n").unwrap();
    fs::write(
        root.join(".badr/brrs/domains/ordering/glossary.md"),
        "# Ordering\n",
    )
    .unwrap();
    fs::write(root.join(BRR_PATH),format!("---\nid: BRR-ORD-001\ndomain: ordering\nstatus: {status}\nowners: [Product]\n{times}---\n{body}")).unwrap();
}
#[test]
fn brr_activation_withdrawal_and_exact_bytes_follow_retained_approval() {
    let dir = repo();
    let root = dir.path();
    let allocate = event(
        root,
        "alloc",
        "allocate",
        vec![brr_change("draft", None, None)],
    );
    let propose = event(
        root,
        "propose",
        "propose",
        vec![brr_change("proposed", Some("draft"), None)],
    );
    let approve = event(
        root,
        "approve",
        "decide",
        vec![brr_change("approved", Some("proposed"), Some("approve"))],
    );
    let mut activated = brr_change("effective", Some("approved"), Some("approve"));
    activated["effective_from"] = json!("2020-01-01T00:00:00Z");
    let activate = event(root, "activate", "activate", vec![activated.clone()]);
    let mut withdrawn = activated;
    withdrawn["from"] = json!("effective");
    withdrawn["to"] = json!("retired");
    withdrawn["effective_until"] = json!("2020-01-01T00:00:00Z");
    let retire = event(root, "retire", "retire", vec![withdrawn]);
    let all = vec![allocate, propose, approve, activate, retire];
    write_brr(
        root,
        "retired",
        "effective_from: '2020-01-01T00:00:00Z'\neffective_until: '2020-01-01T00:00:00Z'\n",
        BRR_BODY,
    );
    ledger(root, &all);
    report(root, 0);
    for modified in [
        BRR_BODY.replace('\n', "\r\n"),
        BRR_BODY.to_owned() + "# comment\n",
        BRR_BODY.replace("Given an", "Given  an"),
    ] {
        write_brr(
            root,
            "retired",
            "effective_from: '2020-01-01T00:00:00Z'\neffective_until: '2020-01-01T00:00:00Z'\n",
            &modified,
        );
        assert!(codes(&report(root, 2)).contains(&"body_digest_mismatch"));
    }
    write_brr(
        root,
        "retired",
        "effective_from: '2020-01-01T00:00:00Z'\neffective_until: '2020-01-01T00:00:00Z'\n",
        BRR_BODY,
    );
    let mut bad = all.clone();
    bad[4]["changes"][0]["effective_from"] = json!("2019-01-01T00:00:00Z");
    ledger(root, &bad);
    report(root, 2);
    let mut bad = all;
    bad[3]["changes"][0]["effective_from"] = json!("2019-01-01T00:00:00Z");
    ledger(root, &bad);
    report(root, 2);
}

#[test]
fn directed_related_cycles_are_allowed_without_implying_replacement() {
    let dir = repo();
    let root = dir.path();
    write_adr(root, "proposed", BODY, "", "related_adrs: [\"0002\"]\n");
    let second = ".badr/adrs/0002-other.md";
    fs::write(root.join(second),format!("---\nid: \"0002\"\nstatus: proposed\ndate: \"2020-01-01\"\nrelated_adrs: [\"0001\"]\n---\n{BODY}")).unwrap();
    let mut second_change = change("proposed", None, None);
    second_change["id"] = json!("0002");
    second_change["path"] = json!(second);
    let mut link1 = change("proposed", Some("proposed"), None);
    link1["related"] = json!(["0002"]);
    let mut link2 = second_change.clone();
    link2["from"] = json!("proposed");
    link2["related"] = json!(["0001"]);
    ledger(
        root,
        &[
            event(
                root,
                "alloc1",
                "allocate",
                vec![change("proposed", None, None)],
            ),
            event(root, "alloc2", "allocate", vec![second_change]),
            event(root, "link1", "link", vec![link1]),
            event(root, "link2", "link", vec![link2]),
        ],
    );
    report(root, 0);
    let human = run(root, &["validate"]);
    assert!(human.status.success());
    assert!(human.stderr.is_empty());
    let first = run(root, &["validate", "--json"]);
    assert_eq!(first.stdout, run(root, &["validate", "--json"]).stdout);
}

#[test]
fn lifecycle_notes_cannot_be_rewritten_in_an_intermediate_commit_then_restored() {
    let dir = repo();
    let root = dir.path();
    let notes = "## Lifecycle notes\n\n### 2020-01-01 Approval\nHuman approved.\n";
    write_adr(root, "approved", BODY, notes, "");
    let mut approval = change("approved", Some("proposed"), Some("approve"));
    approval["notes_sha256"] = json!(digest(notes.as_bytes()));
    let mut all = vec![
        event(
            root,
            "alloc",
            "allocate",
            vec![change("proposed", None, None)],
        ),
        event(root, "approve", "decide", vec![approval.clone()]),
    ];
    ledger(root, &all);
    commit(root);
    let mut linked = approval.clone();
    linked["from"] = json!("approved");
    linked["related"] = json!(["BRR-ORD-001"]);
    linked["notes_sha256"] = json!(digest(b""));
    all.push(event(root, "link", "link", vec![linked]));
    write_adr(
        root,
        "approved",
        BODY,
        "",
        "related_business_rules: [BRR-ORD-001]\n",
    );
    ledger(root, &all);
    commit(root);
    let mut unlinked = approval;
    unlinked["from"] = json!("approved");
    all.push(event(root, "unlink", "unlink", vec![unlinked]));
    write_adr(root, "approved", BODY, notes, "");
    ledger(root, &all);
    assert!(codes(&report(root, 2)).contains(&"lifecycle_notes_rewritten"));
}

#[test]
fn legitimate_note_append_preserves_the_approved_decision_digest() {
    let dir = repo();
    let root = dir.path();
    let old_notes = "## Lifecycle notes\n\n### 2020-01-01 Approval\nHuman approved.\n";
    let new_notes = format!(
        "{old_notes}\n### 2020-01-01 Deprecation\nHuman withdrew direction and recorded treatment of existing code.\n"
    );
    let mut approved = change("approved", Some("proposed"), Some("approve"));
    approved["notes_sha256"] = json!(digest(old_notes.as_bytes()));
    let mut all = vec![
        event(
            root,
            "alloc",
            "allocate",
            vec![change("proposed", None, None)],
        ),
        event(root, "approve", "decide", vec![approved.clone()]),
    ];
    write_adr(root, "approved", BODY, old_notes, "");
    ledger(root, &all);
    commit(root);
    let mut deprecated = approved;
    deprecated["from"] = json!("approved");
    deprecated["to"] = json!("deprecated");
    deprecated["notes_sha256"] = json!(digest(new_notes.as_bytes()));
    let mut withdrawal = event(root, "deprecate", "deprecate", vec![deprecated]);
    withdrawal["decision_gap"] = json!("No replacement selected");
    withdrawal["existing_code_treatment"] = json!("Retain existing code");
    all.push(withdrawal);
    write_adr(root, "deprecated", BODY, &new_notes, "");
    ledger(root, &all);
    report(root, 0);
}

#[test]
fn brr_group_activation_replaces_only_after_prior_approval_and_compatibility_confirmation() {
    let dir = repo();
    let root = dir.path();
    let second = ".badr/brrs/domains/ordering/rules/BRR-ORD-002-successor.feature";
    let second_body = BRR_BODY.replace("BRR-ORD-001", "BRR-ORD-002");
    write_brr(
        root,
        "superseded",
        "effective_from: '2020-01-01T00:00:00Z'\neffective_until: '2020-01-01T00:00:00Z'\nsuperseded_by: [BRR-ORD-002]\n",
        BRR_BODY,
    );
    fs::write(root.join(second),format!("---\nid: BRR-ORD-002\ndomain: ordering\nstatus: effective\nowners: [Product]\neffective_from: '2020-01-01T00:00:00Z'\nsupersedes: [BRR-ORD-001]\n---\n{second_body}")).unwrap();
    use std::io::Write;
    fs::OpenOptions::new()
        .append(true)
        .open(root.join(".badr/brrs/registry.yaml"))
        .unwrap()
        .write_all(b"  BRR-ORD-002: domains/ordering/rules/BRR-ORD-002-successor.feature\n")
        .unwrap();
    let mut all = Vec::new();
    for id in [1, 2] {
        for (op, from, to) in [
            ("allocate", None, "draft"),
            ("propose", Some("draft"), "proposed"),
            ("decide", Some("proposed"), "approved"),
        ] {
            let receipt = format!("decide-{id}");
            let mut c = brr_change(to, from, if op == "decide" { Some(&receipt) } else { None });
            if id == 2 {
                c["id"] = json!("BRR-ORD-002");
                c["path"] = json!(second);
                c["body_sha256"] = json!(digest(second_body.as_bytes()));
            }
            all.push(event(root, &format!("{op}-{id}"), op, vec![c]));
        }
    }
    let mut activation = brr_change("effective", Some("approved"), Some("decide-1"));
    activation["effective_from"] = json!("2020-01-01T00:00:00Z");
    all.push(event(
        root,
        "activate-1",
        "activate",
        vec![activation.clone()],
    ));
    let mut predecessor = activation;
    predecessor["from"] = json!("effective");
    predecessor["to"] = json!("superseded");
    predecessor["effective_until"] = json!("2020-01-01T00:00:00Z");
    predecessor["superseded_by"] = json!(["BRR-ORD-002"]);
    let mut successor = brr_change("effective", Some("approved"), Some("decide-2"));
    successor["id"] = json!("BRR-ORD-002");
    successor["path"] = json!(second);
    successor["body_sha256"] = json!(digest(second_body.as_bytes()));
    successor["supersedes"] = json!(["BRR-ORD-001"]);
    successor["effective_from"] = json!("2020-01-01T00:00:00Z");
    let mut group = event(root, "replace", "activate", vec![predecessor, successor]);
    group["replacement"] = json!({"predecessors":["BRR-ORD-001"],"successors":["BRR-ORD-002"],"complete":true,"compatibility_confirmed":true});
    all.push(group);
    ledger(root, &all);
    report(root, 0);
    let end = all.len() - 1;
    let mut bad = all.clone();
    bad[end]["replacement"]["compatibility_confirmed"] = json!(false);
    ledger(root, &bad);
    report(root, 2);
    let mut bad = all;
    bad[end]["changes"][0]["effective_from"] = json!("2019-01-01T00:00:00Z");
    ledger(root, &bad);
    report(root, 2);
}

#[cfg(unix)]
#[test]
fn unsafe_evidence_files_are_never_opened_or_repaired() {
    use std::os::unix::fs::symlink;
    let dir = repo();
    let root = dir.path();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("keep"), "untouched").unwrap();
    let path = root.join(".badr/evidence.jsonl");
    symlink(outside.path().join("keep"), &path).unwrap();
    report(root, 2);
    assert_eq!(
        fs::read_to_string(outside.path().join("keep")).unwrap(),
        "untouched"
    );
    fs::remove_file(&path).unwrap();
    assert!(
        Command::new("mkfifo")
            .arg(&path)
            .status()
            .unwrap()
            .success()
    );
    report(root, 2);
}

#[test]
fn replacement_sets_support_multiple_predecessors_and_successors_and_reject_cycles() {
    let dir = repo();
    let root = dir.path();
    let mut all = Vec::new();
    let mut changes = Vec::new();
    for number in 1..=4 {
        let id = format!("{number:04}");
        let path = format!(".badr/adrs/{id}-record.md");
        let predecessor = number <= 2;
        let mut allocated = change("proposed", None, None);
        allocated["id"] = json!(id);
        allocated["path"] = json!(path);
        all.push(event(
            root,
            &format!("alloc-{number}"),
            "allocate",
            vec![allocated.clone()],
        ));
        let receipt = format!("approve-{number}");
        if predecessor {
            let mut approved = allocated.clone();
            approved["from"] = json!("proposed");
            approved["to"] = json!("approved");
            approved["approval_ref"] = json!(receipt);
            all.push(event(root, &receipt, "decide", vec![approved]));
        }
        let mut replaced = allocated;
        replaced["from"] = json!(if predecessor { "approved" } else { "proposed" });
        replaced["to"] = json!(if predecessor {
            "superseded"
        } else {
            "approved"
        });
        replaced["approval_ref"] = json!(if predecessor {
            receipt.as_str()
        } else {
            "replace"
        });
        replaced[if predecessor {
            "superseded_by"
        } else {
            "supersedes"
        }] = if predecessor {
            json!(["0003", "0004"])
        } else {
            json!(["0001", "0002"])
        };
        fs::write(
            root.join(&path),
            format!(
                "---\nid: \"{id}\"\nstatus: {}\ndate: \"2020-01-01\"\n{}\n---\n{BODY}",
                if predecessor {
                    "superseded"
                } else {
                    "approved"
                },
                if predecessor {
                    "superseded_by: [\"0003\", \"0004\"]"
                } else {
                    "supersedes: [\"0001\", \"0002\"]"
                }
            ),
        )
        .unwrap();
        changes.push(replaced);
    }
    let mut group = event(root, "replace", "decide", changes);
    group["replacement"] =
        json!({"predecessors":["0001","0002"],"successors":["0003","0004"],"complete":true});
    all.push(group);
    ledger(root, &all);
    report(root, 0);
    let last = all.len() - 1;
    let mut cycle = all.clone();
    cycle[last]["replacement"]["successors"] = json!(["0001", "0004"]);
    ledger(root, &cycle);
    report(root, 2);
    let mut incomplete = all;
    incomplete[last]["changes"].as_array_mut().unwrap().pop();
    ledger(root, &incomplete);
    report(root, 2);
}

#[test]
fn transient_missing_references_cannot_be_hidden_by_unlinking_before_commit() {
    let dir = repo();
    let root = dir.path();
    write_adr(root, "proposed", BODY, "", "");
    let mut linked = change("proposed", Some("proposed"), None);
    linked["related"] = json!(["0002"]);
    ledger(
        root,
        &[
            event(
                root,
                "alloc",
                "allocate",
                vec![change("proposed", None, None)],
            ),
            event(root, "link", "link", vec![linked]),
            event(
                root,
                "unlink",
                "unlink",
                vec![change("proposed", Some("proposed"), None)],
            ),
        ],
    );
    assert!(codes(&report(root, 2)).contains(&"missing_reference"));
}

#[test]
fn fractional_second_withdrawal_follows_activation_chronologically() {
    let dir = repo();
    let root = dir.path();
    let mut active = brr_change("effective", Some("approved"), Some("approve"));
    active["effective_from"] = json!("2020-01-01T00:00:00Z");
    let mut retired = active.clone();
    retired["from"] = json!("effective");
    retired["to"] = json!("retired");
    retired["effective_until"] = json!("2020-01-01T00:00:00.5Z");
    let mut retirement = event(root, "retire", "retire", vec![retired]);
    retirement["at"] = json!("2020-01-01T00:00:00.5Z");
    write_brr(
        root,
        "retired",
        "effective_from: '2020-01-01T00:00:00Z'\neffective_until: '2020-01-01T00:00:00.5Z'\n",
        BRR_BODY,
    );
    ledger(
        root,
        &[
            event(
                root,
                "alloc",
                "allocate",
                vec![brr_change("draft", None, None)],
            ),
            event(
                root,
                "propose",
                "propose",
                vec![brr_change("proposed", Some("draft"), None)],
            ),
            event(
                root,
                "approve",
                "decide",
                vec![brr_change("approved", Some("proposed"), Some("approve"))],
            ),
            event(root, "activate", "activate", vec![active]),
            retirement,
        ],
    );
    report(root, 0);
}

#[cfg(unix)]
#[test]
fn git_history_execution_failure_is_operational_not_incomplete() {
    use std::os::unix::fs::PermissionsExt;
    let dir = repo();
    let root = dir.path();
    let shim = tempfile::tempdir().unwrap();
    let executable = Command::new("sh")
        .args(["-c", "command -v git"])
        .output()
        .unwrap();
    assert!(executable.status.success());
    let executable = String::from_utf8(executable.stdout).unwrap();
    let executable = executable.trim().replace('\'', "'\\''");
    let path = shim.path().join("git");
    fs::write(&path,format!("#!/bin/sh\ncase \" $* \" in *' rev-list '*) printf 'simulated Git I/O failure\\n' >&2; exit 128;; esac\nexec '{executable}' \"$@\"\n")).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_badr"))
        .args(["--repo", root.to_str().unwrap(), "validate", "--json"])
        .env(
            "PATH",
            format!(
                "{}:{}",
                shim.path().display(),
                std::env::var("PATH").unwrap()
            ),
        )
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(4),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
}

#[test]
fn baseline_reconfiguration_cannot_unfreeze_a_durable_record() {
    let dir = repo();
    let root = dir.path();
    let initial = git(root, &["rev-parse", "trunk"]);
    let all = vec![
        event(
            root,
            "alloc",
            "allocate",
            vec![change("proposed", None, None)],
        ),
        event(
            root,
            "approve",
            "decide",
            vec![change("approved", Some("proposed"), Some("approve"))],
        ),
    ];
    write_adr(root, "approved", BODY, "", "");
    ledger(root, &all);
    commit(root);
    git(root, &["branch", "older", &initial]);
    git(root, &["checkout", "-b", "feature"]);
    fs::write(
        root.join(".badr/config.yaml"),
        "version: 1\nbaseline_branch: older\n",
    )
    .unwrap();
    let modified = BODY.replace("Simplicity.", "Privacy.");
    let mut reset = change("proposed", Some("approved"), Some("approve"));
    reset["body_sha256"] = json!(digest(modified.as_bytes()));
    let mut event = event(root, "reset", "edit", vec![reset]);
    event["baseline_commit"] = json!(initial);
    let mut all = all;
    all.push(event);
    ledger(root, &all);
    write_adr(root, "proposed", &modified, "", "");
    assert!(codes(&report(root, 2)).contains(&"baseline_configuration_changed"));
}

#[test]
fn approval_receipts_require_resolved_review_declaration() {
    for declaration in [None, Some(json!(false))] {
        let dir = repo();
        let root = dir.path();
        write_adr(root, "approved", BODY, "", "");
        let mut approval = event(
            root,
            "approve-1",
            "decide",
            vec![change("approved", Some("proposed"), Some("approve-1"))],
        );
        approval.as_object_mut().unwrap().remove("reviewed");
        if let Some(value) = declaration {
            approval["reviewed"] = value;
        }
        ledger(
            root,
            &[
                event(
                    root,
                    "allocate-1",
                    "allocate",
                    vec![change("proposed", None, None)],
                ),
                approval,
            ],
        );
        let bytes = fs::read(root.join(".badr/evidence.jsonl")).unwrap();
        assert!(codes(&report(root, 2)).contains(&"inconsistent_evidence"));
        assert_eq!(fs::read(root.join(".badr/evidence.jsonl")).unwrap(), bytes);
    }
}
