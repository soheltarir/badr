use super::*;
use rustix::fs::{AtFlags, Mode, OFlags, linkat, mkdirat, open, openat, renameat, unlinkat};
use std::{collections::BTreeMap, fs::File, io::Read, os::unix::fs::MetadataExt};

const REGISTRY: &str = ".badr/brrs/registry.yaml";
const LEDGER: &str = ".badr/evidence.jsonl";
const DIRECTORY: OFlags = OFlags::RDONLY
    .union(OFlags::DIRECTORY)
    .union(OFlags::NOFOLLOW)
    .union(OFlags::CLOEXEC);

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

// The open directory descriptors keep publication from following replaced symlinks.
fn directory(root: &File, path: &Path, create: bool) -> io::Result<File> {
    let mut dir = root.try_clone()?;
    for component in path.components() {
        let std::path::Component::Normal(name) = component else {
            return Err(io::Error::other(
                "Expected a repository-relative normal path",
            ));
        };
        match openat(&dir, name, DIRECTORY, Mode::empty()) {
            Ok(fd) => dir = File::from(fd),
            Err(rustix::io::Errno::NOENT) if create => {
                mkdirat(&dir, name, Mode::from_raw_mode(0o755))?;
                dir = File::from(openat(&dir, name, DIRECTORY, Mode::empty())?);
            }
            Err(e) => return Err(e.into()),
        }
    }
    Ok(dir)
}
#[derive(PartialEq, Eq)]
struct Entry {
    identity: (u64, u64),
    bytes: Option<Vec<u8>>,
}
fn snapshot(root: &Path) -> io::Result<BTreeMap<String, Entry>> {
    fn walk(root: &Path, path: &Path, files: &mut BTreeMap<String, Entry>) -> io::Result<()> {
        let meta =
            safe_entry(path)?.ok_or_else(|| io::Error::other("Managed entry disappeared"))?;
        let bytes = if meta.is_file() {
            Some(fs::read(path)?)
        } else if meta.is_dir() {
            None
        } else {
            return Err(io::Error::other(
                "Managed entries must be regular files/directories",
            ));
        };
        files.insert(
            path.strip_prefix(root)
                .unwrap()
                .to_str()
                .ok_or_else(|| io::Error::other("Managed paths must be UTF-8"))?
                .into(),
            Entry {
                identity: (meta.dev(), meta.ino()),
                bytes,
            },
        );
        if meta.is_dir() {
            for entry in fs::read_dir(path)? {
                walk(root, &entry?.path(), files)?;
            }
        }
        Ok(())
    }
    let mut files = BTreeMap::new();
    walk(root, &root.join(".badr"), &mut files)?;
    Ok(files)
}
pub(super) struct WriterLock {
    root: File,
    file: File,
    retain: bool,
}
impl WriterLock {
    pub(super) fn acquire(root: &Path, operation: &str) -> io::Result<Self> {
        let root = File::from(open(root, DIRECTORY, Mode::empty())?);
        let file = File::from(openat(
            &root,
            ".badr.lock",
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::from_raw_mode(0o600),
        )?);
        let mut lock = Self {
            root,
            file,
            retain: false,
        };
        writeln!(
            lock.file,
            "pid={}\nat={}\noperation={operation}",
            std::process::id(),
            chrono::DateTime::<chrono::Utc>::from(std::time::SystemTime::now()).to_rfc3339()
        )?;
        lock.file.sync_all()?;
        Ok(lock)
    }
    fn owned(&self) -> bool {
        let Ok(fd) = openat(
            &self.root,
            ".badr.lock",
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK,
            Mode::empty(),
        ) else {
            return false;
        };
        let Ok(current) = File::from(fd).metadata() else {
            return false;
        };
        self.file
            .metadata()
            .is_ok_and(|old| old.dev() == current.dev() && old.ino() == current.ino())
    }
}
impl Drop for WriterLock {
    fn drop(&mut self) {
        if !self.retain && self.owned() {
            let _ = unlinkat(&self.root, ".badr.lock", AtFlags::empty());
        }
    }
}
fn bytes(snapshot: &BTreeMap<String, Entry>, path: &str) -> Option<Vec<u8>> {
    snapshot.get(path).and_then(|entry| entry.bytes.clone())
}
fn ensure_unchanged(root: &Path, expected: &BTreeMap<String, Entry>) -> io::Result<()> {
    if snapshot(root)? != *expected {
        return Err(io::Error::other(
            "Managed content or file identity changed externally; preserve changes and retry after review",
        ));
    }
    Ok(())
}
fn baseline_unchanged(root: &Path, branch: &str, expected: &str) -> io::Result<()> {
    let out = git(
        root,
        &[
            "rev-parse",
            "--verify",
            &format!("refs/heads/{branch}^{{commit}}"),
        ],
    )?;
    if !out.status.success()
        || out.stdout.strip_suffix(b"\n").unwrap_or(&out.stdout) != expected.as_bytes()
    {
        return Err(io::Error::other(
            "Baseline reference changed before safe application",
        ));
    }
    Ok(())
}
fn publish(
    root: &File,
    path: &str,
    expected: Option<&Entry>,
    content: &[u8],
    published: &mut bool,
) -> io::Result<()> {
    let path = Path::new(path);
    let dir = directory(root, path.parent().unwrap(), true)?;
    let name = path.file_name().unwrap();
    let scratch = format!(".badr-write-{}", std::process::id());
    let mut temp = File::from(openat(
        root,
        &scratch,
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::from_raw_mode(0o600),
    )?);
    let result = (|| -> io::Result<()> {
        temp.write_all(content)?;
        temp.sync_all()?;
        if let Some(expected) = expected {
            let mut old = File::from(openat(
                &dir,
                name,
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK,
                Mode::empty(),
            )?);
            let mut actual = Vec::new();
            old.read_to_end(&mut actual)?;
            let meta = old.metadata()?;
            if expected.bytes.as_deref() != Some(actual.as_slice())
                || expected.identity != (meta.dev(), meta.ino())
            {
                return Err(io::Error::other(
                    "Expected content changed before replacement",
                ));
            }
            renameat(root, &scratch, &dir, name)?;
        } else {
            linkat(root, &scratch, &dir, name, AtFlags::empty())?;
        }
        *published = true;
        Ok(())
    })();
    let cleanup = if *published && expected.is_some() {
        Ok(())
    } else {
        unlinkat(root, &scratch, AtFlags::empty())
    };
    result?;
    cleanup?;
    Ok(())
}

pub(crate) fn run(target: &Path, command: Command) -> Report {
    let Command::Add {
        slug,
        prefix,
        confirm,
        actor,
        rationale,
    } = command;
    let mut report = Report::new("domain_add");
    if !brr::slug(&slug)
        || !brr::prefix(&prefix)
        || !confirm
        || actor.trim().is_empty()
        || rationale.trim().is_empty()
    {
        return Report::invalid("domain_add", "invalid_domain_arguments", "Require a lowercase domain slug, uppercase prefix, --confirm, nonempty --actor and --rationale".into());
    }
    let Some(root) = root(target, &mut report) else {
        return report;
    };
    let mut lock = match WriterLock::acquire(&root, "domain_add") {
        Ok(lock) => lock,
        Err(error) => {
            report.fail(4, "write_locked", format!("Cannot acquire .badr.lock: {error}. Establish that no writer is active before explicit stale-lock cleanup; see docs/cli.md."), None);
            return report;
        }
    };
    let result = (|| -> Result<(), Failure> {
        let config = config(&root)?;
        report.validation_coverage.configuration = "passed";
        baseline(&root, &config.baseline_branch, &mut report);
        inspect_collection(&root, &mut report);
        if report.exit_code != 0 {
            return Ok(());
        }
        if report.validation_coverage.history != "passed" {
            report.fail(
                3,
                "history_incomplete",
                "Complete required history is necessary for registration".into(),
                None,
            );
            return Ok(());
        }
        let baseline_commit = report.baseline_commit.clone().unwrap();
        let mut expected = snapshot(&root)?;
        let mut registry = match bytes(&expected, REGISTRY) {
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
        let branch = git(&root, &["symbolic-ref", "--quiet", "--short", "HEAD"])?;
        if !branch.status.success() {
            return Err(
                "Domain registration requires a named checkout to record the operation branch"
                    .into(),
            );
        }
        let branch = String::from_utf8(branch.stdout)
            .map_err(|e| e.to_string())?
            .trim()
            .to_owned();
        let now: chrono::DateTime<chrono::Utc> = std::time::SystemTime::now().into();
        let at = now.to_rfc3339_opts(chrono::SecondsFormat::Nanos, true);
        let event = serde_json::json!({"version":1,"event_id":format!("domain-{slug}-{at}-{}", std::process::id()),"operation":"domain_add","at":at,"branch":branch,"baseline_commit":baseline_commit,"actor":actor,"rationale":rationale,"confirmed":true,"domain":{"slug":slug,"prefix":prefix},"changes":[]});
        let mut ledger = bytes(&expected, LEDGER).unwrap_or_default();
        ledger.extend(serde_json::to_vec(&event).map_err(|e| e.to_string())?);
        ledger.push(b'\n');
        registry.domains.insert(slug.clone(), prefix.clone());
        let writes = [
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
        let staged = tempfile::tempdir()?;
        for (path, entry) in &expected {
            if let Some(content) = &entry.bytes {
                fs::write(staged.path().join(path), content)?;
            } else {
                fs::create_dir_all(staged.path().join(path))?;
            }
        }
        for (path, content) in &writes {
            fs::create_dir_all(staged.path().join(path).parent().unwrap())?;
            fs::write(staged.path().join(path), content)?;
        }
        let mut proposed = Report::new("domain_add");
        proposed.baseline_commit = Some(baseline_commit.clone());
        proposed.validation_coverage.configuration = "passed";
        inspect_collection_at(staged.path(), &root, &mut proposed);
        if proposed.exit_code != 0 || proposed.validation_coverage.history != "passed" {
            report = proposed;
            if report.exit_code == 0 {
                report.fail(
                    3,
                    "history_incomplete",
                    "Intended state could not be fully validated".into(),
                    None,
                );
            }
            return Ok(());
        }
        report.partial_write.not_completed = writes.iter().map(|(path, _)| path.clone()).collect();
        checkpoint("before_apply")?;
        ensure_unchanged(&root, &expected)?;
        baseline_unchanged(&root, &config.baseline_branch, &baseline_commit)?;
        if !lock.owned() {
            return Err(io::Error::other("Writer lock was replaced externally").into());
        }
        for (index, (path, content)) in writes.iter().enumerate() {
            checkpoint(&format!("before_write_{index}"))?;
            ensure_unchanged(&root, &expected)?;
            baseline_unchanged(&root, &config.baseline_branch, &baseline_commit)?;
            if !lock.owned() {
                return Err(io::Error::other("Writer lock was replaced externally").into());
            }
            lock.retain = true;
            let mut published = false;
            let publication = publish(
                &lock.root,
                path,
                expected.get(path),
                content,
                &mut published,
            );
            if published {
                report.partial_write.completed.push(path.clone());
                report.partial_write.not_completed.remove(0);
            }
            publication?;
            // Capture only our intended changes; external changes must not become the expectation.
            let actual = snapshot(&root)?;
            for (name, entry) in &actual {
                if !expected.contains_key(name)
                    && entry.bytes.is_none()
                    && Path::new(path).starts_with(name)
                {
                    expected.insert(
                        name.clone(),
                        Entry {
                            identity: entry.identity,
                            bytes: None,
                        },
                    );
                }
            }
            let entry = actual.get(path).ok_or("Published file disappeared")?;
            if entry.bytes.as_deref() != Some(content) {
                return Err(io::Error::other("Published file changed externally").into());
            }
            expected.insert(
                path.clone(),
                Entry {
                    identity: entry.identity,
                    bytes: Some(content.clone()),
                },
            );
            ensure_unchanged(&root, &expected)?;
            checkpoint(&format!("after_write_{index}"))?;
        }
        checkpoint("before_postvalidate")?;
        ensure_unchanged(&root, &expected)?;
        baseline_unchanged(&root, &config.baseline_branch, &baseline_commit)?;
        checkpoint("validate_written")?;
        report.records.clear();
        inspect_collection(&root, &mut report);
        if report.exit_code == 0 {
            checkpoint("after_postvalidate")?;
            ensure_unchanged(&root, &expected)?;
            baseline_unchanged(&root, &config.baseline_branch, &baseline_commit)?;
            if !lock.owned() {
                return Err(io::Error::other("Writer lock was replaced externally").into());
            }
            report.result = "created".into();
            report.proposed_audit_commit_message = Some(format!(
                "Register BRR domain {slug} ({prefix})\n\nActor: {actor}\nRationale: {rationale}\nBaseline: {baseline_commit}\nEvidence: {}",
                event["event_id"].as_str().unwrap()
            ));
            lock.retain = false;
        }
        Ok(())
    })();
    if let Err(error) = result {
        match error {
            Failure::Invalid(message) => {
                report.fail(2, "domain_registration_refused", message, None)
            }
            Failure::Operational(error) => report.io(error),
        }
    }
    durability(&root, &mut report);
    if report.exit_code != 0 && !report.partial_write.completed.is_empty() {
        lock.retain = true;
        report.proposed_audit_commit_message = None;
    }
    if lock.retain {
        let _ = writeln!(
            lock.file,
            "manual_repair_required=true\ncompleted={:?}\nnot_completed={:?}",
            report.partial_write.completed, report.partial_write.not_completed
        );
        let _ = lock.file.sync_all();
        report.fail(4, "manual_repair_required", "A write failed or postwrite validation failed. The writer lock is retained even if surviving files parse; review the reported paths and repair manually before explicit cleanup. No rollback was attempted.".into(), None);
    }
    report
}

// Debug builds expose deterministic failure/barrier points for executable integration tests.
fn checkpoint(point: &str) -> io::Result<()> {
    #[cfg(debug_assertions)]
    {
        if std::env::var("BADR_TEST_PAUSE_AT").as_deref() == Ok(point) {
            let dir = PathBuf::from(
                std::env::var_os("BADR_TEST_CONTROL")
                    .ok_or_else(|| io::Error::other("Missing test control directory"))?,
            );
            fs::write(dir.join("ready"), point)?;
            let start = std::time::Instant::now();
            while !dir.join("resume").exists() {
                if start.elapsed().as_secs() > 15 {
                    return Err(io::Error::other("Test barrier timed out"));
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }
        if std::env::var("BADR_TEST_FAIL_AT").as_deref() == Ok(point) {
            return Err(io::Error::other(format!("Injected failure at {point}")));
        }
    }
    let _ = point;
    Ok(())
}
