use std::collections::BTreeSet;

use serde::Serialize;

use crate::{
    AgainstSelection, Comparator, ComparisonDiagnostic, ComparisonDuplicateWindowPolicy,
    ComparisonPlan, RowRange, WindowHistory, compare,
};

/// One directional source-matrix cell.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SourceMatrixCell {
    /// Row source treated as target.
    #[serde(rename = "targetSource")]
    pub target_source: String,
    /// Column source treated as comparison.
    #[serde(rename = "againstSource")]
    pub against_source: String,
    /// Whether the cell is diagonal.
    #[serde(rename = "isDiagonal")]
    pub is_diagonal: bool,
    /// Whether the target source has windows in the matrix window.
    #[serde(rename = "targetHasWindows")]
    pub target_has_windows: bool,
    /// Whether the comparison source has windows in the matrix window.
    #[serde(rename = "againstHasWindows")]
    pub against_has_windows: bool,
    /// Overlap row count.
    #[serde(rename = "overlapRowCount")]
    pub overlap_row_count: usize,
    /// Residual row count.
    #[serde(rename = "residualRowCount")]
    pub residual_row_count: usize,
    /// Missing row count.
    #[serde(rename = "missingRowCount")]
    pub missing_row_count: usize,
    /// Coverage row count.
    #[serde(rename = "coverageRowCount")]
    pub coverage_row_count: usize,
    /// Aggregate coverage ratio, when target coverage exists.
    #[serde(rename = "coverageRatio")]
    pub coverage_ratio: Option<f64>,
}

/// Directional matrix across sources.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SourceMatrixResult {
    /// Matrix name.
    pub name: String,
    /// Window family used for all cells.
    #[serde(rename = "windowName")]
    pub window_name: String,
    /// Sources in requested order.
    pub sources: Vec<String>,
    /// Cells in row-major order.
    pub cells: Vec<SourceMatrixCell>,
}

impl SourceMatrixResult {
    /// Returns one directional matrix cell when present.
    #[must_use]
    pub fn try_get_cell(
        &self,
        target_source: &str,
        against_source: &str,
    ) -> Option<&SourceMatrixCell> {
        self.cells.iter().find(|cell| {
            cell.target_source == target_source && cell.against_source == against_source
        })
    }

    /// Returns one directional matrix cell or panics when absent.
    #[must_use]
    pub fn get_cell(&self, target_source: &str, against_source: &str) -> &SourceMatrixCell {
        self.try_get_cell(target_source, against_source)
            .expect("source matrix cell was not found")
    }
}

/// Hierarchy row interpretation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum HierarchyComparisonRowKind {
    /// Parent activity explained by children.
    ParentExplained,
    /// Parent active without child contribution.
    UnexplainedParent,
    /// Child contribution outside active parent.
    OrphanChild,
}

/// One parent/child hierarchy segment.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct HierarchyComparisonRow {
    /// Row kind.
    pub kind: HierarchyComparisonRowKind,
    /// Shared source scope.
    pub source: Option<String>,
    /// Shared partition scope.
    pub partition: Option<String>,
    /// Segment range.
    pub range: RowRange,
    /// Active parent record IDs.
    #[serde(rename = "parentRecordIds")]
    pub parent_record_ids: Vec<String>,
    /// Active child record IDs.
    #[serde(rename = "childRecordIds")]
    pub child_record_ids: Vec<String>,
}

/// Hierarchy comparison result.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct HierarchyComparisonResult {
    /// Comparison name.
    pub name: String,
    /// Parent window family.
    #[serde(rename = "parentWindowName")]
    pub parent_window_name: String,
    /// Child window family.
    #[serde(rename = "childWindowName")]
    pub child_window_name: String,
    /// Deterministic rows.
    pub rows: Vec<HierarchyComparisonRow>,
    /// Diagnostics.
    pub diagnostics: Vec<ComparisonDiagnostic>,
}

/// Builds a directional source matrix.
#[must_use]
pub fn compare_sources(
    history: &WindowHistory,
    name: &str,
    window_name: &str,
    sources: &[String],
) -> SourceMatrixResult {
    let mut cells = Vec::with_capacity(sources.len() * sources.len());
    for target_source in sources {
        for against_source in sources {
            let target_has_windows = has_window_for_source(history, window_name, target_source);
            let against_has_windows = has_window_for_source(history, window_name, against_source);
            if target_source == against_source {
                cells.push(SourceMatrixCell {
                    target_source: target_source.clone(),
                    against_source: against_source.clone(),
                    is_diagonal: true,
                    target_has_windows,
                    against_has_windows,
                    overlap_row_count: 0,
                    residual_row_count: 0,
                    missing_row_count: 0,
                    coverage_row_count: 0,
                    coverage_ratio: target_has_windows.then_some(1.0),
                });
                continue;
            }

            let result = compare(
                history,
                &ComparisonPlan {
                    name: format!("{name} {target_source} vs {against_source}"),
                    target_source: target_source.clone(),
                    against: AgainstSelection::Sources(vec![against_source.clone()]),
                    target_selector: None,
                    against_selectors: Vec::new(),
                    scope_window: Some(window_name.to_owned()),
                    scope_key: None,
                    scope_partition: None,
                    scope_segments: Vec::new(),
                    scope_tags: Vec::new(),
                    comparators: vec![
                        Comparator::Overlap,
                        Comparator::Residual,
                        Comparator::Missing,
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
                },
            );
            let coverage_ratio = if result.coverage_summaries.is_empty() {
                None
            } else {
                let target: f64 = result
                    .coverage_summaries
                    .iter()
                    .map(|summary| summary.target_magnitude)
                    .sum();
                let covered: f64 = result
                    .coverage_summaries
                    .iter()
                    .map(|summary| summary.covered_magnitude)
                    .sum();
                (target > 0.0).then_some(covered / target)
            };
            cells.push(SourceMatrixCell {
                target_source: target_source.clone(),
                against_source: against_source.clone(),
                is_diagonal: false,
                target_has_windows,
                against_has_windows,
                overlap_row_count: result.overlap_rows.len(),
                residual_row_count: result.residual_rows.len(),
                missing_row_count: result.missing_rows.len(),
                coverage_row_count: result.coverage_rows.len(),
                coverage_ratio,
            });
        }
    }

    SourceMatrixResult {
        name: name.to_owned(),
        window_name: window_name.to_owned(),
        sources: sources.to_vec(),
        cells,
    }
}

/// Builds a hierarchy explanation across parent and child windows.
#[must_use]
pub fn compare_hierarchy(
    history: &WindowHistory,
    name: &str,
    parent_window_name: &str,
    child_window_name: &str,
) -> HierarchyComparisonResult {
    let parents = history
        .closed_windows()
        .iter()
        .filter(|window| window.window_name == parent_window_name)
        .collect::<Vec<_>>();
    let children = history
        .closed_windows()
        .iter()
        .filter(|window| window.window_name == child_window_name)
        .collect::<Vec<_>>();

    let mut diagnostics = Vec::new();
    if parents.is_empty() {
        diagnostics.push(ComparisonDiagnostic {
            code: "MissingLineage".to_owned(),
            severity: crate::DiagnosticSeverity::Warning,
        });
    }
    if children.is_empty() {
        diagnostics.push(ComparisonDiagnostic {
            code: "MissingLineage".to_owned(),
            severity: crate::DiagnosticSeverity::Warning,
        });
    }

    let mut scopes = BTreeSet::new();
    for window in &parents {
        scopes.insert((window.source.clone(), window.partition.clone()));
    }
    for window in &children {
        scopes.insert((window.source.clone(), window.partition.clone()));
    }

    let mut rows = Vec::new();
    for (source, partition) in scopes {
        let scoped_parents = parents
            .iter()
            .filter(|window| window.source == source && window.partition == partition)
            .collect::<Vec<_>>();
        let scoped_children = children
            .iter()
            .filter(|window| window.source == source && window.partition == partition)
            .collect::<Vec<_>>();

        let mut boundaries = BTreeSet::new();
        for window in scoped_parents.iter().chain(scoped_children.iter()) {
            boundaries.insert(window.range.start().magnitude());
            boundaries.insert(window.range.end().magnitude());
        }
        let boundaries = boundaries.into_iter().collect::<Vec<_>>();
        for pair in boundaries.windows(2) {
            let start = pair[0];
            let end = pair[1];
            if start >= end {
                continue;
            }
            let parent_record_ids = scoped_parents
                .iter()
                .filter(|window| {
                    window.range.start().magnitude() <= start
                        && end <= window.range.end().magnitude()
                })
                .map(|window| window.id.as_str().to_owned())
                .collect::<Vec<_>>();
            let child_record_ids = scoped_children
                .iter()
                .filter(|window| {
                    window.range.start().magnitude() <= start
                        && end <= window.range.end().magnitude()
                })
                .map(|window| window.id.as_str().to_owned())
                .collect::<Vec<_>>();
            if parent_record_ids.is_empty() && child_record_ids.is_empty() {
                continue;
            }
            rows.push(HierarchyComparisonRow {
                kind: if !parent_record_ids.is_empty() && !child_record_ids.is_empty() {
                    HierarchyComparisonRowKind::ParentExplained
                } else if !parent_record_ids.is_empty() {
                    HierarchyComparisonRowKind::UnexplainedParent
                } else {
                    HierarchyComparisonRowKind::OrphanChild
                },
                source: source.clone(),
                partition: partition.clone(),
                range: RowRange { start, end },
                parent_record_ids,
                child_record_ids,
            });
        }
    }

    HierarchyComparisonResult {
        name: name.to_owned(),
        parent_window_name: parent_window_name.to_owned(),
        child_window_name: child_window_name.to_owned(),
        rows,
        diagnostics,
    }
}

fn has_window_for_source(history: &WindowHistory, window_name: &str, source: &str) -> bool {
    history
        .closed_windows()
        .iter()
        .any(|window| window.window_name == window_name && window.source.as_deref() == Some(source))
        || history.open_windows().iter().any(|window| {
            window.window_name == window_name && window.source.as_deref() == Some(source)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WindowHistoryFixture;

    #[test]
    fn source_matrix_supports_directional_lookup() {
        let history = WindowHistoryFixture::new()
            .closed_window("DeviceOffline", "device-1", 1, 5, |w| {
                w.source("provider-a")
            })
            .expect("provider-a")
            .closed_window("DeviceOffline", "device-1", 3, 7, |w| {
                w.source("provider-b")
            })
            .expect("provider-b")
            .build();

        let matrix = history.compare_sources(
            "Provider matrix",
            "DeviceOffline",
            &["provider-a".to_owned(), "provider-b".to_owned()],
        );

        let forward = matrix.get_cell("provider-a", "provider-b");
        assert!(!forward.is_diagonal);
        assert_eq!(forward.overlap_row_count, 1);
        assert!(matrix.try_get_cell("provider-b", "provider-a").is_some());
        assert!(matrix.try_get_cell("provider-a", "provider-c").is_none());
    }

    #[test]
    fn hierarchy_marks_unexplained_and_orphan_ranges() {
        let history = WindowHistoryFixture::new()
            .closed_window("Parent", "parent-1", 3, 5, |w| w.source("source-a"))
            .expect("parent")
            .closed_window("Child", "child-1", 1, 7, |w| w.source("source-a"))
            .expect("child")
            .build();

        let result = history.compare_hierarchy("Hierarchy QA", "Parent", "Child");

        assert_eq!(result.rows.len(), 3);
        assert_eq!(result.rows[0].kind, HierarchyComparisonRowKind::OrphanChild);
        assert_eq!(
            result.rows[1].kind,
            HierarchyComparisonRowKind::ParentExplained
        );
        assert_eq!(result.rows[2].kind, HierarchyComparisonRowKind::OrphanChild);
    }
}
