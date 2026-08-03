//! Input adapters, comparison workflows, and artifact sinks for the CLI.

use super::*;

pub(super) fn load_fixture(path: &Path) -> Result<ContractFixture, CliError> {
    let json = fs::read_to_string(path).map_err(CliError::io)?;
    ContractFixture::parse_json(&json).map_err(|error| error.to_string().into())
}

pub(super) fn write_audit_bundle(
    result: &spanfold::ComparisonResult,
    out: &Path,
) -> Result<(), CliError> {
    fs::create_dir_all(out).map_err(CliError::io)?;
    let json = export_result_json(result).map_err(CliError::export)?;
    fs::write(out.join("comparison.json"), json).map_err(CliError::io)?;
    let markdown = export_result_markdown(result);
    fs::write(out.join("comparison.md"), markdown).map_err(CliError::io)?;
    let llm = export_result_llm_context(result).map_err(CliError::export)?;
    fs::write(out.join("comparison.llm.json"), llm).map_err(CliError::io)?;
    let html = export_result_debug_html(result);
    fs::write(out.join("comparison.html"), html).map_err(CliError::io)?;
    let jsonl = fs::File::create(out.join("comparison.rows.jsonl")).map_err(CliError::io)?;
    write_result_json_lines(result, jsonl).map_err(CliError::export)?;
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
    fs::write(out.join("manifest.json"), &manifest).map_err(CliError::io)?;
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
            .or(options.default_window_name)
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
    let history = builder.build();
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
    let import_map = read_import_map(map_path)?;
    let input = import_map.input.as_deref().unwrap_or("jsonl");
    let mut windows = Vec::new();
    match input {
        "jsonl" => import_events_jsonl(path, &import_map, &mut windows),
        "csv" => import_events_csv(path, &import_map, &mut windows),
        _ => Err(format!("unsupported event input format: {input}")),
    }
    .map_err(CliError::from)?;
    Ok(windows)
}

pub(super) fn import_events_to_file(
    path: &Path,
    map_path: &Path,
    output: &Path,
) -> Result<(), CliError> {
    let import_map = read_import_map(map_path)?;
    let input = import_map.input.as_deref().unwrap_or("jsonl");
    let file = fs::File::create(output).map_err(CliError::io)?;
    let mut sink = JsonlWindowSink { writer: file };
    match input {
        "jsonl" => import_events_jsonl(path, &import_map, &mut sink),
        "csv" => import_events_csv(path, &import_map, &mut sink),
        _ => Err(format!("unsupported event input format: {input}")),
    }
    .map_err(CliError::from)
}

trait ImportedWindowSink {
    fn push(&mut self, window: ImportedWindow) -> Result<(), String>;
}

impl ImportedWindowSink for Vec<ImportedWindow> {
    fn push(&mut self, window: ImportedWindow) -> Result<(), String> {
        self.push(window);
        Ok(())
    }
}

struct JsonlWindowSink<W> {
    writer: W,
}

impl<W: Write> ImportedWindowSink for JsonlWindowSink<W> {
    fn push(&mut self, window: ImportedWindow) -> Result<(), String> {
        let line = serde_json::to_string(&window).map_err(|error| error.to_string())?;
        writeln!(self.writer, "{line}").map_err(|error| error.to_string())
    }
}

fn import_events_jsonl(
    path: &Path,
    import_map: &EventImportMap,
    sink: &mut impl ImportedWindowSink,
) -> Result<(), String> {
    let path_label = path.display().to_string();
    let file = fs::File::open(path).map_err(|error| error.to_string())?;
    let reader = BufReader::new(file);
    let mut active: BTreeMap<ImportStateKey, ImportState> = BTreeMap::new();
    let mut last_position: Option<i64> = None;

    for (index, line) in reader.lines().enumerate() {
        let line = line.map_err(|error| error.to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        let event: serde_json::Value = serde_json::from_str(&line)
            .map_err(|error| format!("{path_label}:{}: {error}", index + 1))?;
        process_import_event(
            &event,
            import_map,
            &path_label,
            index + 1,
            &mut active,
            sink,
            &mut last_position,
        )?;
    }

    close_remaining_imported_windows(active, sink)?;
    Ok(())
}

fn import_events_csv(
    path: &Path,
    import_map: &EventImportMap,
    sink: &mut impl ImportedWindowSink,
) -> Result<(), String> {
    let path_label = path.display().to_string();
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        .from_path(path)
        .map_err(|error| format!("{path_label}: {error}"))?;
    let headers = reader
        .headers()
        .map_err(|error| format!("{path_label}:1: {error}"))?
        .iter()
        .enumerate()
        .map(|(index, header)| {
            if index == 0 {
                header.trim_start_matches('\u{feff}').to_owned()
            } else {
                header.to_owned()
            }
        })
        .collect::<Vec<_>>();
    if headers.is_empty() {
        return Err(format!(
            "{path_label}:1: CSV header must contain at least one column"
        ));
    }
    let mut header_names = std::collections::BTreeSet::new();
    for header in &headers {
        if header.trim().is_empty() || !header_names.insert(header.as_str()) {
            return Err(format!(
                "{path_label}:1: CSV headers must be non-empty and unique"
            ));
        }
    }

    let mut active: BTreeMap<ImportStateKey, ImportState> = BTreeMap::new();
    let mut last_position: Option<i64> = None;

    for (index, record) in reader.records().enumerate() {
        let line_number = index + 2;
        let record = record.map_err(|error| format!("{path_label}:{line_number}: {error}"))?;
        let mut event = serde_json::Map::new();
        for (header, field) in headers.iter().zip(record.iter()) {
            event.insert(header.clone(), serde_json::Value::String(field.to_owned()));
        }
        process_import_event(
            &serde_json::Value::Object(event),
            import_map,
            &path_label,
            line_number,
            &mut active,
            sink,
            &mut last_position,
        )?;
    }

    close_remaining_imported_windows(active, sink)?;
    Ok(())
}

fn process_import_event(
    event: &serde_json::Value,
    import_map: &EventImportMap,
    path: &str,
    line_number: usize,
    active: &mut BTreeMap<ImportStateKey, ImportState>,
    sink: &mut impl ImportedWindowSink,
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
                    sink.push(state.to_window_for_key(&state_key, Some(position)))?;
                    *state = ImportState {
                        start_position: position,
                        segments,
                        tags,
                    };
                } else if state.tags != tags {
                    state.tags = tags;
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
            sink.push(state.to_window_for_key(&state_key, Some(position)))?;
        }
    }
    Ok(())
}

fn close_remaining_imported_windows(
    active: BTreeMap<ImportStateKey, ImportState>,
    sink: &mut impl ImportedWindowSink,
) -> Result<(), String> {
    for (state_key, state) in active {
        sink.push(state.to_window_for_key(&state_key, None))?;
    }
    Ok(())
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

fn read_import_map(path: &Path) -> Result<EventImportMap, CliError> {
    let path_label = path.display().to_string();
    let json = fs::read_to_string(path).map_err(CliError::io)?;
    let import_map: EventImportMap = serde_json::from_str(&json)
        .map_err(|error| CliError::from(format!("{path_label}: {error}")))?;
    import_map.validate(&path_label).map_err(CliError::from)?;
    Ok(import_map)
}

impl EventImportMap {
    fn validate(&self, path: &str) -> Result<(), String> {
        if self.windows.is_empty() {
            return Err(format!(
                "{path}: $.windows must contain at least one window"
            ));
        }
        if self.source.field().trim().is_empty() || self.position.field().trim().is_empty() {
            return Err(format!("{path}: source and position fields are required"));
        }
        let mut names = std::collections::BTreeSet::new();
        for window in &self.windows {
            if window.name.trim().is_empty() || !names.insert(window.name.as_str()) {
                return Err(format!("{path}: window names must be non-empty and unique"));
            }
            window.active.validate(path)?;
            for selector in window.segments.iter().chain(window.tags.iter()) {
                if selector.name.trim().is_empty() || selector.selector.field().trim().is_empty() {
                    return Err(format!(
                        "{path}: named selectors require non-empty names and fields"
                    ));
                }
            }
        }
        Ok(())
    }
}

impl EventPredicate {
    fn validate(&self, path: &str) -> Result<(), String> {
        if self.field.trim().is_empty() {
            return Err(format!("{path}: predicate field cannot be empty"));
        }
        let operators = [
            self.equals.is_some(),
            self.not_equals.is_some(),
            self.greater_than.is_some(),
            self.greater_than_or_equal.is_some(),
            self.less_than.is_some(),
            self.less_than_or_equal.is_some(),
            self.is_true.is_some(),
            self.is_false.is_some(),
        ]
        .into_iter()
        .filter(|present| *present)
        .count();
        if operators != 1 {
            return Err(format!(
                "{path}: each predicate must declare exactly one operator"
            ));
        }
        Ok(())
    }
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

pub(super) fn select_field<'a>(
    event: &'a serde_json::Value,
    field_path: &str,
    path: &str,
    line_number: usize,
) -> Result<&'a serde_json::Value, String> {
    let mut current = event;
    let fields = parse_field_path(field_path)
        .map_err(|error| format!("{path}:{line_number}: invalid field '{field_path}': {error}"))?;
    for field in fields {
        let next = match field {
            FieldPathPart::Name(field) => current.get(&field).or_else(|| {
                current
                    .as_array()
                    .and_then(|_| field.parse::<usize>().ok())
                    .and_then(|index| current.get(index))
            }),
            FieldPathPart::Index(index) => current.get(index),
        };
        let Some(next) = next else {
            return Err(format!(
                "{path}:{line_number}: missing event field '{field_path}'"
            ));
        };
        current = next;
    }
    Ok(current)
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum FieldPathPart {
    Name(String),
    Index(usize),
}

fn parse_field_path(field_path: &str) -> Result<Vec<FieldPathPart>, &'static str> {
    if let Some(pointer) = field_path.strip_prefix('/') {
        if pointer.is_empty() {
            return Ok(Vec::new());
        }
        return Ok(pointer
            .split('/')
            .map(|field| FieldPathPart::Name(field.replace("~1", "/").replace("~0", "~")))
            .collect());
    }

    let mut parts = Vec::new();
    let mut name = String::new();
    let mut chars = field_path.chars().peekable();
    while let Some(character) = chars.next() {
        match character {
            '.' => {
                if name.is_empty() {
                    return Err("empty path segment");
                }
                parts.push(FieldPathPart::Name(std::mem::take(&mut name)));
            }
            '[' => {
                if !name.is_empty() {
                    parts.push(FieldPathPart::Name(std::mem::take(&mut name)));
                }
                let mut index = String::new();
                let mut closed = false;
                for next in chars.by_ref() {
                    if next == ']' {
                        closed = true;
                        break;
                    }
                    index.push(next);
                }
                if !closed {
                    return Err("unmatched opening bracket");
                }
                if !index.chars().all(|digit| digit.is_ascii_digit()) {
                    return Err("array indexes must be non-negative integers");
                }
                let index = index
                    .parse::<usize>()
                    .map_err(|_| "array index is too large")?;
                parts.push(FieldPathPart::Index(index));
                if chars.peek() == Some(&'.') {
                    chars.next();
                }
            }
            ']' => return Err("unmatched closing bracket"),
            _ => name.push(character),
        }
    }
    if field_path.ends_with('.') {
        return Err("empty path segment");
    }
    if !name.is_empty() {
        parts.push(FieldPathPart::Name(name));
    }
    if parts.is_empty() {
        return Err("field path is empty");
    }
    Ok(parts)
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
        matches &= compare_numbers(&primitive, expected, |ordering| {
            ordering == std::cmp::Ordering::Greater
        })
        .ok_or_else(|| numeric_predicate_error(path, line_number, predicate))?;
    }
    if let Some(expected) = &predicate.greater_than_or_equal {
        evaluated = true;
        matches &= compare_numbers(&primitive, expected, |ordering| {
            ordering != std::cmp::Ordering::Less
        })
        .ok_or_else(|| numeric_predicate_error(path, line_number, predicate))?;
    }
    if let Some(expected) = &predicate.less_than {
        evaluated = true;
        matches &= compare_numbers(&primitive, expected, |ordering| {
            ordering == std::cmp::Ordering::Less
        })
        .ok_or_else(|| numeric_predicate_error(path, line_number, predicate))?;
    }
    if let Some(expected) = &predicate.less_than_or_equal {
        evaluated = true;
        matches &= compare_numbers(&primitive, expected, |ordering| {
            ordering != std::cmp::Ordering::Greater
        })
        .ok_or_else(|| numeric_predicate_error(path, line_number, predicate))?;
    }
    if predicate.is_true.unwrap_or(false) {
        evaluated = true;
        matches &= primitive == PrimitiveValue::Bool(true)
            || primitive == PrimitiveValue::String("true".to_owned());
    }
    if predicate.is_false.unwrap_or(false) {
        evaluated = true;
        matches &= primitive == PrimitiveValue::Bool(false)
            || primitive == PrimitiveValue::String("false".to_owned());
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
    compare: impl FnOnce(std::cmp::Ordering) -> bool,
) -> Option<bool> {
    let left = csv_numeric(left)?;
    let right = csv_numeric(right)?;
    let ordering = match (&left, &right) {
        (PrimitiveValue::Integer(left), PrimitiveValue::Integer(right)) => left.cmp(right),
        (PrimitiveValue::Float(left), PrimitiveValue::Float(right)) => left.partial_cmp(right)?,
        (PrimitiveValue::Integer(left), PrimitiveValue::Float(right)) => {
            if left.unsigned_abs() > (1_u64 << 53) {
                return None;
            }
            (*left as f64).partial_cmp(right)?
        }
        (PrimitiveValue::Float(left), PrimitiveValue::Integer(right)) => {
            if right.unsigned_abs() > (1_u64 << 53) {
                return None;
            }
            left.partial_cmp(&(*right as f64))?
        }
        _ => return None,
    };
    Some(compare(ordering))
}

fn csv_numeric(value: &PrimitiveValue) -> Option<PrimitiveValue> {
    match value {
        PrimitiveValue::String(value) => value
            .parse::<i64>()
            .ok()
            .map(PrimitiveValue::Integer)
            .or_else(|| {
                value
                    .parse::<f64>()
                    .ok()
                    .and_then(|number| PrimitiveValue::try_float(number).ok())
            }),
        other => Some(other.clone()),
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
                PrimitiveValue::try_float(float).map_err(|_| "must be a finite JSON number")
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
#[serde(deny_unknown_fields)]
struct EventImportMap {
    input: Option<String>,
    source: FieldSelector,
    key: Option<FieldSelector>,
    position: FieldSelector,
    partition: Option<FieldSelector>,
    windows: Vec<EventWindowMap>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
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
