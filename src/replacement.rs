use super::mutation::{Plan, bytes, event};
use super::*;
use std::collections::BTreeSet;

const LEDGER: &str = ".badr/evidence.jsonl";

#[derive(clap::Args)]
pub(crate) struct Replacement {
    #[arg(long, required = true)]
    predecessor: Vec<String>,
    #[arg(long, required = true)]
    successor: Vec<String>,
    /// Declare that the listed successors completely replace every listed predecessor.
    #[arg(long)]
    complete: bool,
    /// Approve proposed successors together with replacement.
    #[arg(long)]
    approve_successors: bool,
    #[arg(long)]
    reviewed: bool,
    #[arg(long)]
    confirm: bool,
    #[arg(long)]
    actor: String,
    #[arg(long)]
    rationale: String,
}

pub(crate) fn run(target: &Path, choice: Replacement) -> Report {
    if !choice.complete
        || !choice.confirm
        || choice.actor.trim().is_empty()
        || choice.rationale.trim().is_empty()
        || choice.approve_successors && !choice.reviewed
        || choice.reviewed && !choice.approve_successors
    {
        return Report::invalid("supersede", "invalid_replacement_arguments",
            "Require --complete, --confirm, nonempty --actor and --rationale; --approve-successors requires --reviewed".into());
    }
    let predecessors: BTreeSet<_> = choice.predecessor.iter().collect();
    let successors: BTreeSet<_> = choice.successor.iter().collect();
    if predecessors.len() != choice.predecessor.len()
        || successors.len() != choice.successor.len()
        || !predecessors.is_disjoint(&successors)
    {
        return Report::invalid(
            "supersede",
            "invalid_replacement_arguments",
            "Require distinct, disjoint predecessor and successor IDs".into(),
        );
    }
    mutation::run(target, "supersede", |root, report, expected| {
        let operation = if choice.approve_successors {
            "decide"
        } else {
            "supersede"
        };
        let mut receipt = event(
            root,
            report,
            operation,
            Some((&choice.actor, &choice.rationale)),
            serde_json::json!({"replacement": {"predecessors": choice.predecessor, "successors": choice.successor, "complete": true}}),
        )?;
        if choice.approve_successors {
            receipt["reviewed"] = true.into();
        }
        let today = &receipt["at"].as_str().unwrap()[..10];
        let ledger = bytes(expected, LEDGER).ok_or("Evidence disappeared")?;
        let mut writes = Vec::new();
        let mut changes = Vec::new();
        let mut affected_ids = Vec::new();
        for id in predecessors.union(&successors) {
            let summary = report
                .records
                .iter()
                .find(|r| r.id() == id.as_str())
                .ok_or("Replacement target not found; use its actual stable ID")?;
            let original = bytes(expected, summary.path()).ok_or("Record disappeared")?;
            let Record::Adr(adr) = authoring::parse(
                std::str::from_utf8(&original).map_err(|e| e.to_string())?,
                summary.path(),
            )?
            else {
                return Err("This operation replaces ADRs only".into());
            };
            let predecessor = predecessors.contains(id);
            let approving =
                !predecessor && adr.metadata.status == "proposed" && choice.approve_successors;
            if predecessor && !matches!(adr.metadata.status.as_str(), "approved" | "deprecated")
                || !predecessor && adr.metadata.status != "approved" && !approving
            {
                return Err("Require approved/deprecated predecessors and approved successors; explicitly approve proposed successors with --approve-successors --reviewed".into());
            }
            let body = adr.decision_body.as_deref().unwrap_or("");
            if approving {
                authoring::review_gaps(body)?;
            }
            let mut metadata = serde_json::to_value(&adr.metadata).map_err(|e| e.to_string())?;
            metadata["status"] = if predecessor {
                "superseded"
            } else {
                "approved"
            }
            .into();
            metadata["date"] = today.into();
            let field = if predecessor {
                "superseded_by"
            } else {
                "supersedes"
            };
            let additions = if predecessor {
                &successors
            } else {
                &predecessors
            };
            for target in additions {
                let value = serde_json::Value::String((*target).clone());
                let links = metadata[field].as_array_mut().unwrap();
                if !links.contains(&value) {
                    links.push(value);
                }
            }
            let mut notes = adr.lifecycle_notes.clone();
            if notes.is_empty() {
                notes.push_str("## Lifecycle notes\n");
            }
            if !notes.ends_with('\n') {
                notes.push('\n');
            }
            notes.push_str(&format!("\n### {today} complete replacement\n\n"));
            let declaration = format!(
                "Actor: {}\nRationale: {}\nComplete predecessors: {:?}\nComplete successors: {:?}\nApproved in group: {approving}",
                choice.actor, choice.rationale, choice.predecessor, choice.successor
            );
            for line in declaration.lines() {
                notes.push_str(&format!("> {line}\n"));
            }
            let header = yaml_serde::to_string(&metadata)
                .map_err(|e| e.to_string())?
                .replace(&format!("date: {today}\n"), &format!("date: \"{today}\"\n"));
            let text = format!("---\n{header}---\n{body}{notes}");
            let changed = authoring::parse(&text, &adr.path)?;
            if authoring::state(&changed).1 != body {
                return Err(
                    "Cannot append lifecycle notes without changing exact decision-body bytes"
                        .into(),
                );
            }
            let mut revision = authoring::change(&changed, Some(&adr.metadata.status));
            revision["approval_ref"] = if approving {
                receipt["event_id"].clone()
            } else {
                serde_json::to_value(evidence::approval_reference(&ledger, id)?)
                    .map_err(|e| e.to_string())?
            };
            changes.push(revision);
            affected_ids.push((*id).clone());
            writes.push((adr.path, text.into_bytes()));
        }
        receipt["changes"] = changes.into();
        writes.push((LEDGER.into(), authoring::append(expected, &receipt)?));
        Ok(Plan {
            writes,
            affected_ids,
            message: format!(
                "Replace ADRs {:?} with {:?}\n\nActor: {}\nRationale: {}\nBaseline: {}\nEvidence: {}",
                choice.predecessor,
                choice.successor,
                choice.actor,
                choice.rationale,
                report.baseline_commit.as_deref().unwrap(),
                receipt["event_id"].as_str().unwrap()
            ),
        })
    })
}
