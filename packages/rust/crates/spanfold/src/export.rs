use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::Serialize;
use serde_json::{Map, Value, json};
use thiserror::Error;

use crate::{AgainstSelection, Comparator, ComparisonFinality, ComparisonPlan, ComparisonResult};

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
}

/// Configures optional debug HTML export during comparison execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComparisonDebugHtmlOptions {
    enabled: bool,
    path: Option<PathBuf>,
}

impl ComparisonDebugHtmlOptions {
    /// Returns options that do not write a debug HTML artifact.
    #[must_use]
    pub const fn disabled() -> Self {
        Self {
            enabled: false,
            path: None,
        }
    }

    /// Creates options that write a debug HTML artifact to a file.
    #[must_use]
    pub fn to_file(path: impl Into<PathBuf>) -> Self {
        Self {
            enabled: true,
            path: Some(path.into()),
        }
    }

    /// Gets whether debug HTML export is enabled.
    #[must_use]
    pub const fn enabled(&self) -> bool {
        self.enabled
    }

    /// Gets the destination file path when export is enabled.
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub(crate) fn export_if_enabled(
        &self,
        result: &ComparisonResult,
    ) -> Result<(), ComparisonExportError> {
        if let Some(path) = self.enabled.then_some(()).and(self.path.as_deref()) {
            write_file(path, export_result_debug_html(result).as_bytes())?;
        }
        Ok(())
    }
}

/// Configures optional LLM context export during comparison execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComparisonLlmContextOptions {
    enabled: bool,
    path: Option<PathBuf>,
}

impl ComparisonLlmContextOptions {
    /// Returns options that do not write an LLM context artifact.
    #[must_use]
    pub const fn disabled() -> Self {
        Self {
            enabled: false,
            path: None,
        }
    }

    /// Creates options that write deterministic LLM context JSON to a file.
    #[must_use]
    pub fn to_file(path: impl Into<PathBuf>) -> Self {
        Self {
            enabled: true,
            path: Some(path.into()),
        }
    }

    /// Gets whether LLM context export is enabled.
    #[must_use]
    pub const fn enabled(&self) -> bool {
        self.enabled
    }

    /// Gets the destination file path when export is enabled.
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub(crate) fn export_if_enabled(
        &self,
        result: &ComparisonResult,
    ) -> Result<(), ComparisonExportError> {
        if let Some(path) = self.enabled.then_some(()).and(self.path.as_deref()) {
            write_file(path, export_result_llm_context(result)?.as_bytes())?;
        }
        Ok(())
    }
}

/// Exports a comparison plan as deterministic JSON.
pub fn export_plan_json(plan: &ComparisonPlan) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(&build_plan_json_value(plan))
}

/// Exports a comparison result as deterministic JSON.
pub fn export_result_json(result: &ComparisonResult) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(&build_result_json_value(result))
}

/// Exports a comparison result as deterministic JSON Lines.
pub fn export_result_json_lines(
    result: &ComparisonResult,
) -> Result<Vec<String>, serde_json::Error> {
    let mut lines = vec![serde_json::to_string(&json!({
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
    }))?];

    append_json_lines(&mut lines, "overlap", &result.overlap_rows)?;
    append_json_lines(&mut lines, "residual", &result.residual_rows)?;
    append_json_lines(&mut lines, "missing", &result.missing_rows)?;
    append_json_lines(&mut lines, "coverage", &result.coverage_rows)?;
    append_json_lines(&mut lines, "gap", &result.gap_rows)?;
    append_json_lines(
        &mut lines,
        "symmetric-difference",
        &result.symmetric_difference_rows,
    )?;
    append_json_lines(&mut lines, "containment", &result.containment_rows)?;
    append_json_lines(&mut lines, "lead-lag", &result.lead_lag_rows)?;
    append_json_lines(&mut lines, "asof", &result.as_of_rows)?;
    Ok(lines)
}

/// Exports a comparison result as deterministic LLM context JSON.
pub fn export_result_llm_context(result: &ComparisonResult) -> Result<String, serde_json::Error> {
    let row_documents = export_result_json_lines(result)?
        .into_iter()
        .map(|line| serde_json::from_str::<Value>(&line))
        .collect::<Result<Vec<_>, _>>()?;

    serde_json::to_string_pretty(&json!({
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
        "fullResult": build_result_json_value(result),
        "rowDocuments": row_documents
    }))
}

/// Exports a comparison result as deterministic Markdown.
pub fn export_result_markdown(result: &ComparisonResult) -> String {
    let mut text = format!("# {}\n\nvalid: {}\n\n", result.plan_name, result.is_valid);
    if let Some(known_at) = result.known_at {
        text.push_str(&format!(
            "known at: {:?}:{}\n\n",
            known_at.axis, known_at.magnitude
        ));
    }
    if let Some(horizon) = result.evaluation_horizon {
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
                diagnostic.severity, diagnostic.code
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

    if !result.comparator_summaries.is_empty() {
        text.push_str("## Comparator Summaries\n\n");
        for summary in &result.comparator_summaries {
            text.push_str(&format!(
                "- {} rows={}\n",
                summary.comparator_name, summary.row_count
            ));
        }
        text.push('\n');
    }

    if !result.coverage_summaries.is_empty() {
        text.push_str("## Coverage Summaries\n\n");
        for summary in &result.coverage_summaries {
            text.push_str(&format!(
                "- {} {} ratio={:.6}\n",
                summary.window_name, summary.key, summary.coverage_ratio
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
                item.extension_id, item.key, item.value
            ));
        }
    }

    text
}

/// Exports a comparison result as self-contained debug HTML.
pub fn export_result_debug_html(result: &ComparisonResult) -> String {
    let prepared = result.prepared.as_ref().cloned().unwrap_or(Value::Null);
    let aligned = result.aligned.as_ref().cloned().unwrap_or(Value::Null);
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>{}</title><style>body{{font-family:ui-sans-serif,system-ui,sans-serif;margin:24px;color:#111827;background:#f8fafc}}section{{margin:24px 0}}pre{{white-space:pre-wrap;word-break:break-word;background:#fff;border:1px solid #d1d5db;padding:12px}}table{{border-collapse:collapse;width:100%;background:#fff}}th,td{{border:1px solid #d1d5db;padding:6px 8px;text-align:left;vertical-align:top}}th{{background:#e5e7eb}}</style></head><body><h1>{}</h1><p>Visual audit of selected windows, aligned segments, comparator rows, finality, and extension metadata.</p><section><table><tbody><tr><th>Overlap</th><td>{}</td></tr><tr><th>Residual</th><td>{}</td></tr><tr><th>Missing</th><td>{}</td></tr><tr><th>Coverage</th><td>{}</td></tr><tr><th>Gap</th><td>{}</td></tr><tr><th>Symmetric difference</th><td>{}</td></tr><tr><th>Containment</th><td>{}</td></tr><tr><th>Lead lag</th><td>{}</td></tr><tr><th>As of</th><td>{}</td></tr><tr><th>Finalities</th><td>{}</td></tr></tbody></table></section><section><h2>Prepared</h2><pre>{}</pre></section><section><h2>Aligned</h2><pre>{}</pre></section><section><h2>Markdown Summary</h2><pre>{}</pre></section></body></html>",
        result.plan_name,
        result.plan_name,
        result.overlap_rows.len(),
        result.residual_rows.len(),
        result.missing_rows.len(),
        result.coverage_rows.len(),
        result.gap_rows.len(),
        result.symmetric_difference_rows.len(),
        result.containment_rows.len(),
        result.lead_lag_rows.len(),
        result.as_of_rows.len(),
        result.row_finalities.len(),
        serde_json::to_string_pretty(&prepared).unwrap_or_default(),
        serde_json::to_string_pretty(&aligned).unwrap_or_default(),
        export_result_markdown(result)
    )
}

fn write_file(path: &Path, content: &[u8]) -> Result<(), std::io::Error> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, content)
}

fn append_json_lines<T: Serialize>(
    lines: &mut Vec<String>,
    row_type: &str,
    rows: &[T],
) -> Result<(), serde_json::Error> {
    for (index, row) in rows.iter().enumerate() {
        lines.push(serde_json::to_string(&json!({
            "schema": ROW_SCHEMA,
            "schemaVersion": SCHEMA_VERSION,
            "artifact": "result-row",
            "rowType": row_type,
            "rowId": format!("{row_type}[{index}]"),
            "row": row
        }))?);
    }
    Ok(())
}

fn build_plan_json_value(plan: &ComparisonPlan) -> Value {
    let against = match &plan.against {
        AgainstSelection::Sources(sources) => sources
            .iter()
            .map(|source| json!({"name": source, "description": source, "isSerializable": true}))
            .collect::<Vec<_>>(),
        AgainstSelection::Cohort {
            name,
            sources,
            activity,
        } => vec![json!({
            "name": name,
            "description": name,
            "isSerializable": true,
            "cohort": {
                "activity": activity.name(),
                "count": activity.count(),
                "sources": sources
            }
        })],
    };

    json!({
        "schema": PLAN_SCHEMA,
        "schemaVersion": SCHEMA_VERSION,
        "artifact": "plan",
        "name": plan.name,
        "isStrict": plan.strict,
        "isSerializable": true,
        "target": {
            "name": plan.target_source,
            "description": plan.target_source,
            "isSerializable": true
        },
        "against": against,
        "scope": {
            "windowName": plan.scope_window,
            "timeAxis": "ProcessingPosition",
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
            "requireClosedWindows": plan.open_window_policy == crate::OpenWindowPolicy::RequireClosed,
            "useHalfOpenRanges": true,
            "timeAxis": "ProcessingPosition",
            "openWindowPolicy": format!("{:?}", plan.open_window_policy),
            "openWindowHorizon": plan.open_window_horizon.map(|point| json!({
                "axis": format!("{:?}", point.axis()),
                "position": point.magnitude()
            })),
            "nullTimestampPolicy": "Reject",
            "coalesceAdjacentWindows": false,
            "duplicateWindowPolicy": "Keep",
            "knownAt": plan.known_at.map(|point| json!({
                "axis": format!("{:?}", point.axis()),
                "position": point.magnitude()
            }))
        },
        "comparators": plan.comparators.iter().map(Comparator::declaration).collect::<Vec<_>>(),
        "output": {
            "includeAlignedSegments": true,
            "includeExplainData": true
        },
        "diagnostics": []
    })
}

fn build_result_json_value(result: &ComparisonResult) -> Value {
    json!({
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
            "overlap": build_row_values("overlap", &result.overlap_rows, &result.row_finalities),
            "residual": build_row_values("residual", &result.residual_rows, &result.row_finalities),
            "missing": build_row_values("missing", &result.missing_rows, &result.row_finalities),
            "coverage": build_row_values("coverage", &result.coverage_rows, &result.row_finalities),
            "gap": build_row_values("gap", &result.gap_rows, &result.row_finalities),
            "symmetricDifference": build_row_values("symmetricDifference", &result.symmetric_difference_rows, &result.row_finalities),
            "containment": build_row_values("containment", &result.containment_rows, &result.row_finalities),
            "leadLag": build_row_values("leadLag", &result.lead_lag_rows, &result.row_finalities),
            "asOf": build_row_values("asOf", &result.as_of_rows, &result.row_finalities)
        },
        "rowFinalities": result.row_finalities,
        "extensionMetadata": result.extension_metadata,
        "coverageSummaries": result.coverage_summaries,
        "leadLagSummaries": result.lead_lag_summaries
    })
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
) -> Vec<Value> {
    rows.iter()
        .enumerate()
        .map(|(index, row)| {
            let mut object = match serde_json::to_value(row).unwrap_or(Value::Null) {
                Value::Object(object) => object,
                _ => Map::new(),
            };
            let row_id = format!("{row_type}[{index}]");
            let finality = finalities
                .iter()
                .find(|item| item.row_type == row_type && item.row_id == row_id)
                .map(|item| item.finality.clone())
                .unwrap_or(ComparisonFinality::Final);
            object.insert("rowId".to_owned(), Value::String(row_id));
            object.insert(
                "finality".to_owned(),
                serde_json::to_value(finality).unwrap_or(Value::String("Final".to_owned())),
            );
            Value::Object(object)
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
        assert!(lines[1].contains("\"rowId\":\"overlap[0]\""));
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
            scope_window: Some("DeviceOffline".to_owned()),
            scope_segments: Vec::new(),
            scope_tags: Vec::new(),
            comparators: vec![
                Comparator::Overlap,
                Comparator::Residual,
                Comparator::Coverage,
            ],
            known_at: None,
            open_window_policy: crate::OpenWindowPolicy::RequireClosed,
            open_window_horizon: None,
            strict: false,
        };
        let result = compare(&history, &plan);

        let json = export_result_json(&result).expect("json");
        assert!(json.contains("\"coverageSummaries\""));
        assert!(json.contains("\"rowFinalities\""));
        assert!(json.contains("\"rowId\": \"overlap[0]\""));
        assert!(json.contains("\"finality\": \"Final\""));
    }
}
