use crate::repository::{Report, safe_entry};
use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, path::Path};
use yaml_rust2::scanner::{Scanner, TScalarStyle, Token, TokenType};
use yaml_serde::Value;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Metadata {
    pub id: String,
    pub status: String,
    date: String,
    #[serde(default, rename = "decision-makers")]
    decision_makers: Vec<String>,
    #[serde(default)]
    supersedes: Vec<String>,
    #[serde(default)]
    superseded_by: Vec<String>,
    #[serde(default)]
    related_adrs: Vec<String>,
    #[serde(default)]
    related_business_rules: Vec<String>,
}
#[derive(Serialize)]
pub(crate) struct Record {
    #[serde(flatten)]
    pub metadata: Metadata,
    #[serde(rename = "type")]
    pub record_type: &'static str,
    pub path: String,
    pub directs_new_work: bool,
    pub body_start_line: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decision_body: Option<String>,
}

struct Invalid {
    code: &'static str,
    message: String,
    stored_file_line: Option<usize>,
    body_line: Option<usize>,
}
impl Invalid {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            stored_file_line: None,
            body_line: None,
        }
    }
    fn at(mut self, line: usize) -> Self {
        self.stored_file_line = Some(line);
        self
    }
    fn body(mut self, line: usize, start: usize) -> Self {
        self.body_line = Some(line);
        self.stored_file_line = Some(start + line - 1);
        self
    }
}
fn diagnose(report: &mut Report, path: &Path, id: Option<&str>, error: Invalid) {
    report.fail(2, error.code, error.message, Some(path));
    let diagnostic = report.diagnostics.last_mut().unwrap();
    diagnostic.record_id = id.map(String::from);
    diagnostic.stored_file_line = error.stored_file_line;
    diagnostic.body_line = error.body_line;
}
fn numeric_id(id: &str) -> bool {
    id.len() >= 4 && id.bytes().all(|b| b.is_ascii_digit())
}
fn identity(id: &str) -> &str {
    let number = id.trim_start_matches('0');
    if number.is_empty() { "0" } else { number }
}
fn title(value: &str) -> bool {
    !value.is_empty()
        && value.split('-').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        })
}
fn calendar_date(value: &str) -> bool {
    if value.len() != 10
        || value.as_bytes()[4] != b'-'
        || value.as_bytes()[7] != b'-'
        || !value
            .bytes()
            .enumerate()
            .all(|(i, b)| i == 4 || i == 7 || b.is_ascii_digit())
    {
        return false;
    }
    let year = value[..4].parse::<u32>().unwrap();
    let month = value[5..7].parse::<u32>().unwrap();
    let day = value[8..].parse::<u32>().unwrap();
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        _ => 0,
    };
    year > 0 && day > 0 && day <= days
}

fn metadata(text: &str) -> Result<Metadata, Invalid> {
    let mut scanner = Scanner::new(text.chars());
    let tokens: Vec<_> = scanner.by_ref().collect();
    if let Some(error) = scanner.get_error() {
        return Err(
            Invalid::new("invalid_metadata", error.to_string()).at(error.marker().line() + 1)
        );
    }
    if let Some(Token(mark, _)) = tokens
        .iter()
        .find(|Token(_, token)| matches!(token, TokenType::Tag(..) | TokenType::TagDirective(..)))
    {
        return Err(
            Invalid::new("unsafe_yaml_tag", "Explicit YAML tags are not permitted")
                .at(mark.line() + 1),
        );
    }
    // Serde's value parser rejects duplicate keys before typed deserialization.
    let value: Value = yaml_serde::from_str(text).map_err(|error| {
        let line = error.location().map(|location| location.line() + 1);
        Invalid {
            stored_file_line: line,
            ..Invalid::new("invalid_metadata", error.to_string())
        }
    })?;
    let field_line = |key: &str| {
        tokens
            .windows(2)
            .find_map(|window| match window {
                [
                    Token(_, TokenType::Key),
                    Token(mark, TokenType::Scalar(_, name)),
                ] if name == key => Some(mark.line() + 1),
                _ => None,
            })
            .unwrap_or(2)
    };
    let map = value
        .as_mapping()
        .ok_or_else(|| Invalid::new("invalid_metadata", "Frontmatter must be a mapping").at(2))?;
    for (key, value) in map {
        let key = key.as_str().ok_or_else(|| {
            Invalid::new("invalid_metadata", "Metadata keys must be strings").at(2)
        })?;
        let valid = match key {
            "id" | "status" | "date" => matches!(value, Value::String(_)),
            "decision-makers"
            | "supersedes"
            | "superseded_by"
            | "related_adrs"
            | "related_business_rules" => value
                .as_sequence()
                .is_some_and(|items| items.iter().all(|v| matches!(v, Value::String(_)))),
            _ => false,
        };
        if !valid {
            return Err(Invalid::new(
                "invalid_metadata",
                format!("Unknown key or invalid type for {key}"),
            )
            .at(field_line(key)));
        }
    }
    for key in ["id", "date", "supersedes", "superseded_by", "related_adrs"] {
        let field = tokens.windows(3).position(|window| matches!(window,
            [Token(_, TokenType::Key), Token(_, TokenType::Scalar(_, name)), Token(_, TokenType::Value)] if name == key));
        let Some(index) = field else {
            continue;
        };
        let values: Vec<_> = tokens[index + 3..]
            .iter()
            .take_while(|Token(_, token)| !matches!(token, TokenType::Key | TokenType::BlockEnd))
            .collect();
        if let Some(Token(mark, _)) = values.iter().find(|Token(_, token)| matches!(token, TokenType::Alias(_)) ||
            matches!(token, TokenType::Scalar(style, _) if !matches!(style, TScalarStyle::SingleQuoted | TScalarStyle::DoubleQuoted))) {
            return Err(Invalid::new("invalid_metadata", format!("{key} requires explicitly quoted strings")).at(mark.line() + 1));
        }
    }
    let metadata: Metadata = yaml_serde::from_value(value)
        .map_err(|error| Invalid::new("invalid_metadata", error.to_string()).at(2))?;
    if !numeric_id(&metadata.id) {
        return Err(Invalid::new(
            "invalid_identity",
            "ADR ID requires at least four numeric digits",
        )
        .at(field_line("id")));
    }
    if !matches!(
        metadata.status.as_str(),
        "proposed" | "approved" | "rejected" | "deprecated" | "superseded"
    ) {
        return Err(Invalid::new(
            "invalid_status",
            "ADR status must be proposed, approved, rejected, deprecated, or superseded",
        )
        .at(field_line("status")));
    }
    if !calendar_date(&metadata.date) {
        return Err(
            Invalid::new("invalid_date", "Expected a real YYYY-MM-DD calendar date")
                .at(field_line("date")),
        );
    }
    if metadata.decision_makers.iter().any(|s| s.trim().is_empty()) {
        return Err(Invalid::new(
            "invalid_metadata",
            "Decision-makers must be nonempty descriptions",
        )
        .at(field_line("decision-makers")));
    }
    for (field, ids) in [
        ("supersedes", &metadata.supersedes),
        ("superseded_by", &metadata.superseded_by),
        ("related_adrs", &metadata.related_adrs),
    ] {
        if ids.iter().any(|id| !numeric_id(id)) {
            return Err(Invalid::new(
                "invalid_relationship",
                "ADR relationships require quoted numeric IDs with at least four digits",
            )
            .at(field_line(field)));
        }
    }
    for id in &metadata.related_business_rules {
        let parts: Vec<_> = id.split('-').collect();
        if parts.len() != 3
            || parts[0] != "BRR"
            || parts[1].is_empty()
            || !parts[1].bytes().all(|b| b.is_ascii_uppercase())
            || parts[2].len() < 3
            || !parts[2].bytes().all(|b| b.is_ascii_digit())
        {
            return Err(
                Invalid::new("invalid_relationship", "Invalid related BRR ID")
                    .at(field_line("related_business_rules")),
            );
        }
    }
    Ok(metadata)
}
struct Heading {
    level: HeadingLevel,
    text: String,
    start: usize,
    end: usize,
}
fn headings(body: &str) -> Vec<Heading> {
    let mut headings = Vec::new();
    let mut depth = 0usize;
    let mut heading = None;
    for (event, range) in Parser::new(body).into_offset_iter() {
        match event {
            Event::Start(tag) => {
                if let Tag::Heading { level, .. } = tag {
                    if depth == 0 {
                        heading = Some(Heading {
                            level,
                            text: String::new(),
                            start: range.start,
                            end: range.end,
                        });
                    }
                }
                depth += 1;
            }
            Event::End(tag) => {
                depth -= 1;
                if matches!(tag, TagEnd::Heading(_)) && depth == 0 {
                    if let Some(h) = heading.take() {
                        headings.push(h);
                    }
                }
            }
            Event::Text(text) | Event::Code(text) => {
                if let Some(h) = &mut heading {
                    h.text.push_str(&text);
                }
            }
            _ => {}
        }
    }
    headings
}
fn line_at(text: &str, offset: usize) -> usize {
    text[..offset].bytes().filter(|b| *b == b'\n').count() + 1
}
fn has_content(text: &str) -> bool {
    let mut in_heading = false;
    Parser::new(text).any(|event| match event {
        Event::Start(Tag::Heading { .. }) => {
            in_heading = true;
            false
        }
        Event::End(TagEnd::Heading(_)) => {
            in_heading = false;
            false
        }
        Event::Text(text) | Event::Code(text) => !in_heading && !text.trim().is_empty(),
        _ => false,
    })
}
fn decision_body(body: &str, start_line: usize) -> Result<&str, Invalid> {
    let headings = headings(body);
    let lifecycle: Vec<_> = headings
        .iter()
        .enumerate()
        .filter(|(_, h)| h.text == "Lifecycle notes")
        .collect();
    let end = if let Some(&(index, section)) = lifecycle.first() {
        if lifecycle.len() != 1
            || section.level != HeadingLevel::H2
            || headings[index + 1..]
                .iter()
                .any(|h| h.level <= HeadingLevel::H2)
        {
            return Err(Invalid::new(
                "invalid_lifecycle_notes",
                "Lifecycle notes must be one final level-two section",
            )
            .body(line_at(body, section.start), start_line));
        }
        let notes = &headings[index + 1..];
        if notes.is_empty() || !body[section.end..notes[0].start].trim().is_empty() {
            return Err(Invalid::new(
                "invalid_lifecycle_notes",
                "Lifecycle notes require dated level-three entries",
            )
            .body(line_at(body, section.start), start_line));
        }
        for (i, note) in notes.iter().enumerate() {
            let date = note.text.get(..10).unwrap_or("");
            let remainder = note.text.get(10..).unwrap_or("");
            let next = notes.get(i + 1).map_or(body.len(), |h| h.start);
            if note.level != HeadingLevel::H3
                || !calendar_date(date)
                || (!remainder.is_empty() && !remainder.starts_with(char::is_whitespace))
                || !has_content(&body[note.end..next])
            {
                return Err(Invalid::new(
                    "invalid_lifecycle_notes",
                    "Each lifecycle note requires a ### YYYY-MM-DD heading and nonempty text",
                )
                .body(line_at(body, note.start), start_line));
            }
        }
        section.start
    } else {
        body.len()
    };
    for name in [
        "Context and Problem Statement",
        "Decision Drivers",
        "Considered Options",
        "Decision Outcome",
        "Consequences",
    ] {
        let matches: Vec<_> = headings
            .iter()
            .filter(|h| {
                h.start < end
                    && h.text == name
                    && (h.level == HeadingLevel::H2
                        || (name == "Consequences" && h.level == HeadingLevel::H3))
            })
            .collect();
        if matches.len() != 1 {
            return Err(Invalid::new(
                "missing_reasoning",
                format!("Require exactly one {name} section"),
            )
            .body(1, start_line));
        }
        let section = matches[0];
        if name == "Consequences"
            && section.level == HeadingLevel::H3
            && headings
                .iter()
                .rev()
                .find(|h| h.start < section.start && h.level <= HeadingLevel::H2)
                .is_none_or(|h| h.text != "Decision Outcome")
        {
            return Err(Invalid::new(
                "missing_reasoning",
                "Level-three Consequences must belong to Decision Outcome",
            )
            .body(line_at(body, section.start), start_line));
        }
        let section_end = headings
            .iter()
            .find(|h| {
                h.start > section.start
                    && (h.level <= section.level
                        || (name == "Decision Outcome"
                            && matches!(h.text.as_str(), "Consequences" | "Confirmation")))
            })
            .map_or(end, |h| h.start.min(end));
        if !has_content(&body[section.end..section_end]) {
            return Err(Invalid::new("empty_reasoning", format!("{name} requires content; Decision Outcome must state rationale (unresolved items may be explicit)")).body(line_at(body, section.start), start_line));
        }
    }
    Ok(&body[..end])
}
fn parse(text: &str, path: &str) -> Result<Record, Invalid> {
    let mut lines = text.split_inclusive('\n');
    if lines.next().map(str::trim_end) != Some("---") {
        return Err(Invalid::new(
            "invalid_frontmatter",
            "ADR must start with --- YAML frontmatter",
        )
        .at(1));
    }
    let start = text.find('\n').unwrap_or(text.len()) + 1;
    let mut end = start;
    let mut body_start = None;
    for line in lines {
        if line.trim_end() == "---" {
            body_start = Some(end + line.len());
            break;
        }
        end += line.len();
    }
    let body_start = body_start.ok_or_else(|| {
        Invalid::new(
            "invalid_frontmatter",
            "Missing closing --- frontmatter delimiter",
        )
        .at(1)
    })?;
    let metadata = metadata(&text[start..end])?;
    let filename = Path::new(path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    let (id, slug) = filename
        .strip_suffix(".md")
        .and_then(|s| s.split_once('-'))
        .ok_or_else(|| Invalid::new("invalid_filename", "Expected NNNN-lowercase-title.md"))?;
    if !numeric_id(id) || !title(slug) || id != metadata.id {
        return Err(Invalid::new(
            "invalid_identity",
            "Filename must have a numeric ID matching metadata and a lowercase title",
        ));
    }
    let start_line = line_at(text, body_start);
    let decision = decision_body(&text[body_start..], start_line)?;
    Ok(Record {
        directs_new_work: metadata.status == "approved",
        metadata,
        record_type: "adr",
        path: path.into(),
        body_start_line: start_line,
        decision_body: Some(decision.into()),
    })
}

pub(crate) fn inspect(root: &Path, report: &mut Report) -> bool {
    let directory = root.join(".badr/adrs");
    let diagnostic_start = report.diagnostics.len();
    let mut populated = false;
    let result = (|| -> std::io::Result<()> {
        let Some(meta) = safe_entry(&directory)? else {
            return Ok(());
        };
        if !meta.is_dir() {
            populated = true;
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "ADR collection must be a directory",
            ));
        }
        let mut entries = fs::read_dir(&directory)?.collect::<Result<Vec<_>, _>>()?;
        populated = !entries.is_empty();
        entries.sort_by_key(|e| e.file_name());
        let mut ids = BTreeMap::new();
        for entry in entries {
            let path = entry.path();
            let relative = path.strip_prefix(root).unwrap();
            let meta = match safe_entry(&path) {
                Ok(Some(meta)) => meta,
                Ok(None) => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        "ADR disappeared during inspection",
                    ));
                }
                Err(error) if error.kind() == std::io::ErrorKind::InvalidInput => {
                    diagnose(
                        report,
                        relative,
                        None,
                        Invalid::new("unsafe_path", error.to_string()),
                    );
                    continue;
                }
                Err(error) => return Err(error),
            };
            if !meta.is_file() {
                diagnose(
                    report,
                    relative,
                    None,
                    Invalid::new(
                        "unsafe_path",
                        "ADRs must be regular files directly inside .badr/adrs",
                    ),
                );
                continue;
            }
            // Reserve filename identities even when a record has invalid metadata.
            if let Some((id, _)) = entry.file_name().to_str().and_then(|s| s.split_once('-')) {
                if id.bytes().all(|b| b.is_ascii_digit()) && !id.is_empty() {
                    let number = identity(id).to_string();
                    if let Some(previous) = ids.insert(number, relative.to_path_buf()) {
                        diagnose(
                            report,
                            relative,
                            Some(id),
                            Invalid::new(
                                "duplicate_identity",
                                format!(
                                    "Numeric ADR identity collides with {}",
                                    previous.display()
                                ),
                            ),
                        );
                    }
                }
            }
            let bytes = fs::read(&path)?;
            match std::str::from_utf8(&bytes) {
                Ok(text) => match parse(text, &relative.to_string_lossy()) {
                    Ok(record) => report.records.push(crate::repository::Record::Adr(record)),
                    Err(error) => {
                        let filename = entry.file_name();
                        let id = filename
                            .to_str()
                            .and_then(|s| s.split_once('-'))
                            .map(|(id, _)| id);
                        diagnose(report, relative, id, error);
                    }
                },
                Err(_) => diagnose(
                    report,
                    relative,
                    None,
                    Invalid::new("invalid_utf8", "ADR must be UTF-8"),
                ),
            }
        }
        Ok(())
    })();
    if let Err(error) = result {
        populated = true;
        if error.kind() == std::io::ErrorKind::InvalidInput {
            diagnose(
                report,
                Path::new(".badr/adrs"),
                None,
                Invalid::new("unsafe_path", error.to_string()),
            );
        } else {
            report.io(error);
        }
    }
    if report.diagnostics.len() != diagnostic_start {
        report.validation_coverage.collection = "incomplete";
    }
    if populated {
        report.validation_coverage.evidence = "incomplete";
        report.fail(3, "evidence_incomplete", "ADR authorization, body digests, relationship targets and append-only notes require evidence/history checks not implemented here; semantic adequacy and existing-code conformance are not assessed.".into(), None);
    }
    populated
}
