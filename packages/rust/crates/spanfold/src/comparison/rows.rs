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

/// Aggregate coverage summary for one window name, key, and partition.
///
/// Unlike [`CoverageRow`], which describes one aligned target segment, this
/// type contains the grouped numerator, denominator, and ratio consumers should
/// use when reporting overall coverage. The exact integer numerator and
/// denominator are the authority for aggregate coverage.
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

/// Closed family of rows emitted by comparison results.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ComparisonRowKind {
    /// Overlap rows.
    Overlap,
    /// Residual rows.
    Residual,
    /// Missing rows.
    Missing,
    /// Coverage rows.
    Coverage,
    /// Gap rows.
    Gap,
    /// Symmetric-difference rows.
    SymmetricDifference,
    /// Containment rows.
    Containment,
    /// Lead/lag rows.
    LeadLag,
    /// As-of rows.
    AsOf,
}

impl ComparisonRowKind {
    /// Returns the canonical comparison-artifact spelling for this row kind.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Overlap => "overlap",
            Self::Residual => "residual",
            Self::Missing => "missing",
            Self::Coverage => "coverage",
            Self::Gap => "gap",
            Self::SymmetricDifference => "symmetricDifference",
            Self::Containment => "containment",
            Self::LeadLag => "leadLag",
            Self::AsOf => "asOf",
        }
    }
}

/// Canonical identity for one row in a materialized comparison result.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ComparisonRowReference {
    kind: ComparisonRowKind,
    row_id: String,
}

impl ComparisonRowReference {
    /// Creates a validated canonical row reference.
    pub fn new(
        kind: ComparisonRowKind,
        row_id: impl Into<String>,
    ) -> Result<Self, ComparisonRowReferenceError> {
        let row_id = row_id.into();
        if row_id.trim().is_empty() {
            return Err(ComparisonRowReferenceError::EmptyRowId);
        }
        Ok(Self { kind, row_id })
    }

    /// Returns the closed comparison row family.
    #[must_use]
    pub const fn kind(&self) -> ComparisonRowKind {
        self.kind
    }

    /// Returns the opaque row identifier.
    #[must_use]
    pub fn row_id(&self) -> &str {
        &self.row_id
    }
}

impl std::fmt::Display for ComparisonRowReference {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}:{}", self.kind, self.row_id)
    }
}

/// Error returned when a canonical comparison-row reference cannot be built.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ComparisonRowReferenceError {
    /// The row-family label is not one of the closed comparison families.
    #[error(transparent)]
    UnknownKind(#[from] ComparisonRowKindParseError),
    /// A canonical row reference cannot contain a blank row ID.
    #[error("comparison row id cannot be blank")]
    EmptyRowId,
}

impl std::fmt::Display for ComparisonRowKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Error returned when a comparison row-kind label is unsupported.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("unsupported comparison row kind '{0}'")]
pub struct ComparisonRowKindParseError(String);

impl std::str::FromStr for ComparisonRowKind {
    type Err = ComparisonRowKindParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "overlap" => Ok(Self::Overlap),
            "residual" => Ok(Self::Residual),
            "missing" => Ok(Self::Missing),
            "coverage" => Ok(Self::Coverage),
            "gap" => Ok(Self::Gap),
            "symmetricDifference" | "symmetric-difference" => Ok(Self::SymmetricDifference),
            "containment" => Ok(Self::Containment),
            "leadLag" | "lead-lag" => Ok(Self::LeadLag),
            "asOf" | "asof" => Ok(Self::AsOf),
            _ => Err(ComparisonRowKindParseError(value.to_owned())),
        }
    }
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
    /// Canonical exported row family.
    #[serde(rename = "rowType")]
    pub row_type: String,
    /// Opaque deterministic identifier assigned by the producing Rust result.
    ///
    /// Consumers should preserve this value rather than recomputing it. The
    /// current scheme is scoped to the Rust artifact/schema contract and is
    /// not promised to be permanent or identical to .NET identifiers.
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

impl ComparisonRowFinality {
    /// Returns the typed row kind represented by this metadata.
    ///
    /// Canonical artifact spellings and the three Rust 0.1.0 JSON Lines aliases
    /// are accepted.
    pub fn row_kind(&self) -> Result<ComparisonRowKind, ComparisonRowKindParseError> {
        self.row_type.parse()
    }

    /// Returns the validated canonical reference represented by this metadata.
    pub fn reference(&self) -> Result<ComparisonRowReference, ComparisonRowReferenceError> {
        ComparisonRowReference::new(self.row_kind()?, self.row_id.clone())
    }
}

/// Borrowed association between a typed comparison row and its result metadata.
#[derive(Clone, Copy, Debug)]
pub struct ComparisonRowWithFinality<'a, R> {
    /// Typed row emitted by the comparison.
    pub row: &'a R,
    /// Authoritative identity and finality produced for the row.
    pub metadata: &'a ComparisonRowFinality,
}

/// Error returned when result rows and finality metadata do not share the
/// canonical count/kind layout.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error(
    "inconsistent {family:?} row metadata at index {metadata_index}: expected \
     {expected_count} {expected_kind:?} records, found {actual_count}; actual \
     kind: {actual_kind:?}"
)]
pub struct ComparisonRowMetadataError {
    /// Row family being validated.
    pub family: ComparisonRowKind,
    /// Absolute index in `ComparisonResult::row_finalities` where validation failed.
    pub metadata_index: usize,
    /// Number of metadata records expected for the family.
    pub expected_count: usize,
    /// Number of metadata records observed in the family's layout span.
    pub actual_count: usize,
    /// Row kind expected at the failing metadata index.
    pub expected_kind: ComparisonRowKind,
    /// Raw row-kind label found at the index, or `None` when metadata is absent.
    pub actual_kind: Option<String>,
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

/// Coverage for one aligned target-active segment.
///
/// A segment is normally either wholly covered or wholly uncovered, so
/// `covered_magnitude` is normally either zero or `target_magnitude`. Use
/// [`CoverageSummary`] rather than per-row ratios for grouped overall coverage.
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
    /// Records bounding the gap and determining its live finality.
    #[serde(rename = "boundaryRecordIds")]
    pub boundary_record_ids: Vec<String>,
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

/// Canonical comparator row collections used for storage and serialization.
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

impl ComparisonRows {
    pub(super) fn family_layouts(&self) -> [(ComparisonRowKind, usize); 9] {
        macro_rules! layouts {
            ($(($kind:ident, $rows:ident, $compat:ident, $view:ident, $debug:literal, $count:literal),)*) => {
                [$(
                    (ComparisonRowKind::$kind, self.$rows.len()),
                )*]
            };
        }
        for_each_comparison_row_family!(layouts)
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
///
/// [`Self::rows`] and [`Self::row_finalities`] are the public result data. For
/// genuine, unmodified Spanfold results,
/// finality metadata is partitioned in the same family order and remains
/// parallel to row order within each family.
/// The typed `*_rows_with_finality` views validate detectable count and kind
/// corruption before exposing that association.
///
/// Independently replacing or reordering the public row or metadata fields is
/// unsupported. Validation deliberately does not rehash rows, so it cannot
/// detect metadata reordered within the same family.
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
    /// Canonical result rows grouped by family.
    pub rows: ComparisonRows,
    /// Lead/lag summaries.
    #[serde(skip)]
    pub lead_lag_summaries: Vec<LeadLagSummary>,
    /// Authoritative row identity and finality metadata in canonical family
    /// and row order.
    #[serde(rename = "rowFinalities")]
    pub row_finalities: Vec<ComparisonRowFinality>,
    /// Serializable extension metadata.
    #[serde(rename = "extensionMetadata")]
    pub extension_metadata: Vec<ComparisonExtensionMetadata>,
    #[serde(skip)]
    pub(super) state: Arc<super::state::ComparisonResultState>,
}

impl ComparisonResult {
    /// Returns canonical grouped result rows without allocating row copies.
    pub fn rows(&self) -> &ComparisonRows {
        &self.rows
    }

    pub(crate) fn canonical_rows(&self) -> &ComparisonRows {
        &self.rows
    }

    pub(crate) fn canonical_row_finalities(&self) -> impl Iterator<Item = &ComparisonRowFinality> {
        self.state.finalities()
    }

    /// Returns overlap rows borrowed from canonical grouped storage.
    pub fn overlap_rows(&self) -> &[OverlapRow] {
        self.rows.overlap.as_slice()
    }

    /// Returns residual rows borrowed from canonical grouped storage.
    pub fn residual_rows(&self) -> &[ResidualRow] {
        self.rows.residual.as_slice()
    }

    /// Returns missing rows borrowed from canonical grouped storage.
    pub fn missing_rows(&self) -> &[MissingRow] {
        self.rows.missing.as_slice()
    }

    /// Returns coverage rows borrowed from canonical grouped storage.
    pub fn coverage_rows(&self) -> &[CoverageRow] {
        self.rows.coverage.as_slice()
    }

    /// Returns gap rows borrowed from canonical grouped storage.
    pub fn gap_rows(&self) -> &[GapRow] {
        self.rows.gap.as_slice()
    }

    /// Returns symmetric-difference rows borrowed from canonical grouped storage.
    pub fn symmetric_difference_rows(&self) -> &[SymmetricDifferenceRow] {
        self.rows.symmetric_difference.as_slice()
    }

    /// Returns containment rows borrowed from canonical grouped storage.
    pub fn containment_rows(&self) -> &[ContainmentRow] {
        self.rows.containment.as_slice()
    }

    /// Returns lead/lag rows borrowed from canonical grouped storage.
    pub fn lead_lag_rows(&self) -> &[LeadLagRow] {
        self.rows.lead_lag.as_slice()
    }

    /// Returns as-of rows borrowed from canonical grouped storage.
    pub fn as_of_rows(&self) -> &[AsOfRow] {
        self.rows.as_of.as_slice()
    }

    /// Returns overlap rows paired with their authoritative result metadata.
    ///
    /// Returns an error if the result's metadata count/kind layout is inconsistent.
    pub fn overlap_rows_with_finality(
        &self,
    ) -> Result<
        impl ExactSizeIterator<Item = ComparisonRowWithFinality<'_, OverlapRow>>,
        ComparisonRowMetadataError,
    > {
        row_finality_pairs(
            self,
            ComparisonRowKind::Overlap,
            self.rows.overlap.as_slice(),
        )
    }

    /// Returns residual rows paired with their authoritative result metadata.
    ///
    /// Returns an error if the result's metadata count/kind layout is inconsistent.
    pub fn residual_rows_with_finality(
        &self,
    ) -> Result<
        impl ExactSizeIterator<Item = ComparisonRowWithFinality<'_, ResidualRow>>,
        ComparisonRowMetadataError,
    > {
        row_finality_pairs(
            self,
            ComparisonRowKind::Residual,
            self.rows.residual.as_slice(),
        )
    }

    /// Returns missing rows paired with their authoritative result metadata.
    ///
    /// Returns an error if the result's metadata count/kind layout is inconsistent.
    pub fn missing_rows_with_finality(
        &self,
    ) -> Result<
        impl ExactSizeIterator<Item = ComparisonRowWithFinality<'_, MissingRow>>,
        ComparisonRowMetadataError,
    > {
        row_finality_pairs(
            self,
            ComparisonRowKind::Missing,
            self.rows.missing.as_slice(),
        )
    }

    /// Returns coverage rows paired with their authoritative result metadata.
    ///
    /// Returns an error if the result's metadata count/kind layout is inconsistent.
    pub fn coverage_rows_with_finality(
        &self,
    ) -> Result<
        impl ExactSizeIterator<Item = ComparisonRowWithFinality<'_, CoverageRow>>,
        ComparisonRowMetadataError,
    > {
        row_finality_pairs(
            self,
            ComparisonRowKind::Coverage,
            self.rows.coverage.as_slice(),
        )
    }

    /// Returns gap rows paired with their authoritative result metadata.
    ///
    /// Returns an error if the result's metadata count/kind layout is inconsistent.
    pub fn gap_rows_with_finality(
        &self,
    ) -> Result<
        impl ExactSizeIterator<Item = ComparisonRowWithFinality<'_, GapRow>>,
        ComparisonRowMetadataError,
    > {
        row_finality_pairs(self, ComparisonRowKind::Gap, self.rows.gap.as_slice())
    }

    /// Returns symmetric-difference rows paired with their authoritative result metadata.
    ///
    /// Returns an error if the result's metadata count/kind layout is inconsistent.
    pub fn symmetric_difference_rows_with_finality(
        &self,
    ) -> Result<
        impl ExactSizeIterator<Item = ComparisonRowWithFinality<'_, SymmetricDifferenceRow>>,
        ComparisonRowMetadataError,
    > {
        row_finality_pairs(
            self,
            ComparisonRowKind::SymmetricDifference,
            self.rows.symmetric_difference.as_slice(),
        )
    }

    /// Returns containment rows paired with their authoritative result metadata.
    ///
    /// Returns an error if the result's metadata count/kind layout is inconsistent.
    pub fn containment_rows_with_finality(
        &self,
    ) -> Result<
        impl ExactSizeIterator<Item = ComparisonRowWithFinality<'_, ContainmentRow>>,
        ComparisonRowMetadataError,
    > {
        row_finality_pairs(
            self,
            ComparisonRowKind::Containment,
            self.rows.containment.as_slice(),
        )
    }

    /// Returns lead/lag rows paired with their authoritative result metadata.
    ///
    /// Returns an error if the result's metadata count/kind layout is inconsistent.
    pub fn lead_lag_rows_with_finality(
        &self,
    ) -> Result<
        impl ExactSizeIterator<Item = ComparisonRowWithFinality<'_, LeadLagRow>>,
        ComparisonRowMetadataError,
    > {
        row_finality_pairs(
            self,
            ComparisonRowKind::LeadLag,
            self.rows.lead_lag.as_slice(),
        )
    }

    /// Returns as-of rows paired with their authoritative result metadata.
    ///
    /// Returns an error if the result's metadata count/kind layout is inconsistent.
    pub fn as_of_rows_with_finality(
        &self,
    ) -> Result<
        impl ExactSizeIterator<Item = ComparisonRowWithFinality<'_, AsOfRow>>,
        ComparisonRowMetadataError,
    > {
        row_finality_pairs(self, ComparisonRowKind::AsOf, self.rows.as_of.as_slice())
    }

    fn row_family_layouts(&self) -> [(ComparisonRowKind, usize); 9] {
        self.rows.family_layouts()
    }

    fn row_family_bounds(&self, kind: ComparisonRowKind) -> (usize, usize) {
        let mut start = 0;
        for (candidate, count) in self.row_family_layouts() {
            if candidate == kind {
                return (start, count);
            }
            start += count;
        }
        unreachable!("all comparison row kinds have a canonical layout")
    }

    fn validate_row_metadata_layout(&self) -> Result<(), ComparisonRowMetadataError> {
        let layouts = self.row_family_layouts();
        let expected_total = layouts.iter().map(|(_, count)| count).sum::<usize>();
        let actual_total = self.row_finalities.len();

        let mut start = 0;
        for (kind, count) in layouts {
            let end = start + count;
            let available_end = end.min(actual_total);
            let metadata = &self.row_finalities[start.min(actual_total)..available_end];
            if let Some((relative_index, actual)) = metadata
                .iter()
                .enumerate()
                .find(|(_, item)| item.row_kind().ok() != Some(kind))
            {
                return Err(ComparisonRowMetadataError {
                    family: kind,
                    metadata_index: start + relative_index,
                    expected_count: count,
                    actual_count: metadata
                        .iter()
                        .filter(|item| item.row_kind().ok() == Some(kind))
                        .count(),
                    expected_kind: kind,
                    actual_kind: Some(actual.row_type.clone()),
                });
            }

            if available_end < end {
                return Err(ComparisonRowMetadataError {
                    family: kind,
                    metadata_index: available_end,
                    expected_count: count,
                    actual_count: available_end.saturating_sub(start),
                    expected_kind: kind,
                    actual_kind: None,
                });
            }
            start = end;
        }

        if actual_total > expected_total {
            let (family, expected_count) = layouts[layouts.len() - 1];
            let family_start = expected_total - expected_count;
            return Err(ComparisonRowMetadataError {
                family,
                metadata_index: expected_total,
                expected_count,
                actual_count: actual_total - family_start,
                expected_kind: family,
                actual_kind: self
                    .row_finalities
                    .get(expected_total)
                    .map(|metadata| metadata.row_type.clone()),
            });
        }

        if !self.row_finalities.iter().eq(self.state.finalities()) {
            let mismatch = self
                .row_finalities
                .iter()
                .zip(self.state.finalities())
                .position(|(public, authoritative)| public != authoritative)
                .unwrap_or(self.row_finalities.len());
            let (family, _, expected_count) = self
                .row_family_layouts()
                .into_iter()
                .scan(0, |offset, (kind, count)| {
                    let start = *offset;
                    *offset += count;
                    Some((kind, start, count))
                })
                .find(|(_, family_start, count)| mismatch < family_start + count)
                .unwrap_or((ComparisonRowKind::AsOf, start, 0));
            return Err(ComparisonRowMetadataError {
                family,
                metadata_index: mismatch,
                expected_count,
                actual_count: expected_count,
                expected_kind: family,
                actual_kind: Some("row finality metadata diverged".to_owned()),
            });
        }

        Ok(())
    }
}

fn row_finality_pairs<'a, R>(
    result: &'a ComparisonResult,
    kind: ComparisonRowKind,
    rows: &'a [R],
) -> Result<
    impl ExactSizeIterator<Item = ComparisonRowWithFinality<'a, R>> + 'a,
    ComparisonRowMetadataError,
> {
    result.validate_row_metadata_layout()?;
    let (_, count) = result.row_family_bounds(kind);
    debug_assert_eq!(rows.len(), count);
    let metadata = result.state.family_finalities(kind);
    Ok(rows
        .iter()
        .zip(metadata)
        .map(|(row, metadata)| ComparisonRowWithFinality { row, metadata }))
}

#[cfg(test)]
mod tests {
    use super::ComparisonRowKind;

    #[test]
    fn row_kinds_parse_canonical_labels_and_rust_0_1_0_aliases() {
        let cases = [
            ("overlap", ComparisonRowKind::Overlap, "overlap"),
            ("residual", ComparisonRowKind::Residual, "residual"),
            ("missing", ComparisonRowKind::Missing, "missing"),
            ("coverage", ComparisonRowKind::Coverage, "coverage"),
            ("gap", ComparisonRowKind::Gap, "gap"),
            (
                "symmetricDifference",
                ComparisonRowKind::SymmetricDifference,
                "symmetricDifference",
            ),
            (
                "symmetric-difference",
                ComparisonRowKind::SymmetricDifference,
                "symmetricDifference",
            ),
            ("containment", ComparisonRowKind::Containment, "containment"),
            ("leadLag", ComparisonRowKind::LeadLag, "leadLag"),
            ("lead-lag", ComparisonRowKind::LeadLag, "leadLag"),
            ("asOf", ComparisonRowKind::AsOf, "asOf"),
            ("asof", ComparisonRowKind::AsOf, "asOf"),
        ];

        for (label, expected_kind, canonical_label) in cases {
            let kind = label.parse::<ComparisonRowKind>().expect("known row kind");
            assert_eq!(kind, expected_kind, "label {label}");
            assert_eq!(kind.as_str(), canonical_label, "label {label}");
        }

        assert!("lead_lag".parse::<ComparisonRowKind>().is_err());
    }
}
