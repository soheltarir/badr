use super::authoring::{Proposal, append, change, parse};
use super::mutation::{Plan, bytes, event};
use super::*;
use std::collections::BTreeSet;

const LEDGER: &str = ".badr/evidence.jsonl";

#[derive(clap::Args)]
pub(crate) struct Activation {
    /// Activate a single BRR, or omit when declaring a replacement group.
    id: Option<String>,
    #[command(flatten)]
    group: Replacement,
}

#[derive(clap::Args)]
pub(crate) struct Replacement {
    #[arg(long)]
    predecessor: Vec<String>,
    #[arg(long)]
    successor: Vec<String>,
    /// Declare that these successors jointly replace the complete predecessor policy.
    #[arg(long)]
    complete: bool,
    /// Declare human review of existing-data and in-flight compatibility treatment.
    #[arg(long)]
    reviewed: bool,
    #[arg(long)]
    confirm: bool,
    #[arg(long)]
    actor: String,
    #[arg(long)]
    rationale: String,
}

pub(crate) fn activate(target: &Path, activation: Activation) -> Report {
    let Activation { id, group } = activation;
    if let Some(id) = id {
        if !group.predecessor.is_empty()
            || !group.successor.is_empty()
            || group.complete
            || group.reviewed
        {
            return Report::invalid(
                "activate",
                "invalid_replacement_arguments",
                "Use either a single ID or an explicit --predecessor/--successor group".into(),
            );
        }
        return authoring::lifecycle(
            target,
            "activate",
            Proposal {
                id,
                confirm: group.confirm,
                actor: group.actor,
                rationale: group.rationale,
            },
        );
    }
    run(target, "activate", group)
}

pub(crate) fn run(target: &Path, operation: &str, group: Replacement) -> Report {
    let predecessors: BTreeSet<_> = group.predecessor.iter().collect();
    let successors: BTreeSet<_> = group.successor.iter().collect();
    if !group.complete
        || !group.reviewed
        || !group.confirm
        || group.actor.trim().is_empty()
        || group.rationale.trim().is_empty()
        || predecessors.is_empty()
        || successors.is_empty()
        || predecessors.len() != group.predecessor.len()
        || successors.len() != group.successor.len()
        || !predecessors.is_disjoint(&successors)
    {
        return Report::invalid(operation, "invalid_replacement_arguments", "Require disjoint nonempty --predecessor and --successor sets, --complete, --reviewed, --confirm, nonempty --actor and --rationale".into());
    }
    mutation::run(target, operation, |root, report, expected| {
        let mut event = event(
            root,
            report,
            operation,
            Some((&group.actor, &group.rationale)),
            serde_json::json!({}),
        )?;
        event["replacement"] = serde_json::json!({"predecessors": predecessors, "successors": successors, "complete": true, "compatibility_confirmed": true});
        let ledger = bytes(expected, LEDGER).ok_or("Missing evidence")?;
        let mut writes = vec![];
        let mut changes = vec![];
        let mut affected_ids = vec![];
        for id in predecessors.union(&successors) {
            let record = report
                .records
                .iter()
                .find(|r| r.id() == id.as_str())
                .ok_or("Replacement target not found")?;
            let Record::Brr(rule) = record else {
                return Err("This replacement operation requires BRRs".into());
            };
            let predecessor = predecessors.contains(id);
            let required = if predecessor || operation == "supersede" {
                "effective"
            } else {
                "approved"
            };
            if rule.metadata.status != required {
                return Err(format!("{id} must be {required} before replacement").into());
            }
            if !predecessor {
                // Approval checked compatibility scenarios only for explicitly marked successors.
                let feature = gherkin::Feature::parse(&rule.body, gherkin::GherkinEnv::default())
                    .map_err(|e| e.to_string())?;
                if !feature.tags.iter().any(|tag| tag == "successor") {
                    return Err("Successors must carry @successor before approval, with reviewed compatibility scenarios".into());
                }
            }
            let mut metadata = serde_json::to_value(&rule.metadata).map_err(|e| e.to_string())?;
            metadata["status"] = if predecessor {
                "superseded"
            } else {
                "effective"
            }
            .into();
            if predecessor {
                metadata["effective_until"] = event["at"].clone();
            } else if operation == "activate" {
                metadata["effective_from"] = event["at"].clone();
            }
            let (key, added) = if predecessor {
                ("superseded_by", &successors)
            } else {
                ("supersedes", &predecessors)
            };
            let mut links: BTreeSet<String> =
                serde_json::from_value(metadata[key].clone()).map_err(|e| e.to_string())?;
            links.extend(added.iter().map(|id| (*id).clone()));
            metadata[key] = serde_json::json!(links);
            let mut header = yaml_serde::to_string(&metadata).map_err(|e| e.to_string())?;
            for key in ["effective_from", "effective_until"] {
                if let Some(time) = metadata[key].as_str() {
                    header = header
                        .replace(&format!("{key}: {time}\n"), &format!("{key}: \"{time}\"\n"));
                }
            }
            let text = format!("---\n{header}---\n{}", rule.body);
            let changed = parse(&text, &rule.path)?;
            let mut revision = change(&changed, Some(&rule.metadata.status));
            revision["approval_ref"] = evidence::approval_reference(&ledger, id)?
                .ok_or("Missing active approval receipt")?
                .into();
            changes.push(revision);
            writes.push((rule.path.clone(), text.into_bytes()));
            affected_ids.push((*id).clone());
        }
        event["changes"] = serde_json::json!(changes);
        writes.push((LEDGER.into(), append(expected, &event)?));
        Ok(Plan {
            writes,
            affected_ids,
            message: format!(
                "{operation} complete BRR replacement\n\nPredecessors: {:?}\nSuccessors: {:?}\nActor: {}\nRationale: {}\nCompatibility: reviewed\nBaseline: {}\nEvidence: {}",
                predecessors,
                successors,
                group.actor,
                group.rationale,
                report.baseline_commit.as_deref().unwrap(),
                event["event_id"].as_str().unwrap()
            ),
        })
    })
}
