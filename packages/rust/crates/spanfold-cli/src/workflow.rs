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
        let mut sink = JsonlWindowSink {
            writer: file,
            output: output.to_owned(),
        };
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

fn validate_import_paths(events: &Path, map: &Path, output: &Path) -> Result<(), CliError> {
    if output.is_dir() {
        return Err(CliError::io(format!(
            "import-events: output path '{}' is a directory",
            output.display()
        )));
    }

    let paths = [
        ("events", events, resolve_import_path("events", events)?),
        ("map", map, resolve_import_path("map", map)?),
        ("output", output, resolve_import_path("output", output)?),
    ];
    for (index, (left_label, left_path, left_resolved)) in paths.iter().enumerate() {
        for (right_label, right_path, right_resolved) in paths.iter().skip(index + 1) {
            if left_resolved == right_resolved {
                return Err(CliError::input(format!(
                    "import-events: {left_label} path '{}' resolves to the same path as {right_label} path '{}'; canonical path aliases must be distinct (hard-linked files are supported by staged publication)",
                    left_path.display(),
                    right_path.display()
                )));
            }
        }
    }
    Ok(())
}

fn resolve_import_path(label: &str, path: &Path) -> Result<PathBuf, CliError> {
    match fs::canonicalize(path) {
        Ok(path) => Ok(path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let parent = path
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."));
            let parent = fs::canonicalize(parent).map_err(|error| {
                CliError::io(format!(
                    "import-events: resolve {label} path '{}': {error}",
                    path.display()
                ))
            })?;
            let file_name = path.file_name().ok_or_else(|| {
                CliError::io(format!(
                    "import-events: resolve {label} path '{}': path has no file name",
                    path.display()
                ))
            })?;
            Ok(parent.join(file_name))
        }
        Err(error) => Err(CliError::io(format!(
            "import-events: resolve {label} path '{}': {error}",
            path.display()
        ))),
    }
}

fn create_import_stage(output: &Path) -> Result<(PathBuf, fs::File), CliError> {
    let file_name = output
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("windows.jsonl");
    for _ in 0..100 {
        let temporary = output.with_file_name(format!(
            ".{file_name}.{}.{}.tmp",
            std::process::id(),
            IMPORT_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => return Ok((temporary, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(CliError::io(format!(
                    "import-events: create staging output '{}': {error}",
                    temporary.display()
                )));
            }
        }
    }
    Err(CliError::io(format!(
        "import-events: create staging output beside '{}' failed after repeated name collisions",
        output.display()
    )))
}

fn publish_import_stage(temporary: &Path, output: &Path) -> Result<(), std::io::Error> {
    // The staged file is a sibling, so rename is an atomic directory-entry
    // replacement on Unix. If replacement fails, the existing destination is
    // untouched. Windows reports an error when the destination already exists;
    // that leaves the existing destination intact and is surfaced to the CLI.
    fs::rename(temporary, output)
}

static IMPORT_TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

enum ImportError {
    Input(String),
    Io(String),
}

impl ImportError {
    fn io(error: impl std::fmt::Display) -> Self {
        Self::Io(error.to_string())
    }

    fn csv(context: &str, error: csv::Error) -> Self {
        let message = format!("import-events: {context}: {error}");
        if error.is_io_error() {
            Self::Io(message)
        } else {
            Self::Input(message)
        }
    }
}

impl From<String> for ImportError {
    fn from(message: String) -> Self {
        if message.starts_with("import-events:") {
            Self::Input(message)
        } else {
            Self::Input(format!("import-events: {message}"))
        }
    }
}

impl From<ImportError> for CliError {
    fn from(error: ImportError) -> Self {
        match error {
            ImportError::Input(message) => Self::from(message),
            ImportError::Io(message) => Self::io(message),
        }
    }
}

trait ImportedWindowSink {
    fn push(&mut self, window: ImportedWindow) -> Result<(), ImportError>;
}

impl ImportedWindowSink for Vec<ImportedWindow> {
    fn push(&mut self, window: ImportedWindow) -> Result<(), ImportError> {
        self.push(window);
        Ok(())
    }
}

struct JsonlWindowSink<W> {
    writer: W,
    output: PathBuf,
}

impl<W: Write> ImportedWindowSink for JsonlWindowSink<W> {
    fn push(&mut self, window: ImportedWindow) -> Result<(), ImportError> {
        let line = serde_json::to_string(&window).map_err(|error| error.to_string())?;
        writeln!(self.writer, "{line}").map_err(|error| {
            ImportError::io(format!(
                "import-events: write output '{}': {error}",
                self.output.display()
            ))
        })
    }
}

impl JsonlWindowSink<fs::File> {
    fn finish(mut self) -> Result<(), ImportError> {
        self.writer.flush().map_err(|error| {
            ImportError::io(format!(
                "import-events: flush output '{}': {error}",
                self.output.display()
            ))
        })?;
        self.writer.sync_all().map_err(|error| {
            ImportError::io(format!(
                "import-events: sync output '{}': {error}",
                self.output.display()
            ))
        })?;
        Ok(())
    }
}

fn import_events_jsonl(
    path: &Path,
    import_map: &CompiledImportMap,
    sink: &mut impl ImportedWindowSink,
) -> Result<(), ImportError> {
    let path_label = path.display().to_string();
    let file = fs::File::open(path).map_err(|error| {
        ImportError::io(format!(
            "import-events: read events '{path_label}': {error}"
        ))
    })?;
    let reader = BufReader::new(file);
    let mut active: BTreeMap<ImportStateKey, ImportState> = BTreeMap::new();
    let mut last_position: Option<i64> = None;

    for (index, line) in reader.lines().enumerate() {
        let line = line.map_err(|error| {
            ImportError::io(format!(
                "import-events: read event record '{path_label}':{}: {error}",
                index + 1
            ))
        })?;
        if line.trim().is_empty() {
            continue;
        }
        let event: serde_json::Value = serde_json::from_str(&line).map_err(|error| {
            format!(
                "import-events: parse event record '{path_label}':{}: {error}",
                index + 1
            )
        })?;
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
    import_map: &CompiledImportMap,
    sink: &mut impl ImportedWindowSink,
) -> Result<(), ImportError> {
    let path_label = path.display().to_string();
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        .from_path(path)
        .map_err(|error| ImportError::csv(&path_label, error))?;
    let headers = reader
        .headers()
        .map_err(|error| ImportError::csv(&format!("{path_label}:1"), error))?
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
            "import-events: parse CSV header '{path_label}:1': CSV header must contain at least one column"
        )
        .into());
    }
    let mut header_names = std::collections::BTreeSet::new();
    for header in &headers {
        if header.trim().is_empty() || !header_names.insert(header.as_str()) {
            return Err(format!(
                "import-events: parse CSV header '{path_label}:1': CSV headers must be non-empty and unique"
            )
            .into());
        }
    }

    let mut active: BTreeMap<ImportStateKey, ImportState> = BTreeMap::new();
    let mut last_position: Option<i64> = None;

    for (index, record) in reader.records().enumerate() {
        let line_number = index + 2;
        let record = record
            .map_err(|error| ImportError::csv(&format!("{path_label}:{line_number}"), error))?;
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
    import_map: &CompiledImportMap,
    path: &str,
    line_number: usize,
    active: &mut BTreeMap<ImportStateKey, ImportState>,
    sink: &mut impl ImportedWindowSink,
    last_position: &mut Option<i64>,
) -> Result<(), ImportError> {
    let position = select_i64(event, &import_map.position, path, line_number)?;
    if last_position.is_some_and(|last| position < last) {
        return Err(format!("{path}:{line_number}: event position cannot move backwards").into());
    }
    *last_position = Some(position);
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
) -> Result<(), ImportError> {
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

fn read_import_map(path: &Path) -> Result<CompiledImportMap, CliError> {
    let path_label = path.display().to_string();
    let json = fs::read_to_string(path).map_err(|error| {
        CliError::io(format!(
            "import-events: read import map '{path_label}': {error}"
        ))
    })?;
    let import_map: EventImportMap = serde_json::from_str(&json)
        .map_err(|error| format!("import-events: parse import map '{path_label}': {error}"))?;
    import_map
        .compile(&path_label)
        .map_err(|error| format!("import-events: compile import map '{path_label}': {error}"))
        .map_err(CliError::from)
}

impl EventImportMap {
    fn compile(self, path: &str) -> Result<CompiledImportMap, String> {
        self.validate(path)?;
        let EventImportMap {
            input,
            source,
            key,
            position,
            partition,
            windows,
        } = self;
        let uses_default_key = windows.iter().any(|window| window.key.is_none());
        let key = if uses_default_key {
            key.map(|selector| selector.compile(path)).transpose()?
        } else {
            None
        };
        Ok(CompiledImportMap {
            input,
            source: source.compile(path)?,
            position: position.compile(path)?,
            partition: partition
                .map(|selector| selector.compile(path))
                .transpose()?,
            windows: windows
                .into_iter()
                .map(|window| window.compile(key.as_ref(), path))
                .collect::<Result<_, _>>()?,
        })
    }

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

    fn compile(self, path: &str) -> Result<CompiledPredicate, String> {
        let field = CompiledFieldSelector::compile(self.field, path)?;
        let operator = if let Some(expected) = self.equals {
            PredicateOperator::Equals(expected)
        } else if let Some(expected) = self.not_equals {
            PredicateOperator::NotEquals(expected)
        } else if let Some(expected) = self.greater_than {
            PredicateOperator::GreaterThan(expected)
        } else if let Some(expected) = self.greater_than_or_equal {
            PredicateOperator::GreaterThanOrEqual(expected)
        } else if let Some(expected) = self.less_than {
            PredicateOperator::LessThan(expected)
        } else if let Some(expected) = self.less_than_or_equal {
            PredicateOperator::LessThanOrEqual(expected)
        } else if let Some(expected) = self.is_true {
            PredicateOperator::IsTrue(expected)
        } else if let Some(expected) = self.is_false {
            PredicateOperator::IsFalse(expected)
        } else {
            unreachable!("validated predicates have exactly one operator")
        };
        Ok(CompiledPredicate { field, operator })
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

#[cfg(test)]
pub(super) fn select_field<'a>(
    event: &'a serde_json::Value,
    field_path: &str,
    path: &str,
    line_number: usize,
) -> Result<&'a serde_json::Value, String> {
    let parts = parse_field_path(field_path)
        .map_err(|error| format!("{path}:{line_number}: invalid field '{field_path}': {error}"))?;
    let selector = CompiledFieldSelector {
        field: field_path.to_owned(),
        parts,
    };
    select_compiled_field(event, &selector, path, line_number)
}

fn select_compiled_field<'a>(
    event: &'a serde_json::Value,
    selector: &CompiledFieldSelector,
    path: &str,
    line_number: usize,
) -> Result<&'a serde_json::Value, String> {
    let mut current = event;
    for part in &selector.parts {
        let next = match part {
            FieldPathPart::Name(field) => current.get(field).or_else(|| {
                current
                    .as_array()
                    .and_then(|_| field.parse::<usize>().ok())
                    .and_then(|index| current.get(index))
            }),
            FieldPathPart::Index(index) => current.get(*index),
        };
        let Some(next) = next else {
            return Err(format!(
                "{path}:{line_number}: missing event field '{}'",
                selector.field
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
    predicate: &CompiledPredicate,
    path: &str,
    line_number: usize,
) -> Result<bool, String> {
    let value = select_compiled_field(event, &predicate.field, path, line_number)?;
    let primitive = primitive_from_json(value).map_err(|error| {
        format!(
            "{path}:{line_number}: predicate field '{}' {error}",
            predicate.field.field
        )
    })?;

    match &predicate.operator {
        PredicateOperator::Equals(expected) => Ok(primitive == *expected),
        PredicateOperator::NotEquals(expected) => Ok(primitive != *expected),
        PredicateOperator::GreaterThan(expected) => {
            compare_numbers(&primitive, expected, |ordering| {
                ordering == std::cmp::Ordering::Greater
            })
            .ok_or_else(|| numeric_predicate_error(path, line_number, predicate))
        }
        PredicateOperator::GreaterThanOrEqual(expected) => {
            compare_numbers(&primitive, expected, |ordering| {
                ordering != std::cmp::Ordering::Less
            })
            .ok_or_else(|| numeric_predicate_error(path, line_number, predicate))
        }
        PredicateOperator::LessThan(expected) => {
            compare_numbers(&primitive, expected, |ordering| {
                ordering == std::cmp::Ordering::Less
            })
            .ok_or_else(|| numeric_predicate_error(path, line_number, predicate))
        }
        PredicateOperator::LessThanOrEqual(expected) => {
            compare_numbers(&primitive, expected, |ordering| {
                ordering != std::cmp::Ordering::Greater
            })
            .ok_or_else(|| numeric_predicate_error(path, line_number, predicate))
        }
        PredicateOperator::IsTrue(true) => Ok(primitive == PrimitiveValue::Bool(true)
            || primitive == PrimitiveValue::String("true".to_owned())),
        PredicateOperator::IsFalse(true) => Ok(primitive == PrimitiveValue::Bool(false)
            || primitive == PrimitiveValue::String("false".to_owned())),
        PredicateOperator::IsTrue(false) | PredicateOperator::IsFalse(false) => Err(format!(
            "{path}:{line_number}: predicate for field '{}' has no condition",
            predicate.field.field
        )),
    }
}

fn numeric_predicate_error(
    path: &str,
    line_number: usize,
    predicate: &CompiledPredicate,
) -> String {
    format!(
        "{path}:{line_number}: predicate field '{}' and threshold must be numeric",
        predicate.field.field
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
        (PrimitiveValue::Float(left), PrimitiveValue::Float(right)) => {
            left.as_f64().partial_cmp(&right.as_f64())?
        }
        (PrimitiveValue::Integer(left), PrimitiveValue::Float(right)) => {
            if left.unsigned_abs() > (1_u64 << 53) {
                return None;
            }
            (*left as f64).partial_cmp(&right.as_f64())?
        }
        (PrimitiveValue::Float(left), PrimitiveValue::Integer(right)) => {
            if right.unsigned_abs() > (1_u64 << 53) {
                return None;
            }
            left.as_f64().partial_cmp(&(*right as f64))?
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
        PrimitiveValue::Float(value) => Some(value.as_f64().to_string()),
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

impl EventWindowMap {
    fn compile(
        self,
        default_key: Option<&CompiledFieldSelector>,
        path: &str,
    ) -> Result<CompiledWindowMap, String> {
        Ok(CompiledWindowMap {
            name: self.name,
            key: self
                .key
                .map(|selector| selector.compile(path))
                .transpose()?
                .or_else(|| default_key.cloned()),
            active: self.active.compile(path)?,
            segments: self
                .segments
                .into_iter()
                .map(|selector| selector.compile(path))
                .collect::<Result<_, _>>()?,
            tags: self
                .tags
                .into_iter()
                .map(|selector| selector.compile(path))
                .collect::<Result<_, _>>()?,
        })
    }
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

    fn compile(self, path: &str) -> Result<CompiledFieldSelector, String> {
        let field = match self {
            Self::FieldName(field) | Self::Field { field } => field,
        };
        CompiledFieldSelector::compile(field, path)
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

impl NamedFieldSelector {
    fn compile(self, path: &str) -> Result<CompiledNamedFieldSelector, String> {
        Ok(CompiledNamedFieldSelector {
            name: self.name,
            selector: self.selector.compile(path)?,
            parent_name: self.parent_name,
            kind: self.kind,
        })
    }
}

#[derive(Clone, Debug)]
struct CompiledImportMap {
    input: Option<String>,
    source: CompiledFieldSelector,
    position: CompiledFieldSelector,
    partition: Option<CompiledFieldSelector>,
    windows: Vec<CompiledWindowMap>,
}

impl CompiledImportMap {
    fn input(&self) -> &str {
        self.input.as_deref().unwrap_or("jsonl")
    }
}

#[derive(Clone, Debug)]
struct CompiledWindowMap {
    name: String,
    key: Option<CompiledFieldSelector>,
    active: CompiledPredicate,
    segments: Vec<CompiledNamedFieldSelector>,
    tags: Vec<CompiledNamedFieldSelector>,
}

#[derive(Clone, Debug)]
struct CompiledNamedFieldSelector {
    name: String,
    selector: CompiledFieldSelector,
    parent_name: Option<String>,
    kind: &'static str,
}

#[derive(Clone, Debug)]
struct CompiledPredicate {
    field: CompiledFieldSelector,
    operator: PredicateOperator,
}

#[derive(Clone, Debug)]
enum PredicateOperator {
    Equals(PrimitiveValue),
    NotEquals(PrimitiveValue),
    GreaterThan(PrimitiveValue),
    GreaterThanOrEqual(PrimitiveValue),
    LessThan(PrimitiveValue),
    LessThanOrEqual(PrimitiveValue),
    IsTrue(bool),
    IsFalse(bool),
}

#[derive(Clone, Debug)]
struct CompiledFieldSelector {
    field: String,
    parts: Vec<FieldPathPart>,
}

impl CompiledFieldSelector {
    fn compile(field: String, path: &str) -> Result<Self, String> {
        let parts = parse_field_path(&field)
            .map_err(|error| format!("{path}: invalid field '{field}': {error}"))?;
        Ok(Self { field, parts })
    }

    fn field(&self) -> &str {
        &self.field
    }
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

    fn compile_map(json: serde_json::Value) -> Result<CompiledImportMap, String> {
        serde_json::from_value::<EventImportMap>(json)
            .expect("import map schema")
            .compile("map.json")
    }

    fn compile_predicate(json: serde_json::Value) -> Result<CompiledPredicate, String> {
        let predicate = serde_json::from_value::<EventPredicate>(json).expect("predicate schema");
        predicate.validate("map.json")?;
        predicate.compile("map.json")
    }

    #[test]
    fn compiled_import_map_selects_nested_bracket_pointer_and_numeric_name_paths() {
        let import_map = compile_map(serde_json::json!({
            "source": { "field": "/metadata/source" },
            "key": "identity.0",
            "position": "rows[0].position",
            "partition": "metadata.partition",
            "windows": [{
                "name": "Online",
                "active": { "field": "states[0].active", "isTrue": true },
                "segments": [{ "name": "escaped", "field": "/a~1b/~0key", "parentName": "root" }],
                "tags": [{ "name": "region", "field": "metadata.region" }]
            }]
        }))
        .expect("compiled map");
        let event = serde_json::json!({
            "metadata": { "source": "provider-a", "partition": "p1", "region": "eu" },
            "identity": ["device-1"],
            "rows": [{ "position": 7 }],
            "states": [{ "active": true }],
            "a/b": { "~key": 42 }
        });
        let mut active = BTreeMap::new();
        let mut windows = Vec::new();
        let mut last_position = None;

        assert!(
            process_import_event(
                &event,
                &import_map,
                "events.jsonl",
                1,
                &mut active,
                &mut windows,
                &mut last_position,
            )
            .is_ok()
        );
        assert!(close_remaining_imported_windows(active, &mut windows).is_ok());

        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].key, "device-1");
        assert_eq!(windows[0].source, "provider-a");
        assert_eq!(windows[0].partition.as_deref(), Some("p1"));
        assert_eq!(windows[0].segments[0].value, PrimitiveValue::Integer(42));
        assert_eq!(windows[0].segments[0].parent_name.as_deref(), Some("root"));
        assert_eq!(
            windows[0].tags[0].value,
            PrimitiveValue::String("eu".to_owned())
        );
    }

    #[test]
    fn compiled_predicates_preserve_operator_semantics() {
        let cases = [
            (
                serde_json::json!({ "field": "x", "equals": 4 }),
                serde_json::json!(4),
            ),
            (
                serde_json::json!({ "field": "x", "notEquals": 3 }),
                serde_json::json!(4),
            ),
            (
                serde_json::json!({ "field": "x", "greaterThan": 3 }),
                serde_json::json!(4),
            ),
            (
                serde_json::json!({ "field": "x", "greaterThanOrEqual": 4 }),
                serde_json::json!(4),
            ),
            (
                serde_json::json!({ "field": "x", "lessThan": 5 }),
                serde_json::json!(4),
            ),
            (
                serde_json::json!({ "field": "x", "lessThanOrEqual": 4 }),
                serde_json::json!(4),
            ),
            (
                serde_json::json!({ "field": "x", "isTrue": true }),
                serde_json::json!(true),
            ),
            (
                serde_json::json!({ "field": "x", "isFalse": true }),
                serde_json::json!(false),
            ),
        ];

        for (predicate, value) in cases {
            let predicate = compile_predicate(predicate).expect("compiled predicate");
            assert_eq!(
                evaluate_predicate(
                    &serde_json::json!({ "x": value }),
                    &predicate,
                    "events.jsonl",
                    2,
                ),
                Ok(true)
            );
        }
    }

    #[test]
    fn compiled_numeric_predicate_preserves_precision_failure() {
        let predicate =
            compile_predicate(serde_json::json!({ "field": "value", "greaterThan": 1.5 }))
                .expect("compiled predicate");

        assert_eq!(
            evaluate_predicate(
                &serde_json::json!({ "value": 9_007_199_254_740_993_i64 }),
                &predicate,
                "events.jsonl",
                3,
            ),
            Err("events.jsonl:3: predicate field 'value' and threshold must be numeric".to_owned())
        );
    }

    #[test]
    fn map_compilation_rejects_invalid_field_path_with_map_and_original_field() {
        let error = compile_map(serde_json::json!({
            "source": "metadata[source",
            "position": "position",
            "windows": [{
                "name": "Online",
                "key": "key",
                "active": { "field": "active", "isTrue": true }
            }]
        }))
        .expect_err("invalid selector");

        assert_eq!(
            error,
            "map.json: invalid field 'metadata[source': unmatched opening bracket"
        );
    }

    #[test]
    fn map_compilation_ignores_invalid_global_key_shadowed_by_window_keys() {
        let import_map = compile_map(serde_json::json!({
            "source": "source",
            "key": "invalid[global",
            "position": "position",
            "windows": [{
                "name": "Online",
                "key": "windowKey",
                "active": { "field": "active", "isTrue": true }
            }]
        }))
        .expect("shadowed global key is unused");

        assert_eq!(
            import_map.windows[0]
                .key
                .as_ref()
                .map(CompiledFieldSelector::field),
            Some("windowKey")
        );
    }

    #[test]
    fn compiled_boolean_false_preserves_has_no_condition_error() {
        let predicate =
            compile_predicate(serde_json::json!({ "field": "active", "isTrue": false }))
                .expect("present operator validates");

        assert_eq!(
            evaluate_predicate(
                &serde_json::json!({ "active": true }),
                &predicate,
                "events.jsonl",
                4,
            ),
            Err("events.jsonl:4: predicate for field 'active' has no condition".to_owned())
        );
    }

    #[test]
    fn predicate_compilation_still_requires_exactly_one_operator() {
        let error = compile_predicate(serde_json::json!({
            "field": "value",
            "equals": 1,
            "notEquals": 2
        }))
        .expect_err("two operators");

        assert_eq!(
            error,
            "map.json: each predicate must declare exactly one operator"
        );
    }
}
