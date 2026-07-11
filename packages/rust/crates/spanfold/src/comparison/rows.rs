//! Result rows, summaries, and finality DTOs.

use super::*;

/// Diagnostic severity.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum DiagnosticSeverity {
    /// Warning-level diagnostic.
    Warning,
    /// Error-level diagnostic.
    Error,
}

/// Structured comparison diagnostic.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ComparisonDiagnostic {
    /// Diagnostic code.
    pub code: String,
    /// Diagnostic severity.
    pub severity: DiagnosticSeverity,
}

impl ComparisonDiagnostic {
    /// Returns an actionable remediation hint for this diagnostic code.
    #[must_use]
    pub fn message(&self) -> &'static str {
        match self.code.as_str() {
            "MissingName" => "set a non-empty comparison plan name",
            "MissingTarget" => "configure a target source or selector",
            "MissingAgainst" => "configure at least one comparison source or selector",
            "MissingComparator" => "configure at least one comparator",
            "FutureWindowExcluded" => "advance known-at or provide an earlier-available window",
            "MissingEventTime" => {
                "provide event timestamps or choose processing-position normalization"
            }
            "TemporalAxisMismatch" => "align the plan axis with the recorded window axis",
            "SelfComparison" => "make target and comparison selectors disjoint",
            "RuntimeNonSerializablePlan" => "use serializable selectors for portable execution",
            _ => "inspect the prepared artifact and plan fields for the invalid contract",
        }
    }
}

/// Comparator summary.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ComparatorSummary {
    /// Comparator name.
    #[serde(rename = "comparatorName")]
    pub comparator_name: String,
    /// Row count.
    #[serde(rename = "rowCount")]
    pub row_count: usize,
}

/// Exported range for a row.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RowRange {
    /// Inclusive start magnitude.
    pub start: i64,
    /// Exclusive end magnitude.
    pub end: i64,
    /// Temporal axis governing the magnitudes.
    pub axis: TemporalAxis,
    /// Timestamp clock identity, when applicable.
    pub clock: Option<String>,
}

/// Exported point for transition-based rows.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RowPoint {
    /// Point axis.
    pub axis: TemporalAxis,
    /// Scalar point magnitude.
    pub magnitude: i64,
    /// Clock identity for timestamp points.
    pub clock: Option<String>,
}

/// The active side for a disagreement segment.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum ComparisonSide {
    /// Target side.
    Target,
    /// Comparison side.
    Against,
}

/// Containment classification for one target-active segment.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum ContainmentStatus {
    /// Segment is covered by at least one comparison window.
    Contained,
    /// Segment is not covered by comparison windows.
    NotContained,
    /// Segment starts at the left edge of the target without coverage.
    LeftOverhang,
    /// Segment ends at the right edge of the target without coverage.
    RightOverhang,
}

/// Transition point used for lead/lag measurement.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum LeadLagTransition {
    /// Compare start transitions.
    Start,
    /// Compare end transitions.
    End,
}

/// Lead/lag direction.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum LeadLagDirection {
    /// Target and comparison are equal.
    Equal,
    /// Target transition occurs first.
    TargetLeads,
    /// Target transition occurs later.
    TargetLags,
    /// No comparison transition exists.
    MissingComparison,
}

/// Summary for one lead/lag comparator declaration.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct LeadLagSummary {
    /// Transition point measured.
    #[serde(rename = "transition")]
    pub transition: LeadLagTransition,
    /// Axis used for measurement.
    #[serde(rename = "axis")]
    pub axis: TemporalAxis,
    /// Configured tolerance.
    #[serde(rename = "toleranceMagnitude")]
    pub tolerance_magnitude: i64,
    /// Number of emitted rows.
    #[serde(rename = "rowCount")]
    pub row_count: usize,
    /// Count of target-lead rows.
    #[serde(rename = "targetLeadCount")]
    pub target_lead_count: usize,
    /// Count of target-lag rows.
    #[serde(rename = "targetLagCount")]
    pub target_lag_count: usize,
    /// Count of equal rows.
    #[serde(rename = "equalCount")]
    pub equal_count: usize,
    /// Count of missing-comparison rows.
    #[serde(rename = "missingComparisonCount")]
    pub missing_comparison_count: usize,
    /// Count of rows outside tolerance.
    #[serde(rename = "outsideToleranceCount")]
    pub outside_tolerance_count: usize,
    /// Minimum signed delta when any paired transitions exist.
    #[serde(rename = "minimumDeltaMagnitude")]
    pub minimum_delta_magnitude: Option<i64>,
    /// Maximum signed delta when any paired transitions exist.
    #[serde(rename = "maximumDeltaMagnitude")]
    pub maximum_delta_magnitude: Option<i64>,
}

/// Coverage summary for one comparison scope.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CoverageSummary {
    /// Window family.
    #[serde(rename = "windowName")]
    pub window_name: String,
    /// Logical key.
    pub key: String,
    /// Optional partition.
    pub partition: Option<String>,
    /// Denominator magnitude.
    #[serde(rename = "targetMagnitude")]
    pub target_magnitude: f64,
    /// Exact integer denominator before presentation conversion.
    #[serde(rename = "targetMagnitudeExact")]
    pub target_magnitude_exact: i128,
    /// Covered numerator magnitude.
    #[serde(rename = "coveredMagnitude")]
    pub covered_magnitude: f64,
    /// Exact integer numerator before presentation conversion.
    #[serde(rename = "coveredMagnitudeExact")]
    pub covered_magnitude_exact: i128,
    /// Covered ratio.
    #[serde(rename = "coverageRatio")]
    pub coverage_ratio: f64,
}

/// Finality state for an emitted row.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ComparisonFinality {
    /// Row is final.
    Final,
    /// Row depends on clipped open windows.
    Provisional,
    /// Row supersedes a prior version.
    Revised,
    /// Row was removed in a later snapshot.
    Retracted,
}

/// Finality metadata for a materialized row.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ComparisonRowFinality {
    /// Exported row family.
    #[serde(rename = "rowType")]
    pub row_type: String,
    /// Deterministic row identifier.
    #[serde(rename = "rowId")]
    pub row_id: String,
    /// Finality state.
    pub finality: ComparisonFinality,
    /// Human-readable reason.
    pub reason: String,
    /// Metadata version.
    pub version: u32,
    /// Superseded row identifier, when any.
    #[serde(rename = "supersedesRowId")]
    pub supersedes_row_id: Option<String>,
}

/// As-of lookup direction.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum AsOfDirection {
    /// Match the latest comparison transition at or before the target point.
    Previous,
    /// Match the earliest comparison transition at or after the target point.
    Next,
    /// Match the nearest comparison transition on either side.
    Nearest,
}

/// As-of lookup status.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum AsOfMatchStatus {
    /// Exact point match.
    Exact,
    /// Matched within tolerance.
    Matched,
    /// No match inside tolerance.
    NoMatch,
    /// A future point existed but was rejected.
    FutureRejected,
    /// Multiple eligible matches existed; selection is deterministic.
    Ambiguous,
}

/// Overlap row.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct OverlapRow {
    /// Window family.
    #[serde(rename = "windowName")]
    pub window_name: String,
    /// Logical key.
    pub key: String,
    /// Optional partition.
    pub partition: Option<String>,
    /// Overlap range.
    pub range: RowRange,
    /// Target record IDs.
    #[serde(rename = "targetRecordIds")]
    pub target_record_ids: Vec<String>,
    /// Against record IDs.
    #[serde(rename = "againstRecordIds")]
    pub against_record_ids: Vec<String>,
}

/// Residual row.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ResidualRow {
    /// Window family.
    #[serde(rename = "windowName")]
    pub window_name: String,
    /// Logical key.
    pub key: String,
    /// Optional partition.
    pub partition: Option<String>,
    /// Target-only range.
    pub range: RowRange,
    /// Target record IDs.
    #[serde(rename = "targetRecordIds")]
    pub target_record_ids: Vec<String>,
}

/// Missing row.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MissingRow {
    /// Window family.
    #[serde(rename = "windowName")]
    pub window_name: String,
    /// Logical key.
    pub key: String,
    /// Optional partition.
    pub partition: Option<String>,
    /// Comparison-only range.
    pub range: RowRange,
    /// Against record IDs.
    #[serde(rename = "againstRecordIds")]
    pub against_record_ids: Vec<String>,
}

/// Coverage row.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CoverageRow {
    /// Window family.
    #[serde(rename = "windowName")]
    pub window_name: String,
    /// Logical key.
    pub key: String,
    /// Optional partition.
    pub partition: Option<String>,
    /// Target segment range.
    pub range: RowRange,
    /// Segment magnitude.
    #[serde(rename = "targetMagnitude")]
    pub target_magnitude: i64,
    /// Covered magnitude.
    #[serde(rename = "coveredMagnitude")]
    pub covered_magnitude: i64,
    /// Target record IDs.
    #[serde(rename = "targetRecordIds")]
    pub target_record_ids: Vec<String>,
    /// Against record IDs.
    #[serde(rename = "againstRecordIds")]
    pub against_record_ids: Vec<String>,
}

/// Gap row.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GapRow {
    /// Window family.
    #[serde(rename = "windowName")]
    pub window_name: String,
    /// Logical key.
    pub key: String,
    /// Optional partition.
    pub partition: Option<String>,
    /// Gap range.
    pub range: RowRange,
}

/// Symmetric-difference row.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SymmetricDifferenceRow {
    /// Window family.
    #[serde(rename = "windowName")]
    pub window_name: String,
    /// Logical key.
    pub key: String,
    /// Optional partition.
    pub partition: Option<String>,
    /// Disagreement range.
    pub range: RowRange,
    /// Active disagreement side.
    pub side: ComparisonSide,
    /// Target record IDs.
    #[serde(rename = "targetRecordIds")]
    pub target_record_ids: Vec<String>,
    /// Against record IDs.
    #[serde(rename = "againstRecordIds")]
    pub against_record_ids: Vec<String>,
}

/// Containment row.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ContainmentRow {
    /// Window family.
    #[serde(rename = "windowName")]
    pub window_name: String,
    /// Logical key.
    pub key: String,
    /// Optional partition.
    pub partition: Option<String>,
    /// Checked range.
    pub range: RowRange,
    /// Containment status.
    pub status: ContainmentStatus,
    /// Target record IDs.
    #[serde(rename = "targetRecordIds")]
    pub target_record_ids: Vec<String>,
    /// Container record IDs.
    #[serde(rename = "containerRecordIds")]
    pub container_record_ids: Vec<String>,
}

/// Lead/lag row.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct LeadLagRow {
    /// Window family.
    #[serde(rename = "windowName")]
    pub window_name: String,
    /// Logical key.
    pub key: String,
    /// Optional partition.
    pub partition: Option<String>,
    /// Measured transition.
    pub transition: LeadLagTransition,
    /// Measurement axis.
    pub axis: TemporalAxis,
    /// Target transition point.
    #[serde(rename = "targetPoint")]
    pub target_point: RowPoint,
    /// Comparison transition point, when any.
    #[serde(rename = "comparisonPoint")]
    pub comparison_point: Option<RowPoint>,
    /// Signed target-minus-comparison delta.
    #[serde(rename = "deltaMagnitude")]
    pub delta_magnitude: Option<i64>,
    /// Configured tolerance.
    #[serde(rename = "toleranceMagnitude")]
    pub tolerance_magnitude: i64,
    /// Whether the row is inside tolerance.
    #[serde(rename = "isWithinTolerance")]
    pub is_within_tolerance: bool,
    /// Lead/lag direction.
    pub direction: LeadLagDirection,
    /// Target record ID.
    #[serde(rename = "targetRecordId")]
    pub target_record_id: String,
    /// Comparison record ID, when any.
    #[serde(rename = "comparisonRecordId")]
    pub comparison_record_id: Option<String>,
}

/// As-of row.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AsOfRow {
    /// Window family.
    #[serde(rename = "windowName")]
    pub window_name: String,
    /// Logical key.
    pub key: String,
    /// Optional partition.
    pub partition: Option<String>,
    /// Lookup axis.
    pub axis: TemporalAxis,
    /// Lookup direction.
    pub direction: AsOfDirection,
    /// Target lookup point.
    #[serde(rename = "targetPoint")]
    pub target_point: RowPoint,
    /// Matched comparison point, when any.
    #[serde(rename = "matchedPoint")]
    pub matched_point: Option<RowPoint>,
    /// Absolute point distance, when evaluated.
    #[serde(rename = "distanceMagnitude")]
    pub distance_magnitude: Option<i64>,
    /// Configured tolerance.
    #[serde(rename = "toleranceMagnitude")]
    pub tolerance_magnitude: i64,
    /// Match status.
    pub status: AsOfMatchStatus,
    /// Target record ID.
    #[serde(rename = "targetRecordId")]
    pub target_record_id: String,
    /// Matched comparison record ID, when any.
    #[serde(rename = "matchedRecordId")]
    pub matched_record_id: Option<String>,
}

/// Comparator row collections.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ComparisonRows {
    /// Overlap rows.
    pub overlap: Arc<Vec<OverlapRow>>,
    /// Residual rows.
    pub residual: Arc<Vec<ResidualRow>>,
    /// Missing rows.
    pub missing: Arc<Vec<MissingRow>>,
    /// Coverage rows.
    pub coverage: Arc<Vec<CoverageRow>>,
    /// Gap rows.
    pub gap: Arc<Vec<GapRow>>,
    /// Symmetric-difference rows.
    #[serde(rename = "symmetricDifference")]
    pub symmetric_difference: Arc<Vec<SymmetricDifferenceRow>>,
    /// Containment rows.
    pub containment: Arc<Vec<ContainmentRow>>,
    /// Lead/lag rows.
    #[serde(rename = "leadLag")]
    pub lead_lag: Arc<Vec<LeadLagRow>>,
    /// As-of rows.
    #[serde(rename = "asOf")]
    pub as_of: Arc<Vec<AsOfRow>>,
}

impl Default for ComparisonRows {
    fn default() -> Self {
        Self {
            overlap: Arc::new(Vec::new()),
            residual: Arc::new(Vec::new()),
            missing: Arc::new(Vec::new()),
            coverage: Arc::new(Vec::new()),
            gap: Arc::new(Vec::new()),
            symmetric_difference: Arc::new(Vec::new()),
            containment: Arc::new(Vec::new()),
            lead_lag: Arc::new(Vec::new()),
            as_of: Arc::new(Vec::new()),
        }
    }
}

#[derive(Default)]
pub(super) struct RowAccumulator {
    pub(super) overlap: Vec<OverlapRow>,
    pub(super) residual: Vec<ResidualRow>,
    pub(super) missing: Vec<MissingRow>,
    pub(super) coverage: Vec<CoverageRow>,
    pub(super) gap: Vec<GapRow>,
    pub(super) symmetric_difference: Vec<SymmetricDifferenceRow>,
    pub(super) containment: Vec<ContainmentRow>,
    pub(super) lead_lag: Vec<LeadLagRow>,
    pub(super) as_of: Vec<AsOfRow>,
}

impl RowAccumulator {
    pub(super) fn into_shared(self) -> ComparisonRows {
        ComparisonRows {
            overlap: Arc::new(self.overlap),
            residual: Arc::new(self.residual),
            missing: Arc::new(self.missing),
            coverage: Arc::new(self.coverage),
            gap: Arc::new(self.gap),
            symmetric_difference: Arc::new(self.symmetric_difference),
            containment: Arc::new(self.containment),
            lead_lag: Arc::new(self.lead_lag),
            as_of: Arc::new(self.as_of),
        }
    }
}

/// Structured comparison result.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ComparisonResult {
    /// Result schema.
    pub schema: String,
    /// Schema version.
    #[serde(rename = "schemaVersion")]
    pub schema_version: u32,
    /// Artifact kind.
    pub artifact: String,
    /// Comparison plan.
    #[serde(skip)]
    pub plan: ComparisonPlan,
    /// Comparison plan name.
    #[serde(rename = "planName")]
    pub plan_name: String,
    /// Whether the result is valid.
    #[serde(rename = "isValid")]
    pub is_valid: bool,
    /// Validation and execution diagnostics.
    pub diagnostics: Vec<ComparisonDiagnostic>,
    /// Prepared artifact, when available.
    pub prepared: Option<Value>,
    /// Aligned artifact, when available.
    pub aligned: Option<Value>,
    /// Known-at point, when available.
    #[serde(rename = "knownAt")]
    pub known_at: Option<RowPoint>,
    /// Evaluation horizon, when available.
    #[serde(rename = "evaluationHorizon")]
    pub evaluation_horizon: Option<RowPoint>,
    /// Comparator summaries.
    #[serde(rename = "comparatorSummaries")]
    pub comparator_summaries: Vec<ComparatorSummary>,
    /// Coverage summaries.
    #[serde(rename = "coverageSummaries")]
    pub coverage_summaries: Vec<CoverageSummary>,
    /// Result rows grouped by family.
    pub rows: ComparisonRows,
    /// Overlap rows.
    #[serde(skip)]
    pub overlap_rows: Arc<Vec<OverlapRow>>,
    /// Residual rows.
    #[serde(skip)]
    pub residual_rows: Arc<Vec<ResidualRow>>,
    /// Missing rows.
    #[serde(skip)]
    pub missing_rows: Arc<Vec<MissingRow>>,
    /// Coverage rows.
    #[serde(skip)]
    pub coverage_rows: Arc<Vec<CoverageRow>>,
    /// Gap rows.
    #[serde(skip)]
    pub gap_rows: Arc<Vec<GapRow>>,
    /// Symmetric-difference rows.
    #[serde(skip)]
    pub symmetric_difference_rows: Arc<Vec<SymmetricDifferenceRow>>,
    /// Containment rows.
    #[serde(skip)]
    pub containment_rows: Arc<Vec<ContainmentRow>>,
    /// Lead/lag rows.
    #[serde(skip)]
    pub lead_lag_rows: Arc<Vec<LeadLagRow>>,
    /// Lead/lag summaries.
    #[serde(skip)]
    pub lead_lag_summaries: Vec<LeadLagSummary>,
    /// As-of rows.
    #[serde(skip)]
    pub as_of_rows: Arc<Vec<AsOfRow>>,
    /// Row finality metadata.
    #[serde(rename = "rowFinalities")]
    pub row_finalities: Vec<ComparisonRowFinality>,
    /// Serializable extension metadata.
    #[serde(rename = "extensionMetadata")]
    pub extension_metadata: Vec<ComparisonExtensionMetadata>,
}
