use super::mutation::{Plan, bytes};
use super::*;
use std::{fs::File, io::Read, process::Stdio};

#[derive(clap::Args)]
pub(crate) struct Edit {
    id: String,
    #[arg(long, required_unless_present_any = ["editor", "register_external"], conflicts_with_all = ["editor", "register_external"])]
    content: Option<PathBuf>,
    /// Absolute editor executable outside the target repository; no shell is used.
    #[arg(long)]
    #[arg(conflicts_with = "register_external")]
    editor: Option<PathBuf>,
    /// Explicitly register the current approved body after an external edit.
    #[arg(long)]
    register_external: bool,
    #[arg(long)]
    confirm: bool,
    #[arg(long)]
    actor: Option<String>,
    #[arg(long)]
    rationale: Option<String>,
}

fn metadata(record: &Record) -> Result<serde_json::Value, Failure> {
    match record {
        Record::Brr(r) => serde_json::to_value(&r.metadata),
        Record::Adr(r) => serde_json::to_value(&r.metadata),
    }
    .map_err(|e| e.to_string().into())
}

pub(crate) fn run(target: &Path, edit: Edit) -> Report {
    let supplied = match edit.content.as_ref().map(fs::read_to_string).transpose() {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::InvalidData => {
            return Report::invalid("edit", "invalid_content", error.to_string());
        }
        Err(error) => {
            let mut report = Report::new("edit");
            report.io(error);
            return report;
        }
    };
    mutation::run_edit(
        target,
        edit.register_external.then_some(edit.id.as_str()),
        |root, report, expected| {
            let record = report
                .records
                .iter()
                .find(|r| r.id() == edit.id)
                .ok_or("Record not found")?;
            let path = record.path();
            let original = bytes(expected, path).ok_or("Record content unavailable")?;
            let original_record = authoring::parse(
                std::str::from_utf8(&original).map_err(|e| e.to_string())?,
                path,
            )?;
            let record = &original_record;
            let supplied = if edit.register_external {
                String::from_utf8(original.clone()).map_err(|e| e.to_string())?
            } else if let Some(text) = supplied {
                text
            } else {
                let executable = edit.editor.as_ref().ok_or("Select --content or --editor")?;
                if !executable.is_absolute() {
                    return Err(
                        "Editor must be an absolute executable path outside the repository".into(),
                    );
                }
                let executable = fs::canonicalize(executable)?;
                if executable.starts_with(root) {
                    return Err("Repository-supplied editors/scripts are not permitted".into());
                }
                let scratch = tempfile::tempdir()?;
                let input = scratch.path().join(Path::new(path).file_name().unwrap());
                fs::write(&input, &original)?;
                let mut command = std::process::Command::new(executable);
                command
                    .arg(&input)
                    .current_dir(scratch.path())
                    .stdin(Stdio::inherit());
                // Terminal editors keep their display off the machine-readable output streams.
                if let Ok(tty) = fs::OpenOptions::new().write(true).open("/dev/tty") {
                    command
                        .stdout(Stdio::from(tty.try_clone()?))
                        .stderr(Stdio::from(tty));
                } else {
                    command.stdout(Stdio::null()).stderr(Stdio::null());
                }
                let status = command.status()?;
                if !status.success() {
                    use std::os::unix::process::ExitStatusExt;
                    if status.code() == Some(130) || status.signal() == Some(2) {
                        return Err(
                            "Editor cancelled; managed content and evidence unchanged".into()
                        );
                    }
                    return Err(io::Error::other(format!(
                        "Editor failed ({status}); managed content and evidence unchanged"
                    ))
                    .into());
                }
                let mut file = File::from(
                    rustix::fs::open(
                        &input,
                        rustix::fs::OFlags::RDONLY
                            | rustix::fs::OFlags::NOFOLLOW
                            | rustix::fs::OFlags::NONBLOCK,
                        rustix::fs::Mode::empty(),
                    )
                    .map_err(io::Error::from)?,
                );
                if !file.metadata()?.is_file() {
                    return Err("Editor result must be a regular file".into());
                }
                let mut text = String::new();
                match file.read_to_string(&mut text) {
                    Ok(_) => text,
                    Err(e) if e.kind() == io::ErrorKind::InvalidData => {
                        return Err(e.to_string().into());
                    }
                    Err(e) => return Err(e.into()),
                }
            };
            let changed = authoring::parse(&supplied, path)?;
            let mut before = metadata(record)?;
            let mut after = metadata(&changed)?;
            let pending = matches!(before["status"].as_str(), Some("draft" | "proposed"));
            for key in ["owners", "decision-makers"] {
                before.as_object_mut().unwrap().remove(key);
                after.as_object_mut().unwrap().remove(key);
            }
            if pending {
                before.as_object_mut().unwrap().remove("description");
                after.as_object_mut().unwrap().remove("description");
            }
            if before != after {
                return Err("Editing cannot change identity, domain, status, date, applicability, relationships, or frozen description; use governed commands".into());
            }
            if let (Record::Adr(old), Record::Adr(new)) = (record, &changed) {
                if old.lifecycle_notes != new.lifecycle_notes {
                    return Err("Lifecycle notes require governed updates".into());
                }
            }
            let body_changed = authoring::state(record).1 != authoring::state(&changed).1;
            let reset = !pending && (body_changed || edit.register_external);
            if reset {
                if authoring::state(record).0 != "approved" {
                    return Err(
                        "Frozen bodies require a successor; only eligible approvals can reset"
                            .into(),
                    );
                }
                let actor = edit.actor.as_deref().filter(|s| !s.trim().is_empty());
                let rationale = edit.rationale.as_deref().filter(|s| !s.trim().is_empty());
                if !edit.confirm || actor.is_none() || rationale.is_none() {
                    return Err(
                        "Approval reset requires --confirm, nonempty --actor and --rationale"
                            .into(),
                    );
                }
                let prior = evidence::reset_approval(root, report, &edit.id)?;
                let mut event = mutation::event(
                    root,
                    report,
                    "edit",
                    Some((actor.unwrap(), rationale.unwrap())),
                    serde_json::json!({}),
                )?;
                let mut header = metadata(&changed)?;
                header["status"] = "proposed".into();
                if matches!(changed, Record::Adr(_)) {
                    header["date"] = event["at"].as_str().unwrap()[..10].into();
                }
                let mut header_text = yaml_serde::to_string(&header).map_err(|e| e.to_string())?;
                if matches!(changed, Record::Adr(_)) {
                    let date = header["date"].as_str().unwrap();
                    header_text = header_text
                        .replace(&format!("date: {date}\n"), &format!("date: \"{date}\"\n"));
                }
                let body = match &changed {
                    Record::Brr(r) => r.body.clone(),
                    Record::Adr(r) => format!(
                        "{}{}",
                        r.decision_body.as_deref().unwrap_or(""),
                        r.lifecycle_notes
                    ),
                };
                let text = format!("---\n{header_text}---\n{body}");
                let revised = authoring::parse(&text, path)?;
                let mut change = authoring::change(&revised, Some("approved"));
                change["approval_ref"] = prior.into();
                event["changes"] = serde_json::json!([change]);
                return Ok(Plan {
                    writes: vec![
                        (path.into(), text.into_bytes()),
                        (
                            ".badr/evidence.jsonl".into(),
                            authoring::append(expected, &event)?,
                        ),
                    ],
                    affected_ids: vec![edit.id.clone()],
                    message: format!(
                        "Reset {} to proposed for editing\n\nActor: {}\nRationale: {}\nBaseline: {}\nEvidence: {}",
                        edit.id,
                        actor.unwrap(),
                        rationale.unwrap(),
                        report.baseline_commit.as_deref().unwrap(),
                        event["event_id"].as_str().unwrap()
                    ),
                });
            }
            if edit.register_external {
                return Err("External edit registration requires an approved record with a known body mismatch".into());
            }
            let unchanged = supplied.as_bytes() == original;
            Ok(Plan {
                writes: if unchanged {
                    vec![]
                } else {
                    vec![(path.into(), supplied.as_bytes().to_vec())]
                },
                affected_ids: if unchanged {
                    vec![]
                } else {
                    vec![edit.id.clone()]
                },
                message: format!(
                    "Edit {} ordinary content\n\nBaseline: {}\nNo approval or lifecycle transition.",
                    edit.id,
                    report.baseline_commit.as_deref().unwrap()
                ),
            })
        },
    )
}
