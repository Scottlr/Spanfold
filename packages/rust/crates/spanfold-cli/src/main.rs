#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! Spanfold command-line entry point.

use clap::{Parser, Subcommand, ValueEnum};
use serde::Deserialize;
use spanfold::{
    AgainstSelection, Comparator, ComparisonFinality, ComparisonPlan, ContractFixture,
    EpisodeAnalysisDocument, OpenWindowPolicy, PrimitiveValue, TemporalPoint, WindowHistoryFixture,
    compare, compare_live, export_result_debug_html, export_result_json, export_result_llm_context,
    export_result_markdown, write_export_files_atomically, write_result_json_lines,
};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

mod workflow;
#[cfg(test)]
use workflow::select_field;
use workflow::{
    WindowAuditOptions, compare_imported_windows, compare_windows_jsonl, import_events,
    import_events_to_file, load_fixture, load_window_history_jsonl, write_audit_bundle,
};

/// Preview CLI for Spanfold temporal evidence workflows.
#[derive(Debug, Parser)]
#[command(name = "spanfold")]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

/// Spanfold CLI commands.
#[derive(Debug, Subcommand)]
enum Command {
    /// Validate a Spanfold fixture plan.
    ValidatePlan {
        /// Fixture JSON path.
        fixture: PathBuf,
    },
    /// Compare a Spanfold fixture.
    Compare {
        /// Fixture JSON path.
        fixture: PathBuf,
        /// Output format.
        #[arg(long, default_value = "json")]
        format: OutputFormat,
    },
    /// Explain a Spanfold fixture as Markdown.
    Explain {
        /// Fixture JSON path.
        fixture: PathBuf,
    },
    /// Write a full audit artifact bundle from a fixture.
    Audit {
        /// Fixture JSON path.
        fixture: PathBuf,
        /// Output directory.
        #[arg(long)]
        out: PathBuf,
    },
    /// Write an audit artifact bundle from flat window JSONL.
    AuditWindows {
        /// Window JSONL path.
        windows: PathBuf,
        /// Window name to use when rows omit `windowName`.
        #[arg(long)]
        window: Option<String>,
        /// Target source.
        #[arg(long)]
        target: String,
        /// Against source. May be repeated.
        #[arg(long)]
        against: Vec<String>,
        /// Comparison plan name.
        #[arg(long)]
        name: Option<String>,
        /// Comparator declaration. May be repeated.
        #[arg(long)]
        comparators: Vec<String>,
        /// Promote strict validation diagnostics.
        #[arg(long)]
        strict: bool,
        /// Include open windows by clipping them to this processing-position horizon.
        #[arg(long = "live-horizon-position")]
        live_horizon_position: Option<i64>,
        /// Output directory.
        #[arg(long)]
        out: PathBuf,
    },
    /// Convert event JSONL to flat Spanfold window JSONL.
    ImportEvents {
        /// Event JSONL path.
        events: PathBuf,
        /// Event import map JSON path.
        #[arg(long)]
        map: PathBuf,
        /// Output window JSONL path.
        #[arg(long)]
        out: PathBuf,
    },
    /// Import event JSONL and write a full audit artifact bundle.
    AuditEvents {
        /// Event JSONL path.
        events: PathBuf,
        /// Event import map JSON path.
        #[arg(long)]
        map: PathBuf,
        /// Target source.
        #[arg(long)]
        target: String,
        /// Against source. May be repeated.
        #[arg(long)]
        against: Vec<String>,
        /// Output directory.
        #[arg(long)]
        out: PathBuf,
    },
    /// Execute a portable Episode analysis document over flat window JSONL.
    Episodes {
        /// Episode analysis document path.
        plan: PathBuf,
        /// Window JSONL path.
        windows: PathBuf,
        /// Output format.
        #[arg(long, default_value = "json")]
        format: EpisodeOutputFormat,
    },
}

/// Supported comparison output formats.
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum OutputFormat {
    /// Deterministic JSON.
    Json,
    /// Deterministic Markdown.
    Markdown,
    /// Deterministic LLM context JSON.
    LlmContext,
}

/// Supported portable Episode output formats.
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum EpisodeOutputFormat {
    /// Deterministic JSON.
    Json,
    /// Deterministic Markdown.
    Markdown,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(code) => code,
        Err(error) => {
            eprintln!(
                "{{\"code\":\"{}\",\"error\":{}}}",
                error.kind,
                serde_json::to_string(&error.message).expect("valid json")
            );
            ExitCode::from(error.kind.exit_code())
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum CliErrorKind {
    Input,
    Io,
    Export,
}

impl CliErrorKind {
    const fn exit_code(self) -> u8 {
        match self {
            Self::Input => 2,
            Self::Io => 3,
            Self::Export => 4,
        }
    }
}

impl std::fmt::Display for CliErrorKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Input => "input",
            Self::Io => "io",
            Self::Export => "export",
        })
    }
}

#[derive(Debug)]
struct CliError {
    kind: CliErrorKind,
    message: String,
}

#[derive(Clone, Copy, Debug)]
enum ImportOperation {
    ImportEvents,
    AuditEvents,
}

impl ImportOperation {
    const fn label(self) -> &'static str {
        match self {
            Self::ImportEvents => "import-events",
            Self::AuditEvents => "audit-events",
        }
    }
}

impl CliError {
    fn input(error: impl std::fmt::Display) -> Self {
        Self {
            kind: CliErrorKind::Input,
            message: error.to_string(),
        }
    }

    fn io(error: impl std::fmt::Display) -> Self {
        Self {
            kind: CliErrorKind::Io,
            message: error.to_string(),
        }
    }

    fn export(error: impl std::fmt::Display) -> Self {
        Self {
            kind: CliErrorKind::Export,
            message: error.to_string(),
        }
    }

    fn relabel_operation(mut self, operation: ImportOperation) -> Self {
        if let Some(suffix) = self.message.strip_prefix("import-events") {
            self.message = format!("{}{suffix}", operation.label());
        }
        self
    }
}

impl From<String> for CliError {
    fn from(message: String) -> Self {
        Self {
            kind: CliErrorKind::Input,
            message,
        }
    }
}

fn run(cli: Cli) -> Result<ExitCode, CliError> {
    match cli.command {
        Command::ValidatePlan { fixture } => {
            let fixture = load_fixture(&fixture)?;
            let result = fixture
                .execute_checked()
                .map_err(|error| error.to_string())?;
            let payload = serde_json::json!({
                "isValid": result.is_valid,
                "diagnostics": result.diagnostics.into_iter().map(|item| item.code).collect::<Vec<_>>(),
            });
            println!(
                "{}",
                serde_json::to_string(&payload).map_err(|error| error.to_string())?
            );
            Ok(if result.is_valid {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            })
        }
        Command::Compare { fixture, format } => {
            let fixture = load_fixture(&fixture)?;
            let result = fixture
                .execute_checked()
                .map_err(|error| error.to_string())?;
            let format = match format {
                OutputFormat::Json => export_result_json(&result).map_err(CliError::export)?,
                OutputFormat::Markdown => export_result_markdown(&result),
                OutputFormat::LlmContext => {
                    export_result_llm_context(&result).map_err(CliError::export)?
                }
            };
            println!("{format}");
            Ok(if result.is_valid {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            })
        }
        Command::Explain { fixture } => {
            let fixture = load_fixture(&fixture)?;
            let result = fixture
                .execute_checked()
                .map_err(|error| error.to_string())?;
            println!("{}", export_result_markdown(&result));
            Ok(if result.is_valid {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            })
        }
        Command::Audit { fixture, out } => {
            let fixture = load_fixture(&fixture)?;
            let result = fixture
                .execute_checked()
                .map_err(|error| error.to_string())?;
            write_audit_bundle(&result, &out)?;
            Ok(if result.is_valid {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            })
        }
        Command::AuditWindows {
            windows,
            window,
            target,
            against,
            name,
            comparators,
            strict,
            live_horizon_position,
            out,
        } => {
            let options = WindowAuditOptions {
                default_window_name: window.as_deref(),
                target: &target,
                against: &against,
                name: name.as_deref().unwrap_or("Spanfold Window Audit"),
                comparators: &comparators,
                strict,
                live_horizon_position,
            };
            let result = compare_windows_jsonl(&windows, options)?;
            write_audit_bundle(&result, &out)?;
            Ok(if result.is_valid {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            })
        }
        Command::ImportEvents { events, map, out } => {
            import_events_to_file(&events, &map, &out)?;
            Ok(ExitCode::SUCCESS)
        }
        Command::AuditEvents {
            events,
            map,
            target,
            against,
            out,
        } => {
            let windows = import_events(&events, &map)?;
            let result = compare_imported_windows(&windows, &target, &against)?;
            write_audit_bundle(&result, &out)?;
            Ok(if result.is_valid {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            })
        }
        Command::Episodes {
            plan,
            windows,
            format,
        } => {
            let json = fs::read_to_string(&plan).map_err(CliError::io)?;
            let document =
                EpisodeAnalysisDocument::parse_json(&json).map_err(|error| error.to_string())?;
            let history = load_window_history_jsonl(&windows, Some(document.window_name()))?;
            let result = document
                .execute(&history)
                .map_err(|error| error.to_string())?;
            let output = match format {
                EpisodeOutputFormat::Json => {
                    result.export_json().map_err(|error| error.to_string())?
                }
                EpisodeOutputFormat::Markdown => result.export_markdown(),
            };
            println!("{output}");
            Ok(ExitCode::SUCCESS)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn audit_windows_supports_basic_jsonl_windows() {
        let mut file = NamedTempFile::new().expect("temp file");
        writeln!(
            file,
            "{{\"key\":\"device-1\",\"source\":\"provider-a\",\"startPosition\":1,\"endPosition\":5}}"
        )
        .expect("write first row");
        writeln!(
            file,
            "{{\"key\":\"device-1\",\"source\":\"provider-b\",\"startPosition\":3,\"endPosition\":7}}"
        )
        .expect("write second row");

        let result = compare_windows_jsonl(
            file.path(),
            WindowAuditOptions {
                default_window_name: Some("DeviceOffline"),
                target: "provider-a",
                against: &[String::from("provider-b")],
                name: "Spanfold Window Audit",
                comparators: &[],
                strict: false,
                live_horizon_position: None,
            },
        )
        .expect("jsonl compare");

        assert!(result.is_valid);
        assert_eq!(result.overlap_rows.len(), 1);
        assert_eq!(result.residual_rows.len(), 1);
        assert_eq!(result.coverage_rows.len(), 2);
        assert_eq!(result.missing_rows.len(), 0);
        assert_eq!(result.gap_rows.len(), 0);
        assert_eq!(result.symmetric_difference_rows.len(), 0);
    }

    #[test]
    fn audit_windows_supports_custom_comparators_name_and_live_horizon() {
        let mut file = NamedTempFile::new().expect("temp file");
        writeln!(
            file,
            "{{\"key\":\"device-1\",\"source\":\"provider-a\",\"startPosition\":1}}"
        )
        .expect("write first row");
        writeln!(
            file,
            "{{\"key\":\"device-1\",\"source\":\"provider-b\",\"startPosition\":3,\"endPosition\":7}}"
        )
        .expect("write second row");
        let against = vec![String::from("provider-b")];
        let comparators = vec![String::from("residual")];

        let result = compare_windows_jsonl(
            file.path(),
            WindowAuditOptions {
                default_window_name: Some("DeviceOffline"),
                target: "provider-a",
                against: &against,
                name: "Live audit",
                comparators: &comparators,
                strict: true,
                live_horizon_position: Some(10),
            },
        )
        .expect("jsonl compare");

        assert!(result.is_valid);
        assert_eq!(result.plan_name, "Live audit");
        assert_eq!(result.comparator_summaries.len(), 1);
        assert_eq!(result.comparator_summaries[0].comparator_name, "residual");
        assert_eq!(result.residual_rows.len(), 2);
        assert!(result.has_provisional_rows());
    }

    #[test]
    fn field_selection_supports_embedded_array_indexes_and_json_pointers() {
        let event = serde_json::json!({
            "items": [{"name": "first"}],
            "a/b": {"~key": 7}
        });
        let dotted =
            select_field(&event, "items[0].name", "events.jsonl", 1).expect("embedded array path");
        assert_eq!(dotted, &serde_json::Value::String("first".to_owned()));
        let pointer =
            select_field(&event, "/a~1b/~0key", "events.jsonl", 1).expect("escaped JSON pointer");
        assert_eq!(pointer, &serde_json::json!(7));
    }
}
