use super::mutation::{Plan, bytes};
use super::*;
use std::{fs::File, io::Read, process::Stdio};

#[derive(clap::Args)]
pub(crate) struct Edit {
    id: String,
    #[arg(long, required_unless_present = "editor", conflicts_with = "editor")]
    content: Option<PathBuf>,
    /// Absolute editor executable outside the target repository; no shell is used.
    #[arg(long)]
    editor: Option<PathBuf>,
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
    mutation::run(target, "edit", |root, report, expected| {
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
        let supplied = if let Some(text) = supplied {
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
                    return Err("Editor cancelled; managed content and evidence unchanged".into());
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
        match (record, &changed) {
            (Record::Brr(old), Record::Brr(new)) if !pending && old.body != new.body => {
                return Err(
                    "Frozen bodies require a successor; approved-body reset is not available"
                        .into(),
                );
            }
            (Record::Adr(old), Record::Adr(new)) => {
                if old.lifecycle_notes != new.lifecycle_notes {
                    return Err("Lifecycle notes require governed updates".into());
                }
                if !pending && old.decision_body != new.decision_body {
                    return Err(
                        "Frozen bodies require a successor; approved-body reset is not available"
                            .into(),
                    );
                }
            }
            _ => {}
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
    })
}
