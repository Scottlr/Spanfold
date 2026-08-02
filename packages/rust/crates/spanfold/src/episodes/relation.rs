use thiserror::Error;

use crate::{
    ComparisonFinality, ComparisonNormalizationPolicy, ComparisonScope, ComparisonSelector,
    OpenWindowPolicy, TemporalAxis, TemporalPoint, WindowHistory,
};

use super::{
    Episode, EpisodeError, EpisodeFormationPolicy, EpisodeSet, TemporalTolerance, formation, graph,
};

/// Closed classification of one exhaustive episode graph component.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum EpisodeRelationKind {
    /// One target episode relates to one against episode.
    OneToOne,
    /// One target episode relates to two or more against episodes.
    Split,
    /// Two or more target episodes relate to one against episode.
    Merge,
    /// Two or more episodes occur on both sides.
    Complex,
    /// One target episode has no related against episode.
    UnmatchedTarget,
    /// One against episode has no related target episode.
    UnmatchedAgainst,
}

/// Axis-tagged maximum cross-side fragment gap.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EpisodeRelationPolicy {
    time_axis: TemporalAxis,
    tolerance: TemporalTolerance,
}

impl EpisodeRelationPolicy {
    pub(crate) const fn new(time_axis: TemporalAxis, tolerance: TemporalTolerance) -> Self {
        Self {
            time_axis,
            tolerance,
        }
    }

    /// Returns the normalized temporal axis.
    #[must_use]
    pub const fn time_axis(&self) -> TemporalAxis {
        self.time_axis
    }

    /// Returns the maximum cross-side fragment gap.
    #[must_use]
    pub const fn tolerance(&self) -> TemporalTolerance {
        self.tolerance
    }
}

/// Active coverage, proximity, and directional deltas for one component.
#[derive(Clone, Debug, PartialEq)]
pub struct EpisodeRelationMetrics {
    pub(crate) time_axis: TemporalAxis,
    pub(crate) target_active_magnitude: i64,
    pub(crate) against_active_magnitude: i64,
    pub(crate) overlap_magnitude: i64,
    pub(crate) target_coverage_ratio: Option<f64>,
    pub(crate) against_coverage_ratio: Option<f64>,
    pub(crate) intersection_over_union: Option<f64>,
    pub(crate) minimum_gap_magnitude: Option<i64>,
    pub(crate) onset_delta_magnitude: Option<i64>,
    pub(crate) recovery_delta_magnitude: Option<i64>,
    pub(crate) active_magnitude_delta: Option<i64>,
    pub(crate) elapsed_magnitude_delta: Option<i64>,
}

macro_rules! metric_getter {
    ($name:ident, $field:ident, $ty:ty, $docs:literal) => {
        #[doc = $docs]
        #[must_use]
        pub const fn $name(&self) -> $ty {
            self.$field
        }
    };
}

impl EpisodeRelationMetrics {
    metric_getter!(
        time_axis,
        time_axis,
        TemporalAxis,
        "Returns the temporal axis and magnitude unit."
    );
    metric_getter!(
        target_active_magnitude,
        target_active_magnitude,
        i64,
        "Returns the union magnitude of target fragments."
    );
    metric_getter!(
        against_active_magnitude,
        against_active_magnitude,
        i64,
        "Returns the union magnitude of against fragments."
    );
    metric_getter!(
        overlap_magnitude,
        overlap_magnitude,
        i64,
        "Returns the intersection magnitude of both active unions."
    );
    metric_getter!(
        target_coverage_ratio,
        target_coverage_ratio,
        Option<f64>,
        "Returns the fraction of target activity covered by against activity."
    );
    metric_getter!(
        against_coverage_ratio,
        against_coverage_ratio,
        Option<f64>,
        "Returns the fraction of against activity covered by target activity."
    );
    metric_getter!(
        intersection_over_union,
        intersection_over_union,
        Option<f64>,
        "Returns active intersection over active union."
    );
    metric_getter!(
        minimum_gap_magnitude,
        minimum_gap_magnitude,
        Option<i64>,
        "Returns the minimum cross-side fragment gap for a matched component."
    );
    metric_getter!(
        onset_delta_magnitude,
        onset_delta_magnitude,
        Option<i64>,
        "Returns earliest against start minus earliest target start."
    );
    metric_getter!(
        recovery_delta_magnitude,
        recovery_delta_magnitude,
        Option<i64>,
        "Returns latest against end minus latest target end."
    );
    metric_getter!(
        active_magnitude_delta,
        active_magnitude_delta,
        Option<i64>,
        "Returns against active magnitude minus target active magnitude."
    );
    metric_getter!(
        elapsed_magnitude_delta,
        elapsed_magnitude_delta,
        Option<i64>,
        "Returns against component envelope magnitude minus target envelope magnitude."
    );
}

/// One connected component in the target-against episode graph.
#[derive(Clone, Debug, PartialEq)]
pub struct EpisodeRelation {
    pub(crate) kind: EpisodeRelationKind,
    pub(crate) target_episodes: Vec<Episode>,
    pub(crate) against_episodes: Vec<Episode>,
    pub(crate) metrics: EpisodeRelationMetrics,
    pub(crate) finality: ComparisonFinality,
}

impl EpisodeRelation {
    /// Returns the directional component classification.
    #[must_use]
    pub const fn kind(&self) -> EpisodeRelationKind {
        self.kind
    }
    /// Returns deterministically ordered target episodes.
    #[must_use]
    pub fn target_episodes(&self) -> &[Episode] {
        &self.target_episodes
    }
    /// Returns deterministically ordered against episodes.
    #[must_use]
    pub fn against_episodes(&self) -> &[Episode] {
        &self.against_episodes
    }
    /// Returns component-level active and timing metrics.
    #[must_use]
    pub const fn metrics(&self) -> &EpisodeRelationMetrics {
        &self.metrics
    }
    /// Returns whether the relation can still change at its evaluation horizon.
    #[must_use]
    pub const fn finality(&self) -> &ComparisonFinality {
        &self.finality
    }
}

/// Validated immutable plan for comparing two episode definitions.
#[derive(Clone, Debug, PartialEq)]
pub struct EpisodeComparisonPlan {
    pub(crate) name: String,
    pub(crate) target_name: String,
    pub(crate) target: ComparisonSelector,
    pub(crate) against_name: String,
    pub(crate) against: ComparisonSelector,
    pub(crate) scope: ComparisonScope,
    pub(crate) normalization: ComparisonNormalizationPolicy,
    pub(crate) formation: EpisodeFormationPolicy,
    pub(crate) relation: EpisodeRelationPolicy,
}

impl EpisodeComparisonPlan {
    /// Returns the analytical comparison name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Returns the target side name.
    #[must_use]
    pub fn target_name(&self) -> &str {
        &self.target_name
    }
    /// Returns the target selector.
    #[must_use]
    pub const fn target(&self) -> &ComparisonSelector {
        &self.target
    }
    /// Returns the against side name.
    #[must_use]
    pub fn against_name(&self) -> &str {
        &self.against_name
    }
    /// Returns the against selector.
    #[must_use]
    pub const fn against(&self) -> &ComparisonSelector {
        &self.against
    }
    /// Returns the shared scope.
    #[must_use]
    pub const fn scope(&self) -> &ComparisonScope {
        &self.scope
    }
    /// Returns the shared normalization policy.
    #[must_use]
    pub const fn normalization(&self) -> &ComparisonNormalizationPolicy {
        &self.normalization
    }
    /// Returns the shared formation policy.
    #[must_use]
    pub const fn formation(&self) -> &EpisodeFormationPolicy {
        &self.formation
    }
    /// Returns the cross-side relation policy.
    #[must_use]
    pub const fn relation(&self) -> &EpisodeRelationPolicy {
        &self.relation
    }
}

/// Two formed episode sets and their exhaustive relation components.
#[derive(Clone, Debug, PartialEq)]
pub struct EpisodeComparisonResult {
    pub(crate) plan: EpisodeComparisonPlan,
    pub(crate) target_episodes: EpisodeSet,
    pub(crate) against_episodes: EpisodeSet,
    pub(crate) relations: Vec<EpisodeRelation>,
    pub(crate) evaluation_horizon: Option<TemporalPoint>,
}

impl EpisodeComparisonResult {
    /// Returns the analytical comparison name.
    #[must_use]
    pub fn name(&self) -> &str {
        self.plan.name()
    }
    /// Returns the effective comparison plan.
    #[must_use]
    pub const fn plan(&self) -> &EpisodeComparisonPlan {
        &self.plan
    }
    /// Returns the formed target episodes.
    #[must_use]
    pub const fn target_episodes(&self) -> &EpisodeSet {
        &self.target_episodes
    }
    /// Returns the formed against episodes.
    #[must_use]
    pub const fn against_episodes(&self) -> &EpisodeSet {
        &self.against_episodes
    }
    /// Returns deterministic exhaustive relation components.
    #[must_use]
    pub fn relations(&self) -> &[EpisodeRelation] {
        &self.relations
    }
    /// Returns the live or configured evaluation horizon, when present.
    #[must_use]
    pub const fn evaluation_horizon(&self) -> Option<&TemporalPoint> {
        self.evaluation_horizon.as_ref()
    }
}

/// Typed failure while configuring or comparing episodes.
#[derive(Clone, Debug, Eq, PartialEq, Error)]
pub enum EpisodeComparisonError {
    /// The comparison name is empty.
    #[error("episode comparison name cannot be empty")]
    EmptyName,
    /// A side name is empty.
    #[error("episode comparison side name cannot be empty")]
    EmptySideName,
    /// No target selector was configured.
    #[error("episode comparison requires one target selector")]
    MissingTarget,
    /// No against selector was configured.
    #[error("episode comparison requires one against selector")]
    MissingAgainst,
    /// More than one target selector was configured.
    #[error("episode comparison accepts exactly one target selector")]
    DuplicateTarget,
    /// More than one against selector was configured.
    #[error("episode comparison accepts exactly one against selector")]
    DuplicateAgainst,
    /// A scope was not configured.
    #[error("episode comparison requires a scope")]
    MissingScope,
    /// The scope does not identify exactly one window family.
    #[error("episode comparison requires one named window family")]
    MissingWindowFamily,
    /// Plan components use incompatible axes.
    #[error("episode comparison plan components must use one temporal axis")]
    AxisMismatch,
    /// A selected normalized record occurs on both sides.
    #[error("record '{record_id}' belongs to both '{target_name}' and '{against_name}'")]
    SelfMembership {
        /// Shared normalized source record ID.
        record_id: String,
        /// Target selector name.
        target_name: String,
        /// Against selector name.
        against_name: String,
    },
    /// An analytical total exceeded the public i64 contract.
    #[error("episode relation magnitude overflow")]
    MagnitudeOverflow,
    /// Episode formation failed for one side.
    #[error(transparent)]
    Formation(#[from] EpisodeError),
}

/// Borrowing staged builder for an exhaustive episode comparison.
#[derive(Clone, Debug)]
pub struct EpisodeComparisonBuilder<'a> {
    history: &'a WindowHistory,
    name: String,
    target: Option<(String, ComparisonSelector)>,
    against: Option<(String, ComparisonSelector)>,
    duplicate_target: bool,
    duplicate_against: bool,
    scope: Option<ComparisonScope>,
    normalization: ComparisonNormalizationPolicy,
    stitch_tolerance: Option<TemporalTolerance>,
    relation_tolerance: Option<TemporalTolerance>,
}

impl<'a> EpisodeComparisonBuilder<'a> {
    pub(crate) fn new(history: &'a WindowHistory, name: String) -> Self {
        Self {
            history,
            name,
            target: None,
            against: None,
            duplicate_target: false,
            duplicate_against: false,
            scope: None,
            normalization: ComparisonNormalizationPolicy::default_policy(),
            stitch_tolerance: None,
            relation_tolerance: None,
        }
    }

    /// Configures the one named target selector.
    #[must_use]
    pub fn target(mut self, name: impl Into<String>, selector: ComparisonSelector) -> Self {
        if self.target.is_some() {
            self.duplicate_target = true;
        } else {
            self.target = Some((name.into(), selector));
        }
        self
    }

    /// Configures the one named against selector.
    #[must_use]
    pub fn against(mut self, name: impl Into<String>, selector: ComparisonSelector) -> Self {
        if self.against.is_some() {
            self.duplicate_against = true;
        } else {
            self.against = Some((name.into(), selector));
        }
        self
    }

    /// Restricts both sides to one named window family and temporal axis.
    #[must_use]
    pub fn scope(mut self, scope: ComparisonScope) -> Self {
        self.scope = Some(scope);
        self
    }

    /// Configures temporal normalization shared by both sides.
    #[must_use]
    pub fn normalization(mut self, normalization: ComparisonNormalizationPolicy) -> Self {
        self.normalization = normalization;
        self
    }

    /// Configures the maximum same-side inactive gap.
    #[must_use]
    pub fn stitch_gaps_up_to(mut self, tolerance: TemporalTolerance) -> Self {
        self.stitch_tolerance = Some(tolerance);
        self
    }

    /// Configures the maximum cross-side fragment gap.
    #[must_use]
    pub fn relate_within(mut self, tolerance: TemporalTolerance) -> Self {
        self.relation_tolerance = Some(tolerance);
        self
    }

    /// Builds and validates an immutable comparison plan.
    pub fn build(&self) -> Result<EpisodeComparisonPlan, EpisodeComparisonError> {
        if self.name.trim().is_empty() {
            return Err(EpisodeComparisonError::EmptyName);
        }
        if self.duplicate_target {
            return Err(EpisodeComparisonError::DuplicateTarget);
        }
        if self.duplicate_against {
            return Err(EpisodeComparisonError::DuplicateAgainst);
        }
        let (target_name, target) = self
            .target
            .clone()
            .ok_or(EpisodeComparisonError::MissingTarget)?;
        let (against_name, against) = self
            .against
            .clone()
            .ok_or(EpisodeComparisonError::MissingAgainst)?;
        if target_name.trim().is_empty() || against_name.trim().is_empty() {
            return Err(EpisodeComparisonError::EmptySideName);
        }
        let scope = self
            .scope
            .clone()
            .ok_or(EpisodeComparisonError::MissingScope)?;
        if scope
            .window_name
            .as_deref()
            .is_none_or(|name| name.trim().is_empty())
        {
            return Err(EpisodeComparisonError::MissingWindowFamily);
        }
        let stitch_tolerance = self
            .stitch_tolerance
            .unwrap_or_else(|| TemporalTolerance::zero(self.normalization.time_axis));
        let relation_tolerance = self
            .relation_tolerance
            .unwrap_or_else(|| TemporalTolerance::zero(self.normalization.time_axis));
        if scope.time_axis != self.normalization.time_axis
            || stitch_tolerance.axis() != self.normalization.time_axis
            || relation_tolerance.axis() != self.normalization.time_axis
        {
            return Err(EpisodeComparisonError::AxisMismatch);
        }
        formation::validate_horizons(&self.normalization)?;
        Ok(EpisodeComparisonPlan {
            name: self.name.clone(),
            target_name,
            target,
            against_name,
            against,
            scope,
            normalization: self.normalization.clone(),
            formation: EpisodeFormationPolicy::new(self.normalization.time_axis, stitch_tolerance),
            relation: EpisodeRelationPolicy::new(self.normalization.time_axis, relation_tolerance),
        })
    }

    /// Forms and relates both episode sets using any configured horizon.
    pub fn run(&self) -> Result<EpisodeComparisonResult, EpisodeComparisonError> {
        graph::run(self.history, self.build()?)
    }

    /// Forms and relates both episode sets at an explicit live horizon.
    pub fn run_live(
        &self,
        evaluation_horizon: TemporalPoint,
    ) -> Result<EpisodeComparisonResult, EpisodeComparisonError> {
        let mut plan = self.build()?;
        if evaluation_horizon.axis() != plan.formation.time_axis() {
            return Err(EpisodeComparisonError::AxisMismatch);
        }
        if plan.normalization.known_at.is_some() || plan.normalization.open_window_horizon.is_some()
        {
            return Err(EpisodeError::LiveHorizonConflict.into());
        }
        plan.normalization.open_window_policy = OpenWindowPolicy::ClipToHorizon;
        plan.normalization.open_window_horizon = Some(evaluation_horizon);
        graph::run(self.history, plan)
    }
}
