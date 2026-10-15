use super::Report;
use chrono::{DateTime, FixedOffset, Utc};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs, io,
    path::{Component, Path},
    time::SystemTime,
};
use yaml_serde::Value;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Metadata {
    pub id: String,
    pub domain: String,
    pub status: String,
    pub owners: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    effective_from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    effective_until: Option<String>,
    #[serde(default)]
    related_architecture_decisions: Vec<String>,
    #[serde(default)]
    supersedes: Vec<String>,
    #[serde(default)]
    superseded_by: Vec<String>,
}
#[derive(Serialize)]
pub(crate) struct Record {
    #[serde(rename = "type")]
    kind: &'static str,
    #[serde(flatten)]
    pub metadata: Metadata,
    pub path: String,
    pub title: String,
    pub selection: &'static str,
    pub currently_effective: bool,
    pub body: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Registry {
    version: u8,
    domains: BTreeMap<String, String>,
    rules: BTreeMap<String, String>,
}

fn slug(value: &str) -> bool {
    !value.is_empty()
        && value.split('-').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        })
}
fn prefix(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|c| c.is_ascii_uppercase())
}
fn id(value: &str) -> Option<(&str, &str)> {
    let (prefix_value, number) = value.strip_prefix("BRR-")?.split_once('-')?;
    (prefix(prefix_value) && number.len() >= 3 && number.bytes().all(|c| c.is_ascii_digit()))
        .then_some((prefix_value, number))
}
fn adr_id(value: &str) -> bool {
    value.len() >= 4 && value.bytes().all(|c| c.is_ascii_digit())
}
fn tagged(value: &Value) -> bool {
    match value {
        Value::Tagged(_) => true,
        Value::Mapping(map) => map.iter().any(|(k, v)| tagged(k) || tagged(v)),
        Value::Sequence(values) => values.iter().any(tagged),
        _ => false,
    }
}
fn yaml(text: &str) -> Result<Value, String> {
    let value: Value =
        yaml_serde::from_str(text).map_err(|e| format!("YAML input-relative location: {e}"))?;
    if tagged(&value) {
        return Err("YAML tags are not permitted".into());
    }
    if !value.is_mapping() {
        return Err("Expected a YAML mapping".into());
    }
    Ok(value)
}
fn string_lists(value: &Value, lists: &[&str]) -> Result<(), String> {
    for (key, value) in value.as_mapping().ok_or("Expected mapping")? {
        let key = key.as_str().ok_or("Metadata keys must be strings")?;
        if lists.contains(&key) {
            if !value
                .as_sequence()
                .is_some_and(|items| items.iter().all(Value::is_string))
            {
                return Err(format!("{key} must be a list of strings"));
            }
        } else if !value.is_string() {
            return Err(format!("{key} must be a string"));
        }
    }
    Ok(())
}
fn timestamp(value: &str) -> Result<DateTime<FixedOffset>, String> {
    // RFC3339 supplies seconds and an explicit timezone; reject its relaxed space separator.
    if value.as_bytes().get(10) != Some(&b'T') {
        return Err("Applicability times require ISO 8601 seconds and an explicit timezone".into());
    }
    DateTime::parse_from_rfc3339(value).map_err(|_| "Applicability times require a valid ISO 8601 timestamp with seconds and explicit timezone".into())
}
fn quoted_times(text: &str) -> Result<(), String> {
    use saphyr_parser::{Event, Parser, ScalarStyle};
    // Serde values discard quoting; parser events retain scalar styles, including anchored values.
    let mut depth = 0;
    let mut key: Option<std::borrow::Cow<'_, str>> = None;
    let mut anchors = BTreeMap::new();
    for event in Parser::new_from_str(text) {
        let (event, _) = event.map_err(|e| e.to_string())?;
        let scalar = match event {
            Event::MappingStart(..) | Event::SequenceStart(..) => {
                if depth == 1 {
                    key = None;
                }
                depth += 1;
                None
            }
            Event::MappingEnd | Event::SequenceEnd => {
                depth -= 1;
                None
            }
            Event::Scalar(value, style, anchor, _) => {
                if anchor != 0 {
                    anchors.insert(anchor, (value.clone(), style));
                }
                Some((value, style))
            }
            Event::Alias(anchor) => {
                let scalar = anchors.get(&anchor).cloned();
                if depth == 1 && scalar.is_none() {
                    key = None;
                }
                scalar
            }
            _ => None,
        };
        if depth == 1 {
            if let Some((value, style)) = scalar {
                if let Some(name) = key.take() {
                    if matches!(name.as_ref(), "effective_from" | "effective_until")
                        && !matches!(style, ScalarStyle::SingleQuoted | ScalarStyle::DoubleQuoted)
                    {
                        return Err(format!("{name} must be a quoted YAML timestamp"));
                    }
                } else {
                    key = Some(value);
                }
            }
        }
    }
    Ok(())
}
fn metadata(text: &str) -> Result<Metadata, String> {
    let value = yaml(text)?;
    string_lists(
        &value,
        &[
            "owners",
            "related_architecture_decisions",
            "supersedes",
            "superseded_by",
        ],
    )?;
    let m: Metadata = yaml_serde::from_value(value).map_err(|e| e.to_string())?;
    if id(&m.id).is_none() || !slug(&m.domain) {
        return Err(
            "Expected BRR-<UPPERCASE-PREFIX>-<at-least-three-digits> and a lowercase domain slug"
                .into(),
        );
    }
    if m.owners.is_empty() || m.owners.iter().any(|s| s.trim().is_empty()) {
        return Err("owners must contain nonempty descriptive strings; owners do not establish authorization".into());
    }
    if !m.related_architecture_decisions.iter().all(|s| adr_id(s))
        || !m
            .supersedes
            .iter()
            .chain(&m.superseded_by)
            .all(|s| id(s).is_some())
    {
        return Err("Relationships must contain stable IDs; ADR IDs must be quoted numeric strings of at least four digits".into());
    }
    if m.effective_from.is_some() || m.effective_until.is_some() {
        quoted_times(text)?;
    }
    let from = m.effective_from.as_deref().map(timestamp).transpose()?;
    let until = m.effective_until.as_deref().map(timestamp).transpose()?;
    match m.status.as_str() {
        "draft" | "proposed" | "approved" if from.is_none() && until.is_none() => {},
        "effective" if from.is_some() && until.is_none() => {},
        "deprecated" | "superseded" | "retired" if from.is_some() && until.is_some() && until >= from => {},
        _ => return Err("Invalid status/applicability: pre-activation records have no times; effective records require only effective_from; withdrawn records retain effective_from and require effective_until >= effective_from".into()),
    }
    let now: DateTime<Utc> = SystemTime::now().into();
    if from.is_some_and(|t| t > now) || until.is_some_and(|t| t > now) {
        return Err("Future applicability events are outside v1".into());
    }
    Ok(m)
}

fn frontmatter(text: &str) -> Result<(&str, &str, usize), String> {
    let mut lines = text.split_inclusive('\n');
    if lines.next().map(|s| s.trim_end_matches(['\r', '\n'])) != Some("---") {
        return Err("Expected leading --- YAML frontmatter delimiter at stored-file line 1".into());
    }
    let start = text.find('\n').ok_or("Unclosed frontmatter")? + 1;
    let mut offset = start;
    for (index, line) in lines.enumerate() {
        if line.trim_end_matches(['\r', '\n']) == "---" {
            return Ok((
                &text[start..offset],
                &text[offset + line.len()..],
                index + 2,
            ));
        }
        offset += line.len();
    }
    Err("Missing closing --- frontmatter delimiter".into())
}
#[derive(Deserialize)]
struct Dialect {
    feature: Vec<String>,
    rule: Vec<String>,
    #[serde(rename = "scenarioOutline")]
    scenario_outline: Vec<String>,
}
fn parse(text: &str, path: &str) -> Result<Record, String> {
    let (header, body, offset) = frontmatter(text)?;
    let metadata = metadata(header)
        .map_err(|e| format!("Frontmatter (input line 1 is stored-file line 2): {e}"))?;
    let feature = gherkin::Feature::parse(body, gherkin::GherkinEnv::default()).map_err(|e| {
        // ParseError's location is private in 0.14; omit its unordered expected-token set.
        let message = e.to_string();
        let line = message
            .strip_prefix("Error at ")
            .and_then(|s| s.split(':').next())
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(1);
        format!(
            "Invalid Gherkin syntax at body-relative line {line}, stored-file line {}",
            offset + line
        )
    })?;
    let location = |line: usize| {
        format!(
            "body-relative line {line}, stored-file line {}",
            offset + line
        )
    };
    if feature.rules.len() != 1
        || feature.rules[0].name.trim().is_empty()
        || !feature.scenarios.is_empty()
    {
        return Err(format!(
            "Require exactly one named Rule with scenarios belonging to that Rule ({})",
            location(feature.position.line)
        ));
    }
    let language = body
        .lines()
        .find(|s| !s.trim().is_empty())
        .and_then(|s| s.trim().strip_prefix('#'))
        .and_then(|s| s.split_once(':'))
        .filter(|(key, _)| key.trim() == "language")
        .map(|(_, language)| language.trim())
        .unwrap_or("en");
    // shortcut: 0.14 exposes no outline kind; use its bundled dialect keywords until the AST exposes it.
    let outlines: BTreeMap<String, Dialect> =
        serde_json::from_str(include_str!("gherkin-keywords.json"))
            .expect("bundled parser keywords");
    let keywords = outlines
        .get(language)
        .ok_or("Unsupported Gherkin language")?;
    // The 0.14 parser accepts a second Feature as description text; reject reserved directives there.
    let descriptions = feature
        .description
        .iter()
        .chain(
            feature
                .background
                .iter()
                .filter_map(|b| b.description.as_ref()),
        )
        .chain(feature.rules.iter().filter_map(|r| r.description.as_ref()))
        .chain(
            feature
                .rules
                .iter()
                .flat_map(|r| r.background.iter())
                .filter_map(|b| b.description.as_ref()),
        )
        .chain(
            feature
                .rules
                .iter()
                .flat_map(|r| r.scenarios.iter())
                .filter_map(|s| s.description.as_ref()),
        );
    if descriptions.flat_map(|s| s.lines()).any(|line| {
        keywords
            .feature
            .iter()
            .chain(&keywords.rule)
            .any(|k| line.trim_start().starts_with(&format!("{k}:")))
    }) {
        return Err(format!(
            "Require exactly one Feature and one named Rule ({})",
            location(feature.position.line)
        ));
    }
    let rule = &feature.rules[0];
    if rule.scenarios.is_empty() {
        return Err(format!(
            "Rule requires at least one named scenario ({})",
            location(rule.position.line)
        ));
    }
    for scenario in &rule.scenarios {
        if scenario.name.trim().is_empty() {
            return Err(format!(
                "Scenarios must be named ({})",
                location(scenario.position.line)
            ));
        }
        if keywords.scenario_outline.contains(&scenario.keyword)
            && (scenario.examples.is_empty()
                || scenario.examples.iter().any(|e| {
                    !e.table.as_ref().is_some_and(|t| {
                        t.rows.len() >= 2 && t.rows[0].iter().all(|s| !s.trim().is_empty())
                    })
                }))
        {
            return Err(format!(
                "Scenario outlines require populated Examples with a header and data row ({})",
                location(scenario.position.line)
            ));
        }
    }
    for (tag, expected) in [
        ("brr:", metadata.id.as_str()),
        ("domain:", metadata.domain.as_str()),
    ] {
        let values: Vec<_> = feature
            .tags
            .iter()
            .filter_map(|s| s.strip_prefix(tag))
            .collect();
        if values != [expected] {
            return Err(format!(
                "Feature @{tag} tag must agree with metadata ({})",
                location(feature.position.line)
            ));
        }
    }
    let filename = Path::new(path)
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or("Non-UTF-8 filename")?;
    let title = filename
        .strip_prefix(&format!("{}-", metadata.id))
        .and_then(|s| s.strip_suffix(".feature"))
        .ok_or("Filename must be <id>-<lowercase-title>.feature")?;
    if !slug(title) {
        return Err("Filename must have a lowercase hyphenated title".into());
    }
    let effective = metadata.status == "effective";
    let pending = matches!(metadata.status.as_str(), "draft" | "proposed" | "approved");
    Ok(Record {
        kind: "brr",
        metadata,
        path: path.into(),
        title: feature.name,
        selection: if effective {
            "effective"
        } else if pending {
            "pending_preview"
        } else {
            "withdrawn"
        },
        currently_effective: effective,
        body: body.into(),
    })
}
fn invalid(report: &mut Report, path: &str, message: String) {
    report.fail(2, "invalid_brr", message, Some(Path::new(path)));
}
fn read(root: &Path, path: &str, report: &mut Report) -> Option<String> {
    match fs::read_to_string(root.join(path)) {
        Ok(text) => Some(text),
        Err(e) => {
            report.fail(
                if e.kind() == io::ErrorKind::InvalidData {
                    2
                } else {
                    4
                },
                "brr_read_error",
                format!("Cannot read UTF-8 managed file: {e}"),
                Some(Path::new(path)),
            );
            None
        }
    }
}

pub(super) fn inspect(root: &Path, files: &BTreeSet<String>, report: &mut Report) {
    const REGISTRY: &str = ".badr/brrs/registry.yaml";
    let features: Vec<_> = files
        .iter()
        .filter(|p| p.starts_with(".badr/brrs/") && p.ends_with(".feature"))
        .collect();
    let registry = if files.contains(REGISTRY) {
        let result = read(root, REGISTRY, report).map(|text| {
            let value = yaml(&text)?;
            let registry: Registry = yaml_serde::from_value(value).map_err(|e| e.to_string())?;
            if registry.version != 1 {
                return Err("Unsupported BRR registry version".into());
            }
            Ok(registry)
        });
        match result {
            Some(Ok(r)) => Some(r),
            Some(Err(e)) => {
                invalid(report, REGISTRY, e);
                None
            }
            None => None,
        }
    } else {
        if !files.is_empty() {
            invalid(
                report,
                REGISTRY,
                "Every managed feature requires registry registration".into(),
            );
        }
        None
    };
    if let Some(registry) = &registry {
        let mut prefixes = BTreeSet::new();
        let mut paths = BTreeSet::new();
        let mut identities = BTreeSet::new();
        for (domain, value) in &registry.domains {
            if !slug(domain) || !prefix(value) || !prefixes.insert(value) {
                invalid(
                    report,
                    REGISTRY,
                    format!("Invalid/duplicate domain or uppercase prefix: {domain}: {value}"),
                );
            }
            let glossary = format!(".badr/brrs/domains/{domain}/glossary.md");
            if !files.contains(&glossary) {
                invalid(
                    report,
                    &glossary,
                    "Registered domains require a regular glossary.md".into(),
                );
            } else {
                read(root, &glossary, report);
            }
        }
        for (key, path) in &registry.rules {
            let identity = id(key).map(|(p, n)| (p, n.trim_start_matches('0')));
            if identity.is_none() || !identities.insert(identity) {
                invalid(
                    report,
                    REGISTRY,
                    format!("Invalid/duplicate BRR identity: {key}"),
                );
            }
            let components: Vec<_> = Path::new(path).components().collect();
            let normalized = !path.is_empty()
                && !path.contains('\\')
                && components.iter().all(|c| matches!(c, Component::Normal(_)))
                && components
                    .iter()
                    .map(|c| c.as_os_str().to_string_lossy())
                    .collect::<Vec<_>>()
                    .join("/")
                    == *path;
            if !normalized || !path.ends_with(".feature") || !paths.insert(path) {
                invalid(
                    report,
                    REGISTRY,
                    format!("Unsafe/duplicate registered path: {path}"),
                );
                continue;
            }
            let stored = format!(".badr/brrs/{path}");
            if !files.contains(&stored) {
                invalid(
                    report,
                    &stored,
                    format!("Registered BRR {key} must exist as a regular feature file"),
                );
            }
        }
    }
    for path in files {
        if path == REGISTRY || path.ends_with(".feature") {
            continue;
        }
        let registered_glossary = registry.as_ref().is_some_and(|r| {
            r.domains
                .keys()
                .any(|domain| *path == format!(".badr/brrs/domains/{domain}/glossary.md"))
        });
        if !registered_glossary {
            invalid(
                report,
                path,
                "Unexpected/unregistered file in BRR storage".into(),
            );
        }
    }
    let mut identities = BTreeSet::new();
    for path in features {
        let Some(text) = read(root, path, report) else {
            continue;
        };
        let record = match parse(&text, path) {
            Ok(r) => r,
            Err(e) => {
                invalid(report, path, e);
                continue;
            }
        };
        let m = &record.metadata;
        let (p, n) = id(&m.id).expect("validated metadata");
        if !identities.insert((p.to_owned(), n.trim_start_matches('0').to_owned())) {
            invalid(report, path, format!("Duplicate BRR identity: {}", m.id));
        }
        if let Some(registry) = &registry {
            let relative = path
                .strip_prefix(".badr/brrs/")
                .expect("feature beneath BRR root");
            if registry.rules.get(&m.id).map(String::as_str) != Some(relative) {
                invalid(
                    report,
                    path,
                    format!("BRR {} is not registered at this path", m.id),
                );
            }
            if registry.domains.get(&m.domain).map(String::as_str) != Some(p) {
                invalid(report, path, "Domain/prefix disagrees with registry".into());
            }
            let expected = format!(".badr/brrs/domains/{}/rules/", m.domain);
            if path.strip_prefix(&expected).is_none_or(|s| s.contains('/')) {
                invalid(
                    report,
                    path,
                    "BRR must be directly beneath its registered domain's rules/ directory".into(),
                );
            }
        }
        report.records.push(record);
    }
    report
        .records
        .sort_by(|a, b| a.metadata.id.cmp(&b.metadata.id).then(a.path.cmp(&b.path)));
}
