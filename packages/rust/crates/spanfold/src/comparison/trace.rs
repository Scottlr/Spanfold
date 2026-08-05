//! In-memory lineage tracing for materialized comparison rows.

use std::collections::BTreeSet;

use super::*;

/// Authoritative immutable lineage for one typed comparison row.
#[derive(Clone, Debug, PartialEq)]
pub struct ComparisonRowTrace<R> {
    row: R,
    metadata: ComparisonRowFinality,
    contributing_records: Vec<WindowArtifact>,
    normalized_windows: Vec<NormalizedWindowRecord>,
    aligned_segments: Vec<AlignedSegmentArtifact>,
    relevant_exclusions: Vec<ExcludedWindowRecord>,
}

impl<R> ComparisonRowTrace<R> {
    /// Returns the typed comparison row.
    #[must_use]
    pub fn row(&self) -> &R {
        &self.row
    }

    /// Returns the authoritative row finality metadata.
    #[must_use]
    pub fn metadata(&self) -> &ComparisonRowFinality {
        &self.metadata
    }

    /// Returns the canonical reference represented by the row metadata.
    pub fn reference(&self) -> Result<ComparisonRowReference, ComparisonRowReferenceError> {
        self.metadata.reference()
    }

    /// Returns source records directly referenced by the row.
    #[must_use]
    pub fn contributing_records(&self) -> &[WindowArtifact] {
        &self.contributing_records
    }

    /// Returns normalized windows directly referenced by the row.
    #[must_use]
    pub fn normalized_windows(&self) -> &[NormalizedWindowRecord] {
        &self.normalized_windows
    }

    /// Returns aligned segments supported by the row's source records.
    #[must_use]
    pub fn aligned_segments(&self) -> &[AlignedSegmentArtifact] {
        &self.aligned_segments
    }

    /// Returns preparation exclusions in the row's logical scope.
    #[must_use]
    pub fn relevant_exclusions(&self) -> &[ExcludedWindowRecord] {
        &self.relevant_exclusions
    }
}

/// Erased trace result for any current comparison row family.
#[derive(Clone, Debug, PartialEq)]
pub enum AnyComparisonRowTrace {
    /// Trace for an overlap row.
    Overlap(ComparisonRowTrace<OverlapRow>),
    /// Trace for a residual row.
    Residual(ComparisonRowTrace<ResidualRow>),
    /// Trace for a missing row.
    Missing(ComparisonRowTrace<MissingRow>),
    /// Trace for a coverage row.
    Coverage(ComparisonRowTrace<CoverageRow>),
    /// Trace for a gap row.
    Gap(ComparisonRowTrace<GapRow>),
    /// Trace for a symmetric-difference row.
    SymmetricDifference(ComparisonRowTrace<SymmetricDifferenceRow>),
    /// Trace for a containment row.
    Containment(ComparisonRowTrace<ContainmentRow>),
    /// Trace for a lead/lag row.
    LeadLag(ComparisonRowTrace<LeadLagRow>),
    /// Trace for an as-of row.
    AsOf(ComparisonRowTrace<AsOfRow>),
}

impl AnyComparisonRowTrace {
    /// Returns authoritative metadata for the erased trace.
    #[must_use]
    pub fn metadata(&self) -> &ComparisonRowFinality {
        match self {
            Self::Overlap(trace) => trace.metadata(),
            Self::Residual(trace) => trace.metadata(),
            Self::Missing(trace) => trace.metadata(),
            Self::Coverage(trace) => trace.metadata(),
            Self::Gap(trace) => trace.metadata(),
            Self::SymmetricDifference(trace) => trace.metadata(),
            Self::Containment(trace) => trace.metadata(),
            Self::LeadLag(trace) => trace.metadata(),
            Self::AsOf(trace) => trace.metadata(),
        }
    }

    /// Returns the canonical reference represented by the erased trace.
    pub fn reference(&self) -> Result<ComparisonRowReference, ComparisonRowReferenceError> {
        self.metadata().reference()
    }
}

/// Error returned when a row cannot be traced from a materialized result.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ComparisonRowTraceError {
    /// The result's row metadata layout is inconsistent.
    #[error(transparent)]
    Metadata(#[from] ComparisonRowMetadataError),
    /// The result's row metadata contains an invalid canonical reference.
    #[error(transparent)]
    InvalidReference(#[from] ComparisonRowReferenceError),
    /// The requested row is not present in this result.
    #[error("comparison row '{reference}' was not found in the result")]
    RowNotFound {
        /// The reference that could not be resolved.
        reference: ComparisonRowReference,
    },
    /// The typed row family does not match its canonical reference.
    #[error("typed row family {expected} does not match canonical reference family {actual}")]
    RowKindMismatch {
        /// The family expected by the typed row.
        expected: ComparisonRowKind,
        /// The family named by the metadata reference.
        actual: ComparisonRowKind,
    },
    /// The result was not produced by the in-memory comparison pipeline.
    #[error("comparison result does not retain trace context")]
    MissingContext,
}

mod sealed {
    pub trait Sealed {}
}

/// Sealed lineage contract implemented by the nine current row families.
///
/// The private supertrait prevents callers from claiming lineage support for
/// arbitrary row types while allowing one typed result entry point.
pub trait ComparisonRowTraceLineage: sealed::Sealed + Clone + PartialEq + 'static {
    /// The canonical row family represented by the implementing type.
    const KIND: ComparisonRowKind;

    /// Locates an exact typed row and its authoritative metadata in a result.
    fn locate(
        result: &ComparisonResult,
        candidate: &Self,
        reference: &ComparisonRowReference,
    ) -> Result<Option<(Self, ComparisonRowFinality)>, ComparisonRowTraceError>;

    /// Returns the direct source record IDs that establish the row.
    fn record_ids(&self) -> Vec<String>;

    /// Returns the row's window/key/partition scope.
    fn trace_scope(&self) -> (String, String, Option<String>);
}

macro_rules! impl_lineage {
    ($row:ty, $kind:expr, $rows_method:ident, $records:expr, $scope:expr) => {
        impl sealed::Sealed for $row {}

        impl ComparisonRowTraceLineage for $row {
            const KIND: ComparisonRowKind = $kind;

            fn locate(
                result: &ComparisonResult,
                candidate: &Self,
                reference: &ComparisonRowReference,
            ) -> Result<Option<(Self, ComparisonRowFinality)>, ComparisonRowTraceError> {
                let rows = result.$rows_method()?;
                for pair in rows {
                    if pair.metadata.reference()? == *reference {
                        return Ok((pair.row == candidate)
                            .then(|| (pair.row.clone(), pair.metadata.clone())));
                    }
                }
                Ok(None)
            }

            fn record_ids(&self) -> Vec<String> {
                ($records)(self)
            }

            fn trace_scope(&self) -> (String, String, Option<String>) {
                ($scope)(self)
            }
        }
    };
}

impl_lineage!(
    OverlapRow,
    ComparisonRowKind::Overlap,
    overlap_rows_with_finality,
    |row: &OverlapRow| row
        .target_record_ids
        .iter()
        .chain(row.against_record_ids.iter())
        .cloned()
        .collect(),
    |row: &OverlapRow| (
        row.window_name.clone(),
        row.key.clone(),
        row.partition.clone()
    )
);
impl_lineage!(
    ResidualRow,
    ComparisonRowKind::Residual,
    residual_rows_with_finality,
    |row: &ResidualRow| row.target_record_ids.clone(),
    |row: &ResidualRow| (
        row.window_name.clone(),
        row.key.clone(),
        row.partition.clone()
    )
);
impl_lineage!(
    MissingRow,
    ComparisonRowKind::Missing,
    missing_rows_with_finality,
    |row: &MissingRow| row.against_record_ids.clone(),
    |row: &MissingRow| (
        row.window_name.clone(),
        row.key.clone(),
        row.partition.clone()
    )
);
impl_lineage!(
    CoverageRow,
    ComparisonRowKind::Coverage,
    coverage_rows_with_finality,
    |row: &CoverageRow| row
        .target_record_ids
        .iter()
        .chain(row.against_record_ids.iter())
        .cloned()
        .collect(),
    |row: &CoverageRow| (
        row.window_name.clone(),
        row.key.clone(),
        row.partition.clone()
    )
);
impl_lineage!(
    GapRow,
    ComparisonRowKind::Gap,
    gap_rows_with_finality,
    |row: &GapRow| row.boundary_record_ids.clone(),
    |row: &GapRow| (
        row.window_name.clone(),
        row.key.clone(),
        row.partition.clone()
    )
);
impl_lineage!(
    SymmetricDifferenceRow,
    ComparisonRowKind::SymmetricDifference,
    symmetric_difference_rows_with_finality,
    |row: &SymmetricDifferenceRow| row
        .target_record_ids
        .iter()
        .chain(row.against_record_ids.iter())
        .cloned()
        .collect(),
    |row: &SymmetricDifferenceRow| (
        row.window_name.clone(),
        row.key.clone(),
        row.partition.clone()
    )
);
impl_lineage!(
    ContainmentRow,
    ComparisonRowKind::Containment,
    containment_rows_with_finality,
    |row: &ContainmentRow| row
        .target_record_ids
        .iter()
        .chain(row.container_record_ids.iter())
        .cloned()
        .collect(),
    |row: &ContainmentRow| (
        row.window_name.clone(),
        row.key.clone(),
        row.partition.clone()
    )
);
impl_lineage!(
    LeadLagRow,
    ComparisonRowKind::LeadLag,
    lead_lag_rows_with_finality,
    |row: &LeadLagRow| row
        .comparison_record_id
        .iter()
        .chain(std::iter::once(&row.target_record_id))
        .cloned()
        .collect(),
    |row: &LeadLagRow| (
        row.window_name.clone(),
        row.key.clone(),
        row.partition.clone()
    )
);
impl_lineage!(
    AsOfRow,
    ComparisonRowKind::AsOf,
    as_of_rows_with_finality,
    |row: &AsOfRow| row
        .matched_record_id
        .iter()
        .chain(std::iter::once(&row.target_record_id))
        .cloned()
        .collect(),
    |row: &AsOfRow| (
        row.window_name.clone(),
        row.key.clone(),
        row.partition.clone()
    )
);

impl ComparisonResult {
    /// Traces a canonical row reference through retained preparation evidence.
    pub fn trace_row(
        &self,
        reference: &ComparisonRowReference,
    ) -> Result<AnyComparisonRowTrace, ComparisonRowTraceError> {
        match reference.kind() {
            ComparisonRowKind::Overlap => Ok(AnyComparisonRowTrace::Overlap(trace_family(
                self,
                reference,
                self.overlap_rows_with_finality()?,
            )?)),
            ComparisonRowKind::Residual => Ok(AnyComparisonRowTrace::Residual(trace_family(
                self,
                reference,
                self.residual_rows_with_finality()?,
            )?)),
            ComparisonRowKind::Missing => Ok(AnyComparisonRowTrace::Missing(trace_family(
                self,
                reference,
                self.missing_rows_with_finality()?,
            )?)),
            ComparisonRowKind::Coverage => Ok(AnyComparisonRowTrace::Coverage(trace_family(
                self,
                reference,
                self.coverage_rows_with_finality()?,
            )?)),
            ComparisonRowKind::Gap => Ok(AnyComparisonRowTrace::Gap(trace_family(
                self,
                reference,
                self.gap_rows_with_finality()?,
            )?)),
            ComparisonRowKind::SymmetricDifference => {
                Ok(AnyComparisonRowTrace::SymmetricDifference(trace_family(
                    self,
                    reference,
                    self.symmetric_difference_rows_with_finality()?,
                )?))
            }
            ComparisonRowKind::Containment => Ok(AnyComparisonRowTrace::Containment(trace_family(
                self,
                reference,
                self.containment_rows_with_finality()?,
            )?)),
            ComparisonRowKind::LeadLag => Ok(AnyComparisonRowTrace::LeadLag(trace_family(
                self,
                reference,
                self.lead_lag_rows_with_finality()?,
            )?)),
            ComparisonRowKind::AsOf => Ok(AnyComparisonRowTrace::AsOf(trace_family(
                self,
                reference,
                self.as_of_rows_with_finality()?,
            )?)),
        }
    }

    /// Traces a typed row view through its canonical result metadata.
    pub fn trace_typed<R>(
        &self,
        row: ComparisonRowWithFinality<'_, R>,
    ) -> Result<ComparisonRowTrace<R>, ComparisonRowTraceError>
    where
        R: ComparisonRowTraceLineage,
    {
        let reference = row.metadata.reference()?;
        if R::KIND != reference.kind() {
            return Err(ComparisonRowTraceError::RowKindMismatch {
                expected: R::KIND,
                actual: reference.kind(),
            });
        }
        let Some((local_row, metadata)) = R::locate(self, row.row, &reference)? else {
            return Err(ComparisonRowTraceError::RowNotFound { reference });
        };
        build_trace(self, local_row, metadata)
    }
}

fn trace_family<'a, R>(
    result: &'a ComparisonResult,
    reference: &ComparisonRowReference,
    rows: impl Iterator<Item = ComparisonRowWithFinality<'a, R>>,
) -> Result<ComparisonRowTrace<R>, ComparisonRowTraceError>
where
    R: ComparisonRowTraceLineage,
{
    for row in rows {
        if row.metadata.reference()? == *reference {
            return build_trace(result, row.row.clone(), row.metadata.clone());
        }
    }
    Err(ComparisonRowTraceError::RowNotFound {
        reference: reference.clone(),
    })
}

fn build_trace<R>(
    result: &ComparisonResult,
    row: R,
    metadata: ComparisonRowFinality,
) -> Result<ComparisonRowTrace<R>, ComparisonRowTraceError>
where
    R: ComparisonRowTraceLineage,
{
    let (Some(prepared), Some(aligned)) = (result.state.prepared(), result.state.aligned()) else {
        return Err(ComparisonRowTraceError::MissingContext);
    };
    let ids = row.record_ids().into_iter().collect::<BTreeSet<_>>();
    let (window_name, key, partition) = row.trace_scope();

    let mut contributing_records = prepared
        .selected_windows
        .iter()
        .filter(|window| ids.contains(&window.record_id))
        .cloned()
        .collect::<Vec<_>>();
    contributing_records.sort_by(|left, right| {
        record_order_key(left, &prepared.normalized_windows)
            .cmp(&record_order_key(right, &prepared.normalized_windows))
    });
    let mut seen_records = BTreeSet::new();
    contributing_records.retain(|record| seen_records.insert(record.record_id.clone()));

    let mut normalized_windows = prepared
        .normalized_windows
        .iter()
        .filter(|window| {
            window
                .record_ids
                .iter()
                .any(|record_id| ids.contains(record_id))
                || ids.contains(&window.record_id)
        })
        .cloned()
        .collect::<Vec<_>>();
    normalized_windows.sort_by(normalized_order);

    let mut aligned_segments = aligned
        .segments
        .iter()
        .filter(|segment| {
            segment
                .target_record_ids
                .iter()
                .chain(segment.against_record_ids.iter())
                .any(|record_id| ids.contains(record_id))
        })
        .cloned()
        .collect::<Vec<_>>();
    aligned_segments.sort_by(|left, right| {
        left.window_name
            .cmp(&right.window_name)
            .then_with(|| left.key.cmp(&right.key))
            .then_with(|| left.partition.cmp(&right.partition))
            .then_with(|| left.range.axis.cmp(&right.range.axis))
            .then_with(|| left.range.clock.cmp(&right.range.clock))
            .then_with(|| left.range.start.cmp(&right.range.start))
            .then_with(|| left.range.end.cmp(&right.range.end))
            .then_with(|| left.target_record_ids.cmp(&right.target_record_ids))
            .then_with(|| left.against_record_ids.cmp(&right.against_record_ids))
            .then_with(|| left.segment_id.cmp(&right.segment_id))
    });

    let mut relevant_exclusions = prepared
        .excluded_windows
        .iter()
        .filter(|exclusion| {
            exclusion.window.window_name == window_name
                && exclusion.window.key == key
                && exclusion.window.partition == partition
        })
        .cloned()
        .collect::<Vec<_>>();
    relevant_exclusions.sort_by(|left, right| {
        left.window
            .source
            .cmp(&right.window.source)
            .then_with(|| {
                left.window
                    .start
                    .magnitude()
                    .cmp(&right.window.start.magnitude())
            })
            .then_with(|| left.window.record_id.cmp(&right.window.record_id))
            .then_with(|| left.reason.cmp(&right.reason))
    });

    Ok(ComparisonRowTrace {
        row,
        metadata,
        contributing_records,
        normalized_windows,
        aligned_segments,
        relevant_exclusions,
    })
}

fn record_order_key(
    record: &WindowArtifact,
    normalized_windows: &[NormalizedWindowRecord],
) -> (u8, i64, i64, String, String) {
    let normalized = normalized_windows
        .iter()
        .filter(|window| {
            window.record_id == record.record_id
                || window
                    .record_ids
                    .iter()
                    .any(|record_id| record_id == &record.record_id)
        })
        .min_by(|left, right| normalized_order(left, right));
    let Some(normalized) = normalized else {
        return (
            2,
            record.start.magnitude(),
            record
                .end
                .as_ref()
                .map_or(i64::MAX, TemporalPoint::magnitude),
            record.source.clone().unwrap_or_default(),
            record.record_id.clone(),
        );
    };
    (
        side_order(&normalized.side),
        normalized.range.start().magnitude(),
        normalized.range.end().magnitude(),
        record.source.clone().unwrap_or_default(),
        record.record_id.clone(),
    )
}

fn normalized_order(
    left: &NormalizedWindowRecord,
    right: &NormalizedWindowRecord,
) -> std::cmp::Ordering {
    side_order(&left.side)
        .cmp(&side_order(&right.side))
        .then_with(|| {
            left.range
                .start()
                .magnitude()
                .cmp(&right.range.start().magnitude())
        })
        .then_with(|| {
            left.range
                .end()
                .magnitude()
                .cmp(&right.range.end().magnitude())
        })
        .then_with(|| left.window.source.cmp(&right.window.source))
        .then_with(|| left.record_id.cmp(&right.record_id))
}

fn side_order(side: &ComparisonSide) -> u8 {
    match side {
        ComparisonSide::Target => 0,
        ComparisonSide::Against => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{WindowHistoryFixture, WindowHistoryImportError};

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
            .closed_window("DeviceOffline", "device-1", 8, 9, |window| {
                window.source("ignored")
            })
            .expect("same-scope exclusion")
            .closed_window("OtherWindow", "device-1", 1, 2, |window| {
                window.source("ignored")
            })
            .expect("other-scope exclusion")
            .build();
        let plan = ComparisonPlan::new(
            "All row families",
            "provider-a",
            AgainstSelection::Sources(vec!["provider-b".to_owned()]),
            vec![
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
        )
        .with_scope_window(Some("DeviceOffline".to_owned()))
        .with_require_closed_windows(true);

        compare(&history, &plan)
    }

    fn assert_typed_trace<R>(
        result: &ComparisonResult,
        row: ComparisonRowWithFinality<'_, R>,
        expected_record_ids: &[&str],
    ) where
        R: ComparisonRowTraceLineage,
    {
        let reference = row.metadata.reference().expect("canonical reference");
        let trace = result.trace_typed(row).expect("typed trace");
        assert_eq!(trace.reference().expect("trace reference"), reference);
        assert_eq!(
            trace
                .contributing_records()
                .iter()
                .map(|record| record.record_id.as_str())
                .collect::<Vec<_>>(),
            expected_record_ids
        );
        assert!(!trace.normalized_windows().is_empty());
        assert!(!trace.aligned_segments().is_empty());
        assert_eq!(
            trace
                .relevant_exclusions()
                .iter()
                .map(|exclusion| exclusion.window.record_id.as_str())
                .collect::<Vec<_>>(),
            vec!["window-0004"]
        );
        assert_eq!(
            result
                .trace_row(&reference)
                .expect("erased trace")
                .reference()
                .expect("erased reference"),
            reference
        );
    }

    #[test]
    fn traces_all_nine_row_families_through_one_canonical_reference() {
        let result = all_row_family_result();

        assert_typed_trace(
            &result,
            result.overlap_rows_with_finality().unwrap().next().unwrap(),
            &["window-0000", "window-0002"],
        );
        assert_typed_trace(
            &result,
            result
                .residual_rows_with_finality()
                .unwrap()
                .next()
                .unwrap(),
            &["window-0000"],
        );
        assert_typed_trace(
            &result,
            result.missing_rows_with_finality().unwrap().next().unwrap(),
            &["window-0002"],
        );
        assert_typed_trace(
            &result,
            result
                .coverage_rows_with_finality()
                .unwrap()
                .next()
                .unwrap(),
            &["window-0000"],
        );
        let gap = result.gap_rows_with_finality().unwrap().next().unwrap();
        assert!(!gap.row.boundary_record_ids.is_empty());
        assert_typed_trace(&result, gap, &["window-0001", "window-0002"]);
        assert_typed_trace(
            &result,
            result
                .symmetric_difference_rows_with_finality()
                .unwrap()
                .next()
                .unwrap(),
            &["window-0000"],
        );
        assert_typed_trace(
            &result,
            result
                .containment_rows_with_finality()
                .unwrap()
                .next()
                .unwrap(),
            &["window-0000"],
        );
        assert_typed_trace(
            &result,
            result
                .lead_lag_rows_with_finality()
                .unwrap()
                .next()
                .unwrap(),
            &["window-0000", "window-0002"],
        );
        assert_typed_trace(
            &result,
            result.as_of_rows_with_finality().unwrap().next().unwrap(),
            &["window-0000"],
        );
    }

    #[test]
    fn gap_boundaries_are_sorted_and_drive_provisional_finality() {
        let history = WindowHistoryFixture::new()
            .closed_window("DeviceOffline", "device-1", 1, 3, |window| {
                window.source("target")
            })
            .expect("target")
            .open_window("DeviceOffline", "device-1", 5, |window| {
                window.source("against")
            })
            .expect("open against")
            .build();
        let target_id = history.closed_windows()[0].id.as_str().to_owned();
        let against_id = history.open_windows()[0].id.as_str().to_owned();
        let plan = ComparisonPlan::new(
            "Gap finality",
            "target",
            AgainstSelection::Sources(vec!["against".to_owned()]),
            vec![Comparator::Gap],
        )
        .with_scope_window(Some("DeviceOffline".to_owned()))
        .with_require_closed_windows(false)
        .with_open_window_policy(
            OpenWindowPolicy::ClipToHorizon,
            Some(TemporalPoint::position(8)),
        );
        let result = compare_live(&history, &plan, TemporalPoint::position(8));
        let row = result.gap_rows.first().expect("internal gap");
        assert_eq!(row.range.start, 3);
        assert_eq!(row.range.end, 5);
        assert_eq!(
            row.boundary_record_ids,
            vec![target_id.clone(), against_id.clone()]
        );
        let metadata = result.gap_rows_with_finality().unwrap().next().unwrap();
        assert_eq!(metadata.metadata.finality, ComparisonFinality::Provisional);
        let trace = result.trace_typed(metadata).expect("gap trace");
        assert_eq!(
            trace
                .contributing_records()
                .iter()
                .map(|record| record.record_id.as_str())
                .collect::<Vec<_>>(),
            vec![target_id.as_str(), against_id.as_str()]
        );
        let open_artifact = trace
            .contributing_records()
            .iter()
            .find(|record| record.record_id == against_id)
            .expect("open artifact");
        assert!(open_artifact.is_open);
        assert_eq!(open_artifact.start, TemporalPoint::position(5));
        assert!(open_artifact.end.is_none());
    }

    #[test]
    fn trace_rejects_blank_unknown_and_missing_references() {
        let result = all_row_family_result();
        assert!(matches!(
            ComparisonRowReference::new(ComparisonRowKind::Residual, " "),
            Err(ComparisonRowReferenceError::EmptyRowId)
        ));

        let unknown = ComparisonRowFinality {
            row_type: "futureFamily".to_owned(),
            row_id: "row-1".to_owned(),
            finality: ComparisonFinality::Final,
            reason: "test".to_owned(),
            version: 1,
            supersedes_row_id: None,
        };
        assert!(matches!(
            unknown.reference(),
            Err(ComparisonRowReferenceError::UnknownKind(_))
        ));

        let reference = ComparisonRowReference::new(ComparisonRowKind::Residual, "other-result")
            .expect("valid opaque reference");
        assert!(matches!(
            result.trace_row(&reference),
            Err(ComparisonRowTraceError::RowNotFound { .. })
        ));
    }

    #[test]
    fn imported_history_rebuilds_indexes_and_rejects_duplicate_open_ids() {
        let source = WindowHistoryFixture::new()
            .closed_window("DeviceOffline", "b", 5, 6, |window| window.source("source"))
            .expect("first closed")
            .closed_window("DeviceOffline", "a", 1, 2, |window| window.source("source"))
            .expect("second closed")
            .open_window("DeviceOffline", "c", 7, |window| window.source("source"))
            .expect("first open")
            .open_window("DeviceOffline", "d", 9, |window| window.source("source"))
            .expect("second open")
            .build();
        let mut closed = source.closed_windows().to_vec();
        closed.reverse();
        let mut imported = WindowHistory::from_records(closed, source.open_windows().to_vec())
            .expect("materialized import");
        let query = imported
            .query()
            .where_window("DeviceOffline")
            .windows()
            .into_iter()
            .map(|window| window.key().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(query, vec!["a", "b", "c", "d"]);

        let second_open_id = source.open_windows()[1].id.clone();
        let removed = imported
            .remove_open(&second_open_id)
            .expect("rebuilt index locates non-first open record");
        assert_eq!(removed.id, second_open_id);
        assert_eq!(imported.open_windows(), &source.open_windows()[..1]);

        let open = source.open_windows()[0].clone();
        assert!(matches!(
            WindowHistory::from_records(Vec::new(), vec![open.clone(), open]),
            Err(WindowHistoryImportError::DuplicateOpenRecordId { .. })
        ));
    }
}
