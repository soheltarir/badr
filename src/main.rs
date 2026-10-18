mod adr;
mod repository;

use clap::{Parser, Subcommand};
use repository::Report;
use std::{
    io::{self, Write},
    path::PathBuf,
    process::ExitCode,
};

#[derive(Parser)]
#[command(
    version,
    about = "Manage local business rules and architecture decisions"
)]
struct Cli {
    #[arg(long, global = true, default_value = ".")]
    repo: PathBuf,
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Operation,
}

#[derive(Subcommand)]
enum Operation {
    /// Initialize configuration; the baseline branch must be chosen explicitly.
    Init {
        #[arg(long)]
        baseline: String,
    },
    /// Register BRR domains through explicit governed writes.
    Domain {
        #[command(subcommand)]
        command: repository::domain::Command,
    },
    /// Create a draft BRR or proposed ADR without prompts.
    New {
        #[command(subcommand)]
        command: repository::authoring::Command,
    },
    /// Edit ordinary content without granting approval.
    Edit(repository::editing::Edit),
    /// Submit a structurally valid draft BRR for review.
    Propose(repository::authoring::Proposal),
    /// Approve after human authorization outside BADR; records a declaration, not authentication.
    Decide(repository::authoring::Approval),
    /// Inspect managed records with explicit applicability selection.
    List {
        #[command(flatten)]
        filter: repository::Filter,
    },
    /// Show a managed record and its exact decision or Gherkin body.
    Show { id: String },
    /// Validate with explicit coverage reporting.
    Validate,
}

fn main() -> ExitCode {
    let args: Vec<_> = std::env::args_os().collect();
    let json = args.iter().any(|arg| arg == "--json");
    let cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(error)
            if matches!(
                error.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            ) =>
        {
            return if error.print().is_ok() {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(4)
            };
        }
        Err(error) => {
            return emit(
                Report::invalid("arguments", "invalid_arguments", error.to_string()),
                json,
            );
        }
    };
    let report = match cli.command {
        Operation::Edit(edit) => repository::editing::run(&cli.repo, edit),
        Operation::Decide(approval) => repository::authoring::decide(&cli.repo, approval),
        Operation::Propose(proposal) => repository::authoring::propose(&cli.repo, proposal),
        Operation::New { command } => repository::authoring::new(&cli.repo, command),
        Operation::Domain { command } => repository::domain::run(&cli.repo, command),
        Operation::Init { baseline } => repository::init(&cli.repo, &baseline),
        Operation::List { filter } => repository::select(&cli.repo, "list", &filter, None),
        Operation::Show { id } => {
            repository::select(&cli.repo, "show", &repository::Filter::default(), Some(&id))
        }
        Operation::Validate => repository::inspect(&cli.repo, "validate"),
    };
    emit(report, cli.json)
}

fn emit(report: Report, json: bool) -> ExitCode {
    let output = if json {
        serde_json::to_string(&report).expect("report contains only JSON-safe values") + "\n"
    } else {
        for diagnostic in &report.diagnostics {
            let location = diagnostic.path.as_deref().unwrap_or("repository");
            let lines = match (diagnostic.stored_file_line, diagnostic.body_line) {
                (Some(file), Some(body)) => format!(" (stored-file line {file}, body line {body})"),
                (Some(file), None) => format!(" (stored-file line {file})"),
                _ => String::new(),
            };
            eprintln!(
                "{}: {location}{lines}: {}",
                diagnostic.code, diagnostic.message
            );
        }
        let mut output = format!("{}: {}\n", report.operation, report.result);
        if matches!(
            report.operation.as_str(),
            "domain_add" | "new_brr" | "new_adr" | "propose" | "edit" | "decide"
        ) {
            output.push_str(&format!(
                "Completed writes: {:?}\nNot completed: {:?}\nPending audit commit: {:?}\n",
                report.partial_write.completed,
                report.partial_write.not_completed,
                report.pending_commit_durability
            ));
            if report.operation == "edit" {
                output.push_str(&format!(
                    "Changed IDs: {:?}\nValidation coverage: configuration={}, collection={}, evidence={}, history={}\n",
                    report.affected_ids,
                    report.validation_coverage.configuration,
                    report.validation_coverage.collection,
                    report.validation_coverage.evidence,
                    report.validation_coverage.history,
                ));
            }
            if let Some(message) = &report.proposed_audit_commit_message {
                output.push_str(&format!("Proposed audit commit message:\n{message}\n"));
            }
        }
        for record in &report.records {
            match record {
                repository::Record::Brr(record) => {
                    output.push_str(&format!(
                        "{} {} {} [{}] {}\n",
                        record.metadata.id,
                        record.metadata.domain,
                        record.metadata.status,
                        record.selection,
                        record.path
                    ));
                    if report.operation == "show" {
                        output.push_str(&record.body);
                    }
                }
                repository::Record::Adr(record) => {
                    output.push_str(&format!(
                        "{} adr {} {}{}\n",
                        record.metadata.id,
                        record.metadata.status,
                        record.path,
                        if record.directs_new_work {
                            " (directs new work; existing-code conformance not assessed)"
                        } else {
                            ""
                        }
                    ));
                    if let Some(body) = &record.decision_body {
                        output.push_str(body);
                    }
                }
            }
        }
        output
    };
    if io::stdout().lock().write_all(output.as_bytes()).is_err() {
        return ExitCode::from(4);
    }
    ExitCode::from(report.exit_code)
}
