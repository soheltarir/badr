use super::mutation::{Plan, bytes, event};
use super::*;
use std::collections::BTreeMap;
const REGISTRY: &str = ".badr/brrs/registry.yaml";
const LEDGER: &str = ".badr/evidence.jsonl";

#[derive(clap::Subcommand)]
pub(crate) enum Command {
    /// Register a new domain and its permanently reserved uppercase prefix.
    Add {
        slug: String,
        #[arg(long)]
        prefix: String,
        #[arg(long)]
        confirm: bool,
        #[arg(long)]
        actor: String,
        #[arg(long)]
        rationale: String,
    },
}

pub(crate) fn run(target: &Path, command: Command) -> Report {
    let Command::Add {
        slug,
        prefix,
        confirm,
        actor,
        rationale,
    } = command;
    if !brr::slug(&slug)
        || !brr::prefix(&prefix)
        || !confirm
        || actor.trim().is_empty()
        || rationale.trim().is_empty()
    {
        return Report::invalid("domain_add", "invalid_domain_arguments", "Require a lowercase domain slug, uppercase prefix, --confirm, nonempty --actor and --rationale".into());
    }
    mutation::run(target, "domain_add", |root, report, expected| {
        let mut registry = match bytes(expected, REGISTRY) {
            Some(text) => {
                yaml_serde::from_slice::<brr::Registry>(&text).map_err(|e| e.to_string())?
            }
            None => brr::Registry {
                version: 1,
                domains: BTreeMap::new(),
                rules: BTreeMap::new(),
            },
        };
        if registry.domains.contains_key(&slug) || registry.domains.values().any(|p| p == &prefix) {
            return Err("Domain slug and prefix must both be unique; an existing registration is not overwritten".into());
        }
        let glossary = format!(".badr/brrs/domains/{slug}/glossary.md");
        if expected.contains_key(&glossary)
            || expected.contains_key(&format!(".badr/brrs/domains/{slug}"))
        {
            return Err("Refusing existing unrelated domain storage".into());
        }
        let event = event(
            root,
            report,
            "domain_add",
            Some((&actor, &rationale)),
            serde_json::json!({"domain":{"slug":slug,"prefix":prefix},"changes":[]}),
        )?;
        let mut ledger = bytes(expected, LEDGER).unwrap_or_default();
        ledger.extend(serde_json::to_vec(&event).map_err(|e| e.to_string())?);
        ledger.push(b'\n');
        registry.domains.insert(slug.clone(), prefix.clone());
        let writes = vec![
            (
                glossary,
                format!("# {slug} glossary\n\nDefine domain terms here.\n").into_bytes(),
            ),
            (
                REGISTRY.to_owned(),
                yaml_serde::to_string(&registry)
                    .map_err(|e| e.to_string())?
                    .into_bytes(),
            ),
            (LEDGER.to_owned(), ledger),
        ];
        Ok(Plan {
            writes,
            affected_ids: vec![],
            message: format!(
                "Register BRR domain {slug} ({prefix})\n\nActor: {actor}\nRationale: {rationale}\nBaseline: {}\nEvidence: {}",
                report.baseline_commit.as_deref().unwrap(),
                event["event_id"].as_str().unwrap()
            ),
        })
    })
}
