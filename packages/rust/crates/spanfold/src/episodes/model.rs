use thiserror::Error;

use crate::{
    ComparisonFinality, ComparisonNormalizationPolicy, ComparisonNullTimestampPolicy,
    ComparisonScope, ComparisonSelector, TemporalAxis, TemporalPoint, TemporalRange,
    TemporalRangeError, WindowRecord,
};

/// Non-negative axis-specific distance used when stitching episode fragments.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TemporalTolerance {
    axis: TemporalAxis,
    magnitude: i64,
}

impl TemporalTolerance {
    /// Creates a processing-position tolerance.
    pub fn processing_positions(magnitude: i64) -> Result<Self, EpisodeError> {
        Self::new(TemporalAxis::ProcessingPosition, magnitude)
    }

    /// Creates a timestamp tolerance expressed in the caller's opaque clock ticks.
    pub fn timestamp_ticks(magnitude: i64) -> Result<Self, EpisodeError> {
        Self::new(TemporalAxis::Timestamp, magnitude)
    }

    pub(crate) fn zero(axis: TemporalAxis) -> Self {
        Self { axis, magnitude: 0 }
    }

    fn new(axis: TemporalAxis, magnitude: i64) -> Result<Self, EpisodeError> {
        if magnitude < 0 {
            return Err(EpisodeError::NegativeTolerance(magnitude));
        }
        Ok(Self { axis, magnitude })
    }

    /// Returns the temporal axis.
    #[must_use]
    pub const fn axis(&self) -> TemporalAxis {
        self.axis
    }

    /// Returns the non-negative tolerance magnitude.
    #[must_use]
    pub const fn magnitude(&self) -> i64 {
        self.magnitude
    }
}

/// Formation policy shared by all episodes in a set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EpisodeFormationPolicy {
    time_axis: TemporalAxis,
    stitch_tolerance: TemporalTolerance,
}

impl EpisodeFormationPolicy {
    pub(crate) const fn new(time_axis: TemporalAxis, stitch_tolerance: TemporalTolerance) -> Self {
        Self {
            time_axis,
            stitch_tolerance,
        }
    }

    /// Returns the normalized temporal axis.
    #[must_use]
    pub const fn time_axis(&self) -> TemporalAxis {
        self.time_axis
    }

    /// Returns the maximum stitchable gap.
    #[must_use]
    pub const fn stitch_tolerance(&self) -> TemporalTolerance {
        self.stitch_tolerance
    }
}

/// Validated immutable episode-formation plan.
#[derive(Clone, Debug, PartialEq)]
pub struct EpisodeFormationPlan {
    pub(crate) name: String,
    pub(crate) selector: ComparisonSelector,
    pub(crate) scope: ComparisonScope,
    pub(crate) normalization: ComparisonNormalizationPolicy,
    pub(crate) formation: EpisodeFormationPolicy,
}

impl EpisodeFormationPlan {
    /// Returns the analytical set name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Returns the evidence selector.
    #[must_use]
    pub const fn selector(&self) -> &ComparisonSelector {
        &self.selector
    }
    /// Returns the single-family scope.
    #[must_use]
    pub const fn scope(&self) -> &ComparisonScope {
        &self.scope
    }
    /// Returns the normalization policy.
    #[must_use]
    pub const fn normalization(&self) -> &ComparisonNormalizationPolicy {
        &self.normalization
    }
    /// Returns the formation policy.
    #[must_use]
    pub const fn formation(&self) -> &EpisodeFormationPolicy {
        &self.formation
    }
}

/// Opaque deterministic identifier for one Rust episode.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct EpisodeId(String);

impl EpisodeId {
    pub(crate) fn new(value: String) -> Self {
        Self(value)
    }
    /// Returns the opaque identifier value.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One normalized source window retained inside an episode.
#[derive(Clone, Debug, PartialEq)]
pub struct EpisodeFragment {
    window: WindowRecord,
    range: TemporalRange,
    finality: ComparisonFinality,
}

impl EpisodeFragment {
    pub(crate) fn new(
        window: WindowRecord,
        range: TemporalRange,
        finality: ComparisonFinality,
    ) -> Self {
        Self {
            window,
            range,
            finality,
        }
    }
    /// Returns the source record.
    #[must_use]
    pub const fn window(&self) -> &WindowRecord {
        &self.window
    }
    /// Returns the normalized effective range.
    #[must_use]
    pub const fn range(&self) -> &TemporalRange {
        &self.range
    }
    /// Returns fragment finality.
    #[must_use]
    pub const fn finality(&self) -> &ComparisonFinality {
        &self.finality
    }
    /// Returns the source record ID.
    #[must_use]
    pub fn record_id(&self) -> &str {
        self.window.id().as_str()
    }
}

/// One stitched occurrence that preserves its authoritative fragments.
#[derive(Clone, Debug, PartialEq)]
pub struct Episode {
    id: EpisodeId,
    window_name: String,
    key: String,
    source: Option<String>,
    partition: Option<String>,
    envelope: TemporalRange,
    fragments: Vec<EpisodeFragment>,
    finality: ComparisonFinality,
    active_magnitude: i64,
    elapsed_magnitude: i64,
    internal_gap_magnitude: i64,
}

impl Episode {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        id: EpisodeId,
        window_name: String,
        key: String,
        source: Option<String>,
        partition: Option<String>,
        envelope: TemporalRange,
        fragments: Vec<EpisodeFragment>,
        finality: ComparisonFinality,
        active_magnitude: i64,
        elapsed_magnitude: i64,
    ) -> Result<Self, EpisodeError> {
        if fragments.is_empty()
            || active_magnitude < 0
            || elapsed_magnitude < 0
            || active_magnitude > elapsed_magnitude
        {
            return Err(EpisodeError::InvalidMetrics);
        }
        Ok(Self {
            id,
            window_name,
            key,
            source,
            partition,
            envelope,
            fragments,
            finality,
            active_magnitude,
            elapsed_magnitude,
            internal_gap_magnitude: elapsed_magnitude - active_magnitude,
        })
    }
    /// Returns the opaque deterministic ID.
    #[must_use]
    pub const fn id(&self) -> &EpisodeId {
        &self.id
    }
    /// Returns the window family.
    #[must_use]
    pub fn window_name(&self) -> &str {
        &self.window_name
    }
    /// Returns the exact logical key.
    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }
    /// Returns the source shared by all fragments.
    #[must_use]
    pub fn source(&self) -> Option<&str> {
        self.source.as_deref()
    }
    /// Returns the partition shared by all fragments.
    #[must_use]
    pub fn partition(&self) -> Option<&str> {
        self.partition.as_deref()
    }
    /// Returns the temporal axis.
    #[must_use]
    pub fn time_axis(&self) -> TemporalAxis {
        self.envelope.start().axis()
    }
    /// Returns the first-start to last-end envelope.
    #[must_use]
    pub const fn envelope(&self) -> &TemporalRange {
        &self.envelope
    }
    /// Returns ordered authoritative fragments.
    #[must_use]
    pub fn fragments(&self) -> &[EpisodeFragment] {
        &self.fragments
    }
    /// Returns episode finality.
    #[must_use]
    pub const fn finality(&self) -> &ComparisonFinality {
        &self.finality
    }
    /// Returns the union magnitude of fragment ranges.
    #[must_use]
    pub const fn active_magnitude(&self) -> i64 {
        self.active_magnitude
    }
    /// Returns the envelope magnitude.
    #[must_use]
    pub const fn elapsed_magnitude(&self) -> i64 {
        self.elapsed_magnitude
    }
    /// Returns inactive magnitude inside the envelope.
    #[must_use]
    pub const fn internal_gap_magnitude(&self) -> i64 {
        self.internal_gap_magnitude
    }
}

/// Materialized episodes produced by one effective plan.
#[derive(Clone, Debug, PartialEq)]
pub struct EpisodeSet {
    plan: EpisodeFormationPlan,
    episodes: Vec<Episode>,
    summary: super::EpisodeSetSummary,
    evaluation_horizon: Option<TemporalPoint>,
}

impl EpisodeSet {
    pub(crate) fn new(
        plan: EpisodeFormationPlan,
        episodes: Vec<Episode>,
        evaluation_horizon: Option<TemporalPoint>,
    ) -> Result<Self, EpisodeError> {
        let summary = super::summary::summarize_set(&plan, &episodes)?;
        Ok(Self {
            plan,
            episodes,
            summary,
            evaluation_horizon,
        })
    }
    /// Returns the analytical set name.
    #[must_use]
    pub fn name(&self) -> &str {
        self.plan.name()
    }
    /// Returns the effective formation plan.
    #[must_use]
    pub const fn plan(&self) -> &EpisodeFormationPlan {
        &self.plan
    }
    /// Returns episodes in deterministic order.
    #[must_use]
    pub fn episodes(&self) -> &[Episode] {
        &self.episodes
    }
    /// Returns the materialized neutral set summary.
    #[must_use]
    pub const fn summary(&self) -> &super::EpisodeSetSummary {
        &self.summary
    }
    /// Returns the live or configured horizon, when present.
    #[must_use]
    pub const fn evaluation_horizon(&self) -> Option<&TemporalPoint> {
        self.evaluation_horizon.as_ref()
    }
}

/// Error returned while configuring or forming episodes.
#[derive(Clone, Debug, Eq, PartialEq, Error)]
pub enum EpisodeError {
    /// The analytical name is empty.
    #[error("episode formation name cannot be empty")]
    EmptyName,
    /// A selector was not configured.
    #[error("episode formation requires a selector")]
    MissingSelector,
    /// A scope was not configured.
    #[error("episode formation requires a scope")]
    MissingScope,
    /// The scope does not identify exactly one window family.
    #[error("episode formation requires one named window family")]
    MissingWindowFamily,
    /// A tolerance was negative.
    #[error("episode stitch tolerance cannot be negative: {0}")]
    NegativeTolerance(i64),
    /// Plan components use incompatible axes.
    #[error("episode plan components must use one temporal axis")]
    AxisMismatch,
    /// More than one evaluation horizon was configured.
    #[error("episode formation accepts only one horizon source")]
    CompetingHorizons,
    /// Known-at analysis is unsupported for event time.
    #[error("known-at episode formation is supported only on processing positions")]
    EventTimeKnownAt,
    /// A live horizon conflicts with a configured horizon.
    #[error("run_live cannot be combined with a configured horizon")]
    LiveHorizonConflict,
    /// A record failed neutral normalization.
    #[error("window '{record_id}' could not form an episode: {cause}")]
    Normalization {
        /// Record that failed normalization.
        record_id: String,
        /// Typed neutral normalization failure.
        cause: EpisodeNormalizationFailure,
    },
    /// A timestamp horizon and selected record use different clocks.
    #[error(
        "window '{record_id}' uses timestamp clock {actual:?}, incompatible with horizon clock {expected:?}"
    )]
    HorizonClockMismatch {
        /// Record with the incompatible clock.
        record_id: String,
        /// Horizon clock.
        expected: Option<String>,
        /// Record clock.
        actual: Option<String>,
    },
    /// Episode totals exceeded the public i64 contract.
    #[error("episode magnitude overflow")]
    MagnitudeOverflow,
    /// Episode metrics violated their invariants.
    #[error("episode metrics are invalid")]
    InvalidMetrics,
}

/// Typed reason a selected window could not be normalized into episode evidence.
#[derive(Clone, Debug, Eq, PartialEq, Error)]
pub enum EpisodeNormalizationFailure {
    /// The record has no event timestamp under the configured policy.
    #[error("missing timestamp under policy {policy:?}; actual axis was {actual:?}")]
    MissingTimestamp {
        /// Actual source axis.
        actual: TemporalAxis,
        /// Configured handling policy.
        policy: ComparisonNullTimestampPolicy,
    },
    /// The source record uses a different temporal axis.
    #[error("temporal axis mismatch: expected {expected:?}, actual {actual:?}")]
    TemporalAxisMismatch {
        /// Requested axis.
        expected: TemporalAxis,
        /// Actual source axis.
        actual: TemporalAxis,
    },
    /// An open record had no permitted clipping horizon.
    #[error("open window requires an explicit clipping policy and horizon")]
    OpenWindowWithoutPolicy,
    /// The open-window horizon was incompatible with or earlier than the start.
    #[error("invalid open-window range: start={start:?}, horizon={horizon:?}")]
    InvalidRangeDuration {
        /// Open-window start.
        start: TemporalPoint,
        /// Requested horizon.
        horizon: TemporalPoint,
    },
    /// Construction of the normalized temporal range failed.
    #[error(transparent)]
    InvalidTemporalRange(TemporalRangeError),
}
