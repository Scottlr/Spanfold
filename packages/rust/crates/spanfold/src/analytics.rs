use std::collections::BTreeSet;

use serde::Serialize;

use crate::{ComparisonDiagnostic, RowRange, TemporalAxis, WindowHistory};

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

    /// Returns one directional matrix cell, or `None` when absent.
    #[must_use]
    pub fn get_cell(&self, target_source: &str, against_source: &str) -> Option<&SourceMatrixCell> {
        self.try_get_cell(target_source, against_source)
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
    let mut unique_sources = Vec::with_capacity(sources.len());
    let mut seen_sources = BTreeSet::new();
    for source in sources {
        if !source.trim().is_empty() && seen_sources.insert(source.as_str()) {
            unique_sources.push(source.clone());
        }
    }
    let mut metrics =
        vec![SourceMatrixMetrics::default(); unique_sources.len() * unique_sources.len()];
    let source_index = unique_sources
        .iter()
        .enumerate()
        .map(|(index, source)| (source.as_str(), index))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut groups =
        std::collections::BTreeMap::<(String, Option<String>), Vec<SourceEvent>>::new();
    for window in history.closed_windows() {
        if window.window_name != window_name
            || window.range.start().axis() != TemporalAxis::ProcessingPosition
        {
            continue;
        }
        let Some(source) = window
            .source
            .as_deref()
            .and_then(|value| source_index.get(value))
        else {
            continue;
        };
        let key = (window.key.clone(), window.partition.clone());
        let events = groups.entry(key).or_default();
        events.push(SourceEvent {
            position: window.range.start().magnitude(),
            source: *source,
            active: true,
        });
        events.push(SourceEvent {
            position: window.range.end().magnitude(),
            source: *source,
            active: false,
        });
    }

    for events in groups.values_mut() {
        events.sort_by_key(|event| event.position);
        let mut active = vec![0_usize; unique_sources.len()];
        let mut index = 0;
        while index < events.len() {
            let position = events[index].position;
            while index < events.len() && events[index].position == position {
                let event = events[index];
                if event.active {
                    active[event.source] += 1;
                } else {
                    active[event.source] = active[event.source].saturating_sub(1);
                }
                index += 1;
            }
            let next_position = events.get(index).map_or(position, |event| event.position);
            let length = next_position.saturating_sub(position);
            if length <= 0 {
                continue;
            }
            for target in 0..unique_sources.len() {
                for against in 0..unique_sources.len() {
                    let target_active = active[target] > 0;
                    let against_active = active[against] > 0;
                    let metric = &mut metrics[target * unique_sources.len() + against];
                    metric.target_has_windows |= target_active;
                    metric.against_has_windows |= against_active;
                    if target_active {
                        metric.coverage_row_count += 1;
                        metric.target_magnitude += i128::from(length);
                        if against_active {
                            metric.covered_magnitude += i128::from(length);
                        }
                    }
                    if target_active && against_active {
                        metric.overlap_row_count += 1;
                    } else if target_active {
                        metric.residual_row_count += 1;
                    } else if against_active {
                        metric.missing_row_count += 1;
                    }
                }
            }
        }
    }

    let mut cells = Vec::with_capacity(unique_sources.len() * unique_sources.len());
    for target in 0..unique_sources.len() {
        for against in 0..unique_sources.len() {
            let metric = &metrics[target * unique_sources.len() + against];
            cells.push(SourceMatrixCell {
                target_source: unique_sources[target].clone(),
                against_source: unique_sources[against].clone(),
                is_diagonal: target == against,
                target_has_windows: metric.target_has_windows,
                against_has_windows: metric.against_has_windows,
                overlap_row_count: metric.overlap_row_count,
                residual_row_count: metric.residual_row_count,
                missing_row_count: metric.missing_row_count,
                coverage_row_count: metric.coverage_row_count,
                coverage_ratio: (metric.target_magnitude > 0)
                    .then_some(metric.covered_magnitude as f64 / metric.target_magnitude as f64),
            });
        }
    }

    SourceMatrixResult {
        name: name.to_owned(),
        window_name: window_name.to_owned(),
        sources: unique_sources,
        cells,
    }
}

#[derive(Clone, Copy)]
struct SourceEvent {
    position: i64,
    source: usize,
    active: bool,
}

#[derive(Clone, Debug, Default)]
struct SourceMatrixMetrics {
    target_has_windows: bool,
    against_has_windows: bool,
    overlap_row_count: usize,
    residual_row_count: usize,
    missing_row_count: usize,
    coverage_row_count: usize,
    target_magnitude: i128,
    covered_magnitude: i128,
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
        scopes.insert((
            window.source.clone(),
            window.partition.clone(),
            window.range.start().axis(),
            window.range.start().clock().map(str::to_owned),
        ));
    }
    for window in &children {
        scopes.insert((
            window.source.clone(),
            window.partition.clone(),
            window.range.start().axis(),
            window.range.start().clock().map(str::to_owned),
        ));
    }

    let mut rows = Vec::new();
    for (source, partition, axis, clock) in scopes {
        let scoped_parents = parents
            .iter()
            .filter(|window| {
                window.source == source
                    && window.partition == partition
                    && window.range.start().axis() == axis
                    && window.range.start().clock() == clock.as_deref()
            })
            .collect::<Vec<_>>();
        let scoped_children = children
            .iter()
            .filter(|window| {
                window.source == source
                    && window.partition == partition
                    && window.range.start().axis() == axis
                    && window.range.start().clock() == clock.as_deref()
            })
            .collect::<Vec<_>>();

        let mut boundaries = BTreeSet::new();
        for window in scoped_parents.iter().chain(scoped_children.iter()) {
            boundaries.insert(window.range.start().magnitude());
            boundaries.insert(window.range.end().magnitude());
        }
        let boundaries = boundaries.into_iter().collect::<Vec<_>>();
        let mut parent_starts = scoped_parents
            .iter()
            .enumerate()
            .map(|(index, window)| (window.range.start().magnitude(), index))
            .collect::<Vec<_>>();
        let mut parent_ends = scoped_parents
            .iter()
            .enumerate()
            .map(|(index, window)| (window.range.end().magnitude(), index))
            .collect::<Vec<_>>();
        let mut child_starts = scoped_children
            .iter()
            .enumerate()
            .map(|(index, window)| (window.range.start().magnitude(), index))
            .collect::<Vec<_>>();
        let mut child_ends = scoped_children
            .iter()
            .enumerate()
            .map(|(index, window)| (window.range.end().magnitude(), index))
            .collect::<Vec<_>>();
        parent_starts.sort_unstable();
        parent_ends.sort_unstable();
        child_starts.sort_unstable();
        child_ends.sort_unstable();
        let mut active_parents = BTreeSet::new();
        let mut active_children = BTreeSet::new();
        let mut parent_start_index = 0;
        let mut parent_end_index = 0;
        let mut child_start_index = 0;
        let mut child_end_index = 0;
        for pair in boundaries.windows(2) {
            let start = pair[0];
            let end = pair[1];
            if start >= end {
                continue;
            }
            while parent_end_index < parent_ends.len() && parent_ends[parent_end_index].0 <= start {
                active_parents.remove(&parent_ends[parent_end_index].1);
                parent_end_index += 1;
            }
            while child_end_index < child_ends.len() && child_ends[child_end_index].0 <= start {
                active_children.remove(&child_ends[child_end_index].1);
                child_end_index += 1;
            }
            while parent_start_index < parent_starts.len()
                && parent_starts[parent_start_index].0 <= start
            {
                active_parents.insert(parent_starts[parent_start_index].1);
                parent_start_index += 1;
            }
            while child_start_index < child_starts.len()
                && child_starts[child_start_index].0 <= start
            {
                active_children.insert(child_starts[child_start_index].1);
                child_start_index += 1;
            }
            let parent_record_ids = active_parents
                .iter()
                .map(|index| scoped_parents[*index].id.as_str().to_owned())
                .collect::<Vec<_>>();
            let child_record_ids = active_children
                .iter()
                .map(|index| scoped_children[*index].id.as_str().to_owned())
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
                range: RowRange {
                    start,
                    end,
                    axis,
                    clock: clock.clone(),
                },
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

        let forward = matrix.get_cell("provider-a", "provider-b").expect("cell");
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
