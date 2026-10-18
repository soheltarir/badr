use super::mutation::{Plan, bytes, event};
use super::*;
use std::collections::BTreeSet;

#[derive(clap::Args)]
pub(crate) struct Link {
    id: String,
    target: String,
    #[arg(long)]
    confirm: bool,
    #[arg(long)]
    actor: String,
    #[arg(long)]
    rationale: String,
}

pub(crate) fn targets(record: &Record) -> Vec<&str> {
    match record {
        Record::Brr(r) => r.metadata.related_architecture_decisions.iter(),
        Record::Adr(r) => {
            return r
                .metadata
                .related_adrs
                .iter()
                .chain(&r.metadata.related_business_rules)
                .map(String::as_str)
                .collect();
        }
    }
    .map(String::as_str)
    .collect()
}

pub(crate) fn run(target: &Path, link: Link, remove: bool) -> Report {
    let operation = if remove { "unlink" } else { "link" };
    if !link.confirm || link.actor.trim().is_empty() || link.rationale.trim().is_empty() {
        return Report::invalid(
            operation,
            "invalid_relationship_arguments",
            "Require --confirm, nonempty --actor and --rationale".into(),
        );
    }
    mutation::run(target, operation, |root, report, expected| {
        let record = report
            .records
            .iter()
            .find(|r| r.id() == link.id)
            .ok_or("Source record not found; use its actual stable ID")?;
        // Reparse the captured bytes: inspection omits ADR decision bodies from summaries.
        let original = bytes(expected, record.path()).ok_or("Record disappeared")?;
        let record = authoring::parse(
            std::str::from_utf8(&original).map_err(|e| e.to_string())?,
            record.path(),
        )?;
        if matches!(&record, Record::Brr(_)) && link.target.starts_with("BRR-") {
            return Err("BRRs permit only related architecture decisions".into());
        }
        if !report.records.iter().any(|r| r.id() == link.target) {
            let available = if link.target.starts_with("BRR-") {
                root.join(".badr/brrs").exists()
            } else {
                root.join(".badr/adrs").exists()
            };
            if available {
                return Err(
                    "Target missing from available collection; use its actual stable ID".into(),
                );
            }
        }
        let (metadata, body) = match &record {
            Record::Brr(r) => (serde_json::to_value(&r.metadata), r.body.clone()),
            Record::Adr(r) => (
                serde_json::to_value(&r.metadata),
                format!(
                    "{}{}",
                    r.decision_body.as_deref().unwrap_or(""),
                    r.lifecycle_notes
                ),
            ),
        };
        let field = match &record {
            Record::Brr(_) => "related_architecture_decisions",
            Record::Adr(_) if link.target.starts_with("BRR-") => "related_business_rules",
            Record::Adr(_) => "related_adrs",
        };
        let mut metadata = metadata.map_err(|e| e.to_string())?;
        let related = metadata[field].as_array_mut().unwrap();
        let value = serde_json::Value::String(link.target.clone());
        if related.contains(&value) == remove {
            if remove {
                related.retain(|id| id != &value);
            } else {
                related.push(value);
            }
        } else {
            return Err("Reference is already linked or is not present for unlink".into());
        }
        let mut event = event(
            root,
            report,
            operation,
            Some((&link.actor, &link.rationale)),
            serde_json::json!({}),
        )?;
        let today = &event["at"].as_str().unwrap()[..10];
        let mut body = body;
        if let Record::Adr(r) = &record {
            metadata["date"] = today.into();
            if r.lifecycle_notes.is_empty() {
                body.push_str("## Lifecycle notes\n");
            }
            body.push_str(&format!(
                "\n### {today}\n{operation} {}.\nActor: {}\nRationale: {}\n",
                link.target, link.actor, link.rationale
            ));
        }
        let mut header = yaml_serde::to_string(&metadata).map_err(|e| e.to_string())?;
        if matches!(&record, Record::Adr(_)) {
            header = header.replace(&format!("date: {today}\n"), &format!("date: \"{today}\"\n"));
        }
        let text = format!("---\n{header}---\n{body}");
        let changed = authoring::parse(&text, record.path())?;
        if authoring::state(&record).1 != authoring::state(&changed).1 {
            return Err("Relationship updates must preserve exact body bytes; ADR lifecycle notes need an existing line boundary".into());
        }
        let mut revision = authoring::change(&changed, Some(authoring::state(&record).0));
        if let Record::Brr(r) = &record {
            revision["effective_from"] = serde_json::json!(r.metadata.effective_from);
            revision["effective_until"] = serde_json::json!(r.metadata.effective_until);
        }
        let ledger = bytes(expected, ".badr/evidence.jsonl").ok_or("Missing evidence")?;
        for line in std::str::from_utf8(&ledger)
            .map_err(|e| e.to_string())?
            .lines()
            .rev()
        {
            let prior: serde_json::Value = serde_json::from_str(line).map_err(|e| e.to_string())?;
            if let Some(change) = prior["changes"]
                .as_array()
                .and_then(|changes| changes.iter().find(|c| c["id"] == link.id))
            {
                if change["to"] != "proposed" {
                    revision["approval_ref"] = change["approval_ref"].clone();
                }
                break;
            }
        }
        event["changes"] = serde_json::json!([revision]);
        Ok(Plan {
            writes: vec![
                (record.path().into(), text.into_bytes()),
                (
                    ".badr/evidence.jsonl".into(),
                    authoring::append(expected, &event)?,
                ),
            ],
            affected_ids: vec![link.id.clone()],
            message: format!(
                "{operation} {} -> {}\n\nActor: {}\nRationale: {}\nBaseline: {}\nEvidence: {}",
                link.id,
                link.target,
                link.actor,
                link.rationale,
                report.baseline_commit.as_deref().unwrap(),
                event["event_id"].as_str().unwrap()
            ),
        })
    })
}

pub(crate) fn related(target: &Path, id: &str) -> Report {
    let mut report = inspect(target, "related");
    if !report.records.iter().any(|r| r.id() == id) {
        report.fail(
            2,
            "record_not_found",
            "Record not found; use its actual stable ID".into(),
            None,
        );
        report.records.clear();
        return report;
    }
    let mut visited = BTreeSet::new();
    let mut pending = vec![id.to_owned()];
    while let Some(id) = pending.pop() {
        if !visited.insert(id.clone()) {
            continue;
        }
        if let Some(record) = report.records.iter().find(|r| r.id() == id) {
            pending.extend(targets(record).into_iter().map(str::to_owned));
        }
    }
    report.records.retain(|r| visited.contains(r.id()));
    report
}
