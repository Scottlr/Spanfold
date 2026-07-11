use std::fmt::Write;

use crate::{
    AlignedComparison, CohortEvidenceMetadata, ComparisonPlan, ComparisonResult,
    ComparisonRowFinality, PreparedComparison, align,
};

/// Deterministic explanation output format.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComparisonExplanationFormat {
    /// Markdown with section headings.
    Markdown,
    /// Plain text without Markdown headings.
    PlainText,
}

impl ComparisonPlan {
    /// Renders a deterministic explanation in Markdown.
    #[must_use]
    pub fn explain(&self) -> String {
        self.explain_with_format(ComparisonExplanationFormat::Markdown)
    }

    /// Renders a deterministic explanation in the requested format.
    #[must_use]
    pub fn explain_with_format(&self, format: ComparisonExplanationFormat) -> String {
        let mut out = String::new();
        write_title(
            &mut out,
            format,
            &format!("Comparison Explain: {}", self.name),
        );
        write_section(&mut out, format, "Plan");
        write_item(&mut out, "name", &self.name);
        write_item(&mut out, "strict", &self.strict.to_string());
        write_item(&mut out, "target", &self.target_source);
        write_item(&mut out, "against", &format!("{:?}", self.against));
        write_item(&mut out, "scopeWindow", &format!("{:?}", self.scope_window));
        write_item(&mut out, "scopeKey", &format!("{:?}", self.scope_key));
        write_item(
            &mut out,
            "scopePartition",
            &format!("{:?}", self.scope_partition),
        );
        write_item(&mut out, "comparators", &format!("{:?}", self.comparators));
        write_item(&mut out, "knownAt", &format!("{:?}", self.known_at));
        write_item(
            &mut out,
            "coalesceAdjacentWindows",
            &self.coalesce_adjacent_windows.to_string(),
        );
        write_item(
            &mut out,
            "duplicateWindowPolicy",
            &format!("{:?}", self.duplicate_window_policy),
        );
        write_item(
            &mut out,
            "openWindowHorizon",
            &format!("{:?}", self.open_window_horizon),
        );
        out
    }
}

impl PreparedComparison {
    /// Aligns the prepared comparison.
    #[must_use]
    pub fn align(&self) -> AlignedComparison {
        align(self)
    }

    /// Renders a deterministic explanation in Markdown.
    #[must_use]
    pub fn explain(&self) -> String {
        self.explain_with_format(ComparisonExplanationFormat::Markdown)
    }

    /// Renders a deterministic explanation in the requested format.
    #[must_use]
    pub fn explain_with_format(&self, format: ComparisonExplanationFormat) -> String {
        let mut out = self.plan.explain_with_format(format);
        write_section(&mut out, format, "Preparation");
        write_item(
            &mut out,
            "selected windows",
            &self.selected_windows.len().to_string(),
        );
        write_item(
            &mut out,
            "excluded windows",
            &self.excluded_windows.len().to_string(),
        );
        write_item(
            &mut out,
            "normalized windows",
            &self.normalized_windows.len().to_string(),
        );
        for (index, window) in self.selected_windows.iter().enumerate() {
            write_item(
                &mut out,
                &format!("selected[{index}]"),
                &format!(
                    "record={}; window={}; key={}; source={:?}",
                    window.record_id, window.window_name, window.key, window.source
                ),
            );
        }
        for (index, window) in self.normalized_windows.iter().enumerate() {
            write_item(
                &mut out,
                &format!("normalized[{index}]"),
                &format!(
                    "record={}; side={:?}; selector={}; range={}..{}",
                    window.record_id,
                    window.side,
                    window.selector_name,
                    window.range.start().magnitude(),
                    window.range.end().magnitude()
                ),
            );
        }
        out
    }
}

impl AlignedComparison {
    /// Renders a deterministic explanation in Markdown.
    #[must_use]
    pub fn explain(&self) -> String {
        self.explain_with_format(ComparisonExplanationFormat::Markdown)
    }

    /// Renders a deterministic explanation in the requested format.
    #[must_use]
    pub fn explain_with_format(&self, format: ComparisonExplanationFormat) -> String {
        let mut out = String::new();
        write_section(&mut out, format, "Alignment");
        write_item(&mut out, "segments", &self.segments.len().to_string());
        for (index, segment) in self.segments.iter().enumerate() {
            write_item(
                &mut out,
                &format!("segment[{index}]"),
                &format!(
                    "window={}; key={}; range={}..{}; target={:?}; against={:?}",
                    segment.window_name,
                    segment.key,
                    segment.range.start,
                    segment.range.end,
                    segment.target_record_ids,
                    segment.against_record_ids
                ),
            );
        }
        out
    }
}

impl ComparisonResult {
    /// Renders a deterministic explanation in Markdown.
    #[must_use]
    pub fn explain(&self) -> String {
        self.explain_with_format(ComparisonExplanationFormat::Markdown)
    }

    /// Renders a deterministic explanation in the requested format.
    #[must_use]
    pub fn explain_with_format(&self, format: ComparisonExplanationFormat) -> String {
        let mut out = self.plan.explain_with_format(format);
        write_section(&mut out, format, "Result");
        write_item(&mut out, "valid", &self.is_valid.to_string());
        write_item(
            &mut out,
            "evaluation horizon",
            &format!("{:?}", self.evaluation_horizon),
        );
        write_item(
            &mut out,
            "overlap rows",
            &self.overlap_rows.len().to_string(),
        );
        write_item(
            &mut out,
            "residual rows",
            &self.residual_rows.len().to_string(),
        );
        write_item(
            &mut out,
            "missing rows",
            &self.missing_rows.len().to_string(),
        );
        write_item(
            &mut out,
            "coverage rows",
            &self.coverage_rows.len().to_string(),
        );
        write_item(
            &mut out,
            "extension metadata",
            &self.extension_metadata.len().to_string(),
        );
        for (index, finality) in self.row_finalities.iter().enumerate() {
            write_finality(&mut out, index, finality);
        }
        for (index, row) in self.overlap_rows.iter().enumerate() {
            write_item(
                &mut out,
                &format!("overlap[{index}]"),
                &format!(
                    "window={}; key={}; range={}..{}; target={:?}; against={:?}",
                    row.window_name,
                    row.key,
                    row.range.start,
                    row.range.end,
                    row.target_record_ids,
                    row.against_record_ids
                ),
            );
        }
        for (index, row) in self.residual_rows.iter().enumerate() {
            write_item(
                &mut out,
                &format!("residual[{index}]"),
                &format!(
                    "window={}; key={}; range={}..{}; target={:?}",
                    row.window_name, row.key, row.range.start, row.range.end, row.target_record_ids
                ),
            );
        }
        for (index, metadata) in self.extension_metadata.iter().enumerate() {
            write_item(
                &mut out,
                &format!("extensionMetadata[{index}]"),
                &format!(
                    "{}.{}={}",
                    metadata.extension_id, metadata.key, metadata.value
                ),
            );
        }
        out
    }

    /// Returns whether any emitted row is provisional.
    #[must_use]
    pub fn has_provisional_rows(&self) -> bool {
        self.row_finalities
            .iter()
            .any(|row| row.finality == crate::ComparisonFinality::Provisional)
    }

    /// Returns only provisional row-finality metadata.
    #[must_use]
    pub fn provisional_row_finalities(&self) -> Vec<&crate::ComparisonRowFinality> {
        self.row_finalities
            .iter()
            .filter(|row| row.finality == crate::ComparisonFinality::Provisional)
            .collect()
    }

    /// Returns only final row-finality metadata.
    #[must_use]
    pub fn final_row_finalities(&self) -> Vec<&crate::ComparisonRowFinality> {
        self.row_finalities
            .iter()
            .filter(|row| row.finality == crate::ComparisonFinality::Final)
            .collect()
    }

    /// Returns warning-level diagnostics.
    #[must_use]
    pub fn warning_diagnostics(&self) -> Vec<&crate::ComparisonDiagnostic> {
        self.diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == crate::DiagnosticSeverity::Warning)
            .collect()
    }

    /// Returns error-level diagnostics.
    #[must_use]
    pub fn error_diagnostics(&self) -> Vec<&crate::ComparisonDiagnostic> {
        self.diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == crate::DiagnosticSeverity::Error)
            .collect()
    }

    /// Parses cohort evidence emitted in extension metadata.
    #[must_use]
    pub fn cohort_evidence(&self) -> Vec<CohortEvidenceMetadata> {
        self.extension_metadata
            .iter()
            .filter(|metadata| metadata.extension_id == "spanfold.cohort")
            .filter_map(parse_cohort_evidence)
            .collect()
    }
}

fn parse_cohort_evidence(
    metadata: &crate::ComparisonExtensionMetadata,
) -> Option<CohortEvidenceMetadata> {
    let segment_index = metadata
        .key
        .strip_prefix("segment[")?
        .strip_suffix(']')?
        .parse::<usize>()
        .ok()?;
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&metadata.value) {
        return Some(CohortEvidenceMetadata {
            segment_index,
            rule: value.get("rule")?.as_str()?.to_owned(),
            required_count: value.get("required")?.as_u64()? as usize,
            active_count: value.get("activeCount")?.as_u64()? as usize,
            is_active: value.get("isActive")?.as_bool()?,
            active_sources: value
                .get("activeSources")?
                .as_array()?
                .iter()
                .map(|source| source.as_str().map(str::to_owned))
                .collect::<Option<Vec<_>>>()?,
            raw_value: metadata.value.clone(),
        });
    }
    let fields = parse_metadata_fields(&metadata.value);
    Some(CohortEvidenceMetadata {
        segment_index,
        rule: fields.get("rule")?.to_string(),
        required_count: fields.get("required")?.parse().ok()?,
        active_count: fields.get("activeCount")?.parse().ok()?,
        is_active: fields.get("isActive")?.eq_ignore_ascii_case("true"),
        active_sources: fields
            .get("activeSources")
            .map(|value| {
                value
                    .split(',')
                    .filter(|item| !item.is_empty())
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default(),
        raw_value: metadata.value.clone(),
    })
}

fn parse_metadata_fields(value: &str) -> std::collections::BTreeMap<&str, &str> {
    let mut fields = std::collections::BTreeMap::new();
    for part in value.split(';') {
        let mut pieces = part.trim().splitn(2, '=');
        let Some(key) = pieces.next() else {
            continue;
        };
        let Some(field_value) = pieces.next() else {
            continue;
        };
        if !key.is_empty() {
            fields.insert(key, field_value);
        }
    }
    fields
}

fn write_title(out: &mut String, format: ComparisonExplanationFormat, value: &str) {
    match format {
        ComparisonExplanationFormat::Markdown => {
            let _ = writeln!(out, "# {value}\n");
        }
        ComparisonExplanationFormat::PlainText => {
            let _ = writeln!(out, "{value}\n");
        }
    }
}

fn write_section(out: &mut String, format: ComparisonExplanationFormat, value: &str) {
    match format {
        ComparisonExplanationFormat::Markdown => {
            let _ = writeln!(out, "## {value}\n");
        }
        ComparisonExplanationFormat::PlainText => {
            let _ = writeln!(out, "{value}:");
        }
    }
}

fn write_item(out: &mut String, key: &str, value: &str) {
    let _ = writeln!(out, "- {key}: {value}");
}

fn write_finality(out: &mut String, index: usize, finality: &ComparisonRowFinality) {
    write_item(
        out,
        &format!("finality[{index}]"),
        &format!(
            "{}={:?}; reason={}",
            finality.row_id, finality.finality, finality.reason
        ),
    );
}

#[cfg(test)]
mod tests {
    use crate::{ComparisonDuplicateWindowPolicy, WindowHistoryFixture, compare};

    #[test]
    fn result_explain_includes_row_ids_and_extension_metadata() {
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
        let result = compare(
            &history,
            &crate::ComparisonPlan {
                name: "Provider QA".to_owned(),
                target_source: "provider-a".to_owned(),
                against: crate::AgainstSelection::Sources(vec!["provider-b".to_owned()]),
                target_selector: None,
                against_selectors: Vec::new(),
                scope_window: Some("DeviceOffline".to_owned()),
                scope_key: None,
                scope_partition: None,
                scope_segments: Vec::new(),
                scope_tags: Vec::new(),
                comparators: vec![crate::Comparator::Overlap],
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
            },
        );

        let explanation = result.explain();
        assert!(explanation.contains("overlap:"));
        assert!(explanation.contains("finality[0]"));
        assert_eq!(result.final_row_finalities().len(), 1);
        assert!(result.provisional_row_finalities().is_empty());
        assert!(result.warning_diagnostics().is_empty());
        assert!(result.error_diagnostics().is_empty());
    }

    #[test]
    fn result_helpers_filter_diagnostics_by_severity() {
        let history = WindowHistoryFixture::new()
            .closed_window("DeviceOffline", "device-1", 1, 5, |w| {
                w.source("provider-a")
            })
            .expect("target")
            .build();
        let result = compare(
            &history,
            &crate::ComparisonPlan {
                name: "Strict broad QA".to_owned(),
                target_source: "provider-a".to_owned(),
                against: crate::AgainstSelection::Sources(vec!["provider-b".to_owned()]),
                target_selector: None,
                against_selectors: Vec::new(),
                scope_window: None,
                scope_key: None,
                scope_partition: None,
                scope_segments: Vec::new(),
                scope_tags: Vec::new(),
                comparators: vec![crate::Comparator::Overlap],
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
                strict: true,
            },
        );

        assert!(result.warning_diagnostics().is_empty());
        assert_eq!(result.error_diagnostics().len(), 1);
    }
}
