//! Input adapters, comparison workflows, and artifact sinks for the CLI.

use super::*;
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{BufRead, BufReader},
};

use spanfold::recorder::WindowTransitionKind;
use spanfold::{
    OpenWindow, PrimitiveValue, WindowObservation, WindowRecordId, WindowRecorder,
    WindowRecorderTransition, WindowSegment, WindowTag,
};

mod import;

use import::{
    CompiledFieldSelector, CompiledImportMap, CompiledNamedFieldSelector, ImportError,
    ImportedWindowSink, JsonlWindowSink, create_import_stage, evaluate_predicate,
    import_events_csv, import_events_jsonl, primitive_from_json, primitive_to_string,
    publish_import_stage, select_compiled_field, validate_import_paths,
};

#[cfg(test)]
pub(super) use import::select_field;

pub(super) fn load_fixture(path: &Path) -> Result<ContractFixture, CliError> {
    let json = fs::read_to_string(path).map_err(CliError::io)?;
    ContractFixture::parse_json(&json).map_err(|error| error.to_string().into())
}

pub(super) fn write_audit_bundle(
    result: &spanfold::ComparisonResult,
    out: &Path,
) -> Result<(), CliError> {
    let json = export_result_json(result).map_err(CliError::export)?;
    let markdown = export_result_markdown(result);
    let llm = export_result_llm_context(result).map_err(CliError::export)?;
    let html = export_result_debug_html(result);
    let mut jsonl = Vec::new();
    write_result_json_lines(result, &mut jsonl).map_err(CliError::export)?;
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
    let manifest = serde_json::to_string_pretty(&manifest).map_err(CliError::export)?;
    let paths = [
        out.join("comparison.json"),
        out.join("comparison.md"),
        out.join("comparison.llm.json"),
        out.join("comparison.html"),
        out.join("comparison.rows.jsonl"),
        out.join("manifest.json"),
    ];
    let artifacts = [
        (paths[0].as_path(), json.as_bytes()),
        (paths[1].as_path(), markdown.as_bytes()),
        (paths[2].as_path(), llm.as_bytes()),
        (paths[3].as_path(), html.as_bytes()),
        (paths[4].as_path(), jsonl.as_slice()),
        (paths[5].as_path(), manifest.as_bytes()),
    ];
    write_export_files_atomically(&artifacts).map_err(CliError::io)?;
    println!("{manifest}");
    Ok(())
}

fn row_counts_json(result: &spanfold::ComparisonResult) -> serde_json::Value {
    serde_json::json!({
        "overlap": result.overlap_rows().len(),
        "residual": result.residual_rows().len(),
        "missing": result.missing_rows().len(),
        "coverage": result.coverage_rows().len(),
        "gap": result.gap_rows().len(),
        "symmetricDifference": result.symmetric_difference_rows().len(),
        "containment": result.containment_rows().len(),
        "leadLag": result.lead_lag_rows().len(),
        "asOf": result.as_of_rows().len()
    })
}

pub(super) struct WindowAuditOptions<'a> {
    pub(super) default_window_name: Option<&'a str>,
    pub(super) target: &'a str,
    pub(super) against: &'a [String],
    pub(super) name: &'a str,
    pub(super) comparators: &'a [String],
    pub(super) strict: bool,
    pub(super) live_horizon_position: Option<i64>,
}

pub(super) fn compare_windows_jsonl(
    path: &Path,
    options: WindowAuditOptions<'_>,
) -> Result<spanfold::ComparisonResult, String> {
    if options.against.is_empty() {
        return Err("audit-windows requires at least one --against value".to_owned());
    }
    let comparators = parse_comparators(options.comparators)?;
    let history = load_window_history_jsonl(path, options.default_window_name)?;
    let plan = ComparisonPlan::new(
        options.name,
        options.target,
        AgainstSelection::Sources(options.against.to_vec()),
        comparators,
    )
    .with_scope_window(options.default_window_name.map(str::to_owned))
    .with_require_closed_windows(options.live_horizon_position.is_none())
    .with_open_window_policy(
        if options.live_horizon_position.is_some() {
            OpenWindowPolicy::ClipToHorizon
        } else {
            OpenWindowPolicy::RequireClosed
        },
        options.live_horizon_position.map(TemporalPoint::position),
    )
    .with_strict(options.strict);
    Ok(if let Some(horizon) = options.live_horizon_position {
        compare_live(&history, &plan, TemporalPoint::position(horizon))
    } else {
        compare(&history, &plan)
    })
}

pub(super) fn load_window_history_jsonl(
    path: &Path,
    default_window_name: Option<&str>,
) -> Result<spanfold::WindowHistory, String> {
    let path_label = path.display().to_string();
    let file = fs::File::open(path).map_err(|error| error.to_string())?;
    let reader = BufReader::new(file);
    let mut builder = WindowHistoryFixture::new();
    for (index, line) in reader.lines().enumerate() {
        let line = line.map_err(|error| format!("{path_label}:{}: {error}", index + 1))?;
        if line.trim().is_empty() {
            continue;
        }
        let row: JsonlWindow = serde_json::from_str(&line)
            .map_err(|error| format!("{path_label}:{}: {error}", index + 1))?;
        let window_name = row
            .window_name
            .as_deref()
            .or(default_window_name)
            .ok_or_else(|| {
                format!(
                    "{path_label}:{}: windowName missing and --window not supplied",
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
            builder = builder
                .open_window(resolved_window_name, key, row.start_position, |w| {
                    apply_jsonl_metadata(w, &row)
                })
                .map_err(|error| error.to_string())?;
        }
    }
    Ok(builder.build())
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
        .map(|value| Comparator::parse_result(&value).map_err(|error| error.to_string()))
        .collect()
}

pub(super) fn import_events(path: &Path, map_path: &Path) -> Result<Vec<ImportedWindow>, CliError> {
    let operation = ImportOperation::AuditEvents;
    let import_map =
        read_import_map(map_path).map_err(|error| error.relabel_operation(operation))?;
    let input = import_map.input();
    let mut windows = Vec::new();
    match input {
        "jsonl" => import_events_jsonl(path, &import_map, &mut windows),
        "csv" => import_events_csv(path, &import_map, &mut windows),
        _ => Err(ImportError::Input(format!(
            "import-events: unsupported event input format: {input}"
        ))),
    }
    .map_err(CliError::from)
    .map_err(|error| error.relabel_operation(operation))?;
    Ok(windows)
}

pub(super) fn import_events_to_file(
    path: &Path,
    map_path: &Path,
    output: &Path,
) -> Result<(), CliError> {
    let operation = ImportOperation::ImportEvents;
    validate_import_paths(path, map_path, output)?;
    let import_map =
        read_import_map(map_path).map_err(|error| error.relabel_operation(operation))?;
    let input = import_map.input();
    if !matches!(input, "jsonl" | "csv") {
        return Err(CliError::input(format!(
            "import-events: unsupported event input format: {input}"
        )));
    }

    let (temporary, file) = create_import_stage(output)?;
    let result = (|| {
        let mut sink = JsonlWindowSink::new(file, output.to_owned());
        match input {
            "jsonl" => import_events_jsonl(path, &import_map, &mut sink),
            "csv" => import_events_csv(path, &import_map, &mut sink),
            _ => unreachable!("the input format was validated above"),
        }
        .map_err(CliError::from)?;
        sink.finish().map_err(CliError::from)?;
        publish_import_stage(&temporary, output).map_err(|error| {
            CliError::io(format!(
                "import-events: publish output '{}': {error}",
                output.display()
            ))
        })
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

struct ImportRecorder {
    recorder: WindowRecorder,
    last_position: Option<i64>,
    metadata: BTreeMap<WindowRecordId, ImportedMetadata>,
}

struct ImportedMetadata {
    segments: Vec<JsonlNamedValue>,
    tags: Vec<JsonlNamedValue>,
}

impl ImportRecorder {
    fn new() -> Self {
        Self {
            recorder: WindowRecorder::new(true),
            last_position: None,
            metadata: BTreeMap::new(),
        }
    }
}

fn process_import_event(
    event: &serde_json::Value,
    import_map: &CompiledImportMap,
    path: &str,
    line_number: usize,
    lifecycle: &mut ImportRecorder,
    sink: &mut impl ImportedWindowSink,
) -> Result<(), ImportError> {
    let position = select_i64(event, &import_map.position, path, line_number)?;
    if lifecycle.last_position.is_some_and(|last| position < last) {
        return Err(format!("{path}:{line_number}: event position cannot move backwards").into());
    }
    lifecycle.last_position = Some(position);
    let source = select_string(event, &import_map.source, path, line_number)?;
    let partition = import_map
        .partition
        .as_ref()
        .map(|selector| select_string(event, selector, path, line_number))
        .transpose()?;

    for window in &import_map.windows {
        let key_selector = window.key.as_ref().ok_or_else(|| {
            ImportError::Input(format!(
                "import-events: {path}:{line_number}: $.windows[].key or $.key is required"
            ))
        })?;
        let key = select_string(event, key_selector, path, line_number)?;
        let segments = select_named_values(event, &window.segments, path, line_number)?;
        let tags = select_named_values(event, &window.tags, path, line_number)?;
        let is_active = evaluate_predicate(event, &window.active, path, line_number)?;
        let open_record_id = lifecycle
            .recorder
            .history()
            .open_windows()
            .iter()
            .find(|open| {
                open.window_name == window.name
                    && open.key == key
                    && open.source.as_deref() == Some(source.as_str())
                    && open.partition == partition
            })
            .map(|open| open.id.clone());

        let observation = WindowObservation::new(
            window.name.clone(),
            key.clone(),
            TemporalPoint::position(position),
            is_active,
        )
        .and_then(|observation| observation.with_scope(Some(source.clone()), partition.clone()))
        .and_then(|observation| observation.with_segments(to_window_segments(&segments)))
        .and_then(|observation| observation.with_tags(to_window_tags(&tags)))
        .map_err(|error| format!("{path}:{line_number}: {error}"))?;

        let transitions = lifecycle
            .recorder
            .observe(observation)
            .map_err(|error| format!("{path}:{line_number}: {error}"))?;
        let is_tag_only_update = transitions.is_empty() && open_record_id.is_some();
        let metadata = ImportedMetadata { segments, tags };
        for transition in transitions {
            if transition.kind == WindowTransitionKind::Closed {
                let previous_metadata = lifecycle
                    .metadata
                    .remove(&transition.record_id)
                    .expect("closed import transition has metadata");
                sink.push(imported_window_from_transition(
                    &lifecycle.recorder,
                    &transition,
                    previous_metadata,
                ))?;
            } else {
                lifecycle.metadata.insert(
                    transition.record_id.clone(),
                    ImportedMetadata {
                        segments: metadata.segments.clone(),
                        tags: metadata.tags.clone(),
                    },
                );
            }
        }
        if is_tag_only_update && let Some(record_id) = open_record_id {
            lifecycle.metadata.insert(record_id, metadata);
        }
    }
    Ok(())
}

fn close_remaining_imported_windows(
    mut lifecycle: ImportRecorder,
    sink: &mut impl ImportedWindowSink,
) -> Result<(), ImportError> {
    let mut open_windows = lifecycle.recorder.history().open_windows().to_vec();
    open_windows.sort_by(|left, right| {
        (
            left.window_name.as_str(),
            left.key.as_str(),
            left.source.as_deref(),
            left.partition.as_deref(),
        )
            .cmp(&(
                right.window_name.as_str(),
                right.key.as_str(),
                right.source.as_deref(),
                right.partition.as_deref(),
            ))
    });
    for window in open_windows {
        let metadata = lifecycle
            .metadata
            .remove(&window.id)
            .expect("open import window has metadata");
        sink.push(imported_window_from_open(window, metadata))?;
    }
    Ok(())
}

fn imported_window_from_transition(
    recorder: &WindowRecorder,
    transition: &WindowRecorderTransition,
    metadata: ImportedMetadata,
) -> ImportedWindow {
    let window = recorder
        .history()
        .closed_windows()
        .iter()
        .find(|window| window.id == transition.record_id)
        .expect("closed transition is recorded before it is emitted");
    ImportedWindow {
        window_name: window.window_name.clone(),
        key: window.key.clone(),
        source: window
            .source
            .clone()
            .expect("import observations always have a source"),
        partition: window.partition.clone(),
        start_position: window.range.start().magnitude(),
        end_position: Some(window.range.end().magnitude()),
        segments: metadata.segments,
        tags: metadata.tags,
    }
}

fn imported_window_from_open(window: OpenWindow, metadata: ImportedMetadata) -> ImportedWindow {
    ImportedWindow {
        window_name: window.window_name,
        key: window.key,
        source: window
            .source
            .expect("import observations always have a source"),
        partition: window.partition,
        start_position: window.start.magnitude(),
        end_position: None,
        segments: metadata.segments,
        tags: metadata.tags,
    }
}

fn to_window_segments(values: &[JsonlNamedValue]) -> Vec<WindowSegment> {
    let values_by_name = values
        .iter()
        .map(|value| (value.name.as_str(), value))
        .collect::<BTreeMap<_, _>>();
    let mut emitted = BTreeSet::<String>::new();
    let mut projected = Vec::new();

    for value in values {
        if emitted.contains(value.name.as_str()) {
            continue;
        }
        let parent = value
            .parent_name
            .as_deref()
            .filter(|parent| !parent.trim().is_empty());
        if let Some(parent) = parent
            && !emitted.contains(parent)
        {
            let parent_value = values_by_name
                .get(parent)
                .map_or(PrimitiveValue::Null, |value| value.value.clone());
            projected.push(
                WindowSegment::new(parent.to_owned(), parent_value)
                    .expect("compiled import map guarantees non-empty segment names"),
            );
            emitted.insert(parent.to_owned());
        }
        let segment = WindowSegment::new(value.name.clone(), value.value.clone())
            .expect("compiled import map guarantees non-empty segment names");
        projected.push(if let Some(parent) = parent {
            segment
                .with_parent(parent.to_owned())
                .expect("projected parent segments precede their children")
        } else {
            segment
        });
        emitted.insert(value.name.clone());
    }
    projected
}

fn to_window_tags(values: &[JsonlNamedValue]) -> Vec<WindowTag> {
    values
        .iter()
        .map(|value| {
            WindowTag::new(value.name.clone(), value.value.clone())
                .expect("compiled import map guarantees non-empty tag names")
        })
        .collect()
}

pub(super) fn compare_imported_windows(
    windows: &[ImportedWindow],
    target: &str,
    against: &[String],
) -> Result<spanfold::ComparisonResult, CliError> {
    if against.is_empty() {
        return Err("audit-events requires at least one --against value"
            .to_owned()
            .into());
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
            builder = builder
                .open_window(
                    window.window_name.clone(),
                    window.key.clone(),
                    window.start_position,
                    |metadata| apply_imported_metadata(metadata, window),
                )
                .map_err(|error| error.to_string())?;
        }
    }

    let history = builder.build();
    let plan = ComparisonPlan::new(
        "Spanfold Event Audit",
        target,
        AgainstSelection::Sources(against.to_vec()),
        vec![
            Comparator::Overlap,
            Comparator::Residual,
            Comparator::Missing,
            Comparator::Coverage,
            Comparator::Gap,
            Comparator::SymmetricDifference,
        ],
    );
    Ok(compare(&history, &plan))
}

fn read_import_map(path: &Path) -> Result<CompiledImportMap, CliError> {
    let path_label = path.display().to_string();
    let json = fs::read_to_string(path).map_err(|error| {
        CliError::io(format!(
            "import-events: read import map '{path_label}': {error}"
        ))
    })?;
    let import_map = import::deserialize_import_map(&json)
        .map_err(|error| format!("import-events: parse import map '{path_label}': {error}"))?;
    import::compile_import_map(import_map, &path_label)
        .map_err(|error| format!("import-events: compile import map '{path_label}': {error}"))
        .map_err(CliError::from)
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
    selectors: &[CompiledNamedFieldSelector],
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
    selector: &CompiledFieldSelector,
    path: &str,
    line_number: usize,
) -> Result<i64, String> {
    let value = select_value(event, selector, path, line_number)?;
    value
        .as_i64()
        .or_else(|| value.as_str()?.parse::<i64>().ok())
        .ok_or_else(|| {
            format!(
                "{path}:{line_number}: field '{}' must be an integer",
                selector.field()
            )
        })
}

fn select_string(
    event: &serde_json::Value,
    selector: &CompiledFieldSelector,
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
    selector: &CompiledFieldSelector,
    path: &str,
    line_number: usize,
) -> Result<&'a serde_json::Value, String> {
    select_compiled_field(event, selector, path, line_number)
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
pub(super) struct ImportedWindow {
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
