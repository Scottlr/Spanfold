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
    ComparisonResult, ComparisonSelector,
};

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
    write_files_atomically(&artifacts)?;
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
    write_json_line(&mut writer, &result_summary_json_value(result))?;
    write_json_lines(&mut writer, "overlap", &result.overlap_rows)?;
    write_json_lines(&mut writer, "residual", &result.residual_rows)?;
    write_json_lines(&mut writer, "missing", &result.missing_rows)?;
    write_json_lines(&mut writer, "coverage", &result.coverage_rows)?;
    write_json_lines(&mut writer, "gap", &result.gap_rows)?;
    write_json_lines(
        &mut writer,
        "symmetric-difference",
        &result.symmetric_difference_rows,
    )?;
    write_json_lines(&mut writer, "containment", &result.containment_rows)?;
    write_json_lines(&mut writer, "lead-lag", &result.lead_lag_rows)?;
    write_json_lines(&mut writer, "asof", &result.as_of_rows)?;
    Ok(())
}

fn append_result_json_lines(
    result: &ComparisonResult,
    lines: &mut Vec<String>,
) -> Result<(), serde_json::Error> {
    lines.push(serde_json::to_string(&result_summary_json_value(result))?);

    append_json_lines(lines, "overlap", &result.overlap_rows)?;
    append_json_lines(lines, "residual", &result.residual_rows)?;
    append_json_lines(lines, "missing", &result.missing_rows)?;
    append_json_lines(lines, "coverage", &result.coverage_rows)?;
    append_json_lines(lines, "gap", &result.gap_rows)?;
    append_json_lines(
        lines,
        "symmetric-difference",
        &result.symmetric_difference_rows,
    )?;
    append_json_lines(lines, "containment", &result.containment_rows)?;
    append_json_lines(lines, "lead-lag", &result.lead_lag_rows)?;
    append_json_lines(lines, "asof", &result.as_of_rows)?;
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
    for (row_type, rows) in [
        (
            "overlap",
            build_row_values("overlap", &result.overlap_rows, &result.row_finalities)?,
        ),
        (
            "residual",
            build_row_values("residual", &result.residual_rows, &result.row_finalities)?,
        ),
        (
            "missing",
            build_row_values("missing", &result.missing_rows, &result.row_finalities)?,
        ),
        (
            "coverage",
            build_row_values("coverage", &result.coverage_rows, &result.row_finalities)?,
        ),
        (
            "gap",
            build_row_values("gap", &result.gap_rows, &result.row_finalities)?,
        ),
        (
            "symmetric-difference",
            build_row_values(
                "symmetricDifference",
                &result.symmetric_difference_rows,
                &result.row_finalities,
            )?,
        ),
        (
            "containment",
            build_row_values(
                "containment",
                &result.containment_rows,
                &result.row_finalities,
            )?,
        ),
        (
            "lead-lag",
            build_row_values("leadLag", &result.lead_lag_rows, &result.row_finalities)?,
        ),
        (
            "asof",
            build_row_values("asOf", &result.as_of_rows, &result.row_finalities)?,
        ),
    ] {
        row_documents.extend(rows.into_iter().map(|row| {
            let mut object = match row {
                Value::Object(object) => object,
                _ => Map::new(),
            };
            object.insert("rowType".to_owned(), Value::String(row_type.to_owned()));
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
    text.push_str(&format!("- overlap rows: {}\n", result.overlap_rows.len()));
    text.push_str(&format!(
        "- residual rows: {}\n",
        result.residual_rows.len()
    ));
    text.push_str(&format!("- missing rows: {}\n", result.missing_rows.len()));
    text.push_str(&format!(
        "- coverage rows: {}\n",
        result.coverage_rows.len()
    ));
    text.push_str(&format!("- gap rows: {}\n", result.gap_rows.len()));
    text.push_str(&format!(
        "- symmetric difference rows: {}\n",
        result.symmetric_difference_rows.len()
    ));
    text.push_str(&format!(
        "- containment rows: {}\n",
        result.containment_rows.len()
    ));
    text.push_str(&format!(
        "- lead lag rows: {}\n",
        result.lead_lag_rows.len()
    ));
    text.push_str(&format!("- as of rows: {}\n", result.as_of_rows.len()));
    text.push_str(&format!(
        "- row finalities: {}\n\n",
        result.row_finalities.len()
    ));

    text.push_str("## Row Evidence\n\n");
    append_markdown_rows(&mut text, "overlap", &result.overlap_rows);
    append_markdown_rows(&mut text, "residual", &result.residual_rows);
    append_markdown_rows(&mut text, "missing", &result.missing_rows);
    append_markdown_rows(&mut text, "coverage", &result.coverage_rows);
    append_markdown_rows(&mut text, "gap", &result.gap_rows);
    append_markdown_rows(
        &mut text,
        "symmetricDifference",
        &result.symmetric_difference_rows,
    );
    append_markdown_rows(&mut text, "containment", &result.containment_rows);
    append_markdown_rows(&mut text, "leadLag", &result.lead_lag_rows);
    append_markdown_rows(&mut text, "asOf", &result.as_of_rows);
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

/// Exports a comparison result as self-contained debug HTML.
pub fn export_result_debug_html(result: &ComparisonResult) -> String {
    let mut html = String::with_capacity(32 * 1024);
    let row_count = result.overlap_rows.len()
        + result.residual_rows.len()
        + result.missing_rows.len()
        + result.coverage_rows.len()
        + result.gap_rows.len()
        + result.symmetric_difference_rows.len()
        + result.containment_rows.len()
        + result.lead_lag_rows.len()
        + result.as_of_rows.len();
    let provisional_rows = result
        .row_finalities
        .iter()
        .filter(|row| row.finality == ComparisonFinality::Provisional)
        .count();

    write!(
        html,
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>{} - Spanfold debug</title><style>{}</style></head><body><main>",
        escape_html(&result.plan_name),
        DEBUG_HTML_STYLE
    )
    .expect("write debug html");
    write!(
        html,
        "<section class=\"hero\"><div class=\"eyebrow\">Spanfold comparison debug</div><h1>{}</h1><p class=\"lead\">Inspect selected windows, aligned segments, comparator rows, finality, diagnostics, and extension metadata.</p><div class=\"badges\"><span class=\"badge {}\">{}</span>",
        escape_html(&result.plan_name),
        if result.is_valid { "valid" } else { "invalid" },
        if result.is_valid {
            "Valid result"
        } else {
            "Invalid result"
        }
    )
    .expect("write debug html");
    if let Some(horizon) = result.evaluation_horizon.as_ref() {
        write!(
            html,
            "<span class=\"badge live\">Live horizon {}</span>",
            horizon.magnitude
        )
        .expect("write debug html");
    }
    if let Some(known_at) = result.known_at.as_ref() {
        write!(
            html,
            "<span class=\"badge\">Known at {}</span>",
            known_at.magnitude
        )
        .expect("write debug html");
    }
    html.push_str("</div></section>");

    html.push_str("<section class=\"grid\" aria-label=\"Comparison summary\">");
    append_debug_card(
        &mut html,
        "Selected windows",
        prepared_len(result, "selectedWindows"),
    );
    append_debug_card(
        &mut html,
        "Normalized windows",
        prepared_len(result, "normalizedWindows"),
    );
    append_debug_card(&mut html, "Aligned segments", aligned_len(result));
    append_debug_card(&mut html, "Result rows", row_count);
    append_debug_card(&mut html, "Diagnostics", result.diagnostics.len());
    append_debug_card(&mut html, "Provisional rows", provisional_rows);
    append_debug_card(&mut html, "Comparators", result.comparator_summaries.len());
    append_debug_card(
        &mut html,
        "Excluded windows",
        prepared_len(result, "excludedWindows"),
    );
    html.push_str("</section>");

    append_debug_json_section(
        &mut html,
        "Window Timeline",
        "Normalized windows after selector, scope, known-at, and open-window policy have been applied.",
        result
            .prepared
            .as_ref()
            .and_then(|value| value.get("normalizedWindows")),
        80,
        "normalized windows",
    );
    append_debug_json_section(
        &mut html,
        "Aligned Segments",
        "Prepared target and against windows after boundary alignment.",
        result
            .aligned
            .as_ref()
            .and_then(|value| value.get("segments")),
        80,
        "aligned segments",
    );
    append_debug_diagnostics(&mut html, result);
    append_debug_metadata(&mut html, result);
    append_debug_rows(&mut html, result);
    append_debug_pretty_json(&mut html, "Prepared", result.prepared.as_ref());
    append_debug_pretty_json(&mut html, "Aligned", result.aligned.as_ref());
    write!(
        html,
        "<section class=\"panel\"><h2>Markdown Summary</h2><pre>{}</pre></section>",
        escape_html(&export_result_markdown(result))
    )
    .expect("write debug html");
    html.push_str("</main></body></html>");
    html
}

const DEBUG_HTML_STYLE: &str = r#"
body{margin:0;background:#f6f4ee;color:#26231f;font:14px/1.5 ui-sans-serif,system-ui,-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif}
main{width:min(1220px,calc(100% - 32px));margin:0 auto;padding:28px 0 48px}
.hero,.panel{background:#fffaf0;border:1px solid #d8cfb9;border-radius:8px}.hero{padding:28px;border-top:4px solid #3e6b5c}
.eyebrow{color:#3e6b5c;font-size:12px;font-weight:700;letter-spacing:.08em;text-transform:uppercase}h1,h2,h3,p{margin:0}h1{margin-top:8px;font-size:40px;line-height:1.02}.lead{max-width:760px;margin-top:14px;color:#6b6659;font-size:16px}
.badges{display:flex;flex-wrap:wrap;gap:8px;margin-top:20px}.badge{display:inline-flex;min-height:28px;align-items:center;padding:4px 10px;border:1px solid #d8cfb9;border-radius:8px;background:#eee5cf;font-weight:650}.badge.valid{border-color:#3e6b5c;color:#3e6b5c}.badge.invalid{border-color:#b8742a;color:#b8742a}.badge.live{border-color:#c97a3a;color:#9c5525}
.grid{display:grid;grid-template-columns:repeat(4,minmax(0,1fr));gap:12px;margin-top:18px}.card{min-height:86px;padding:16px;border:1px solid #d8cfb9;border-radius:8px;background:#fffaf0}.value{margin-top:6px;font-size:30px;font-weight:760;line-height:1}.label{color:#6b6659;font-size:12px;font-weight:700;text-transform:uppercase}
.panel{margin-top:18px;padding:22px}.section-note{max-width:760px;margin-top:6px;color:#6b6659}.table-wrap{overflow-x:auto;border:1px solid #d8cfb9;border-radius:8px}table{width:100%;border-collapse:collapse;background:#fffaf0}th,td{padding:10px 12px;border-bottom:1px solid #d8cfb9;text-align:left;vertical-align:top}th{color:#6b6659;font-size:12px;font-weight:750;text-transform:uppercase}tr:last-child td{border-bottom:0}
.mono,pre{font-family:ui-monospace,SFMono-Regular,Menlo,Monaco,Consolas,"Liberation Mono",monospace;font-size:12px}pre{white-space:pre-wrap;word-break:break-word;background:#25231f;color:#f3ead6;border-radius:8px;padding:14px}.empty{padding:18px;border:1px dashed #d8cfb9;border-radius:8px;color:#6b6659;background:#fffaf0}.severity-error{color:#b8742a;font-weight:720}.severity-warning{color:#9c5525;font-weight:720}
@media(max-width:820px){main{width:min(100% - 20px,1220px);padding-top:16px}.hero,.panel{padding:18px}.grid{grid-template-columns:repeat(2,minmax(0,1fr))}}@media(max-width:520px){.grid{grid-template-columns:1fr}h1{font-size:30px}}
"#;

fn append_debug_card(html: &mut String, label: &str, value: usize) {
    write!(
        html,
        "<div class=\"card\"><div class=\"label\">{}</div><div class=\"value\">{}</div></div>",
        escape_html(label),
        value
    )
    .expect("write debug html");
}

fn append_debug_json_section(
    html: &mut String,
    title: &str,
    note: &str,
    value: Option<&Value>,
    max_rows: usize,
    unit: &str,
) {
    write!(
        html,
        "<section class=\"panel\"><h2>{}</h2><p class=\"section-note\">{}</p>",
        escape_html(title),
        escape_html(note)
    )
    .expect("write debug html");
    match value.and_then(Value::as_array) {
        Some(rows) if !rows.is_empty() => {
            html.push_str("<div class=\"table-wrap\" style=\"margin-top:18px\"><table><tbody>");
            for row in rows.iter().take(max_rows) {
                write!(
                    html,
                    "<tr><td class=\"mono\"><pre>{}</pre></td></tr>",
                    escape_html(
                        &serde_json::to_string_pretty(row)
                            .unwrap_or_else(|_| "<serialization-error>".to_owned())
                    )
                )
                .expect("write debug html");
            }
            if rows.len() > max_rows {
                write!(
                    html,
                    "<tr><td>Showing first {} of {} {}.</td></tr>",
                    max_rows,
                    rows.len(),
                    escape_html(unit)
                )
                .expect("write debug html");
            }
            html.push_str("</tbody></table></div>");
        }
        _ => {
            html.push_str("<div class=\"empty\" style=\"margin-top:18px\">No data available.</div>")
        }
    }
    html.push_str("</section>");
}

fn append_debug_diagnostics(html: &mut String, result: &ComparisonResult) {
    html.push_str("<section class=\"panel\"><h2>Diagnostics</h2>");
    if result.diagnostics.is_empty() {
        html.push_str(
            "<div class=\"empty\" style=\"margin-top:18px\">No diagnostics.</div></section>",
        );
        return;
    }
    html.push_str("<div class=\"table-wrap\" style=\"margin-top:18px\"><table><tbody>");
    for diagnostic in result.diagnostics.iter().take(120) {
        let value = serde_json::to_value(diagnostic).unwrap_or(Value::Null);
        write!(
            html,
            "<tr><td class=\"mono\"><pre>{}</pre></td></tr>",
            escape_html(
                &serde_json::to_string_pretty(&value)
                    .unwrap_or_else(|_| "<serialization-error>".to_owned())
            )
        )
        .expect("write debug html");
    }
    if result.diagnostics.len() > 120 {
        write!(
            html,
            "<tr><td>Showing first 120 of {} diagnostics.</td></tr>",
            result.diagnostics.len()
        )
        .expect("write debug html");
    }
    html.push_str("</tbody></table></div></section>");
}

fn append_debug_metadata(html: &mut String, result: &ComparisonResult) {
    html.push_str("<section class=\"panel\"><h2>Extension Metadata</h2>");
    if result.extension_metadata.is_empty() {
        html.push_str(
            "<div class=\"empty\" style=\"margin-top:18px\">No extension metadata.</div></section>",
        );
        return;
    }
    html.push_str("<div class=\"table-wrap\" style=\"margin-top:18px\"><table><tbody>");
    for item in result.extension_metadata.iter().take(120) {
        let value = serde_json::to_value(item).unwrap_or(Value::Null);
        write!(
            html,
            "<tr><td class=\"mono\"><pre>{}</pre></td></tr>",
            escape_html(
                &serde_json::to_string_pretty(&value)
                    .unwrap_or_else(|_| "<serialization-error>".to_owned())
            )
        )
        .expect("write debug html");
    }
    if result.extension_metadata.len() > 120 {
        write!(
            html,
            "<tr><td>Showing first 120 of {} metadata items.</td></tr>",
            result.extension_metadata.len()
        )
        .expect("write debug html");
    }
    html.push_str("</tbody></table></div></section>");
}

fn append_debug_rows(html: &mut String, result: &ComparisonResult) {
    html.push_str("<section class=\"panel\"><h2>Comparator Rows</h2>");
    append_debug_row_table(html, "overlap", &result.overlap_rows);
    append_debug_row_table(html, "residual", &result.residual_rows);
    append_debug_row_table(html, "missing", &result.missing_rows);
    append_debug_row_table(html, "coverage", &result.coverage_rows);
    append_debug_row_table(html, "gap", &result.gap_rows);
    append_debug_row_table(
        html,
        "symmetric-difference",
        &result.symmetric_difference_rows,
    );
    append_debug_row_table(html, "containment", &result.containment_rows);
    append_debug_row_table(html, "lead-lag", &result.lead_lag_rows);
    append_debug_row_table(html, "as-of", &result.as_of_rows);
    if result.row_finalities.is_empty() {
        html.push_str("<div class=\"empty\" style=\"margin-top:18px\">No row finalities.</div>");
    } else {
        append_debug_row_table(html, "row-finality", &result.row_finalities);
    }
    html.push_str("</section>");
}

fn append_debug_row_table<T: Serialize>(html: &mut String, label: &str, rows: &[T]) {
    const MAX_ROWS_PER_TYPE: usize = 200;
    if rows.is_empty() {
        return;
    }
    write!(
        html,
        "<h3 style=\"margin-top:18px\">{}</h3><div class=\"table-wrap\"><table><tbody>",
        escape_html(label)
    )
    .expect("write debug html");
    for row in rows.iter().take(MAX_ROWS_PER_TYPE) {
        let value = serde_json::to_value(row).unwrap_or(Value::Null);
        write!(
            html,
            "<tr><td class=\"mono\"><pre>{}</pre></td></tr>",
            escape_html(
                &serde_json::to_string_pretty(&value)
                    .unwrap_or_else(|_| "<serialization-error>".to_owned())
            )
        )
        .expect("write debug html");
    }
    if rows.len() > MAX_ROWS_PER_TYPE {
        write!(
            html,
            "<tr><td>{}: showing {} of {} rows.</td></tr>",
            escape_html(label),
            MAX_ROWS_PER_TYPE,
            rows.len()
        )
        .expect("write debug html");
    }
    html.push_str("</tbody></table></div>");
}

fn append_debug_pretty_json(html: &mut String, title: &str, value: Option<&Value>) {
    let payload = value.cloned().unwrap_or(Value::Null);
    write!(
        html,
        "<section class=\"panel\"><h2>{}</h2><pre>{}</pre></section>",
        escape_html(title),
        escape_html(
            &serde_json::to_string_pretty(&payload)
                .unwrap_or_else(|_| "<serialization-error>".to_owned())
        )
    )
    .expect("write debug html");
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn write_files_atomically(files: &[(&Path, &[u8])]) -> Result<(), std::io::Error> {
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
    for (path, temporary) in &staged {
        if let Err(error) = fs::rename(temporary, path) {
            for (_, remaining) in &staged {
                let _ = fs::remove_file(remaining);
            }
            return Err(error);
        }
    }
    Ok(())
}

fn append_json_lines<T: Serialize>(
    lines: &mut Vec<String>,
    row_type: &str,
    rows: &[T],
) -> Result<(), serde_json::Error> {
    for row in rows {
        let row_id = crate::comparison::stable_row_id_for_export(row_type, row);
        lines.push(serde_json::to_string(&json!({
            "schema": ROW_SCHEMA,
            "schemaVersion": SCHEMA_VERSION,
            "artifact": "result-row",
            "rowType": row_type,
            "rowId": row_id,
            "row": row
        }))?);
    }
    Ok(())
}

fn write_json_lines<W: Write, T: Serialize>(
    writer: &mut W,
    row_type: &str,
    rows: &[T],
) -> Result<(), ComparisonExportError> {
    for row in rows {
        let row_id = crate::comparison::stable_row_id_for_export(row_type, row);
        write_json_line(
            writer,
            &json!({
                "schema": ROW_SCHEMA,
                "schemaVersion": SCHEMA_VERSION,
                "artifact": "result-row",
                "rowType": row_type,
                "rowId": row_id,
                "row": row
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
        "against": against.iter().map(selector_json).collect::<Vec<_>>(),
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
            "overlap": build_row_values("overlap", &result.overlap_rows, &result.row_finalities)?,
            "residual": build_row_values("residual", &result.residual_rows, &result.row_finalities)?,
            "missing": build_row_values("missing", &result.missing_rows, &result.row_finalities)?,
            "coverage": build_row_values("coverage", &result.coverage_rows, &result.row_finalities)?,
            "gap": build_row_values("gap", &result.gap_rows, &result.row_finalities)?,
            "symmetricDifference": build_row_values("symmetricDifference", &result.symmetric_difference_rows, &result.row_finalities)?,
            "containment": build_row_values("containment", &result.containment_rows, &result.row_finalities)?,
            "leadLag": build_row_values("leadLag", &result.lead_lag_rows, &result.row_finalities)?,
            "asOf": build_row_values("asOf", &result.as_of_rows, &result.row_finalities)?
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

fn build_row_values<T: Serialize>(
    row_type: &str,
    rows: &[T],
    finalities: &[crate::ComparisonRowFinality],
) -> Result<Vec<Value>, ComparisonExportError> {
    let finality_by_id = finalities
        .iter()
        .map(|item| {
            (
                (item.row_type.as_str(), item.row_id.as_str()),
                &item.finality,
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    rows.iter()
        .map(|row| {
            let mut object = match serde_json::to_value(row)? {
                Value::Object(object) => object,
                _ => Map::new(),
            };
            let row_id = crate::comparison::stable_row_id_for_export(row_type, row);
            let finality = finality_by_id
                .get(&(row_type, row_id.as_str()))
                .map(|item| (*item).clone())
                .unwrap_or(ComparisonFinality::Final);
            object.insert("rowId".to_owned(), Value::String(row_id));
            object.insert("finality".to_owned(), serde_json::to_value(finality)?);
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
        AgainstSelection, Comparator, ComparisonPlan, ContractFixture, WindowHistoryFixture,
        compare,
    };

    use super::*;

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
        let plan = ComparisonPlan {
            name: "Runtime selector QA".to_owned(),
            target_source: "dynamic-target".to_owned(),
            against: AgainstSelection::Sources(vec!["provider-b".to_owned()]),
            target_selector: Some(crate::ComparisonSelector::runtime_only(
                "dynamic-target",
                "runtime target predicate",
                |_| true,
            )),
            against_selectors: vec![crate::ComparisonSelector::for_source("provider-b")],
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
            output: crate::ComparisonOutputOptions::default_options(),
            strict: false,
        };

        let error = export_plan_json(&plan).expect_err("runtime selectors are not portable");

        assert!(matches!(error, ComparisonExportError::NonPortablePlan));
    }

    #[test]
    fn plan_json_reports_custom_output_options() {
        let plan = ComparisonPlan {
            name: "Output QA".to_owned(),
            target_source: "provider-a".to_owned(),
            against: AgainstSelection::Sources(vec!["provider-b".to_owned()]),
            target_selector: None,
            against_selectors: Vec::new(),
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
            target_source: "provider-a".to_owned(),
            against: AgainstSelection::Sources(vec!["provider-b".to_owned()]),
            target_selector: None,
            against_selectors: Vec::new(),
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
