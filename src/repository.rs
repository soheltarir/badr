#[path = "authoring.rs"]
pub(crate) mod authoring;
#[path = "brr.rs"]
mod brr;
#[path = "domain.rs"]
pub(crate) mod domain;
#[path = "editing.rs"]
pub(crate) mod editing;
#[path = "evidence.rs"]
mod evidence;
#[path = "mutation.rs"]
mod mutation;
#[path = "relationships.rs"]
pub(crate) mod relationships;

use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{self, Write},
    os::unix::ffi::OsStringExt,
    path::{Path, PathBuf},
    process::{Command, Output},
};

#[derive(Serialize)]
pub(crate) struct Diagnostic {
    pub code: String,
    pub message: String,
    pub path: Option<String>,
    pub record_id: Option<String>,
    pub stored_file_line: Option<usize>,
    pub body_line: Option<usize>,
}
#[derive(Serialize)]
pub(crate) struct Coverage {
    pub(crate) configuration: &'static str,
    pub(crate) collection: &'static str,
    pub(crate) evidence: &'static str,
    pub(crate) history: &'static str,
}
#[derive(Serialize)]
pub(crate) struct Writes {
    pub(crate) completed: Vec<String>,
    pub(crate) not_completed: Vec<String>,
}
#[derive(Serialize)]
pub(crate) struct Report {
    schema_version: u8,
    success: bool,
    pub operation: String,
    pub result: String,
    pub exit_code: u8,
    pub(crate) affected_ids: Vec<String>,
    pub records: Vec<Record>,
    pub diagnostics: Vec<Diagnostic>,
    baseline_commit: Option<String>,
    pub(crate) validation_coverage: Coverage,
    pub(crate) pending_commit_durability: Option<bool>,
    pub proposed_audit_commit_message: Option<String>,
    pub(crate) partial_write: Writes,
}
#[derive(Serialize)]
#[serde(untagged)]
pub(crate) enum Record {
    Brr(brr::Record),
    Adr(crate::adr::Record),
}
impl Record {
    pub(crate) fn id(&self) -> &str {
        match self {
            Self::Brr(r) => &r.metadata.id,
            Self::Adr(r) => &r.metadata.id,
        }
    }
    fn path(&self) -> &str {
        match self {
            Self::Brr(r) => &r.path,
            Self::Adr(r) => &r.path,
        }
    }
}
impl Report {
    fn new(operation: &str) -> Self {
        Self {
            schema_version: 1,
            success: true,
            operation: operation.into(),
            result: "ok".into(),
            exit_code: 0,
            affected_ids: vec![],
            records: vec![],
            diagnostics: vec![],
            baseline_commit: None,
            validation_coverage: Coverage {
                configuration: "not_checked",
                collection: "not_checked",
                evidence: "not_checked",
                history: "not_checked",
            },
            pending_commit_durability: None,
            proposed_audit_commit_message: None,
            partial_write: Writes {
                completed: vec![],
                not_completed: vec![],
            },
        }
    }
    pub fn invalid(operation: &str, code: &str, message: String) -> Self {
        let mut report = Self::new(operation);
        report.fail(2, code, message, None);
        report
    }
    pub(crate) fn fail(&mut self, exit: u8, code: &str, message: String, path: Option<&Path>) {
        // Invalidity outranks incomplete coverage; operational failures outrank both.
        if self.exit_code == 0 || exit == 4 || (exit == 2 && self.exit_code == 3) {
            self.exit_code = exit;
        }
        self.success = false;
        self.result = match self.exit_code {
            2 => "invalid",
            3 => "incomplete",
            _ => "operational_failure",
        }
        .into();
        self.diagnostics.push(Diagnostic {
            code: code.into(),
            message,
            path: path.map(|p| p.to_string_lossy().into_owned()),
            record_id: None,
            stored_file_line: None,
            body_line: None,
        });
    }
    pub(crate) fn io(&mut self, error: io::Error) {
        self.fail(4, "io_error", error.to_string(), None);
    }
}

#[derive(Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Config {
    version: u8,
    baseline_branch: String,
}

fn git(root: &Path, args: &[&str]) -> io::Result<Output> {
    Command::new("git")
        .args([
            "-c",
            "core.fsmonitor=false",
            "-c",
            "core.hooksPath=/dev/null",
        ])
        .arg("-C")
        .arg(root)
        .args(args)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_NO_LAZY_FETCH", "1")
        .output()
}
fn root(target: &Path, report: &mut Report) -> Option<PathBuf> {
    let target = match target.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            report.io(e);
            return None;
        }
    };
    match git(&target, &["rev-parse", "--show-toplevel"]) {
        Ok(out) if out.status.success() => Some(PathBuf::from(std::ffi::OsString::from_vec(
            out.stdout
                .strip_suffix(b"\n")
                .unwrap_or(&out.stdout)
                .to_vec(),
        ))),
        Ok(_) => {
            report.fail(
                2,
                "not_repository",
                "Choose an existing non-bare Git working tree; BADR does not initialize Git."
                    .into(),
                Some(&target),
            );
            None
        }
        Err(e) => {
            report.io(e);
            None
        }
    }
}
pub(crate) fn safe_entry(path: &Path) -> io::Result<Option<fs::Metadata>> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("Refusing symlink: {}", path.display()),
        )),
        Ok(meta) => Ok(Some(meta)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}
enum Failure {
    Invalid(String),
    Operational(io::Error),
}
impl From<io::Error> for Failure {
    fn from(error: io::Error) -> Self {
        if error.kind() == io::ErrorKind::InvalidInput {
            Self::Invalid(error.to_string())
        } else {
            Self::Operational(error)
        }
    }
}
impl From<String> for Failure {
    fn from(message: String) -> Self {
        Self::Invalid(message)
    }
}
impl From<&str> for Failure {
    fn from(message: &str) -> Self {
        Self::Invalid(message.into())
    }
}
fn failure(report: &mut Report, error: Failure) {
    match error {
        Failure::Invalid(message) => report.fail(
            2,
            "invalid_configuration",
            message,
            Some(Path::new(".badr/config.yaml")),
        ),
        Failure::Operational(error) => report.io(error),
    }
}
fn config(root: &Path) -> Result<Config, Failure> {
    let managed = root.join(".badr");
    if let Some(meta) = safe_entry(&managed)? {
        if !meta.is_dir() {
            return Err(".badr must be a directory".into());
        }
    }
    let path = managed.join("config.yaml");
    let meta = safe_entry(&path)?.ok_or("Run badr init --baseline <branch> first")?;
    if !meta.is_file() {
        return Err("config.yaml must be a regular file".into());
    }
    let text = fs::read_to_string(path)?;
    let value: yaml_serde::Value = yaml_serde::from_str(&text).map_err(|e| e.to_string())?;
    fn tagged(value: &yaml_serde::Value) -> bool {
        match value {
            yaml_serde::Value::Tagged(_) => true,
            yaml_serde::Value::Mapping(m) => m.iter().any(|(k, v)| tagged(k) || tagged(v)),
            yaml_serde::Value::Sequence(s) => s.iter().any(tagged),
            _ => false,
        }
    }
    if tagged(&value) {
        return Err("YAML tags are not permitted".into());
    }
    let config: Config = yaml_serde::from_str(&text).map_err(|e| e.to_string())?;
    if config.version != 1 {
        return Err("Unsupported configuration version; expected 1".into());
    }
    check_branch(root, &config.baseline_branch)?;
    Ok(config)
}
fn check_branch(root: &Path, branch: &str) -> Result<(), String> {
    let out = git(root, &["check-ref-format", "--branch", branch]).map_err(|e| e.to_string())?;
    if !out.status.success() || branch.starts_with('-') || branch.starts_with("@{-") {
        return Err("Use a literal valid local branch name (for example main); revision expressions are not allowed".into());
    }
    Ok(())
}
fn baseline(root: &Path, branch: &str, report: &mut Report) {
    let reference = format!("refs/heads/{branch}^{{commit}}");
    match git(root, &["rev-parse", "--verify", &reference]) {
        Ok(out) if out.status.success() => report.baseline_commit = Some(String::from_utf8_lossy(&out.stdout).trim().into()),
        Ok(_) => report.fail(3, "baseline_unavailable", format!("Create and commit local branch '{branch}' before governed writes; BADR never fetches automatically."), None),
        Err(e) => report.io(e),
    }
}
pub(crate) fn init(target: &Path, branch: &str) -> Report {
    let mut report = Report::new("init");
    let Some(root) = root(target, &mut report) else {
        return report;
    };
    if let Err(e) = check_branch(&root, branch) {
        return Report::invalid("init", "invalid_baseline", e);
    }
    let _lock = match mutation::WriterLock::acquire(&root, "init") {
        Ok(lock) => lock,
        Err(e) => {
            report.fail(
                4,
                "write_locked",
                format!("Cannot acquire .badr.lock: {e}. See README for safe stale-lock cleanup."),
                None,
            );
            return report;
        }
    };
    let managed = root.join(".badr");
    let path = managed.join("config.yaml");
    let expected = Config {
        version: 1,
        baseline_branch: branch.into(),
    };
    let result = (|| -> Result<bool, Failure> {
        if let Some(meta) = safe_entry(&managed)? {
            if !meta.is_dir() {
                return Err(".badr must be a directory".into());
            }
        }
        if safe_entry(&path)?.is_some() {
            if config(&root)? != expected {
                return Err("Existing configuration conflicts with the requested baseline; it was preserved".into());
            }
            return Ok(false);
        }
        if managed.exists() && fs::read_dir(&managed)?.next().is_some() {
            return Err(
                "Unconfigured .badr contains unrelated data; refusing initialization".into(),
            );
        }
        if !managed.exists() {
            fs::create_dir(&managed)?;
        }
        report
            .partial_write
            .not_completed
            .push(".badr/config.yaml".into());
        let mut temp = tempfile::NamedTempFile::new_in(&managed)?;
        let text = yaml_serde::to_string(&expected).map_err(|e| e.to_string())?;
        temp.write_all(text.as_bytes())?;
        temp.as_file().sync_all()?;
        temp.persist_noclobber(&path).map_err(|e| e.error)?;
        report.partial_write.not_completed.clear();
        Ok(true)
    })();
    match result {
        Ok(created) => {
            report.result = if created { "created" } else { "unchanged" }.into();
            if created {
                report
                    .partial_write
                    .completed
                    .push(".badr/config.yaml".into());
            }
            report.validation_coverage.configuration = "passed";
            baseline(&root, branch, &mut report);
            inspect_collection(&root, &mut report);
            durability(&root, &mut report);
        }
        Err(e) => failure(&mut report, e),
    }
    report
}

pub(crate) fn inspect(target: &Path, operation: &str) -> Report {
    let mut report = Report::new(operation);
    let Some(root) = root(target, &mut report) else {
        return report;
    };
    let config = match config(&root) {
        Ok(config) => config,
        Err(e) => {
            failure(&mut report, e);
            return report;
        }
    };
    report.validation_coverage.configuration = "passed";
    baseline(&root, &config.baseline_branch, &mut report);
    inspect_collection(&root, &mut report);
    durability(&root, &mut report);
    report
}

fn inspect_collection(root: &Path, report: &mut Report) {
    inspect_collection_at(root, root, report);
}
fn inspect_collection_at(root: &Path, history_root: &Path, report: &mut Report) {
    fn walk(
        root: &Path,
        path: &Path,
        report: &mut Report,
        files: &mut std::collections::BTreeSet<String>,
    ) -> io::Result<()> {
        let mut entries = fs::read_dir(path)?.collect::<Result<Vec<_>, _>>()?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let path = entry.path();
            let relative = path.strip_prefix(root).expect("managed path beneath root");
            if relative == Path::new(".badr/adrs") {
                continue;
            }
            let meta = safe_entry(&path)?.ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    "Managed entry disappeared during inspection",
                )
            })?;
            if relative == Path::new(".badr/config.yaml") {
                continue;
            }
            if matches!(relative.to_str(), Some(".badr/brrs" | ".badr/adrs")) && !meta.is_dir() {
                report.fail(
                    2,
                    "unsafe_path",
                    "Record roots must be directories".into(),
                    Some(relative),
                );
                continue;
            }
            if meta.is_dir() {
                if relative.starts_with(".badr/brrs") {
                    let components: Vec<_> = relative.iter().map(|s| s.to_str()).collect();
                    let valid = matches!(
                        components.as_slice(),
                        [Some(".badr"), Some("brrs")]
                            | [Some(".badr"), Some("brrs"), Some("domains")]
                            | [Some(".badr"), Some("brrs"), Some("domains"), Some(_)]
                            | [
                                Some(".badr"),
                                Some("brrs"),
                                Some("domains"),
                                Some(_),
                                Some("rules")
                            ]
                    );
                    if !valid {
                        report.fail(
                            2,
                            "unexpected_path",
                            "Unexpected directory in BRR storage".into(),
                            Some(relative),
                        );
                        continue;
                    }
                }
                if !matches!(
                    relative
                        .components()
                        .nth(1)
                        .and_then(|c| c.as_os_str().to_str()),
                    Some("brrs" | "adrs")
                ) {
                    report.fail(
                        2,
                        "unexpected_path",
                        "Unexpected directory in managed storage".into(),
                        Some(relative),
                    );
                } else {
                    walk(root, &path, report, files)?;
                }
            } else if meta.is_file() {
                if relative == Path::new(".badr/evidence.jsonl") {
                    continue;
                }
                if relative.starts_with(".badr/brrs") {
                    files.insert(
                        relative
                            .to_str()
                            .ok_or_else(|| {
                                io::Error::new(
                                    io::ErrorKind::InvalidInput,
                                    "Managed paths must be UTF-8",
                                )
                            })?
                            .to_owned(),
                    );
                } else {
                    report.fail(
                        2,
                        "unexpected_path",
                        "Unexpected file in managed storage".into(),
                        Some(relative),
                    );
                }
            } else {
                report.fail(
                    2,
                    "unsafe_path",
                    "Managed entries must be regular files or directories".into(),
                    Some(relative),
                );
            }
        }
        Ok(())
    }
    let start = report.diagnostics.len();
    let mut files = std::collections::BTreeSet::new();
    match walk(root, &root.join(".badr"), report, &mut files) {
        Ok(()) => {
            brr::inspect(root, &files, report);
            report.validation_coverage.collection = if report.diagnostics.len() == start {
                "passed"
            } else {
                "incomplete"
            };
            report.validation_coverage.evidence =
                if report.records.is_empty() && report.diagnostics.len() == start {
                    "passed"
                } else {
                    "incomplete"
                };
        }
        Err(e) => {
            report.validation_coverage.collection = "incomplete";
            report.validation_coverage.evidence = "incomplete";
            report.fail(
                if e.kind() == io::ErrorKind::InvalidInput {
                    2
                } else {
                    4
                },
                "unsafe_path",
                e.to_string(),
                None,
            );
        }
    }
    crate::adr::inspect(root, report);
    report
        .records
        .sort_by(|a, b| a.id().cmp(b.id()).then(a.path().cmp(b.path())));
    evidence::inspect(root, history_root, report);
    report.diagnostics.retain(|d| d.code != "policy_gap");
    for record in &report.records {
        if let Record::Brr(rule) = record {
            if matches!(rule.metadata.status.as_str(), "deprecated" | "retired") {
                report.diagnostics.push(Diagnostic {
                    code: "policy_gap".into(),
                    message: "This rule no longer applies. Review any policy gap for explicit human resolution; no replacement is selected and no predecessor is revived. Code conformance is not assessed.".into(),
                    path: Some(rule.path.clone()),
                    record_id: Some(rule.metadata.id.clone()),
                    stored_file_line: None,
                    body_line: None,
                });
            }
        }
    }
    if report.operation != "show" {
        for record in &mut report.records {
            if let Record::Adr(record) = record {
                record.decision_body = None;
            }
        }
    }
}

fn durability(root: &Path, report: &mut Report) {
    match git(
        root,
        &[
            "status",
            "--porcelain=v1",
            "--untracked-files=all",
            "--ignored",
            "--",
            ".badr",
        ],
    ) {
        Ok(out) if out.status.success() => {
            report.pending_commit_durability = Some(!out.stdout.is_empty())
        }
        Ok(out) => report.fail(
            4,
            "git_error",
            String::from_utf8_lossy(&out.stderr).into_owned(),
            None,
        ),
        Err(e) => report.io(e),
    }
}

#[derive(clap::Args, Default)]
pub(crate) struct Filter {
    #[arg(long = "type", value_parser = ["brr", "adr"])]
    kind: Option<String>,
    #[arg(long)]
    domain: Option<String>,
    #[arg(long, value_parser = ["draft", "proposed", "approved", "effective", "deprecated", "superseded", "retired", "rejected"])]
    status: Option<String>,
    #[arg(long, conflicts_with = "pending_preview")]
    effective: bool,
    #[arg(long, conflicts_with = "directing")]
    pending_preview: bool,
    #[arg(long, conflicts_with = "effective")]
    directing: bool,
}
pub(crate) fn select(target: &Path, operation: &str, filter: &Filter, id: Option<&str>) -> Report {
    let mut report = inspect(target, operation);
    report.records.retain(|record| {
        if id.is_some_and(|id| record.id() != id) {
            return false;
        }
        match record {
            Record::Brr(record) => {
                let m = &record.metadata;
                filter.kind.as_deref().is_none_or(|kind| kind == "brr")
                    && filter.domain.as_ref().is_none_or(|d| m.domain == *d)
                    && filter.status.as_ref().is_none_or(|s| m.status == *s)
                    && (!filter.effective || record.currently_effective)
                    && (!filter.pending_preview || record.selection == "pending_preview")
                    && !filter.directing
            }
            Record::Adr(record) => {
                filter.kind.as_deref().is_none_or(|kind| kind == "adr")
                    && filter.domain.is_none()
                    && filter
                        .status
                        .as_ref()
                        .is_none_or(|s| record.metadata.status == *s)
                    && (!filter.directing || record.directs_new_work)
                    && !filter.effective
                    && !filter.pending_preview
            }
        }
    });
    if let Some(id) = id {
        if report.records.is_empty() {
            report.fail(
                2,
                "record_not_found",
                format!("No structurally valid managed record with ID {id}"),
                None,
            );
        }
    }
    report
}
