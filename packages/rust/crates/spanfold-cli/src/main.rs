#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! Spanfold command-line entry point.

use clap::{Parser, Subcommand, ValueEnum};
use serde::Deserialize;
use spanfold::{
    AgainstSelection, Comparator, ComparisonDuplicateWindowPolicy, ComparisonFinality,
    ComparisonPlan, ContractFixture, OpenWindowPolicy, PrimitiveValue, TemporalPoint,
    WindowHistoryFixture, compare, compare_live, export_result_debug_html, export_result_json,
    export_result_llm_context, export_result_markdown, write_result_json_lines,
};
use std::{
    collections::BTreeMap,
    fs,
    io::{BufRead, BufReader, Write},
    process::ExitCode,
};

/// Production high-throughput CLI for Spanfold temporal evidence workflows.
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
        fixture: String,
    },
    /// Compare a Spanfold fixture.
    Compare {
        /// Fixture JSON path.
        fixture: String,
        /// Output format.
        #[arg(long, default_value = "json")]
        format: OutputFormat,
    },
    /// Explain a Spanfold fixture as Markdown.
    Explain {
        /// Fixture JSON path.
        fixture: String,
    },
    /// Write a full audit artifact bundle from a fixture.
    Audit {
        /// Fixture JSON path.
        fixture: String,
        /// Output directory.
        #[arg(long)]
        out: String,
    },
    /// Write an audit artifact bundle from flat window JSONL.
    AuditWindows {
        /// Window JSONL path.
        windows: String,
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
        out: String,
    },
    /// Convert event JSONL to flat Spanfold window JSONL.
    ImportEvents {
        /// Event JSONL path.
        events: String,
        /// Event import map JSON path.
        #[arg(long)]
        map: String,
        /// Output window JSONL path.
        #[arg(long)]
        out: String,
    },
    /// Import event JSONL and write a full audit artifact bundle.
    AuditEvents {
        /// Event JSONL path.
        events: String,
        /// Event import map JSON path.
        #[arg(long)]
        map: String,
        /// Target source.
        #[arg(long)]
        target: String,
        /// Against source. May be repeated.
        #[arg(long)]
        against: Vec<String>,
        /// Output directory.
        #[arg(long)]
        out: String,
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

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(code) => code,
        Err(message) => {
            eprintln!(
                "{{\"error\":{}}}",
                serde_json::to_string(&message).expect("valid json")
            );
            ExitCode::from(2)
        }
    }
}

fn run(cli: Cli) -> Result<ExitCode, String> {
    match cli.command {
        Command::ValidatePlan { fixture } => {
            let fixture = load_fixture(&fixture)?;
            let result = fixture.execute();
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
            let result = fixture.execute();
            let format = match format {
                OutputFormat::Json => {
                    export_result_json(&result).map_err(|error| error.to_string())?
                }
                OutputFormat::Markdown => export_result_markdown(&result),
                OutputFormat::LlmContext => {
                    export_result_llm_context(&result).map_err(|error| error.to_string())?
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
            println!("{}", export_result_markdown(&fixture.execute()));
            Ok(ExitCode::SUCCESS)
        }
        Command::Audit { fixture, out } => {
            let fixture = load_fixture(&fixture)?;
            let result = fixture.execute();
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
            let windows = import_events(&events, &map)?;
            write_windows_jsonl(&windows, &out)?;
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
    }
}

fn load_fixture(path: &str) -> Result<ContractFixture, String> {
    let json = fs::read_to_string(path).map_err(|error| error.to_string())?;
    ContractFixture::parse_json(&json).map_err(|error| error.to_string())
}

fn write_audit_bundle(result: &spanfold::ComparisonResult, out: &str) -> Result<(), String> {
    fs::create_dir_all(out).map_err(|error| error.to_string())?;
    let json = export_result_json(result).map_err(|error| error.to_string())?;
    let markdown = export_result_markdown(result);
    let llm = export_result_llm_context(result).map_err(|error| error.to_string())?;
    let html = export_result_debug_html(result);
    fs::write(format!("{out}/comparison.json"), &json).map_err(|error| error.to_string())?;
    fs::write(format!("{out}/comparison.md"), &markdown).map_err(|error| error.to_string())?;
    fs::write(format!("{out}/comparison.llm.json"), &llm).map_err(|error| error.to_string())?;
    fs::write(format!("{out}/comparison.html"), html).map_err(|error| error.to_string())?;
    let jsonl = fs::File::create(format!("{out}/comparison.rows.jsonl"))
        .map_err(|error| error.to_string())?;
    write_result_json_lines(result, jsonl).map_err(|error| error.to_string())?;
    let manifest = serde_json::json!({
        "schema": "spanfold.audit.bundle",
        "schemaVersion": 0,
        "artifact": "audit-bundle",
        "planName": result.plan_name,
        "isValid": result.is_valid,
        "diagnosticCount": result.diagnostics.len(),
        "provisionalRowCount": result.row_finalities.iter().filter(|item| item.finality == ComparisonFinality::Provisional).count(),
        "rowCounts": row_counts_json(result),
        "artifacts": {
            "json": "comparison.json",
            "jsonLines": "comparison.rows.jsonl",
            "markdown": "comparison.md",
            "debugHtml": "comparison.html",
            "llmContext": "comparison.llm.json",
            "manifest": "manifest.json"
        }
    });
    let manifest = serde_json::to_string_pretty(&manifest).map_err(|error| error.to_string())?;
    fs::write(format!("{out}/manifest.json"), &manifest).map_err(|error| error.to_string())?;
    println!("{manifest}");
    Ok(())
}

fn row_counts_json(result: &spanfold::ComparisonResult) -> serde_json::Value {
    serde_json::json!({
        "overlap": result.overlap_rows.len(),
        "residual": result.residual_rows.len(),
        "missing": result.missing_rows.len(),
        "coverage": result.coverage_rows.len(),
        "gap": result.gap_rows.len(),
        "symmetricDifference": result.symmetric_difference_rows.len(),
        "containment": result.containment_rows.len(),
        "leadLag": result.lead_lag_rows.len(),
        "asOf": result.as_of_rows.len()
    })
}

struct WindowAuditOptions<'a> {
    default_window_name: Option<&'a str>,
    target: &'a str,
    against: &'a [String],
    name: &'a str,
    comparators: &'a [String],
    strict: bool,
    live_horizon_position: Option<i64>,
}

fn compare_windows_jsonl(
    path: &str,
    options: WindowAuditOptions<'_>,
) -> Result<spanfold::ComparisonResult, String> {
    if options.against.is_empty() {
        return Err("audit-windows requires at least one --against value".to_owned());
    }
    let comparators = parse_comparators(options.comparators)?;
    let lines = fs::read_to_string(path).map_err(|error| error.to_string())?;
    let mut builder = WindowHistoryFixture::new();
    for (index, line) in lines.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let row: JsonlWindow =
            serde_json::from_str(line).map_err(|error| format!("{path}:{}: {error}", index + 1))?;
        let window_name = row
            .window_name
            .as_deref()
            .or(options.default_window_name)
            .ok_or_else(|| {
                format!(
                    "{path}:{}: windowName missing and --window not supplied",
                    index + 1
                )
            })?;
        let key = row.key.clone();
        let resolved_window_name = window_name.to_owned();
        if let Some(end) = row.end_position {
            builder = builder
                .closed_window(
                    resolved_window_name.clone(),
                    key.clone(),
                    row.start_position,
                    end,
                    |w| apply_jsonl_metadata(w, &row),
                )
                .map_err(|error| error.to_string())?;
        } else {
            builder = builder.open_window(resolved_window_name, key, row.start_position, |w| {
                apply_jsonl_metadata(w, &row)
            });
        }
    }
    let history = builder.build();
    let plan = ComparisonPlan {
        name: options.name.to_owned(),
        target_source: options.target.to_owned(),
        against: AgainstSelection::Sources(options.against.to_vec()),
        target_selector: None,
        against_selectors: Vec::new(),
        scope_window: options.default_window_name.map(str::to_owned),
        scope_key: None,
        scope_partition: None,
        scope_segments: Vec::new(),
        scope_tags: Vec::new(),
        comparators,
        require_closed_windows: options.live_horizon_position.is_none(),
        use_half_open_ranges: true,
        time_axis: spanfold::TemporalAxis::ProcessingPosition,
        null_timestamp_policy: spanfold::ComparisonNullTimestampPolicy::Reject,
        known_at: None,
        open_window_policy: if options.live_horizon_position.is_some() {
            OpenWindowPolicy::ClipToHorizon
        } else {
            OpenWindowPolicy::RequireClosed
        },
        open_window_horizon: options.live_horizon_position.map(TemporalPoint::position),
        coalesce_adjacent_windows: false,
        duplicate_window_policy: ComparisonDuplicateWindowPolicy::Preserve,
        output: spanfold::ComparisonOutputOptions::default_options(),
        strict: options.strict,
    };
    Ok(if let Some(horizon) = options.live_horizon_position {
        compare_live(&history, &plan, TemporalPoint::position(horizon))
    } else {
        compare(&history, &plan)
    })
}

fn parse_comparators(values: &[String]) -> Result<Vec<Comparator>, String> {
    let declarations = if values.is_empty() {
        vec![
            "overlap".to_owned(),
            "residual".to_owned(),
            "coverage".to_owned(),
        ]
    } else {
        values.to_vec()
    };
    declarations
        .into_iter()
        .map(|value| {
            Comparator::parse(&value).ok_or_else(|| format!("unknown comparator: {value}"))
        })
        .collect()
}

fn import_events(path: &str, map_path: &str) -> Result<Vec<ImportedWindow>, String> {
    let import_map = read_import_map(map_path)?;
    let input = import_map.input.as_deref().unwrap_or("jsonl");
    match input {
        "jsonl" => import_events_jsonl(path, &import_map),
        "csv" => import_events_csv(path, &import_map),
        _ => Err(format!("unsupported event input format: {input}")),
    }
}

fn import_events_jsonl(
    path: &str,
    import_map: &EventImportMap,
) -> Result<Vec<ImportedWindow>, String> {
    let file = fs::File::open(path).map_err(|error| error.to_string())?;
    let reader = BufReader::new(file);
    let mut active: BTreeMap<ImportStateKey, ImportState> = BTreeMap::new();
    let mut windows = Vec::new();
    let mut last_position: Option<i64> = None;

    for (index, line) in reader.lines().enumerate() {
        let line = line.map_err(|error| error.to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        let event: serde_json::Value = serde_json::from_str(&line)
            .map_err(|error| format!("{path}:{}: {error}", index + 1))?;
        process_import_event(
            &event,
            import_map,
            path,
            index + 1,
            &mut active,
            &mut windows,
            &mut last_position,
        )?;
    }

    close_remaining_imported_windows(active, &mut windows);
    Ok(windows)
}

fn import_events_csv(
    path: &str,
    import_map: &EventImportMap,
) -> Result<Vec<ImportedWindow>, String> {
    let file = fs::File::open(path).map_err(|error| error.to_string())?;
    let mut lines = BufReader::new(file).lines();
    let Some(header_line) = lines.next() else {
        return Err(format!("{path}: CSV input is empty"));
    };
    let header_line = header_line.map_err(|error| error.to_string())?;
    let headers = parse_csv_record(&header_line).map_err(|error| format!("{path}:1: {error}"))?;
    if headers.is_empty() {
        return Err(format!(
            "{path}:1: CSV header must contain at least one column"
        ));
    }

    let mut active: BTreeMap<ImportStateKey, ImportState> = BTreeMap::new();
    let mut windows = Vec::new();
    let mut last_position: Option<i64> = None;

    for (index, line) in lines.enumerate() {
        let line_number = index + 2;
        let line = line.map_err(|error| error.to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        let fields =
            parse_csv_record(&line).map_err(|error| format!("{path}:{line_number}: {error}"))?;
        if fields.len() != headers.len() {
            return Err(format!(
                "{path}:{line_number}: CSV row has {} fields, expected {}",
                fields.len(),
                headers.len()
            ));
        }
        let mut event = serde_json::Map::new();
        for (header, field) in headers.iter().zip(fields) {
            event.insert(header.clone(), csv_field_to_json(&field));
        }
        process_import_event(
            &serde_json::Value::Object(event),
            import_map,
            path,
            line_number,
            &mut active,
            &mut windows,
            &mut last_position,
        )?;
    }

    close_remaining_imported_windows(active, &mut windows);
    Ok(windows)
}

fn process_import_event(
    event: &serde_json::Value,
    import_map: &EventImportMap,
    path: &str,
    line_number: usize,
    active: &mut BTreeMap<ImportStateKey, ImportState>,
    windows: &mut Vec<ImportedWindow>,
    last_position: &mut Option<i64>,
) -> Result<(), String> {
    let position = select_i64(event, &import_map.position, path, line_number)?;
    if last_position.is_some_and(|last| position < last) {
        return Err(format!(
            "{path}:{line_number}: event position cannot move backwards"
        ));
    }
    *last_position = Some(position);
    let source = select_string(event, &import_map.source, path, line_number)?;
    let partition = import_map
        .partition
        .as_ref()
        .map(|selector| select_string(event, selector, path, line_number))
        .transpose()?;

    for window in &import_map.windows {
        let key_selector = window
            .key
            .as_ref()
            .or(import_map.key.as_ref())
            .ok_or_else(|| "$.windows[].key or $.key is required".to_owned())?;
        let key = select_string(event, key_selector, path, line_number)?;
        let state_key = ImportStateKey {
            window_name: window.name.clone(),
            key: key.clone(),
            source: source.clone(),
            partition: partition.clone(),
        };
        let segments = select_named_values(event, &window.segments, path, line_number)?;
        let tags = select_named_values(event, &window.tags, path, line_number)?;
        let is_active = evaluate_predicate(event, &window.active, path, line_number)?;

        if is_active {
            if let Some(state) = active.get_mut(&state_key) {
                if state.segments != segments {
                    windows.push(state.to_window_for_key(&state_key, Some(position)));
                    *state = ImportState {
                        start_position: position,
                        segments,
                        tags,
                    };
                }
                continue;
            }
            active.insert(
                state_key,
                ImportState {
                    start_position: position,
                    segments,
                    tags,
                },
            );
            continue;
        }

        if let Some(state) = active.remove(&state_key) {
            windows.push(state.to_window_for_key(&state_key, Some(position)));
        }
    }
    Ok(())
}

fn close_remaining_imported_windows(
    active: BTreeMap<ImportStateKey, ImportState>,
    windows: &mut Vec<ImportedWindow>,
) {
    for (state_key, state) in active {
        windows.push(state.to_window_for_key(&state_key, None));
    }
}

fn compare_imported_windows(
    windows: &[ImportedWindow],
    target: &str,
    against: &[String],
) -> Result<spanfold::ComparisonResult, String> {
    if against.is_empty() {
        return Err("audit-events requires at least one --against value".to_owned());
    }

    let mut builder = WindowHistoryFixture::new();
    for window in windows {
        if let Some(end) = window.end_position {
            builder = builder
                .closed_window(
                    window.window_name.clone(),
                    window.key.clone(),
                    window.start_position,
                    end,
                    |metadata| apply_imported_metadata(metadata, window),
                )
                .map_err(|error| error.to_string())?;
        } else {
            builder = builder.open_window(
                window.window_name.clone(),
                window.key.clone(),
                window.start_position,
                |metadata| apply_imported_metadata(metadata, window),
            );
        }
    }

    let history = builder.build();
    let plan = ComparisonPlan {
        name: "Spanfold Event Audit".to_owned(),
        target_source: target.to_owned(),
        against: AgainstSelection::Sources(against.to_vec()),
        target_selector: None,
        against_selectors: Vec::new(),
        scope_window: None,
        scope_key: None,
        scope_partition: None,
        scope_segments: Vec::new(),
        scope_tags: Vec::new(),
        comparators: vec![
            Comparator::Overlap,
            Comparator::Residual,
            Comparator::Missing,
            Comparator::Coverage,
            Comparator::Gap,
            Comparator::SymmetricDifference,
        ],
        require_closed_windows: true,
        use_half_open_ranges: true,
        time_axis: spanfold::TemporalAxis::ProcessingPosition,
        null_timestamp_policy: spanfold::ComparisonNullTimestampPolicy::Reject,
        known_at: None,
        open_window_policy: OpenWindowPolicy::RequireClosed,
        open_window_horizon: None,
        coalesce_adjacent_windows: false,
        duplicate_window_policy: ComparisonDuplicateWindowPolicy::Preserve,
        output: spanfold::ComparisonOutputOptions::default_options(),
        strict: false,
    };
    Ok(compare(&history, &plan))
}

fn write_windows_jsonl(windows: &[ImportedWindow], path: &str) -> Result<(), String> {
    let mut file = fs::File::create(path).map_err(|error| error.to_string())?;
    for window in windows {
        let line = serde_json::to_string(window).map_err(|error| error.to_string())?;
        writeln!(file, "{line}").map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn read_import_map(path: &str) -> Result<EventImportMap, String> {
    let json = fs::read_to_string(path).map_err(|error| error.to_string())?;
    let import_map: EventImportMap =
        serde_json::from_str(&json).map_err(|error| format!("{path}: {error}"))?;
    if import_map.windows.is_empty() {
        return Err(format!(
            "{path}: $.windows must contain at least one window"
        ));
    }
    Ok(import_map)
}

fn apply_jsonl_metadata(
    mut builder: spanfold::WindowHistoryFixtureWindow,
    row: &JsonlWindow,
) -> spanfold::WindowHistoryFixtureWindow {
    builder = builder.source(row.source.clone());
    if let Some(partition) = &row.partition {
        builder = builder.partition(partition.clone());
    }
    for segment in &row.segments {
        builder = if let Some(parent) = &segment.parent_name {
            builder.child_segment(segment.name.clone(), segment.value.clone(), parent.clone())
        } else {
            builder.segment(segment.name.clone(), segment.value.clone())
        };
    }
    for tag in &row.tags {
        builder = builder.tag(tag.name.clone(), tag.value.clone());
    }
    builder
}

fn apply_imported_metadata(
    mut builder: spanfold::WindowHistoryFixtureWindow,
    window: &ImportedWindow,
) -> spanfold::WindowHistoryFixtureWindow {
    builder = builder.source(window.source.clone());
    if let Some(partition) = &window.partition {
        builder = builder.partition(partition.clone());
    }
    for segment in &window.segments {
        builder = if let Some(parent) = &segment.parent_name {
            builder.child_segment(segment.name.clone(), segment.value.clone(), parent.clone())
        } else {
            builder.segment(segment.name.clone(), segment.value.clone())
        };
    }
    for tag in &window.tags {
        builder = builder.tag(tag.name.clone(), tag.value.clone());
    }
    builder
}

fn select_named_values(
    event: &serde_json::Value,
    selectors: &[NamedFieldSelector],
    path: &str,
    line_number: usize,
) -> Result<Vec<JsonlNamedValue>, String> {
    selectors
        .iter()
        .map(|selector| {
            let value = select_value(event, &selector.selector, path, line_number)?;
            Ok(JsonlNamedValue {
                name: selector.name.clone(),
                value: primitive_from_json(value).map_err(|error| {
                    format!(
                        "{path}:{line_number}: {} selector '{}' {error}",
                        selector.kind, selector.name
                    )
                })?,
                parent_name: selector.parent_name.clone(),
            })
        })
        .collect()
}

fn select_i64(
    event: &serde_json::Value,
    selector: &FieldSelector,
    path: &str,
    line_number: usize,
) -> Result<i64, String> {
    let value = select_value(event, selector, path, line_number)?;
    value.as_i64().ok_or_else(|| {
        format!(
            "{path}:{line_number}: field '{}' must be an integer",
            selector.field()
        )
    })
}

fn select_string(
    event: &serde_json::Value,
    selector: &FieldSelector,
    path: &str,
    line_number: usize,
) -> Result<String, String> {
    let value = select_value(event, selector, path, line_number)?;
    primitive_to_string(value).ok_or_else(|| {
        format!(
            "{path}:{line_number}: field '{}' must be a scalar stringable value",
            selector.field()
        )
    })
}

fn select_value<'a>(
    event: &'a serde_json::Value,
    selector: &FieldSelector,
    path: &str,
    line_number: usize,
) -> Result<&'a serde_json::Value, String> {
    select_field(event, selector.field(), path, line_number)
}

fn select_field<'a>(
    event: &'a serde_json::Value,
    field_path: &str,
    path: &str,
    line_number: usize,
) -> Result<&'a serde_json::Value, String> {
    let mut current = event;
    for field in field_path.split('.') {
        let Some(next) = current.get(field) else {
            return Err(format!(
                "{path}:{line_number}: missing event field '{field_path}'"
            ));
        };
        current = next;
    }
    Ok(current)
}

fn evaluate_predicate(
    event: &serde_json::Value,
    predicate: &EventPredicate,
    path: &str,
    line_number: usize,
) -> Result<bool, String> {
    let value = select_field(event, &predicate.field, path, line_number)?;
    let primitive = primitive_from_json(value).map_err(|error| {
        format!(
            "{path}:{line_number}: predicate field '{}' {error}",
            predicate.field
        )
    })?;

    let mut evaluated = false;
    let mut matches = true;
    if let Some(expected) = &predicate.equals {
        evaluated = true;
        matches &= primitive == *expected;
    }
    if let Some(expected) = &predicate.not_equals {
        evaluated = true;
        matches &= primitive != *expected;
    }
    if let Some(expected) = &predicate.greater_than {
        evaluated = true;
        matches &= compare_numbers(&primitive, expected, |left, right| left > right)
            .ok_or_else(|| numeric_predicate_error(path, line_number, predicate))?;
    }
    if let Some(expected) = &predicate.greater_than_or_equal {
        evaluated = true;
        matches &= compare_numbers(&primitive, expected, |left, right| left >= right)
            .ok_or_else(|| numeric_predicate_error(path, line_number, predicate))?;
    }
    if let Some(expected) = &predicate.less_than {
        evaluated = true;
        matches &= compare_numbers(&primitive, expected, |left, right| left < right)
            .ok_or_else(|| numeric_predicate_error(path, line_number, predicate))?;
    }
    if let Some(expected) = &predicate.less_than_or_equal {
        evaluated = true;
        matches &= compare_numbers(&primitive, expected, |left, right| left <= right)
            .ok_or_else(|| numeric_predicate_error(path, line_number, predicate))?;
    }
    if predicate.is_true.unwrap_or(false) {
        evaluated = true;
        matches &= primitive == PrimitiveValue::Bool(true);
    }
    if predicate.is_false.unwrap_or(false) {
        evaluated = true;
        matches &= primitive == PrimitiveValue::Bool(false);
    }
    if !evaluated {
        return Err(format!(
            "{path}:{line_number}: predicate for field '{}' has no condition",
            predicate.field
        ));
    }
    Ok(matches)
}

fn numeric_predicate_error(path: &str, line_number: usize, predicate: &EventPredicate) -> String {
    format!(
        "{path}:{line_number}: predicate field '{}' and threshold must be numeric",
        predicate.field
    )
}

fn compare_numbers(
    left: &PrimitiveValue,
    right: &PrimitiveValue,
    compare: impl FnOnce(f64, f64) -> bool,
) -> Option<bool> {
    Some(compare(primitive_to_f64(left)?, primitive_to_f64(right)?))
}

fn primitive_to_f64(value: &PrimitiveValue) -> Option<f64> {
    match value {
        PrimitiveValue::Integer(value) => Some(*value as f64),
        PrimitiveValue::Float(value) => Some(*value),
        PrimitiveValue::String(_) | PrimitiveValue::Bool(_) | PrimitiveValue::Null => None,
    }
}

fn primitive_from_json(value: &serde_json::Value) -> Result<PrimitiveValue, &'static str> {
    match value {
        serde_json::Value::Null => Ok(PrimitiveValue::Null),
        serde_json::Value::Bool(value) => Ok(PrimitiveValue::Bool(*value)),
        serde_json::Value::Number(value) => {
            if let Some(integer) = value.as_i64() {
                Ok(PrimitiveValue::Integer(integer))
            } else if let Some(float) = value.as_f64() {
                Ok(PrimitiveValue::Float(float))
            } else {
                Err("must be a finite JSON number")
            }
        }
        serde_json::Value::String(value) => Ok(PrimitiveValue::String(value.clone())),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            Err("must be a scalar JSON value")
        }
    }
}

fn primitive_to_string(value: &serde_json::Value) -> Option<String> {
    match primitive_from_json(value).ok()? {
        PrimitiveValue::String(value) => Some(value),
        PrimitiveValue::Integer(value) => Some(value.to_string()),
        PrimitiveValue::Float(value) => Some(value.to_string()),
        PrimitiveValue::Bool(value) => Some(value.to_string()),
        PrimitiveValue::Null => None,
    }
}

fn parse_csv_record(line: &str) -> Result<Vec<String>, &'static str> {
    let mut fields = Vec::new();
    let mut field = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();

    while let Some(character) = chars.next() {
        match character {
            '"' if in_quotes && chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            '"' => {
                in_quotes = !in_quotes;
            }
            ',' if !in_quotes => {
                fields.push(field);
                field = String::new();
            }
            _ => field.push(character),
        }
    }

    if in_quotes {
        return Err("CSV row contains an unterminated quoted field");
    }
    fields.push(field);
    Ok(fields)
}

fn csv_field_to_json(field: &str) -> serde_json::Value {
    let trimmed = field.trim();
    if trimmed.is_empty() {
        return serde_json::Value::Null;
    }
    match trimmed {
        "true" => return serde_json::Value::Bool(true),
        "false" => return serde_json::Value::Bool(false),
        _ => {}
    }
    if let Ok(value) = trimmed.parse::<i64>() {
        return serde_json::Value::Number(value.into());
    }
    if let Ok(value) = trimmed.parse::<f64>()
        && let Some(number) = serde_json::Number::from_f64(value)
    {
        return serde_json::Value::Number(number);
    }
    serde_json::Value::String(field.to_owned())
}

#[derive(Clone, Debug, Deserialize)]
struct JsonlWindow {
    #[serde(rename = "windowName")]
    window_name: Option<String>,
    key: String,
    source: String,
    partition: Option<String>,
    #[serde(rename = "startPosition")]
    start_position: i64,
    #[serde(rename = "endPosition")]
    end_position: Option<i64>,
    #[serde(default)]
    segments: Vec<JsonlNamedValue>,
    #[serde(default)]
    tags: Vec<JsonlNamedValue>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, Deserialize)]
struct JsonlNamedValue {
    name: String,
    value: PrimitiveValue,
    #[serde(rename = "parentName")]
    #[serde(skip_serializing_if = "Option::is_none")]
    parent_name: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize)]
struct ImportedWindow {
    #[serde(rename = "windowName")]
    window_name: String,
    key: String,
    source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    partition: Option<String>,
    #[serde(rename = "startPosition")]
    start_position: i64,
    #[serde(rename = "endPosition")]
    end_position: Option<i64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    segments: Vec<JsonlNamedValue>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tags: Vec<JsonlNamedValue>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ImportStateKey {
    window_name: String,
    key: String,
    source: String,
    partition: Option<String>,
}

#[derive(Clone, Debug)]
struct ImportState {
    start_position: i64,
    segments: Vec<JsonlNamedValue>,
    tags: Vec<JsonlNamedValue>,
}

impl ImportState {
    fn to_window_for_key(&self, key: &ImportStateKey, end_position: Option<i64>) -> ImportedWindow {
        ImportedWindow {
            window_name: key.window_name.clone(),
            key: key.key.clone(),
            source: key.source.clone(),
            partition: key.partition.clone(),
            start_position: self.start_position,
            end_position,
            segments: self.segments.clone(),
            tags: self.tags.clone(),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
struct EventImportMap {
    input: Option<String>,
    source: FieldSelector,
    key: Option<FieldSelector>,
    position: FieldSelector,
    partition: Option<FieldSelector>,
    windows: Vec<EventWindowMap>,
}

#[derive(Clone, Debug, Deserialize)]
struct EventWindowMap {
    name: String,
    key: Option<FieldSelector>,
    active: EventPredicate,
    #[serde(default, deserialize_with = "deserialize_segments")]
    segments: Vec<NamedFieldSelector>,
    #[serde(default, deserialize_with = "deserialize_tags")]
    tags: Vec<NamedFieldSelector>,
}

#[derive(Clone, Debug, Deserialize)]
struct EventPredicate {
    field: String,
    equals: Option<PrimitiveValue>,
    #[serde(rename = "notEquals")]
    not_equals: Option<PrimitiveValue>,
    #[serde(rename = "greaterThan")]
    greater_than: Option<PrimitiveValue>,
    #[serde(rename = "greaterThanOrEqual")]
    greater_than_or_equal: Option<PrimitiveValue>,
    #[serde(rename = "lessThan")]
    less_than: Option<PrimitiveValue>,
    #[serde(rename = "lessThanOrEqual")]
    less_than_or_equal: Option<PrimitiveValue>,
    #[serde(rename = "isTrue")]
    is_true: Option<bool>,
    #[serde(rename = "isFalse")]
    is_false: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
enum FieldSelector {
    FieldName(String),
    Field { field: String },
}

impl FieldSelector {
    fn field(&self) -> &str {
        match self {
            Self::FieldName(field) | Self::Field { field } => field,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
struct RawNamedFieldSelector {
    name: String,
    field: String,
    #[serde(rename = "parentName")]
    parent_name: Option<String>,
}

#[derive(Clone, Debug)]
struct NamedFieldSelector {
    name: String,
    selector: FieldSelector,
    parent_name: Option<String>,
    kind: &'static str,
}

fn deserialize_segments<'de, D>(deserializer: D) -> Result<Vec<NamedFieldSelector>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_named_selectors(deserializer, "segment")
}

fn deserialize_tags<'de, D>(deserializer: D) -> Result<Vec<NamedFieldSelector>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_named_selectors(deserializer, "tag")
}

fn deserialize_named_selectors<'de, D>(
    deserializer: D,
    kind: &'static str,
) -> Result<Vec<NamedFieldSelector>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = Vec::<RawNamedFieldSelector>::deserialize(deserializer)?;
    Ok(raw
        .into_iter()
        .map(|selector| NamedFieldSelector {
            name: selector.name,
            selector: FieldSelector::Field {
                field: selector.field,
            },
            parent_name: selector.parent_name,
            kind,
        })
        .collect())
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
            file.path().to_str().expect("utf8 path"),
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
            file.path().to_str().expect("utf8 path"),
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
}
