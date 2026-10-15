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
    /// Inspect the managed collection (empty collections supported in this milestone).
    List,
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
        Operation::Init { baseline } => repository::init(&cli.repo, &baseline),
        Operation::List => repository::inspect(&cli.repo, "list"),
        Operation::Validate => repository::inspect(&cli.repo, "validate"),
    };
    emit(report, cli.json)
}

fn emit(report: Report, json: bool) -> ExitCode {
    let output = if json {
        serde_json::to_string(&report).expect("report contains only JSON-safe values") + "\n"
    } else {
        for diagnostic in &report.diagnostics {
            eprintln!("{}: {}", diagnostic.code, diagnostic.message);
        }
        format!("{}: {}\n", report.operation, report.result)
    };
    if io::stdout().lock().write_all(output.as_bytes()).is_err() {
        return ExitCode::from(4);
    }
    ExitCode::from(report.exit_code)
}
