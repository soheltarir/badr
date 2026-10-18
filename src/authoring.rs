use super::mutation::{Entry, Plan, bytes, event};
use super::*;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

const LEDGER: &str = ".badr/evidence.jsonl";
const REGISTRY: &str = ".badr/brrs/registry.yaml";

#[derive(clap::Subcommand)]
pub(crate) enum Command {
    /// Create a registered draft business rule.
    Brr {
        title: String,
        #[arg(long)]
        domain: String,
        #[arg(long, required_unless_present = "content")]
        owner: Vec<String>,
        #[arg(long)]
        content: Option<PathBuf>,
    },
    /// Create a proposed architecture decision.
    Adr {
        title: String,
        #[arg(long)]
        content: Option<PathBuf>,
    },
}

fn digest(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

// Increment decimal text directly: identity numbers are not bounded by machine integers.
fn next_number(ids: &[String], prefix: &str, width: usize) -> String {
    let mut highest = "0";
    for id in ids {
        if let Some(number) = id.strip_prefix(prefix) {
            if number.is_empty() || !number.bytes().all(|b| b.is_ascii_digit()) {
                continue;
            }
            let number = number.trim_start_matches('0');
            if (number.len(), number) > (highest.len(), highest) {
                highest = number;
            }
        }
    }
    let mut next = highest.as_bytes().to_vec();
    let mut carry = true;
    for digit in next.iter_mut().rev() {
        if *digit == b'9' {
            *digit = b'0';
        } else {
            *digit += 1;
            carry = false;
            break;
        }
    }
    if carry {
        next.insert(0, b'1');
    }
    format!("{prefix}{:0>width$}", String::from_utf8(next).unwrap())
}

pub(crate) fn new(target: &Path, command: Command) -> Report {
    let (title, domain, owners, content) = match command {
        Command::Brr {
            title,
            domain,
            owner,
            content,
        } => (title, Some(domain), owner, content),
        Command::Adr { title, content } => (title, None, vec![], content),
    };
    let operation = if domain.is_some() {
        "new_brr"
    } else {
        "new_adr"
    };
    if !brr::slug(&title) || owners.iter().any(|s| s.trim().is_empty()) {
        return Report::invalid(
            operation,
            "invalid_authoring_arguments",
            "Require a lowercase filename title and nonempty owners".into(),
        );
    }
    // Resolve supplied input relative to the caller, before examining the target repository.
    let supplied = match content.map(fs::read_to_string).transpose() {
        Ok(supplied) => supplied,
        Err(error) if error.kind() == io::ErrorKind::InvalidData => {
            return Report::invalid(operation, "invalid_content", error.to_string());
        }
        Err(error) => {
            let mut report = Report::new(operation);
            report.io(error);
            return report;
        }
    };
    mutation::run(target, operation, |root, report, expected| {
        let mut ids: Vec<_> = report.records.iter().map(|r| r.id().to_owned()).collect();
        ids.extend(evidence::reserved_ids(root)?);
        let mut writes = vec![];
        let mut event = event(root, report, "allocate", None, serde_json::json!({}))?;
        let (id, path, text) = if let Some(domain) = &domain {
            let mut registry: brr::Registry = yaml_serde::from_slice(
                &bytes(expected, REGISTRY).ok_or("Register the BRR domain first")?,
            )
            .map_err(|e| e.to_string())?;
            let prefix = registry
                .domains
                .get(domain)
                .ok_or("Register the BRR domain first")?;
            let id = next_number(&ids, &format!("BRR-{prefix}-"), 3);
            let path = format!(".badr/brrs/domains/{domain}/rules/{id}-{title}.feature");
            let text = supplied.clone().unwrap_or_else(|| {
                let header =
                    serde_json::json!({"id":id,"domain":domain,"status":"draft","owners":owners});
                format!(
                    "---\n{}---\n{}",
                    yaml_serde::to_string(&header).unwrap(),
                    include_str!("templates/brr.feature")
                        .replace("{{id}}", &id)
                        .replace("{{domain}}", domain)
                        .replace("{{title}}", &title)
                )
            });
            registry
                .rules
                .insert(id.clone(), path.strip_prefix(".badr/brrs/").unwrap().into());
            writes.push((
                REGISTRY.into(),
                yaml_serde::to_string(&registry)
                    .map_err(|e| e.to_string())?
                    .into_bytes(),
            ));
            (id, path, text)
        } else {
            let id = next_number(&ids, "", 4);
            let path = format!(".badr/adrs/{id}-{title}.md");
            let today = &event["at"].as_str().unwrap()[..10];
            let text = supplied.clone().unwrap_or_else(|| {
                format!(
                    "---\nid: \"{id}\"\nstatus: proposed\ndate: \"{}\"\n---\n{}",
                    today,
                    include_str!("templates/adr.md").replace("{{title}}", &title)
                )
            });
            (id, path, text)
        };
        if expected.contains_key(&path) {
            return Err("Refusing to overwrite existing record storage".into());
        }
        let record = parse(&text, &path)?;
        let (status, _) = state(&record);
        if record.id() != id
            || status
                != if domain.is_some() {
                    "draft"
                } else {
                    "proposed"
                }
        {
            return Err(
                "Supplied content must match the allocated identity and legal initial state".into(),
            );
        }
        if let Record::Brr(r) = &record {
            if Some(&r.metadata.domain) != domain.as_ref() {
                return Err("Supplied BRR domain must match the selected domain".into());
            }
        }
        event["changes"] = serde_json::json!([change(&record, None)]);
        // Publish the allocation receipt first so even an interrupted creation reserves the ID.
        writes.insert(0, (LEDGER.into(), append(expected, &event)?));
        writes.insert(1, (path, text.into_bytes()));
        Ok(Plan {
            writes,
            affected_ids: vec![id.clone()],
            message: format!(
                "Allocate {id} ({title})\n\nBaseline: {}\nEvidence: {}",
                report.baseline_commit.as_deref().unwrap(),
                event["event_id"].as_str().unwrap()
            ),
        })
    })
}

fn append(
    expected: &BTreeMap<String, Entry>,
    event: &serde_json::Value,
) -> Result<Vec<u8>, Failure> {
    let mut ledger = bytes(expected, LEDGER).unwrap_or_default();
    ledger.extend(serde_json::to_vec(event).map_err(|e| e.to_string())?);
    ledger.push(b'\n');
    Ok(ledger)
}
pub(super) fn parse(text: &str, path: &str) -> Result<Record, Failure> {
    if path.ends_with(".feature") {
        Ok(Record::Brr(brr::parse(text, path)?))
    } else {
        Ok(Record::Adr(crate::adr::retained(text, path)?))
    }
}
fn state(record: &Record) -> (&str, &str) {
    match record {
        Record::Brr(r) => (&r.metadata.status, &r.body),
        Record::Adr(r) => (&r.metadata.status, r.decision_body.as_deref().unwrap_or("")),
    }
}
fn change(record: &Record, from: Option<&str>) -> serde_json::Value {
    let (status, body) = state(record);
    let mut value = serde_json::json!({"id":record.id(),"path":record.path(),"from":from,"to":status,"body_sha256":digest(body)});
    match record {
        Record::Brr(r) => {
            value["related"] = serde_json::json!(r.metadata.related_architecture_decisions);
            value["supersedes"] = serde_json::json!(r.metadata.supersedes);
            value["superseded_by"] = serde_json::json!(r.metadata.superseded_by);
        }
        Record::Adr(r) => {
            value["date"] = r.metadata.date.clone().into();
            value["notes_sha256"] = digest(&r.lifecycle_notes).into();
            value["related"] = serde_json::json!(
                r.metadata
                    .related_adrs
                    .iter()
                    .chain(&r.metadata.related_business_rules)
                    .collect::<Vec<_>>()
            );
            value["supersedes"] = serde_json::json!(r.metadata.supersedes);
            value["superseded_by"] = serde_json::json!(r.metadata.superseded_by);
        }
    }
    value
}

#[derive(clap::Args)]
pub(crate) struct Proposal {
    id: String,
    #[arg(long)]
    confirm: bool,
    #[arg(long)]
    actor: String,
    #[arg(long)]
    rationale: String,
}
pub(crate) fn propose(target: &Path, proposal: Proposal) -> Report {
    let Proposal {
        id,
        confirm,
        actor,
        rationale,
    } = proposal;
    if !confirm || actor.trim().is_empty() || rationale.trim().is_empty() {
        return Report::invalid(
            "propose",
            "invalid_proposal_arguments",
            "Require --confirm, nonempty --actor and --rationale".into(),
        );
    }
    mutation::run(target, "propose", |root, report, expected| {
        let record = report
            .records
            .iter()
            .find(|r| r.id() == id)
            .ok_or("Record not found")?;
        let Record::Brr(rule) = record else {
            return Err("Only draft BRRs can be proposed".into());
        };
        if rule.metadata.status != "draft" {
            return Err("Only draft BRRs can be proposed".into());
        }
        let mut metadata = serde_json::to_value(&rule.metadata).map_err(|e| e.to_string())?;
        metadata["status"] = "proposed".into();
        let text = format!(
            "---\n{}---\n{}",
            yaml_serde::to_string(&metadata).map_err(|e| e.to_string())?,
            rule.body
        );
        let changed = parse(&text, &rule.path)?;
        let mut event = event(
            root,
            report,
            "propose",
            Some((&actor, &rationale)),
            serde_json::json!({}),
        )?;
        event["changes"] = serde_json::json!([change(&changed, Some("draft"))]);
        Ok(Plan {
            writes: vec![
                (rule.path.clone(), text.into_bytes()),
                (LEDGER.into(), append(expected, &event)?),
            ],
            affected_ids: vec![id.clone()],
            message: format!(
                "Propose {id}\n\nActor: {actor}\nRationale: {rationale}\nBaseline: {}\nEvidence: {}",
                report.baseline_commit.as_deref().unwrap(),
                event["event_id"].as_str().unwrap()
            ),
        })
    })
}

#[derive(clap::Args)]
pub(crate) struct Approval {
    id: String,
    #[arg(long)]
    confirm: bool,
    /// Declare that applicable content and gaps were reviewed and resolved by a human.
    #[arg(long)]
    reviewed: bool,
    #[arg(long)]
    actor: String,
    #[arg(long)]
    rationale: String,
}

pub(crate) fn decide(target: &Path, approval: Approval) -> Report {
    let Approval {
        id,
        confirm,
        reviewed,
        actor,
        rationale,
    } = approval;
    if !confirm || !reviewed || actor.trim().is_empty() || rationale.trim().is_empty() {
        return Report::invalid("decide", "invalid_approval_arguments",
            "Require --confirm, --reviewed (applicable content/gaps reviewed and resolved), nonempty --actor and --rationale. Agents obtain human authorization outside BADR; this records a declaration without authenticating the actor or proving an interview occurred.".into());
    }
    mutation::run(target, "decide", |root, report, expected| {
        let record = report
            .records
            .iter()
            .find(|r| r.id() == id)
            .ok_or("Record not found")?;
        let original =
            String::from_utf8(bytes(expected, record.path()).ok_or("Record disappeared")?)
                .map_err(|e| e.to_string())?;
        let record = &parse(&original, record.path())?;
        if state(record).0 != "proposed" {
            return Err("Only proposed BRRs and ADRs can be approved; an existing approval cannot be replaced".into());
        }
        if state(record)
            .1
            .split(|c: char| !c.is_ascii_alphanumeric())
            .any(|word| matches!(word, "TODO" | "TBD" | "FIXME"))
        {
            return Err("Resolve explicit TODO, TBD, and FIXME review gaps before approval".into());
        }
        if let Record::Brr(rule) = record {
            let feature = gherkin::Feature::parse(&rule.body, gherkin::GherkinEnv::default())
                .map_err(|e| e.to_string())?;
            if feature.tags.iter().any(|tag| tag == "successor") {
                for scope in ["existing-data", "in-flight"] {
                    if !feature
                        .rules
                        .iter()
                        .flat_map(|r| &r.scenarios)
                        .any(|scenario| {
                            scenario
                                .tags
                                .iter()
                                .any(|tag| tag == &format!("compatibility:{scope}"))
                                && scenario.tags.iter().any(|tag| {
                                    matches!(
                                        tag.as_str(),
                                        "treatment:grandfather"
                                            | "treatment:migrate"
                                            | "treatment:re-evaluate"
                                            | "treatment:reject"
                                    )
                                })
                                && scenario.steps.iter().any(|step| {
                                    step.ty == gherkin::StepType::Then
                                        && !step.value.trim().is_empty()
                                })
                        })
                    {
                        return Err(format!("Successor BRRs require an observable @compatibility:{scope} scenario with a Then outcome and @treatment:grandfather, migrate, re-evaluate, or reject").into());
                    }
                }
            }
        }
        let mut event = event(
            root,
            report,
            "decide",
            Some((&actor, &rationale)),
            serde_json::json!({"reviewed":true}),
        )?;
        let (metadata, body) = match record {
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
        let mut metadata = metadata.map_err(|e| e.to_string())?;
        metadata["status"] = "approved".into();
        if let Record::Adr(_) = record {
            metadata["date"] = event["at"].as_str().unwrap()[..10].into();
        }
        let mut header = yaml_serde::to_string(&metadata).map_err(|e| e.to_string())?;
        if let Record::Adr(_) = record {
            let date = metadata["date"].as_str().unwrap();
            header = header.replace(&format!("date: {date}\n"), &format!("date: \"{date}\"\n"));
        }
        let text = format!("---\n{header}---\n{body}");
        let changed = parse(&text, record.path())?;
        let mut revision = change(&changed, Some("proposed"));
        revision["approval_ref"] = event["event_id"].clone();
        event["changes"] = serde_json::json!([revision]);
        Ok(Plan {
            writes: vec![
                (record.path().into(), text.into_bytes()),
                (LEDGER.into(), append(expected, &event)?),
            ],
            affected_ids: vec![id.clone()],
            message: format!(
                "Approve {id}\n\nActor: {actor}\nRationale: {rationale}\nReview: applicable content and gaps reviewed and resolved\nBaseline: {}\nEvidence: {}",
                report.baseline_commit.as_deref().unwrap(),
                event["event_id"].as_str().unwrap()
            ),
        })
    })
}
