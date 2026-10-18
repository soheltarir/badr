use super::{Record, Report, git};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs, io,
    path::{Component, Path},
    time::SystemTime,
};

const LEDGER: &str = ".badr/evidence.jsonl";
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Event {
    version: u8,
    event_id: String,
    operation: String,
    at: String,
    branch: String,
    baseline_commit: String,
    actor: Option<String>,
    rationale: Option<String>,
    confirmed: Option<bool>,
    reviewed: Option<bool>,
    replacement: Option<Replacement>,
    changes: Vec<Change>,
    domain: Option<Domain>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Domain {
    slug: String,
    prefix: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Replacement {
    predecessors: Vec<String>,
    successors: Vec<String>,
    complete: bool,
    compatibility_confirmed: Option<bool>,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Change {
    id: String,
    path: String,
    from: Option<String>,
    to: String,
    body_sha256: String,
    approval_ref: Option<String>,
    date: Option<String>,
    notes_sha256: Option<String>,
    effective_from: Option<String>,
    effective_until: Option<String>,
    #[serde(default)]
    related: Vec<String>,
    #[serde(default)]
    supersedes: Vec<String>,
    #[serde(default)]
    superseded_by: Vec<String>,
}
struct State {
    revision: Change,
    approval: Option<(String, String)>,
    activated: bool,
    replaced: bool,
}
fn hash(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}
fn hex(value: &str, lengths: &[usize]) -> bool {
    lengths.contains(&value.len())
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn identity(value: &str) -> Option<String> {
    if let Some(rest) = value.strip_prefix("BRR-") {
        let (prefix, number) = rest.split_once('-')?;
        if prefix.is_empty()
            || !prefix.bytes().all(|b| b.is_ascii_uppercase())
            || number.len() < 3
            || !number.bytes().all(|b| b.is_ascii_digit())
        {
            return None;
        }
        Some(format!("BRR-{prefix}-{}", number.trim_start_matches('0')))
    } else if value.len() >= 4 && value.bytes().all(|b| b.is_ascii_digit()) {
        Some(format!("ADR-{}", value.trim_start_matches('0')))
    } else {
        None
    }
}
fn same_ids(a: &[String], b: &[String]) -> bool {
    a.iter().map(|s| identity(s)).collect::<BTreeSet<_>>()
        == b.iter().map(|s| identity(s)).collect::<BTreeSet<_>>()
}
fn issue(
    report: &mut Report,
    code: &str,
    message: impl Into<String>,
    path: &str,
    id: Option<&str>,
    line: Option<usize>,
) {
    report.fail(2, code, message.into(), Some(Path::new(path)));
    let d = report.diagnostics.last_mut().unwrap();
    d.record_id = id.map(str::to_owned);
    d.stored_file_line = line;
}
fn events(text: &str, report: &mut Report) -> Vec<Event> {
    let mut result = Vec::new();
    for (line, text) in text.lines().enumerate() {
        match serde_json::from_str(text) {
            Ok(event) => result.push(event),
            Err(e) => {
                issue(
                    report,
                    "invalid_evidence",
                    e.to_string(),
                    LEDGER,
                    None,
                    Some(line + 1),
                );
                break;
            }
        }
    }
    if !text.is_empty() && !text.ends_with('\n') {
        issue(
            report,
            "invalid_evidence",
            "Evidence must end in a newline; a partial event is not durable evidence",
            LEDGER,
            None,
            None,
        );
    }
    result
}
fn valid_change(change: &Change, at: &str) -> Result<(), String> {
    let brr = change.id.starts_with("BRR-");
    let expected_root = if brr {
        ".badr/brrs/domains/"
    } else {
        ".badr/adrs/"
    };
    let path = Path::new(&change.path);
    if identity(&change.id).is_none()
        || !change.path.starts_with(expected_root)
        || !path.components().all(|c| matches!(c, Component::Normal(_)))
        || change.path.contains('\\')
        || change.path.contains("//")
        || change.path.contains("/./")
        || !path.file_name().and_then(|s| s.to_str()).is_some_and(|s| {
            s.starts_with(&format!("{}-", change.id))
                && s.ends_with(if brr { ".feature" } else { ".md" })
        })
        || (!brr && path.components().count() != 3)
    {
        return Err("Invalid record identity or fixed-root path".into());
    }
    if !hex(&change.body_sha256, &[64])
        || change
            .notes_sha256
            .as_deref()
            .is_some_and(|s| !hex(s, &[64]))
    {
        return Err("Expected lowercase SHA-256 revision references".into());
    }
    for ids in [&change.related, &change.supersedes, &change.superseded_by] {
        let keys: BTreeSet<_> = ids.iter().map(|id| identity(id)).collect();
        if keys.contains(&None) || keys.len() != ids.len() {
            return Err("Invalid or duplicate relationship IDs".into());
        }
    }
    if change
        .supersedes
        .iter()
        .chain(&change.superseded_by)
        .any(|s| s.starts_with("BRR-") != brr)
    {
        return Err("Replacement links must have the same record type".into());
    }
    if brr {
        if change.date.is_some() || change.notes_sha256.is_some() {
            return Err("BRR evidence cannot contain ADR date/notes".into());
        }
        match change.to.as_str() {
            "draft" | "proposed" | "approved"
                if change.effective_from.is_none() && change.effective_until.is_none() => {}
            "effective" if change.effective_from.is_some() && change.effective_until.is_none() => {}
            "deprecated" | "superseded" | "retired"
                if change.effective_from.is_some() && change.effective_until.is_some() => {}
            _ => return Err("Invalid BRR evidenced applicability".into()),
        }
        for time in [&change.effective_from, &change.effective_until]
            .into_iter()
            .flatten()
        {
            let parsed = DateTime::parse_from_rfc3339(time).map_err(|e| e.to_string())?;
            if !time.ends_with('Z')
                || parsed > DateTime::parse_from_rfc3339(at).map_err(|e| e.to_string())?
            {
                return Err("Evidence applicability times must be actual UTC events".into());
            }
        }
        if let (Some(from), Some(until)) = (&change.effective_from, &change.effective_until) {
            if DateTime::parse_from_rfc3339(until).map_err(|e| e.to_string())?
                < DateTime::parse_from_rfc3339(from).map_err(|e| e.to_string())?
            {
                return Err("Applicability ends before activation".into());
            }
        }
    } else if change.date.as_deref() != at.get(..10)
        || change.notes_sha256.is_none()
        || change.effective_from.is_some()
        || change.effective_until.is_some()
        || !matches!(
            change.to.as_str(),
            "proposed" | "approved" | "rejected" | "deprecated" | "superseded"
        )
    {
        return Err(
            "ADR changes require the actual event date, notes digest, and legal status".into(),
        );
    }
    Ok(())
}
fn replay(events: &[Event], available: [bool; 2], report: &mut Report) -> BTreeMap<String, State> {
    let mut states: BTreeMap<String, State> = BTreeMap::new();
    let mut ids = BTreeSet::new();
    let mut previous = None;
    let mut domains = BTreeMap::new();
    for (line, event) in events.iter().enumerate() {
        let result = (|| -> Result<(), String> {
            if event.version != 1
                || event.event_id.trim().is_empty()
                || !ids.insert(&event.event_id)
                || (event.changes.is_empty() && event.operation != "domain_add")
            {
                return Err(
                    "Unsupported version, duplicate/empty event identity, or empty changes".into(),
                );
            }
            let at = DateTime::parse_from_rfc3339(&event.at).map_err(|e| e.to_string())?;
            let now: DateTime<Utc> = SystemTime::now().into();
            if !event.at.ends_with('Z')
                || event.at.as_bytes().get(10) != Some(&b'T')
                || at > now
                || previous.is_some_and(|p| at < p)
                || event.branch.trim().is_empty()
                || !hex(&event.baseline_commit, &[40, 64])
            {
                return Err(
                    "Event requires chronological actual UTC time, branch, and resolved commit"
                        .into(),
                );
            }
            previous = Some(at);
            if event.operation != "allocate"
                && (event.actor.as_ref().is_none_or(|s| s.trim().is_empty())
                    || event.rationale.as_ref().is_none_or(|s| s.trim().is_empty())
                    || event.confirmed != Some(true))
            {
                return Err(
                    "Governed changes require actor, rationale, and explicit confirmation".into(),
                );
            }
            if event.operation == "decide" && event.reviewed != Some(true) {
                return Err("Approval requires a declaration that applicable content and gaps were reviewed and resolved".into());
            }
            if event.operation == "domain_add" {
                let domain = event
                    .domain
                    .as_ref()
                    .ok_or("Domain registration requires slug/prefix")?;
                if !event.changes.is_empty()
                    || event.replacement.is_some()
                    || !super::brr::slug(&domain.slug)
                    || !super::brr::prefix(&domain.prefix)
                    || domains.contains_key(&domain.slug)
                    || domains.values().any(|p| *p == &domain.prefix)
                {
                    return Err("Invalid or repeated domain/prefix registration".into());
                }
                domains.insert(&domain.slug, &domain.prefix);
                return Ok(());
            }
            if event.domain.is_some() {
                return Err("Only domain_add may contain domain registration".into());
            }
            if event.replacement.is_none() && event.changes.len() != 1 {
                return Err(
                    "Multiple changes require an explicit complete replacement declaration".into(),
                );
            }
            validate_group(event, &states)?;
            let mut next = Vec::new();
            let mut changed = BTreeSet::new();
            for change in &event.changes {
                valid_change(change, &event.at)?;
                let key = identity(&change.id).unwrap();
                if !changed.insert(key.clone()) {
                    return Err("Duplicate affected identity".into());
                }
                let old = states.get(&key);
                if old.map(|s| s.revision.to.as_str()) != change.from.as_deref() {
                    return Err(format!("{} prior state disagrees with replay", change.id));
                }
                let brr = change.id.starts_with("BRR-");
                let predecessor = event.replacement.as_ref().is_some_and(|r| {
                    r.predecessors
                        .iter()
                        .any(|id| identity(id) == Some(key.clone()))
                });
                let legal = if predecessor {
                    change.to == "superseded"
                        && matches!(change.from.as_deref(), Some("effective") if brr)
                        || change.to == "superseded"
                            && matches!(change.from.as_deref(), Some("approved" | "deprecated"))
                            && !brr
                } else {
                    match event.operation.as_str() {
                        "allocate" => {
                            old.is_none() && change.to == if brr { "draft" } else { "proposed" }
                        }
                        "propose" => {
                            brr && change.from.as_deref() == Some("draft")
                                && change.to == "proposed"
                        }
                        "decide" => {
                            change.from.as_deref() == Some("proposed") && change.to == "approved"
                        }
                        "edit" => {
                            change.from.as_deref() == Some("approved")
                                && change.to == "proposed"
                                && old.is_some_and(|s| !s.activated && !s.replaced)
                        }
                        "activate" => {
                            brr && change.from.as_deref() == Some("approved")
                                && change.to == "effective"
                        }
                        "reject" => {
                            !brr && change.from.as_deref() == Some("proposed")
                                && change.to == "rejected"
                        }
                        "deprecate" => {
                            change.from.as_deref()
                                == Some(if brr { "effective" } else { "approved" })
                                && change.to == "deprecated"
                        }
                        "retire" => {
                            brr && change.from.as_deref() == Some("effective")
                                && change.to == "retired"
                        }
                        "supersede" => {
                            event.replacement.is_some()
                                && change.to == if brr { "effective" } else { "approved" }
                                && change.from.as_deref() == Some(change.to.as_str())
                        }
                        "link" | "unlink" => {
                            old.is_some() && change.from.as_deref() == Some(change.to.as_str())
                        }
                        _ => false,
                    }
                };
                if !legal {
                    return Err(format!(
                        "Illegal {} transition for {}",
                        event.operation, change.id
                    ));
                }
                if let Some(old) = old {
                    if old.revision.path != change.path {
                        return Err("Record identity/path cannot be reassigned".into());
                    }
                    if event.operation != "edit"
                        && frozen(&old.revision.to)
                        && old.revision.body_sha256 != change.body_sha256
                    {
                        return Err("An operation cannot replace a frozen body digest".into());
                    }
                    if event.replacement.is_none()
                        && (!same_ids(&old.revision.supersedes, &change.supersedes)
                            || !same_ids(&old.revision.superseded_by, &change.superseded_by))
                    {
                        return Err("Replacement links require a declared group".into());
                    }
                    if event.operation != "link"
                        && event.operation != "unlink"
                        && !same_ids(&old.revision.related, &change.related)
                    {
                        return Err("Related links require link/unlink evidence".into());
                    }
                    if matches!(event.operation.as_str(), "link" | "unlink") {
                        let before: BTreeSet<_> =
                            old.revision.related.iter().map(|id| identity(id)).collect();
                        let after: BTreeSet<_> =
                            change.related.iter().map(|id| identity(id)).collect();
                        if before == after
                            || (event.operation == "link" && !before.is_subset(&after))
                            || (event.operation == "unlink" && !after.is_subset(&before))
                        {
                            return Err(
                                "Link adds and unlink removes directed related references only"
                                    .into(),
                            );
                        }
                    }
                    if event.operation == "edit" && old.revision.body_sha256 == change.body_sha256 {
                        return Err("An editor no-op cannot reset approval".into());
                    }
                    if (event.operation != "activate" || predecessor)
                        && old.revision.effective_from != change.effective_from
                    {
                        return Err("Activation time must be retained".into());
                    }
                    if !predecessor
                        && !matches!(event.operation.as_str(), "deprecate" | "retire")
                        && old.revision.effective_until != change.effective_until
                    {
                        return Err("Withdrawal time must be retained".into());
                    }
                } else if !change.supersedes.is_empty()
                    || !change.superseded_by.is_empty()
                    || !change.related.is_empty()
                {
                    return Err("Allocation cannot imply links or replacement".into());
                }
                if event.operation == "activate"
                    && !predecessor
                    && change.effective_from.as_deref() != Some(&event.at)
                    || (predecessor || matches!(event.operation.as_str(), "deprecate" | "retire"))
                        && brr
                        && change.effective_until.as_deref() != Some(&event.at)
                {
                    return Err("Activation/withdrawal must record the actual event time".into());
                }
                let approval = if event.operation == "decide" && !predecessor {
                    if change.approval_ref.as_deref() != Some(&event.event_id) {
                        return Err("Approval must reference its own receipt".into());
                    }
                    Some((event.event_id.clone(), change.body_sha256.clone()))
                } else {
                    if change.approval_ref.as_deref()
                        != old.and_then(|s| s.approval.as_ref().map(|a| a.0.as_str()))
                    {
                        return Err(
                            "Revision references an inactive or missing approval receipt".into(),
                        );
                    }
                    if event.operation == "edit" {
                        None
                    } else {
                        old.and_then(|s| s.approval.clone())
                    }
                };
                next.push((
                    key,
                    State {
                        activated: old.is_some_and(|s| s.activated)
                            || event.operation == "activate",
                        replaced: old.is_some_and(|s| s.replaced) || event.replacement.is_some(),
                        revision: change.clone(),
                        approval,
                    },
                ));
            }
            states.extend(next);
            references(&states, available, report);
            if event.replacement.is_some() {
                replacement_graph(&states)?;
            }
            Ok(())
        })();
        if let Err(message) = result {
            issue(
                report,
                "inconsistent_evidence",
                message,
                LEDGER,
                None,
                Some(line + 1),
            );
            break;
        }
    }
    states
}
fn validate_group(event: &Event, states: &BTreeMap<String, State>) -> Result<(), String> {
    let Some(group) = &event.replacement else {
        return Ok(());
    };
    let predecessors: BTreeSet<_> = group
        .predecessors
        .iter()
        .filter_map(|id| identity(id))
        .collect();
    let successors: BTreeSet<_> = group
        .successors
        .iter()
        .filter_map(|id| identity(id))
        .collect();
    let affected: BTreeSet<_> = event
        .changes
        .iter()
        .filter_map(|c| identity(&c.id))
        .collect();
    if !group.complete
        || predecessors.is_empty()
        || successors.is_empty()
        || predecessors.len() != group.predecessors.len()
        || successors.len() != group.successors.len()
        || !predecessors.is_disjoint(&successors)
        || predecessors
            .union(&successors)
            .cloned()
            .collect::<BTreeSet<_>>()
            != affected
        || affected.len() != event.changes.len()
    {
        return Err(
            "Replacement must explicitly confirm the complete predecessor/successor set".into(),
        );
    }
    let brr = event.changes[0].id.starts_with("BRR-");
    if event
        .changes
        .iter()
        .any(|c| c.id.starts_with("BRR-") != brr)
        || !matches!(
            (brr, event.operation.as_str()),
            (true, "activate" | "supersede") | (false, "decide" | "supersede")
        )
        || (brr && group.compatibility_confirmed != Some(true))
    {
        return Err("Replacement requires one record type, a legal operation, and BRR compatibility review confirmation".into());
    }
    for change in &event.changes {
        let key = identity(&change.id).ok_or("Invalid replacement identity")?;
        let old = &states
            .get(&key)
            .ok_or("Replacement targets must already be allocated")?
            .revision;
        let mut expected_predecessors = old.supersedes.clone();
        let mut expected_successors = old.superseded_by.clone();
        if predecessors.contains(&key) {
            expected_successors.extend(group.successors.clone());
        } else {
            expected_predecessors.extend(group.predecessors.clone());
        }
        if !same_ids(&expected_predecessors, &change.supersedes)
            || !same_ids(&expected_successors, &change.superseded_by)
        {
            return Err(
                "Completed replacement requires exact reciprocal links and retained previous links"
                    .into(),
            );
        }
    }
    Ok(())
}
fn replacement_graph(states: &BTreeMap<String, State>) -> Result<(), String> {
    let mut incoming: BTreeMap<String, usize> = states.keys().map(|key| (key.clone(), 0)).collect();
    for state in states.values() {
        for target in &state.revision.supersedes {
            *incoming
                .entry(identity(target).ok_or("Invalid replacement target")?)
                .or_default() += 1;
        }
    }
    let mut ready: Vec<_> = incoming
        .iter()
        .filter(|(_, n)| **n == 0)
        .map(|(id, _)| id.clone())
        .collect();
    let mut visited = 0;
    while let Some(id) = ready.pop() {
        visited += 1;
        if let Some(state) = states.get(&id) {
            for target in &state.revision.supersedes {
                let target = identity(target).unwrap();
                let count = incoming.get_mut(&target).unwrap();
                *count -= 1;
                if *count == 0 {
                    ready.push(target);
                }
            }
        }
    }
    if visited != incoming.len() {
        return Err("Replacement graph contains a cycle".into());
    }
    Ok(())
}

fn references(states: &BTreeMap<String, State>, available: [bool; 2], report: &mut Report) {
    for state in states.values() {
        let c = &state.revision;
        for id in c
            .related
            .iter()
            .chain(&c.supersedes)
            .chain(&c.superseded_by)
        {
            if !states.contains_key(&identity(id).unwrap()) {
                if available[usize::from(id.starts_with("BRR-"))] {
                    issue(
                        report,
                        "missing_reference",
                        format!("Missing target {id} inside an available collection"),
                        &c.path,
                        Some(&c.id),
                        None,
                    );
                } else {
                    report.fail(
                        3,
                        "unverified_reference",
                        format!("Reference {id} is unverified: supporting collection is absent"),
                        Some(Path::new(&c.path)),
                    );
                }
            }
        }
    }
}
fn frozen(status: &str) -> bool {
    !matches!(status, "draft" | "proposed")
}
fn compare(
    states: &BTreeMap<String, State>,
    records: &[Record],
    report: &mut Report,
    reset_target: Option<&str>,
) {
    let mut present = BTreeSet::new();
    for record in records {
        let id = record.id();
        let key = identity(id).unwrap();
        present.insert(key.clone());
        let Some(state) = states.get(&key) else {
            issue(
                report,
                "missing_evidence",
                "Record has no allocation/transition evidence",
                record.path(),
                Some(id),
                None,
            );
            continue;
        };
        let c = &state.revision;
        let (status, body, notes, related, supersedes, superseded_by, date, from, until) =
            match record {
                Record::Brr(r) => (
                    &r.metadata.status,
                    r.body.as_str(),
                    "",
                    r.metadata.related_architecture_decisions.clone(),
                    &r.metadata.supersedes,
                    &r.metadata.superseded_by,
                    None,
                    r.metadata.effective_from.as_deref(),
                    r.metadata.effective_until.as_deref(),
                ),
                Record::Adr(r) => (
                    &r.metadata.status,
                    r.decision_body.as_deref().unwrap_or(""),
                    r.lifecycle_notes.as_str(),
                    r.metadata
                        .related_adrs
                        .iter()
                        .chain(&r.metadata.related_business_rules)
                        .cloned()
                        .collect(),
                    &r.metadata.supersedes,
                    &r.metadata.superseded_by,
                    Some(r.metadata.date.as_str()),
                    None,
                    None,
                ),
            };
        if c.path != record.path()
            || c.to != *status
            || !same_ids(&c.related, &related)
            || !same_ids(&c.supersedes, supersedes)
            || !same_ids(&c.superseded_by, superseded_by)
            || c.date.as_deref() != date
            || !same_time(c.effective_from.as_deref(), from)
            || !same_time(c.effective_until.as_deref(), until)
        {
            issue(
                report,
                "record_evidence_mismatch",
                "Current status, path, date, applicability, or relationships disagree with replay",
                record.path(),
                Some(id),
                None,
            );
        }
        if frozen(status)
            && hash(body) != c.body_sha256
            && !(reset_target == Some(id) && status == "approved")
        {
            issue(
                report,
                "body_digest_mismatch",
                "Exact UTF-8 body differs from its active frozen revision",
                record.path(),
                Some(id),
                None,
            );
        }
        if c.notes_sha256
            .as_ref()
            .is_some_and(|digest| *digest != hash(notes))
        {
            issue(
                report,
                "notes_digest_mismatch",
                "Lifecycle notes differ from the retained event revision",
                record.path(),
                Some(id),
                None,
            );
        }
    }
    for (id, state) in states {
        if !present.contains(id) {
            issue(
                report,
                "removed_record",
                "An allocated record is missing or structurally invalid",
                &state.revision.path,
                Some(&state.revision.id),
                None,
            );
        }
    }
}
fn same_time(a: Option<&str>, b: Option<&str>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => {
            DateTime::parse_from_rfc3339(a).ok() == DateTime::parse_from_rfc3339(b).ok()
        }
        _ => false,
    }
}
enum HistoryError {
    Unavailable(String),
    Operational(io::Error),
}
impl From<io::Error> for HistoryError {
    fn from(error: io::Error) -> Self {
        Self::Operational(error)
    }
}
impl From<String> for HistoryError {
    fn from(message: String) -> Self {
        Self::Unavailable(message)
    }
}
impl From<&str> for HistoryError {
    fn from(message: &str) -> Self {
        Self::Unavailable(message.into())
    }
}
fn git_text(root: &Path, args: &[&str]) -> Result<String, HistoryError> {
    let out = git(root, args)?;
    if !out.status.success() {
        return Err(io::Error::other(String::from_utf8_lossy(&out.stderr).into_owned()).into());
    }
    String::from_utf8(out.stdout).map_err(|e| HistoryError::Unavailable(e.to_string()))
}
fn require_revision(root: &Path, reference: &str) -> Result<(), HistoryError> {
    let out = git(root, &["rev-parse", "--verify", "--quiet", reference])?;
    if out.status.success() {
        return Ok(());
    }
    if out.stderr.is_empty() {
        Err(HistoryError::Unavailable(format!(
            "Missing local revision {reference}"
        )))
    } else {
        Err(io::Error::other(String::from_utf8_lossy(&out.stderr).into_owned()).into())
    }
}
fn retained(
    root: &Path,
    commit: &str,
    expected_baseline: &str,
    report: &mut Report,
) -> Result<(String, Vec<Record>, [bool; 2]), HistoryError> {
    let out = git(root, &["ls-tree", "-r", "-z", commit, "--", ".badr"])?;
    if !out.status.success() {
        return Err(io::Error::other(String::from_utf8_lossy(&out.stderr).into_owned()).into());
    }
    let mut ledger = String::new();
    let mut domains = BTreeMap::new();
    let mut glossaries = BTreeSet::new();
    let mut records = Vec::new();
    let mut available = [false; 2];
    for entry in out.stdout.split(|b| *b == 0).filter(|s| !s.is_empty()) {
        let entry = std::str::from_utf8(entry).map_err(|e| e.to_string())?;
        let (meta, path) = entry.split_once('\t').ok_or("Malformed Git tree")?;
        let fields: Vec<_> = meta.split_whitespace().collect();
        available[0] |= path.starts_with(".badr/adrs/");
        available[1] |= path.starts_with(".badr/brrs/");
        let record = (path.starts_with(".badr/adrs/") && path.ends_with(".md"))
            || (path.starts_with(".badr/brrs/") && path.ends_with(".feature"));
        if path.starts_with(".badr/brrs/domains/")
            && path.ends_with("/glossary.md")
            && fields.len() == 3
            && matches!(fields[0], "100644" | "100755")
        {
            glossaries.insert(path.to_owned());
        }
        if path != LEDGER
            && path != ".badr/config.yaml"
            && path != ".badr/brrs/registry.yaml"
            && !record
        {
            continue;
        }
        if fields.len() != 3 || !matches!(fields[0], "100644" | "100755") || fields[1] != "blob" {
            issue(
                report,
                "unsafe_history_path",
                format!("Non-regular retained file in {commit}"),
                path,
                None,
                None,
            );
            continue;
        }
        let blob = git(root, &["cat-file", "blob", fields[2]])?;
        if !blob.status.success() {
            return Err(
                io::Error::other(String::from_utf8_lossy(&blob.stderr).into_owned()).into(),
            );
        }
        let text = match String::from_utf8(blob.stdout) {
            Ok(text) => text,
            Err(error) => {
                issue(
                    report,
                    "invalid_historical_utf8",
                    format!("{commit}: {error}"),
                    path,
                    None,
                    None,
                );
                continue;
            }
        };
        if path == LEDGER {
            ledger = text;
        } else if path == ".badr/brrs/registry.yaml" {
            match super::brr::parse_registry(&text) {
                Ok(registry) => domains = registry.domains,
                Err(error) => issue(
                    report,
                    "invalid_historical_registry",
                    format!("{commit}: {error}"),
                    path,
                    None,
                    None,
                ),
            }
        } else if path == ".badr/config.yaml" {
            match yaml_serde::from_str::<super::Config>(&text) {
                Ok(config)
                    if config.version == 1 && config.baseline_branch == expected_baseline => {}
                Ok(_) => issue(
                    report,
                    "baseline_configuration_changed",
                    format!(
                        "{commit}: committed baseline configuration cannot be retargeted; v1 has no baseline migration operation"
                    ),
                    path,
                    None,
                    None,
                ),
                Err(error) => issue(
                    report,
                    "invalid_historical_configuration",
                    format!("{commit}: {error}"),
                    path,
                    None,
                    None,
                ),
            }
        } else {
            let parsed = if path.ends_with(".feature") {
                super::brr::parse(&text, path).map(Record::Brr)
            } else {
                crate::adr::retained(&text, path).map(Record::Adr)
            };
            match parsed {
                Ok(record) => records.push(record),
                Err(e) => issue(
                    report,
                    "invalid_historical_record",
                    format!("{commit}: {e}"),
                    path,
                    None,
                    None,
                ),
            }
        }
    }
    for slug in domains.keys() {
        let glossary = format!(".badr/brrs/domains/{slug}/glossary.md");
        if !glossaries.contains(&glossary) {
            issue(
                report,
                "missing_historical_glossary",
                format!("{commit}: registered domain requires its glossary"),
                &glossary,
                None,
                None,
            );
        }
    }
    check_registered_domains(&events(&ledger, report), &domains, report);
    Ok((ledger, records, available))
}
fn baseline_contains(root: &Path, commit: &str, id: &str) -> Result<bool, HistoryError> {
    let paths = git_text(
        root,
        &[
            "log",
            "--full-history",
            "--format=",
            "--name-only",
            commit,
            "--",
            ".badr/adrs",
            ".badr/brrs",
        ],
    )?;
    let key = identity(id);
    Ok(paths.lines().any(|p| {
        let filename = Path::new(p)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        let name = if filename.starts_with("BRR-") {
            filename.split('-').take(3).collect::<Vec<_>>().join("-")
        } else {
            filename.split('-').next().unwrap_or("").into()
        };
        identity(&name).is_some() && identity(&name) == key
    }))
}
struct Snapshot {
    commit: String,
    ledger: String,
    records: Vec<Record>,
    parents: Vec<String>,
}
fn notes_append_only(older: &[Record], newer: &[Record], commit: &str, report: &mut Report) {
    for old in older {
        if let Record::Adr(old) = old {
            if let Some(Record::Adr(new)) = newer
                .iter()
                .find(|r| identity(r.id()) == identity(&old.metadata.id))
            {
                if !new.lifecycle_notes.starts_with(&old.lifecycle_notes) {
                    issue(
                        report,
                        "lifecycle_notes_rewritten",
                        format!("Lifecycle notes must retain exact bytes across {commit}"),
                        &old.path,
                        Some(&old.metadata.id),
                        None,
                    );
                }
            }
        }
    }
}
fn history(root: &Path, text: &str, evidence: &[Event], report: &mut Report) {
    report.validation_coverage.history = "incomplete";
    let Some(baseline) = report.baseline_commit.clone() else {
        return;
    };
    let start = report.diagnostics.len();
    let result = (|| -> Result<(), HistoryError> {
        if git_text(root, &["rev-parse", "--is-shallow-repository"])?.trim() != "false" {
            return Err("Supply complete local Git history".into());
        }
        let config = super::config(root)
            .map_err(|_| "Configuration unavailable during history inspection")?;
        let mut commits = BTreeSet::new();
        for event in evidence {
            if !hex(&event.baseline_commit, &[40, 64]) {
                continue;
            }
            if commits.insert(&event.baseline_commit) {
                require_revision(root, &format!("{}^{{object}}", event.baseline_commit))?;
                let kind = git_text(root, &["cat-file", "-t", &event.baseline_commit])?;
                if kind.trim() != "commit" {
                    issue(
                        report,
                        "invalid_revision_reference",
                        "Baseline reference is not a commit",
                        LEDGER,
                        None,
                        None,
                    );
                    continue;
                }
                let ancestor = git(
                    root,
                    &[
                        "merge-base",
                        "--is-ancestor",
                        &event.baseline_commit,
                        &baseline,
                    ],
                )?;
                if ancestor.status.code().is_none_or(|code| code > 1) {
                    return Err(io::Error::other(
                        String::from_utf8_lossy(&ancestor.stderr).into_owned(),
                    )
                    .into());
                }
                if !ancestor.status.success() {
                    issue(
                        report,
                        "invalid_revision_reference",
                        "Recorded baseline is outside the configured baseline history",
                        LEDGER,
                        None,
                        None,
                    );
                }
            }
            if event.operation == "edit" {
                for change in &event.changes {
                    if event.branch == config.baseline_branch
                        || baseline_contains(root, &event.baseline_commit, &change.id)?
                    {
                        issue(
                            report,
                            "ineligible_reset",
                            "Approval reset is forbidden on the baseline or after any baseline appearance",
                            LEDGER,
                            Some(&change.id),
                            None,
                        );
                    }
                }
            }
        }
        require_revision(root, "HEAD^{commit}")?;
        let commits = git_text(
            root,
            &[
                "rev-list",
                "--full-history",
                "--parents",
                "--reverse",
                "--topo-order",
                "HEAD",
                &baseline,
                "--",
                ".badr",
            ],
        )?;
        let mut snapshots = Vec::new();
        for line in commits.lines() {
            let mut fields = line.split_whitespace();
            let commit = fields.next().ok_or("Malformed Git history")?;
            let parents = fields.map(str::to_owned).collect::<Vec<_>>();
            let (ledger, records, available) =
                retained(root, commit, &config.baseline_branch, report)?;
            if !text.starts_with(&ledger) {
                issue(
                    report,
                    "evidence_rewritten",
                    format!("Evidence must retain exact bytes from {commit}"),
                    LEDGER,
                    None,
                    None,
                );
            }
            let old_events = events(&ledger, report);
            let states = replay(&old_events, available, report);
            compare(&states, &records, report, None);
            references(&states, available, report);
            snapshots.push(Snapshot {
                commit: commit.into(),
                ledger,
                records,
                parents,
            });
        }
        for snapshot in &snapshots {
            for parent in &snapshot.parents {
                if let Some(parent) = snapshots.iter().find(|s| s.commit == *parent) {
                    if !snapshot.ledger.starts_with(&parent.ledger) {
                        issue(
                            report,
                            "evidence_rewritten",
                            format!("{} rewrites parent evidence", snapshot.commit),
                            LEDGER,
                            None,
                            None,
                        );
                    }
                    notes_append_only(&parent.records, &snapshot.records, &snapshot.commit, report);
                }
            }
            let current = std::mem::take(&mut report.records);
            notes_append_only(&snapshot.records, &current, &snapshot.commit, report);
            for old in &snapshot.records {
                if !current
                    .iter()
                    .any(|r| identity(r.id()) == identity(old.id()))
                {
                    issue(
                        report,
                        "removed_record",
                        format!("Required record retained in {} is missing", snapshot.commit),
                        old.path(),
                        Some(old.id()),
                        None,
                    );
                }
            }
            report.records = current;
        }
        Ok(())
    })();
    match result {
        Ok(())
            if report.diagnostics.len() == start
                && report.validation_coverage.evidence == "passed"
                && report.validation_coverage.collection == "passed" =>
        {
            report.validation_coverage.history = "passed"
        }
        Ok(()) => {}
        Err(HistoryError::Unavailable(message)) => report.fail(
            3,
            "history_incomplete",
            format!("Trusted history unavailable: {message}"),
            None,
        ),
        Err(HistoryError::Operational(error)) => report.io(error),
    }
}
pub(super) fn inspect(
    root: &Path,
    history_root: &Path,
    report: &mut Report,
    reset_target: Option<&str>,
) {
    let start = report.diagnostics.len();
    report.validation_coverage.evidence = "incomplete";
    let text = match super::safe_entry(&root.join(LEDGER)) {
        Ok(None) => String::new(),
        Ok(Some(meta)) if meta.is_file() => match fs::read_to_string(root.join(LEDGER)) {
            Ok(s) => s,
            Err(e) if e.kind() == io::ErrorKind::InvalidData => {
                issue(
                    report,
                    "invalid_evidence",
                    e.to_string(),
                    LEDGER,
                    None,
                    None,
                );
                return;
            }
            Err(e) => {
                report.io(e);
                return;
            }
        },
        Ok(Some(_)) => {
            issue(
                report,
                "unsafe_path",
                "Evidence must be a regular file",
                LEDGER,
                None,
                None,
            );
            return;
        }
        Err(e) if e.kind() == io::ErrorKind::InvalidInput => {
            issue(report, "unsafe_path", e.to_string(), LEDGER, None, None);
            return;
        }
        Err(e) => {
            report.io(e);
            return;
        }
    };
    let events = events(&text, report);
    for event in &events {
        if super::check_branch(root, &event.branch).is_err() {
            issue(
                report,
                "invalid_evidence_branch",
                "Event branch must be a literal branch name",
                LEDGER,
                None,
                None,
            );
        }
    }
    let available = [
        root.join(".badr/adrs").exists(),
        root.join(".badr/brrs").exists(),
    ];
    let states = replay(&events, available, report);
    let records = std::mem::take(&mut report.records);
    compare(&states, &records, report, reset_target);
    references(
        &states,
        [
            root.join(".badr/adrs").exists(),
            root.join(".badr/brrs").exists(),
        ],
        report,
    );
    report.records = records;
    report.validation_coverage.evidence =
        if report.diagnostics.len() == start && report.validation_coverage.collection == "passed" {
            "passed"
        } else {
            "incomplete"
        };
    let before_domains = report.diagnostics.len();
    if report.validation_coverage.collection == "passed" {
        domain_ownership(root, history_root, &events, report);
    }
    if report.diagnostics.len() != before_domains {
        report.validation_coverage.evidence = "incomplete";
    }
    history(history_root, &text, &events, report);
}

// Retain ownership from every locally referenced registry, including deleted domains.
fn domain_ownership(root: &Path, history_root: &Path, events: &[Event], report: &mut Report) {
    const REGISTRY: &str = ".badr/brrs/registry.yaml";
    let result = (|| -> Result<(), HistoryError> {
        let current = match fs::read_to_string(root.join(REGISTRY)) {
            Ok(text) => super::brr::parse_registry(&text)?.domains,
            Err(e) if e.kind() == io::ErrorKind::NotFound => BTreeMap::new(),
            Err(e) => return Err(e.into()),
        };
        let has_baseline = report.baseline_commit.is_some();
        let mut owners = BTreeMap::new();
        let mut slugs = BTreeMap::new();
        let mut reserve = |slug: &str, prefix: &str| {
            if owners.get(prefix).is_some_and(|old| old != slug)
                || slugs.get(slug).is_some_and(|old| old != prefix)
            {
                issue(
                    report,
                    "domain_identity_changed",
                    "Reserved domain/prefix ownership cannot change",
                    REGISTRY,
                    None,
                    None,
                );
            }
            owners.insert(prefix.to_owned(), slug.to_owned());
            slugs.insert(slug.to_owned(), prefix.to_owned());
        };
        for event in events {
            if let Some(domain) = &event.domain {
                reserve(&domain.slug, &domain.prefix);
            }
        }
        let revisions = if has_baseline {
            require_revision(history_root, "HEAD^{commit}")?;
            git_text(
                history_root,
                &[
                    "rev-list",
                    "--full-history",
                    "--all",
                    "HEAD",
                    "--",
                    REGISTRY,
                ],
            )?
        } else {
            String::new()
        };
        for commit in revisions.lines() {
            let tree = git_text(history_root, &["ls-tree", commit, "--", REGISTRY])?;
            if tree.is_empty() {
                continue;
            }
            if !tree.starts_with("100644 blob ") && !tree.starts_with("100755 blob ") {
                return Err("Retained registry is not a regular file".into());
            }
            let text = git_text(history_root, &["show", &format!("{commit}:{REGISTRY}")])?;
            let registry = super::brr::parse_registry(&text)?;
            for (slug, prefix) in registry.domains {
                reserve(&slug, &prefix);
            }
        }
        for (slug, prefix) in &current {
            reserve(slug, prefix);
        }
        check_registered_domains(events, &current, report);
        Ok(())
    })();
    match result {
        Ok(()) => {}
        Err(HistoryError::Unavailable(message)) => {
            report.fail(3, "domain_history_incomplete", message, None)
        }
        Err(HistoryError::Operational(error)) => report.io(error),
    }
}

fn check_registered_domains(
    events: &[Event],
    domains: &BTreeMap<String, String>,
    report: &mut Report,
) {
    for event in events {
        if let Some(domain) = &event.domain {
            if domains.get(&domain.slug) != Some(&domain.prefix) {
                issue(
                    report,
                    "removed_domain",
                    "An evidenced domain registration must remain present",
                    ".badr/brrs/registry.yaml",
                    None,
                    None,
                );
            }
        }
    }
}

// Reserve identities across all local refs, including records/evidence later deleted there.
pub(super) fn reserved_ids(root: &Path) -> Result<Vec<String>, super::Failure> {
    fn checked(root: &Path, args: &[&str]) -> Result<String, super::Failure> {
        let out = git(root, args)?;
        if !out.status.success() {
            return Err(io::Error::other(String::from_utf8_lossy(&out.stderr).into_owned()).into());
        }
        String::from_utf8(out.stdout).map_err(|e| e.to_string().into())
    }
    fn allocations(text: &str, ids: &mut Vec<String>) -> Result<(), super::Failure> {
        for line in text.lines().filter(|line| !line.trim().is_empty()) {
            let event: Event = serde_json::from_str(line).map_err(|e| e.to_string())?;
            ids.extend(event.changes.into_iter().map(|c| c.id));
        }
        Ok(())
    }
    let mut ids = Vec::new();
    match fs::read_to_string(root.join(LEDGER)) {
        Ok(text) => allocations(&text, &mut ids)?,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }
    let commits = checked(
        root,
        &["rev-list", "--full-history", "--all", "HEAD", "--", ".badr"],
    )?;
    let mut blobs = BTreeSet::new();
    for commit in commits.lines() {
        let tree = checked(root, &["ls-tree", "-r", "-z", commit, "--", ".badr"])?;
        for entry in tree.split('\0').filter(|s| !s.is_empty()) {
            let (meta, path) = entry.split_once('\t').ok_or("Malformed Git tree")?;
            let record = path.starts_with(".badr/adrs/") && path.ends_with(".md")
                || path.starts_with(".badr/brrs/") && path.ends_with(".feature");
            if !record && path != LEDGER {
                continue;
            }
            let fields: Vec<_> = meta.split_whitespace().collect();
            if fields.len() != 3 || !matches!(fields[0], "100644" | "100755") || fields[1] != "blob"
            {
                return Err("Cannot allocate from unsafe retained record/evidence storage".into());
            }
            if !blobs.insert((fields[2].to_owned(), path.to_owned())) {
                continue;
            }
            let text = checked(root, &["cat-file", "blob", fields[2]])?;
            if path == LEDGER {
                allocations(&text, &mut ids)?;
            } else if path.ends_with(".feature") {
                ids.push(super::brr::parse(&text, path)?.metadata.id);
            } else {
                ids.push(crate::adr::retained(&text, path)?.metadata.id);
            }
        }
    }
    Ok(ids)
}

// The caller has already validated the complete collection, evidence, and required history.
pub(super) fn reset_approval(
    root: &Path,
    report: &Report,
    id: &str,
) -> Result<String, super::Failure> {
    let mut checked = Report::new("edit");
    let events = events(&fs::read_to_string(root.join(LEDGER))?, &mut checked);
    let states = replay(&events, [true, true], &mut checked);
    if checked.exit_code != 0 {
        return Err("Existing evidence is invalid".into());
    }
    let state = states
        .get(&identity(id).ok_or("Invalid identity")?)
        .ok_or("Missing evidence")?;
    if state.revision.to != "approved" || state.activated || state.replaced {
        return Err("A previously activated or replaced record requires a successor".into());
    }
    let branch = git(root, &["symbolic-ref", "--quiet", "--short", "HEAD"])?;
    if branch.status.code().is_none_or(|code| code > 1) {
        return Err(io::Error::other(String::from_utf8_lossy(&branch.stderr).into_owned()).into());
    }
    let branch = String::from_utf8(branch.stdout).map_err(|e| e.to_string())?;
    let config = super::config(root)?;
    if !branch.trim().is_empty() && branch.trim() != config.baseline_branch {
        let mut commits = BTreeSet::new();
        commits.insert(
            report
                .baseline_commit
                .as_deref()
                .ok_or("Baseline unavailable")?,
        );
        commits.extend(events.iter().map(|e| e.baseline_commit.as_str()));
        for commit in commits {
            match baseline_contains(root, commit, id) {
                Ok(false) => {}
                Ok(true) => {
                    return Err(
                        "An ID that ever appeared in baseline history requires a successor".into(),
                    );
                }
                Err(HistoryError::Operational(e)) => return Err(e.into()),
                Err(HistoryError::Unavailable(e)) => return Err(e.into()),
            }
        }
    } else {
        return Err("Approval reset requires a named branch other than the baseline".into());
    }
    Ok(state
        .approval
        .as_ref()
        .ok_or("Missing active approval")?
        .0
        .clone())
}
