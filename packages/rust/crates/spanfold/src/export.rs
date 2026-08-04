use std::{
    fmt::Write as FmtWrite,
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use serde::Serialize;
use serde_json::{Map, Value, json};
use thiserror::Error;

use crate::{
    Comparator, ComparisonDuplicateWindowPolicy, ComparisonFinality, ComparisonPlan,
    ComparisonResult, ComparisonRowKind, ComparisonRowMetadataError, ComparisonRowWithFinality,
    ComparisonSelector,
};

mod debug;
pub use debug::export_result_debug_html;

const PLAN_SCHEMA: &str = "spanfold.comparison.plan";
const RESULT_SCHEMA: &str = "spanfold.comparison.result";
const ROW_SCHEMA: &str = "spanfold.comparison.result-row";
const LLM_CONTEXT_SCHEMA: &str = "spanfold.comparison.llm-context";
const SCHEMA_VERSION: u32 = 0;

/// Error returned while writing configured comparison export artifacts.
#[derive(Debug, Error)]
pub enum ComparisonExportError {
    /// File system error while creating directories or writing output.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// JSON serialization error while building an export payload.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    /// Runtime-only selectors cannot be represented as portable export data.
    #[error(
        "comparison plan contains runtime-only selectors and cannot be exported as portable data"
    )]
    NonPortablePlan,
    /// Result rows and finality metadata do not share the canonical layout.
    #[error(transparent)]
    InconsistentRowMetadata(#[from] ComparisonRowMetadataError),
}

/// Configures optional debug HTML export during comparison execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComparisonDebugHtmlOptions {
    path: Option<PathBuf>,
}

impl ComparisonDebugHtmlOptions {
    /// Returns options that do not write a debug HTML artifact.
    #[must_use]
    pub const fn disabled() -> Self {
        Self { path: None }
    }

    /// Creates options that write a debug HTML artifact to a file.
    #[must_use]
    pub fn to_file(path: impl Into<PathBuf>) -> Self {
        Self {
            path: Some(path.into()),
        }
    }

    /// Gets whether debug HTML export is enabled.
    #[must_use]
    pub const fn enabled(&self) -> bool {
        self.path.is_some()
    }

    /// Gets the destination file path when export is enabled.
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }
}

/// Configures optional LLM context export during comparison execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComparisonLlmContextOptions {
    path: Option<PathBuf>,
}

impl ComparisonLlmContextOptions {
    /// Returns options that do not write an LLM context artifact.
    #[must_use]
    pub const fn disabled() -> Self {
        Self { path: None }
    }

    /// Creates options that write deterministic LLM context JSON to a file.
    #[must_use]
    pub fn to_file(path: impl Into<PathBuf>) -> Self {
        Self {
            path: Some(path.into()),
        }
    }

    /// Gets whether LLM context export is enabled.
    #[must_use]
    pub const fn enabled(&self) -> bool {
        self.path.is_some()
    }

    /// Gets the destination file path when export is enabled.
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }
}

pub(crate) fn export_configured_bundle(
    result: &ComparisonResult,
    debug_html: &ComparisonDebugHtmlOptions,
    llm_context: &ComparisonLlmContextOptions,
) -> Result<(), ComparisonExportError> {
    let debug_payload = debug_html
        .path
        .as_deref()
        .map(|_| export_result_debug_html(result));
    let llm_payload = llm_context
        .path
        .as_deref()
        .map(|_| export_result_llm_context(result))
        .transpose()?;
    let mut artifacts = Vec::new();
    if let (Some(path), Some(payload)) = (debug_html.path.as_deref(), debug_payload.as_ref()) {
        artifacts.push((path, payload.as_bytes()));
    }
    if let (Some(path), Some(payload)) = (llm_context.path.as_deref(), llm_payload.as_ref()) {
        artifacts.push((path, payload.as_bytes()));
    }
    write_export_files_atomically(&artifacts)?;
    Ok(())
}

/// Exports a comparison plan as deterministic JSON.
pub fn export_plan_json(plan: &ComparisonPlan) -> Result<String, ComparisonExportError> {
    ensure_exportable(plan)?;
    Ok(serde_json::to_string_pretty(&build_plan_json_value(plan))?)
}

/// Exports a comparison result as deterministic JSON.
pub fn export_result_json(result: &ComparisonResult) -> Result<String, ComparisonExportError> {
    ensure_exportable(&result.plan)?;
    Ok(serde_json::to_string_pretty(&build_result_json_value(
        result,
    )?)?)
}

/// Exports a comparison result as deterministic JSON Lines.
pub fn export_result_json_lines(
    result: &ComparisonResult,
) -> Result<Vec<String>, ComparisonExportError> {
    ensure_exportable(&result.plan)?;
    let mut lines = Vec::new();
    append_result_json_lines(result, &mut lines)?;
    Ok(lines)
}

/// Writes a comparison result as deterministic JSON Lines without materializing all lines first.
pub fn write_result_json_lines<W: Write>(
    result: &ComparisonResult,
    mut writer: W,
) -> Result<(), ComparisonExportError> {
    ensure_exportable(&result.plan)?;
    macro_rules! write_families {
        (
            ($first_kind:ident, $first_rows:ident, $first_compat:ident, $first_view:ident, $first_debug:literal, $first_count:literal),
            $(($kind:ident, $rows:ident, $compat:ident, $view:ident, $debug:literal, $count:literal),)*
        ) => {
            let first_rows = result.$first_view()?;
            write_json_line(&mut writer, &result_summary_json_value(result))?;
            write_json_lines(&mut writer, ComparisonRowKind::$first_kind, first_rows)?;
            $(
                write_json_lines(
                    &mut writer,
                    ComparisonRowKind::$kind,
                    result.$view()?,
                )?;
            )*
        };
    }
    crate::comparison::for_each_comparison_row_family!(write_families);
    Ok(())
}

fn append_result_json_lines(
    result: &ComparisonResult,
    lines: &mut Vec<String>,
) -> Result<(), ComparisonExportError> {
    macro_rules! append_families {
        (
            ($first_kind:ident, $first_rows:ident, $first_compat:ident, $first_view:ident, $first_debug:literal, $first_count:literal),
            $(($kind:ident, $rows:ident, $compat:ident, $view:ident, $debug:literal, $count:literal),)*
        ) => {
            let first_rows = result.$first_view()?;
            lines.push(serde_json::to_string(&result_summary_json_value(result))?);
            append_json_lines(lines, ComparisonRowKind::$first_kind, first_rows)?;
            $(
                append_json_lines(lines, ComparisonRowKind::$kind, result.$view()?)?;
            )*
        };
    }
    crate::comparison::for_each_comparison_row_family!(append_families);
    Ok(())
}

fn result_summary_json_value(result: &ComparisonResult) -> Value {
    json!({
        "schema": ROW_SCHEMA,
        "schemaVersion": SCHEMA_VERSION,
        "artifact": "result-summary",
        "planName": result.plan_name,
        "isValid": result.is_valid,
        "knownAt": result.known_at,
        "evaluationHorizon": result.evaluation_horizon,
        "diagnosticCount": result.diagnostics.len(),
        "overlapRowCount": result.overlap_rows.len(),
        "residualRowCount": result.residual_rows.len(),
        "missingRowCount": result.missing_rows.len(),
        "coverageRowCount": result.coverage_rows.len(),
        "gapRowCount": result.gap_rows.len(),
        "symmetricDifferenceRowCount": result.symmetric_difference_rows.len(),
        "containmentRowCount": result.containment_rows.len(),
        "leadLagRowCount": result.lead_lag_rows.len(),
        "asOfRowCount": result.as_of_rows.len()
    })
}

/// Exports a comparison result as deterministic LLM context JSON.
pub fn export_result_llm_context(
    result: &ComparisonResult,
) -> Result<String, ComparisonExportError> {
    ensure_exportable(&result.plan)?;
    let mut row_documents = vec![result_summary_json_value(result)];
    macro_rules! materialize_row_documents {
        ($(($kind:ident, $rows:ident, $compat:ident, $view:ident, $debug:literal, $count:literal),)*) => {
            [$(
                (
                    ComparisonRowKind::$kind,
                    build_row_values(result.$view()?)?,
                ),
            )*]
        };
    }
    for (kind, rows) in
        crate::comparison::for_each_comparison_row_family!(materialize_row_documents)
    {
        row_documents.extend(rows.into_iter().map(|row| {
            let mut object = match row {
                Value::Object(object) => object,
                _ => Map::new(),
            };
            object.insert(
                "rowType".to_owned(),
                Value::String(kind.as_str().to_owned()),
            );
            Value::Object(object)
        }));
    }

    Ok(serde_json::to_string_pretty(&json!({
        "schema": LLM_CONTEXT_SCHEMA,
        "schemaVersion": SCHEMA_VERSION,
        "artifact": "llm-context",
        "purpose": "Portable comparison context for LLMs, coding agents, CI triage, and support handoff.",
        "analysisInstructions": [
            "Treat fullResult as the source of truth for exact fields, ranges, windows, segments, tags, diagnostics, summaries, and row evidence.",
            "Use resultMarkdown for a concise natural-language orientation before drilling into fullResult.",
            "Use rowDocuments when chunking or streaming row-level analysis; rowDocuments[0] is the result summary and later entries are individual comparison rows.",
            "Preserve rowId, recordIds, window ids, temporal ranges, knownAt, evaluationHorizon, and finality metadata when citing evidence.",
            "Do not infer missing source data from absence alone; check diagnostics, normalization, excluded windows, and row finalities first."
        ],
        "summary": {
            "planName": result.plan_name,
            "isValid": result.is_valid,
            "knownAt": result.known_at,
            "evaluationHorizon": result.evaluation_horizon,
            "diagnosticCount": result.diagnostics.len(),
            "selectedWindowCount": prepared_len(result, "selectedWindows"),
            "excludedWindowCount": prepared_len(result, "excludedWindows"),
            "normalizedWindowCount": prepared_len(result, "normalizedWindows"),
            "alignedSegmentCount": aligned_len(result),
            "rowCounts": row_counts_json(result)
        },
        "resultMarkdown": export_result_markdown(result),
        "fullResult": build_result_json_value(result)?,
        "rowDocuments": row_documents
    }))?)
}

fn ensure_exportable(plan: &ComparisonPlan) -> Result<(), ComparisonExportError> {
    plan.is_serializable()
        .then_some(())
        .ok_or(ComparisonExportError::NonPortablePlan)
}

/// Exports a comparison result as deterministic Markdown.
pub fn export_result_markdown(result: &ComparisonResult) -> String {
    let mut text = format!(
        "# {}\n\nvalid: {}\n\n",
        escape_markdown(&result.plan_name),
        result.is_valid
    );
    if let Some(known_at) = result.known_at.as_ref() {
        text.push_str(&format!(
            "known at: {:?}:{}\n\n",
            known_at.axis, known_at.magnitude
        ));
    }
    if let Some(horizon) = result.evaluation_horizon.as_ref() {
        text.push_str(&format!(
            "evaluation horizon: {:?}:{}\n\n",
            horizon.axis, horizon.magnitude
        ));
    }
    if !result.diagnostics.is_empty() {
        text.push_str("## Diagnostics\n\n");
        for (index, diagnostic) in result.diagnostics.iter().enumerate() {
            text.push_str(&format!(
                "- diagnostic[{index}]: {:?} {}\n",
                diagnostic.severity,
                escape_markdown(&diagnostic.code)
            ));
        }
        text.push('\n');
    }

    text.push_str("## Row Counts\n\n");
    macro_rules! append_row_counts {
        ($(($kind:ident, $rows:ident, $compat:ident, $view:ident, $debug:literal, $count:literal),)*) => {
            $(
                text.push_str(&format!("- {}: {}\n", $count, result.$compat.len()));
            )*
        };
    }
    crate::comparison::for_each_comparison_row_family!(append_row_counts);
    text.push_str(&format!(
        "- row finalities: {}\n\n",
        result.row_finalities.len()
    ));

    text.push_str("## Row Evidence\n\n");
    macro_rules! append_row_evidence {
        ($(($kind:ident, $rows:ident, $compat:ident, $view:ident, $debug:literal, $count:literal),)*) => {
            $(
                append_markdown_rows(
                    &mut text,
                    ComparisonRowKind::$kind.as_str(),
                    &result.$compat,
                );
            )*
        };
    }
    crate::comparison::for_each_comparison_row_family!(append_row_evidence);
    text.push('\n');

    if !result.comparator_summaries.is_empty() {
        text.push_str("## Comparator Summaries\n\n");
        for summary in &result.comparator_summaries {
            text.push_str(&format!(
                "- {} rows={}\n",
                escape_markdown(&summary.comparator_name),
                summary.row_count
            ));
        }
        text.push('\n');
    }

    if !result.coverage_summaries.is_empty() {
        text.push_str("## Coverage Summaries\n\n");
        for summary in &result.coverage_summaries {
            text.push_str(&format!(
                "- {} {} ratio={:.6}\n",
                escape_markdown(&summary.window_name),
                escape_markdown(&summary.key),
                summary.coverage_ratio
            ));
        }
        text.push('\n');
    }

    if !result.lead_lag_summaries.is_empty() {
        text.push_str("## Lead Lag Summaries\n\n");
        for summary in &result.lead_lag_summaries {
            text.push_str(&format!(
                "- {:?} {:?} tolerance={} rows={} leads={} lags={} equal={} missing={} outside={}\n",
                summary.transition,
                summary.axis,
                summary.tolerance_magnitude,
                summary.row_count,
                summary.target_lead_count,
                summary.target_lag_count,
                summary.equal_count,
                summary.missing_comparison_count,
                summary.outside_tolerance_count
            ));
        }
    }

    if !result.extension_metadata.is_empty() {
        text.push_str("\n## Extension Metadata\n\n");
        for (index, item) in result.extension_metadata.iter().enumerate() {
            text.push_str(&format!(
                "- extensionMetadata[{index}]: {}.{}={}\n",
                escape_markdown(&item.extension_id),
                escape_markdown(&item.key),
                escape_markdown(&item.value)
            ));
        }
    }

    text
}

fn append_markdown_rows<T: Serialize>(text: &mut String, label: &str, rows: &[T]) {
    for (index, row) in rows.iter().enumerate() {
        let payload =
            serde_json::to_string(row).unwrap_or_else(|_| "<serialization-error>".to_owned());
        text.push_str(&format!(
            "- {}[{}]: `{}`\n",
            escape_markdown(label),
            index,
            escape_markdown(&payload)
        ));
    }
}

fn escape_markdown(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('#', "\\#")
        .replace('*', "\\*")
        .replace('_', "\\_")
        .replace('`', "\\`")
        .replace('[', "\\[")
        .replace(']', "\\]")
        .replace('|', "\\|")
        .replace('>', "\\>")
}

/// Writes a set of export artifacts through sibling staging files.
///
/// All payloads are staged before publication begins. Existing destination
/// files are preserved and restored if a later artifact cannot be published.
pub fn write_export_files_atomically(files: &[(&Path, &[u8])]) -> Result<(), std::io::Error> {
    if files.is_empty() {
        return Ok(());
    }
    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);
    let mut destinations = std::collections::BTreeSet::new();
    for (path, _) in files {
        if !destinations.insert((*path).to_owned()) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "duplicate export destination",
            ));
        }
        if path.is_dir() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::IsADirectory,
                format!("export destination is a directory: {}", path.display()),
            ));
        }
    }
    let mut staged = Vec::with_capacity(files.len());
    for (path, content) in files {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent)?;
        }
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("artifact");
        let temporary = path.with_file_name(format!(
            ".{file_name}.{}.{}.tmp",
            std::process::id(),
            TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let result = (|| {
            let mut file = fs::File::create(&temporary)?;
            file.write_all(content)?;
            file.sync_all()
        })();
        if let Err(error) = result {
            for (_, temporary) in &staged {
                let _ = fs::remove_file(temporary);
            }
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }
        staged.push((*path, temporary));
    }
    let mut published = Vec::with_capacity(staged.len());
    for (index, (path, temporary)) in staged.iter().enumerate() {
        let backup = if path.exists() {
            let file_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("artifact");
            let backup = path.with_file_name(format!(
                ".{file_name}.{}.{}.bak",
                std::process::id(),
                TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
            ));
            if let Err(error) = fs::rename(path, &backup) {
                rollback_published_files(&published);
                cleanup_staged_files(&staged[index..]);
                return Err(error);
            }
            Some(backup)
        } else {
            None
        };
        if let Err(error) = fs::rename(temporary, path) {
            if let Some(backup) = backup.as_ref() {
                let _ = fs::rename(backup, path);
            }
            rollback_published_files(&published);
            cleanup_staged_files(&staged[index..]);
            return Err(error);
        }
        published.push((*path, backup));
    }
    for (_, backup) in published {
        if let Some(backup) = backup {
            let _ = fs::remove_file(backup);
        }
    }
    Ok(())
}

fn rollback_published_files(published: &[(&Path, Option<PathBuf>)]) {
    for (path, backup) in published.iter().rev() {
        let _ = fs::remove_file(path);
        if let Some(backup) = backup {
            let _ = fs::rename(backup, path);
        }
    }
}

fn cleanup_staged_files(staged: &[(&Path, PathBuf)]) {
    for (_, temporary) in staged {
        let _ = fs::remove_file(temporary);
    }
}

fn append_json_lines<'a, T: Serialize + 'a>(
    lines: &mut Vec<String>,
    kind: ComparisonRowKind,
    rows: impl IntoIterator<Item = ComparisonRowWithFinality<'a, T>>,
) -> Result<(), ComparisonExportError> {
    for entry in rows {
        let metadata = entry.metadata;
        lines.push(serde_json::to_string(&json!({
            "schema": ROW_SCHEMA,
            "schemaVersion": SCHEMA_VERSION,
            "artifact": "result-row",
            "rowType": kind.as_str(),
            "rowId": metadata.row_id.as_str(),
            "finality": &metadata.finality,
            "reason": metadata.reason.as_str(),
            "version": metadata.version,
            "supersedesRowId": metadata.supersedes_row_id.as_deref(),
            "row": entry.row
        }))?);
    }
    Ok(())
}

fn write_json_lines<'a, W: Write, T: Serialize + 'a>(
    writer: &mut W,
    kind: ComparisonRowKind,
    rows: impl IntoIterator<Item = ComparisonRowWithFinality<'a, T>>,
) -> Result<(), ComparisonExportError> {
    for entry in rows {
        let metadata = entry.metadata;
        write_json_line(
            writer,
            &json!({
                "schema": ROW_SCHEMA,
                "schemaVersion": SCHEMA_VERSION,
                "artifact": "result-row",
                "rowType": kind.as_str(),
                "rowId": metadata.row_id.as_str(),
                "finality": &metadata.finality,
                "reason": metadata.reason.as_str(),
                "version": metadata.version,
                "supersedesRowId": metadata.supersedes_row_id.as_deref(),
                "row": entry.row
            }),
        )?;
    }
    Ok(())
}

fn write_json_line<W: Write, T: Serialize>(
    writer: &mut W,
    value: &T,
) -> Result<(), ComparisonExportError> {
    serde_json::to_writer(&mut *writer, value)?;
    writer.write_all(b"\n")?;
    Ok(())
}

fn build_plan_json_value(plan: &ComparisonPlan) -> Value {
    let target = plan.effective_target_selector();
    let against = plan.effective_against_selectors();

    json!({
        "schema": PLAN_SCHEMA,
        "schemaVersion": SCHEMA_VERSION,
        "artifact": "plan",
        "name": plan.name,
        "isStrict": plan.strict,
        "isSerializable": plan.is_serializable(),
        "target": selector_json(&target),
        "against": against
            .iter()
            .map(|selector| selector_json(selector))
            .collect::<Vec<_>>(),
        "scope": {
            "windowName": plan.scope_window,
            "key": plan.scope_key,
            "partition": plan.scope_partition,
            "timeAxis": format!("{:?}", plan.time_axis),
            "segmentFilters": plan.scope_segments.iter().map(|item| json!({
                "name": item.name,
                "value": item.value
            })).collect::<Vec<_>>(),
            "tagFilters": plan.scope_tags.iter().map(|item| json!({
                "name": item.name,
                "value": item.value
            })).collect::<Vec<_>>()
        },
        "normalization": {
            "requireClosedWindows": plan.require_closed_windows,
            "useHalfOpenRanges": plan.use_half_open_ranges,
            "timeAxis": format!("{:?}", plan.time_axis),
            "openWindowPolicy": format!("{:?}", plan.open_window_policy),
            "openWindowHorizon": plan.open_window_horizon.as_ref().map(|point| json!({
                "axis": format!("{:?}", point.axis()),
                "position": point.magnitude(),
                "clock": point.clock()
            })),
            "nullTimestampPolicy": format!("{:?}", plan.null_timestamp_policy),
            "coalesceAdjacentWindows": plan.coalesce_adjacent_windows,
            "duplicateWindowPolicy": match plan.duplicate_window_policy {
                ComparisonDuplicateWindowPolicy::Preserve => "Preserve",
                ComparisonDuplicateWindowPolicy::Reject => "Reject",
            },
            "knownAt": plan.known_at.as_ref().map(|point| json!({
                "axis": format!("{:?}", point.axis()),
                "position": point.magnitude(),
                "clock": point.clock()
            }))
        },
        "comparators": plan.comparators.iter().map(Comparator::declaration).collect::<Vec<_>>(),
        "output": {
            "includeAlignedSegments": plan.output.include_aligned_segments,
            "includeExplainData": plan.output.include_explain_data
        },
        "diagnostics": plan.validate()
    })
}

fn selector_json(selector: &ComparisonSelector) -> Value {
    let mut value = json!({
        "name": selector.name,
        "description": selector.description,
        "isSerializable": selector.is_serializable
    });
    if let Some(activity) = &selector.cohort_activity
        && let Some(object) = value.as_object_mut()
    {
        object.insert(
            "cohort".to_owned(),
            json!({
                "activity": activity.name(),
                "count": activity.count(),
                "sources": selector.cohort_sources
            }),
        );
    }
    if let Some(object) = value.as_object_mut() {
        object.insert("expression".to_owned(), selector.export_expression());
    }
    value
}

fn build_result_json_value(result: &ComparisonResult) -> Result<Value, ComparisonExportError> {
    Ok(json!({
        "schema": RESULT_SCHEMA,
        "schemaVersion": SCHEMA_VERSION,
        "artifact": "result",
        "isValid": result.is_valid,
        "knownAt": result.known_at,
        "evaluationHorizon": result.evaluation_horizon,
        "plan": build_plan_payload(&result.plan),
        "diagnostics": result.diagnostics,
        "prepared": result.prepared,
        "aligned": result.aligned,
        "comparatorSummaries": result.comparator_summaries,
        "rows": {
            "overlap": build_row_values(result.overlap_rows_with_finality()?)?,
            "residual": build_row_values(result.residual_rows_with_finality()?)?,
            "missing": build_row_values(result.missing_rows_with_finality()?)?,
            "coverage": build_row_values(result.coverage_rows_with_finality()?)?,
            "gap": build_row_values(result.gap_rows_with_finality()?)?,
            "symmetricDifference": build_row_values(result.symmetric_difference_rows_with_finality()?)?,
            "containment": build_row_values(result.containment_rows_with_finality()?)?,
            "leadLag": build_row_values(result.lead_lag_rows_with_finality()?)?,
            "asOf": build_row_values(result.as_of_rows_with_finality()?)?
        },
        "rowFinalities": result.row_finalities,
        "extensionMetadata": result.extension_metadata,
        "coverageSummaries": result.coverage_summaries,
        "leadLagSummaries": result.lead_lag_summaries
    }))
}

fn build_plan_payload(plan: &ComparisonPlan) -> Value {
    let mut value = build_plan_json_value(plan);
    if let Some(object) = value.as_object_mut() {
        object.remove("schema");
        object.remove("schemaVersion");
        object.remove("artifact");
    }
    value
}

fn build_row_values<'a, T: Serialize + 'a>(
    rows: impl IntoIterator<Item = ComparisonRowWithFinality<'a, T>>,
) -> Result<Vec<Value>, ComparisonExportError> {
    rows.into_iter()
        .map(|entry| {
            let mut object = match serde_json::to_value(entry.row)? {
                Value::Object(object) => object,
                _ => Map::new(),
            };
            object.insert(
                "rowId".to_owned(),
                Value::String(entry.metadata.row_id.clone()),
            );
            object.insert(
                "finality".to_owned(),
                serde_json::to_value(&entry.metadata.finality)?,
            );
            Ok(Value::Object(object))
        })
        .collect()
}

fn row_counts_json(result: &ComparisonResult) -> Value {
    json!({
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

fn prepared_len(result: &ComparisonResult, key: &str) -> usize {
    result
        .prepared
        .as_ref()
        .and_then(Value::as_object)
        .and_then(|value| value.get(key))
        .and_then(Value::as_array)
        .map_or(0, Vec::len)
}

fn aligned_len(result: &ComparisonResult) -> usize {
    result
        .aligned
        .as_ref()
        .and_then(Value::as_object)
        .and_then(|value| value.get("segments"))
        .and_then(Value::as_array)
        .map_or(0, Vec::len)
}

#[cfg(test)]
mod tests {
    use crate::{
        AgainstSelection, AsOfDirection, Comparator, ComparisonNullTimestampPolicy,
        ComparisonOutputOptions, ComparisonPlan, ComparisonResult, ContractFixture,
        LeadLagTransition, OpenWindowPolicy, TemporalAxis, TemporalPoint, WindowHistoryFixture,
        compare, compare_live,
    };

    use super::*;

    fn all_row_family_result() -> ComparisonResult {
        let history = WindowHistoryFixture::new()
            .closed_window("DeviceOffline", "device-1", 1, 5, |window| {
                window.source("provider-a")
            })
            .expect("first target")
            .closed_window("DeviceOffline", "device-1", 9, 11, |window| {
                window.source("provider-a")
            })
            .expect("second target")
            .closed_window("DeviceOffline", "device-1", 3, 7, |window| {
                window.source("provider-b")
            })
            .expect("first comparison")
            .closed_window("DeviceOffline", "device-1", 12, 13, |window| {
                window.source("provider-b")
            })
            .expect("second comparison")
            .build();
        let plan = ComparisonPlan {
            name: "All row families".to_owned(),
            selection: crate::comparison::ComparisonSelection::legacy(
                "provider-a",
                AgainstSelection::Sources(vec!["provider-b".to_owned()]),
            ),
            scope_window: Some("DeviceOffline".to_owned()),
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
                Comparator::Containment,
                Comparator::LeadLag {
                    transition: LeadLagTransition::Start,
                    axis: TemporalAxis::ProcessingPosition,
                    tolerance_magnitude: 100,
                },
                Comparator::AsOf {
                    direction: AsOfDirection::Previous,
                    axis: TemporalAxis::ProcessingPosition,
                    tolerance_magnitude: 100,
                },
            ],
            require_closed_windows: true,
            use_half_open_ranges: true,
            time_axis: TemporalAxis::ProcessingPosition,
            null_timestamp_policy: ComparisonNullTimestampPolicy::Reject,
            known_at: None,
            open_window_policy: OpenWindowPolicy::RequireClosed,
            open_window_horizon: None,
            coalesce_adjacent_windows: false,
            duplicate_window_policy: ComparisonDuplicateWindowPolicy::Preserve,
            output: ComparisonOutputOptions::default_options(),
            strict: false,
        };

        compare(&history, &plan)
    }

    fn association(row_type: &str, value: &Value) -> (String, String, Value) {
        (
            row_type.to_owned(),
            value["rowId"].as_str().expect("exported row ID").to_owned(),
            value["finality"].clone(),
        )
    }

    #[test]
    fn export_json_lines_streams_summary_and_rows() {
        let fixture = ContractFixture::parse_json(include_str!(
            "../../../../dotnet/tests/Spanfold.Tests/Comparison/Fixtures/basic-overlap.json"
        ))
        .expect("fixture");
        let result = fixture.execute();

        let lines = export_result_json_lines(&result).expect("json lines");
        assert_eq!(lines.len(), 5);
        assert!(lines[0].contains("\"artifact\":\"result-summary\""));
        assert!(lines[1].contains("\"rowId\":\"overlap:"));

        let mut streamed = Vec::new();
        write_result_json_lines(&result, &mut streamed).expect("streaming json lines");
        let streamed = String::from_utf8(streamed).expect("utf8");
        assert_eq!(streamed.lines().collect::<Vec<_>>(), lines);
    }

    #[test]
    fn all_row_families_share_authoritative_metadata_across_exports() {
        let result = all_row_family_result();
        let kinds = [
            ComparisonRowKind::Overlap,
            ComparisonRowKind::Residual,
            ComparisonRowKind::Missing,
            ComparisonRowKind::Coverage,
            ComparisonRowKind::Gap,
            ComparisonRowKind::SymmetricDifference,
            ComparisonRowKind::Containment,
            ComparisonRowKind::LeadLag,
            ComparisonRowKind::AsOf,
        ];
        macro_rules! assert_family_view {
            ($method:ident, $kind:expr) => {{
                let entries = result.$method().expect("valid row metadata layout");
                assert_ne!(entries.len(), 0, "{} rows", $kind.as_str());
                for entry in entries {
                    assert_eq!(entry.metadata.row_kind().expect("typed metadata"), $kind);
                }
            }};
        }
        assert_family_view!(overlap_rows_with_finality, ComparisonRowKind::Overlap);
        assert_family_view!(residual_rows_with_finality, ComparisonRowKind::Residual);
        assert_family_view!(missing_rows_with_finality, ComparisonRowKind::Missing);
        assert_family_view!(coverage_rows_with_finality, ComparisonRowKind::Coverage);
        assert_family_view!(gap_rows_with_finality, ComparisonRowKind::Gap);
        assert_family_view!(
            symmetric_difference_rows_with_finality,
            ComparisonRowKind::SymmetricDifference
        );
        assert_family_view!(
            containment_rows_with_finality,
            ComparisonRowKind::Containment
        );
        assert_family_view!(lead_lag_rows_with_finality, ComparisonRowKind::LeadLag);
        assert_family_view!(as_of_rows_with_finality, ComparisonRowKind::AsOf);

        let expected = result
            .row_finalities
            .iter()
            .map(|metadata| {
                (
                    metadata.row_type.clone(),
                    metadata.row_id.clone(),
                    serde_json::to_value(&metadata.finality).expect("finality value"),
                )
            })
            .collect::<Vec<_>>();

        let result_json: Value = serde_json::from_str(
            &export_result_json(&result).expect("result JSON with authoritative metadata"),
        )
        .expect("result JSON payload");
        let mut result_json_associations = Vec::new();
        for kind in kinds {
            for row in result_json["rows"][kind.as_str()]
                .as_array()
                .expect("typed result row array")
            {
                result_json_associations.push(association(kind.as_str(), row));
            }
        }
        assert_eq!(result_json_associations, expected);

        let json_lines = export_result_json_lines(&result).expect("JSON Lines");
        let json_line_associations = json_lines
            .iter()
            .skip(1)
            .map(|line| {
                let row: Value = serde_json::from_str(line).expect("JSON Line row");
                association(row["rowType"].as_str().expect("row type"), &row)
            })
            .collect::<Vec<_>>();
        assert_eq!(json_line_associations, expected);

        let mut streamed = Vec::new();
        write_result_json_lines(&result, &mut streamed).expect("streaming JSON Lines");
        let streamed = String::from_utf8(streamed).expect("UTF-8 JSON Lines");
        assert_eq!(streamed.lines().collect::<Vec<_>>(), json_lines);

        let llm_context: Value =
            serde_json::from_str(&export_result_llm_context(&result).expect("LLM context"))
                .expect("LLM context payload");
        let llm_associations = llm_context["rowDocuments"]
            .as_array()
            .expect("row documents")
            .iter()
            .skip(1)
            .map(|row| association(row["rowType"].as_str().expect("row type"), row))
            .collect::<Vec<_>>();
        assert_eq!(llm_associations, expected);
    }

    #[test]
    fn typed_views_and_exports_reject_detectable_metadata_corruption() {
        let mut missing_metadata = all_row_family_result();
        missing_metadata.row_finalities.pop();
        let missing_error = match missing_metadata.as_of_rows_with_finality() {
            Ok(_) => panic!("missing metadata must fail closed"),
            Err(error) => error,
        };
        assert_eq!(missing_error.family, ComparisonRowKind::AsOf);
        assert_eq!(
            missing_error.metadata_index,
            missing_metadata.row_finalities.len()
        );
        assert_eq!(
            missing_error.expected_count,
            missing_metadata.rows.as_of.len()
        );
        assert_eq!(
            missing_error.actual_count,
            missing_metadata.rows.as_of.len() - 1
        );
        assert_eq!(missing_error.expected_kind, ComparisonRowKind::AsOf);
        assert_eq!(missing_error.actual_kind, None);

        let mut partial_stream = Vec::new();
        let stream_error = write_result_json_lines(&missing_metadata, &mut partial_stream)
            .expect_err("missing metadata must fail streaming export");
        assert!(matches!(
            stream_error,
            ComparisonExportError::InconsistentRowMetadata(_)
        ));
        assert!(partial_stream.is_empty());

        let mut wrong_kind = all_row_family_result();
        let residual_index = wrong_kind.rows.overlap.len();
        wrong_kind.row_finalities[residual_index].row_type = "lead-lag".to_owned();
        let error = export_result_json(&wrong_kind).expect_err("wrong kind must fail export");
        let ComparisonExportError::InconsistentRowMetadata(error) = error else {
            panic!("unexpected export error: {error}");
        };
        assert_eq!(error.family, ComparisonRowKind::Residual);
        assert_eq!(error.metadata_index, residual_index);
        assert_eq!(error.expected_count, wrong_kind.rows.residual.len());
        assert_eq!(error.actual_count, wrong_kind.rows.residual.len() - 1);
        assert_eq!(error.expected_kind, ComparisonRowKind::Residual);
        assert_eq!(error.actual_kind.as_deref(), Some("lead-lag"));
    }

    #[test]
    fn provisional_typed_rows_keep_their_full_finality_envelope_in_json_lines() {
        let history = WindowHistoryFixture::new()
            .open_window("DeviceOffline", "device-1", 1, |window| {
                window.source("provider-a")
            })
            .expect("open target")
            .closed_window("DeviceOffline", "device-1", 3, 5, |window| {
                window.source("provider-b")
            })
            .expect("closed comparison")
            .build();
        let plan = ComparisonPlan {
            name: "Live finality export".to_owned(),
            selection: crate::comparison::ComparisonSelection::legacy(
                "provider-a",
                AgainstSelection::Sources(vec!["provider-b".to_owned()]),
            ),
            scope_window: Some("DeviceOffline".to_owned()),
            scope_key: None,
            scope_partition: None,
            scope_segments: Vec::new(),
            scope_tags: Vec::new(),
            comparators: vec![Comparator::Residual],
            require_closed_windows: false,
            use_half_open_ranges: true,
            time_axis: TemporalAxis::ProcessingPosition,
            null_timestamp_policy: ComparisonNullTimestampPolicy::Reject,
            known_at: None,
            open_window_policy: OpenWindowPolicy::ClipToHorizon,
            open_window_horizon: Some(TemporalPoint::position(10)),
            coalesce_adjacent_windows: false,
            duplicate_window_policy: ComparisonDuplicateWindowPolicy::Preserve,
            output: ComparisonOutputOptions::default_options(),
            strict: false,
        };
        let result = compare_live(&history, &plan, TemporalPoint::position(10));
        let typed_rows = result
            .residual_rows_with_finality()
            .expect("valid live row metadata")
            .collect::<Vec<_>>();
        assert!(!typed_rows.is_empty());
        assert!(
            typed_rows
                .iter()
                .all(|entry| { entry.metadata.finality == ComparisonFinality::Provisional })
        );

        let lines = export_result_json_lines(&result).expect("live JSON Lines");
        assert_eq!(lines.len(), typed_rows.len() + 1);
        for (line, entry) in lines.iter().skip(1).zip(typed_rows) {
            let row: Value = serde_json::from_str(line).expect("live JSON Line row");
            assert_eq!(row["rowType"], ComparisonRowKind::Residual.as_str());
            assert_eq!(row["rowId"], entry.metadata.row_id);
            assert_eq!(
                row["finality"],
                serde_json::to_value(&entry.metadata.finality).expect("finality value")
            );
            assert_eq!(row["reason"], entry.metadata.reason);
            assert_eq!(row["version"], entry.metadata.version);
            assert_eq!(
                row["supersedesRowId"],
                serde_json::to_value(&entry.metadata.supersedes_row_id)
                    .expect("supersession value")
            );
        }
    }

    #[test]
    fn debug_html_contains_audit_sections_and_capped_rows() {
        let fixture = ContractFixture::parse_json(include_str!(
            "../../../../dotnet/tests/Spanfold.Tests/Comparison/Fixtures/basic-overlap.json"
        ))
        .expect("fixture");
        let result = fixture.execute();

        let html = export_result_debug_html(&result);

        assert!(html.contains("Spanfold comparison debug"));
        assert!(html.contains("Window Timeline"));
        assert!(html.contains("Aligned Segments"));
        assert!(html.contains("Diagnostics"));
        assert!(html.contains("Extension Metadata"));
        assert!(html.contains("Comparator Rows"));
        assert!(html.contains("Markdown Summary"));
    }

    #[test]
    fn plan_json_rejects_runtime_only_selectors() {
        let mut plan = ComparisonPlan::new(
            "Runtime selector QA",
            "dynamic-target",
            AgainstSelection::Sources(vec!["provider-b".to_owned()]),
            vec![Comparator::Overlap],
        );
        plan.set_target_selector(crate::ComparisonSelector::runtime_only(
            "dynamic-target",
            "runtime target predicate",
            |_| true,
        ));
        plan.scope_window = Some("DeviceOffline".to_owned());
        plan.time_axis = crate::TemporalAxis::Timestamp;
        plan.null_timestamp_policy = crate::ComparisonNullTimestampPolicy::Exclude;

        let error = export_plan_json(&plan).expect_err("runtime selectors are not portable");

        assert!(matches!(error, ComparisonExportError::NonPortablePlan));
    }

    #[test]
    fn plan_json_reports_custom_output_options() {
        let plan = ComparisonPlan {
            name: "Output QA".to_owned(),
            selection: crate::comparison::ComparisonSelection::legacy(
                "provider-a",
                AgainstSelection::Sources(vec!["provider-b".to_owned()]),
            ),
            scope_window: Some("DeviceOffline".to_owned()),
            scope_key: None,
            scope_partition: None,
            scope_segments: Vec::new(),
            scope_tags: Vec::new(),
            comparators: vec![Comparator::Overlap],
            require_closed_windows: true,
            use_half_open_ranges: true,
            time_axis: crate::TemporalAxis::Timestamp,
            null_timestamp_policy: crate::ComparisonNullTimestampPolicy::Exclude,
            known_at: None,
            open_window_policy: crate::OpenWindowPolicy::RequireClosed,
            open_window_horizon: None,
            coalesce_adjacent_windows: false,
            duplicate_window_policy: ComparisonDuplicateWindowPolicy::Preserve,
            output: crate::ComparisonOutputOptions {
                include_aligned_segments: false,
                include_explain_data: false,
            },
            strict: false,
        };

        let json = export_plan_json(&plan).expect("plan json");
        let payload: Value = serde_json::from_str(&json).expect("plan payload");

        assert_eq!(payload["output"]["includeAlignedSegments"], false);
        assert_eq!(payload["output"]["includeExplainData"], false);
        assert_eq!(payload["normalization"]["timeAxis"], "Timestamp");
        assert_eq!(payload["normalization"]["nullTimestampPolicy"], "Exclude");
    }

    #[test]
    fn export_json_contains_row_finality_and_coverage_summaries() {
        let history = WindowHistoryFixture::new()
            .closed_window("DeviceOffline", "device-1", 1, 5, |w| {
                w.source("provider-a")
            })
            .expect("target")
            .closed_window("DeviceOffline", "device-1", 3, 7, |w| {
                w.source("provider-b")
            })
            .expect("against")
            .build();
        let plan = ComparisonPlan {
            name: "Provider QA".to_owned(),
            selection: crate::comparison::ComparisonSelection::legacy(
                "provider-a",
                AgainstSelection::Sources(vec!["provider-b".to_owned()]),
            ),
            scope_window: Some("DeviceOffline".to_owned()),
            scope_key: None,
            scope_partition: None,
            scope_segments: Vec::new(),
            scope_tags: Vec::new(),
            comparators: vec![
                Comparator::Overlap,
                Comparator::Residual,
                Comparator::Coverage,
            ],
            require_closed_windows: true,
            use_half_open_ranges: true,
            time_axis: crate::TemporalAxis::ProcessingPosition,
            null_timestamp_policy: crate::ComparisonNullTimestampPolicy::Reject,
            known_at: None,
            open_window_policy: crate::OpenWindowPolicy::RequireClosed,
            open_window_horizon: None,
            coalesce_adjacent_windows: false,
            duplicate_window_policy: ComparisonDuplicateWindowPolicy::Preserve,
            output: crate::ComparisonOutputOptions::default_options(),
            strict: false,
        };
        let result = compare(&history, &plan);

        let json = export_result_json(&result).expect("json");
        assert!(json.contains("\"coverageSummaries\""));
        assert!(json.contains("\"rowFinalities\""));
        assert!(json.contains("\"rowId\": \"overlap:"));
        assert!(json.contains("\"finality\": \"Final\""));
    }
}
