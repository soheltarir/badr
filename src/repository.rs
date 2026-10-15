#[path = "brr.rs"]
mod brr;

use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
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
}
#[derive(Serialize)]
pub(crate) struct Coverage {
    configuration: &'static str,
    collection: &'static str,
    evidence: &'static str,
    history: &'static str,
}
#[derive(Serialize)]
pub(crate) struct Writes {
    completed: Vec<String>,
    not_completed: Vec<String>,
}
#[derive(Serialize)]
pub(crate) struct Report {
    schema_version: u8,
    success: bool,
    pub operation: String,
    pub result: String,
    pub exit_code: u8,
    affected_ids: Vec<String>,
    pub records: Vec<brr::Record>,
    pub diagnostics: Vec<Diagnostic>,
    baseline_commit: Option<String>,
    validation_coverage: Coverage,
    pending_commit_durability: Option<bool>,
    partial_write: Writes,
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
    fn fail(&mut self, exit: u8, code: &str, message: String, path: Option<&Path>) {
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
        });
    }
    fn io(&mut self, error: io::Error) {
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
fn safe_entry(path: &Path) -> io::Result<Option<fs::Metadata>> {
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
struct Lock(PathBuf);
impl Drop for Lock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
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
    let lock_path = root.join(".badr.lock");
    let _lock = match OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock_path)
    {
        Ok(_) => Lock(lock_path),
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
                if relative == Path::new(".badr/evidence.jsonl") && meta.len() == 0 {
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
                    report.fail(3, "checks_not_implemented", "ADR/evidence validation is not implemented; no complete result is available".into(), Some(relative));
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
    let has_brrs = files.iter().any(|path| path.ends_with(".feature"));
    if has_brrs {
        report.fail(3, "authorization_checks_incomplete", "BRR structural checks do not establish business-rule adequacy, approval authorization, frozen bodies, historical prefix ownership, or implementation conformance; evidence and historical authorization checks are deferred.".into(), None);
    }
    report.validation_coverage.history = "incomplete";
    let Some(commit) = report.baseline_commit.clone() else {
        return;
    };
    let checks = (|| -> io::Result<bool> {
        let shallow = git(root, &["rev-parse", "--is-shallow-repository"])?;
        if !shallow.status.success() {
            return Err(io::Error::other(
                String::from_utf8_lossy(&shallow.stderr).into_owned(),
            ));
        }
        if shallow.stdout != b"false\n" {
            return Ok(false);
        }
        let history = git(
            root,
            &[
                "log",
                "--full-history",
                "--format=",
                "--name-only",
                &commit,
                "--",
                ".badr",
            ],
        )?;
        if !history.status.success() {
            return Err(io::Error::other(
                String::from_utf8_lossy(&history.stderr).into_owned(),
            ));
        }
        Ok(String::from_utf8_lossy(&history.stdout)
            .lines()
            .all(|line| line.is_empty() || line == ".badr/config.yaml"))
    })();
    match checks {
        Ok(true) if !has_brrs => report.validation_coverage.history = "passed",
        Ok(true) => {},
        Ok(false) => report.fail(3, "history_incomplete", "Shallow history or historical managed records require checks not implemented in this milestone. Supply complete local history; historical record validation is deferred.".into(), None),
        Err(e) => report.io(e),
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
    #[arg(long = "type", value_parser = ["brr"])]
    kind: Option<String>,
    #[arg(long)]
    domain: Option<String>,
    #[arg(long, value_parser = ["draft", "proposed", "approved", "effective", "deprecated", "superseded", "retired"])]
    status: Option<String>,
    #[arg(long, conflicts_with = "pending_preview")]
    effective: bool,
    #[arg(long)]
    pending_preview: bool,
}
pub(crate) fn select(target: &Path, operation: &str, filter: &Filter, id: Option<&str>) -> Report {
    let mut report = inspect(target, operation);
    report.records.retain(|record| {
        let m = &record.metadata;
        id.is_none_or(|id| m.id == id)
            && filter.domain.as_ref().is_none_or(|d| m.domain == *d)
            && filter.status.as_ref().is_none_or(|s| m.status == *s)
            && (!filter.effective || record.currently_effective)
            && (!filter.pending_preview || record.selection == "pending_preview")
    });
    if let Some(id) = id {
        if report.records.is_empty() && report.validation_coverage.collection == "passed" {
            report.fail(
                2,
                "record_not_found",
                format!("No managed BRR with ID {id}"),
                None,
            );
        }
    }
    report
}
