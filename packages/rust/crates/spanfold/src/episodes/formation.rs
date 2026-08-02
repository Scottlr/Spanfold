use std::collections::BTreeMap;

use crate::{
    ComparisonFinality, ComparisonNormalizationPolicy, ComparisonNullTimestampPolicy,
    ComparisonScope, ComparisonSelector, OpenWindowPolicy, TemporalAxis, TemporalPoint,
    TemporalRange, WindowHistory,
    window_normalization::{
        NormalizedWindowEvidence, WindowNormalizationFailure, WindowNormalizationRequest,
        normalize_window, ordered_candidates,
    },
};

use super::{
    Episode, EpisodeError, EpisodeFormationPlan, EpisodeFormationPolicy, EpisodeFragment,
    EpisodeNormalizationFailure, EpisodeSet, TemporalTolerance, identity,
};

/// Fluent episode-formation builder over an existing history.
#[derive(Clone, Debug)]
pub struct EpisodeFormationBuilder<'a> {
    history: &'a WindowHistory,
    name: String,
    selector: Option<ComparisonSelector>,
    scope: Option<ComparisonScope>,
    normalization: ComparisonNormalizationPolicy,
    tolerance: Option<TemporalTolerance>,
}

impl<'a> EpisodeFormationBuilder<'a> {
    pub(crate) fn new(history: &'a WindowHistory, name: String) -> Self {
        Self {
            history,
            name,
            selector: None,
            scope: None,
            normalization: ComparisonNormalizationPolicy::default_policy(),
            tolerance: None,
        }
    }

    /// Selects records that can contribute fragments.
    #[must_use]
    pub fn from(mut self, selector: ComparisonSelector) -> Self {
        self.selector = Some(selector);
        self
    }

    /// Restricts formation to one named window family and temporal axis.
    #[must_use]
    pub fn scope(mut self, scope: ComparisonScope) -> Self {
        self.scope = Some(scope);
        self
    }

    /// Configures shared window normalization.
    #[must_use]
    pub fn normalization(mut self, normalization: ComparisonNormalizationPolicy) -> Self {
        self.normalization = normalization;
        self
    }

    /// Configures the largest inactive gap that may be stitched.
    #[must_use]
    pub fn stitch_gaps_up_to(mut self, tolerance: TemporalTolerance) -> Self {
        self.tolerance = Some(tolerance);
        self
    }

    /// Builds and validates an immutable formation plan.
    pub fn build(&self) -> Result<EpisodeFormationPlan, EpisodeError> {
        if self.name.trim().is_empty() {
            return Err(EpisodeError::EmptyName);
        }
        let selector = self.selector.clone().ok_or(EpisodeError::MissingSelector)?;
        let scope = self.scope.clone().ok_or(EpisodeError::MissingScope)?;
        if scope
            .window_name
            .as_deref()
            .is_none_or(|name| name.trim().is_empty())
        {
            return Err(EpisodeError::MissingWindowFamily);
        }
        let tolerance = self
            .tolerance
            .unwrap_or_else(|| TemporalTolerance::zero(self.normalization.time_axis));
        if scope.time_axis != self.normalization.time_axis
            || tolerance.axis() != self.normalization.time_axis
        {
            return Err(EpisodeError::AxisMismatch);
        }
        validate_horizons(&self.normalization)?;
        Ok(EpisodeFormationPlan {
            name: self.name.clone(),
            selector,
            scope,
            normalization: self.normalization.clone(),
            formation: EpisodeFormationPolicy::new(self.normalization.time_axis, tolerance),
        })
    }

    /// Forms episodes using any horizon configured on the normalization policy.
    pub fn run(&self) -> Result<EpisodeSet, EpisodeError> {
        run(self.history, self.build()?)
    }

    /// Forms episodes at an explicit live horizon.
    pub fn run_live(&self, evaluation_horizon: TemporalPoint) -> Result<EpisodeSet, EpisodeError> {
        let mut plan = self.build()?;
        if evaluation_horizon.axis() != plan.formation.time_axis() {
            return Err(EpisodeError::AxisMismatch);
        }
        if plan.normalization.known_at.is_some() || plan.normalization.open_window_horizon.is_some()
        {
            return Err(EpisodeError::LiveHorizonConflict);
        }
        plan.normalization.open_window_policy = OpenWindowPolicy::ClipToHorizon;
        plan.normalization.open_window_horizon = Some(evaluation_horizon);
        run(self.history, plan)
    }
}

pub(crate) fn validate_horizons(
    policy: &ComparisonNormalizationPolicy,
) -> Result<(), EpisodeError> {
    if policy.known_at.is_some() && policy.open_window_horizon.is_some() {
        return Err(EpisodeError::CompetingHorizons);
    }
    if let Some(known_at) = &policy.known_at
        && (known_at.axis() != TemporalAxis::ProcessingPosition
            || policy.time_axis == TemporalAxis::Timestamp)
    {
        return Err(EpisodeError::EventTimeKnownAt);
    }
    if policy
        .open_window_horizon
        .as_ref()
        .is_some_and(|point| point.axis() != policy.time_axis)
    {
        return Err(EpisodeError::AxisMismatch);
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct GroupKey {
    window_name: String,
    key: String,
    source: Option<String>,
    partition: Option<String>,
    axis: TemporalAxis,
    clock: Option<String>,
}

pub(crate) fn run(
    history: &WindowHistory,
    plan: EpisodeFormationPlan,
) -> Result<EpisodeSet, EpisodeError> {
    let evaluation_horizon = plan
        .normalization
        .known_at
        .clone()
        .or_else(|| plan.normalization.open_window_horizon.clone());
    let request = WindowNormalizationRequest {
        scope: &plan.scope,
        time_axis: plan.normalization.time_axis,
        known_at: plan.normalization.known_at.as_ref(),
        null_timestamp_policy: plan.normalization.null_timestamp_policy,
        require_closed: plan.normalization.require_closed_windows,
        open_window_policy: plan.normalization.open_window_policy,
        evaluation_horizon: evaluation_horizon.as_ref(),
    };
    let mut groups = BTreeMap::<GroupKey, Vec<EpisodeFragment>>::new();
    for candidate in ordered_candidates(history) {
        let record = candidate.to_window_record();
        if !plan.selector.matches(&record) {
            continue;
        }
        let normalized = match normalize_window(candidate, &request) {
            Ok(Some(value)) => value,
            Ok(None) | Err(WindowNormalizationFailure::FutureWindowExcluded { .. }) => continue,
            Err(WindowNormalizationFailure::MissingTimestamp {
                policy: ComparisonNullTimestampPolicy::Exclude,
                ..
            }) => continue,
            Err(failure) => {
                return Err(EpisodeError::Normalization {
                    record_id: candidate.record_id().to_owned(),
                    cause: map_normalization_failure(failure),
                });
            }
        };
        if plan.formation.time_axis() == TemporalAxis::Timestamp
            && let Some(horizon) = evaluation_horizon.as_ref()
            && normalized.range.start().clock() != horizon.clock()
        {
            return Err(EpisodeError::HorizonClockMismatch {
                record_id: candidate.record_id().to_owned(),
                expected: horizon.clock().map(str::to_owned),
                actual: normalized.range.start().clock().map(str::to_owned),
            });
        }
        push_fragment(&mut groups, record, normalized);
    }

    let mut episodes = Vec::new();
    for (group, mut fragments) in groups {
        fragments.sort_by(|left, right| {
            (
                left.range().start().magnitude(),
                left.range().end().magnitude(),
                left.record_id(),
            )
                .cmp(&(
                    right.range().start().magnitude(),
                    right.range().end().magnitude(),
                    right.record_id(),
                ))
        });
        let mut current = vec![fragments.remove(0)];
        let mut current_end = current[0].range().end().magnitude();
        for fragment in fragments {
            let next_start = fragment.range().start().magnitude();
            let gap = i128::from(next_start) - i128::from(current_end);
            if gap <= i128::from(plan.formation.stitch_tolerance().magnitude()) {
                current_end = current_end.max(fragment.range().end().magnitude());
                current.push(fragment);
            } else {
                episodes.push(materialize(
                    &group,
                    current,
                    &plan,
                    evaluation_horizon.as_ref(),
                )?);
                current_end = fragment.range().end().magnitude();
                current = vec![fragment];
            }
        }
        episodes.push(materialize(
            &group,
            current,
            &plan,
            evaluation_horizon.as_ref(),
        )?);
    }
    Ok(EpisodeSet::new(plan, episodes, evaluation_horizon))
}

fn push_fragment(
    groups: &mut BTreeMap<GroupKey, Vec<EpisodeFragment>>,
    record: crate::WindowRecord,
    normalized: NormalizedWindowEvidence<'_>,
) {
    let start = normalized.range.start();
    let group = GroupKey {
        window_name: record.window_name().to_owned(),
        key: record.key().to_owned(),
        source: record.source().map(str::to_owned),
        partition: record.partition().map(str::to_owned),
        axis: start.axis(),
        clock: start.clock().map(str::to_owned),
    };
    let finality = if normalized.is_provisional {
        ComparisonFinality::Provisional
    } else {
        ComparisonFinality::Final
    };
    groups
        .entry(group)
        .or_default()
        .push(EpisodeFragment::new(record, normalized.range, finality));
}

fn materialize(
    group: &GroupKey,
    fragments: Vec<EpisodeFragment>,
    plan: &EpisodeFormationPlan,
    horizon: Option<&TemporalPoint>,
) -> Result<Episode, EpisodeError> {
    let start = fragments[0].range().start();
    let mut end = fragments[0].range().end();
    let mut union_start = start.magnitude();
    let mut union_end = end.magnitude();
    let mut active = 0_i128;
    let contains_provisional = fragments
        .iter()
        .any(|fragment| fragment.finality() == &ComparisonFinality::Provisional);
    for fragment in fragments.iter().skip(1) {
        let fragment_start = fragment.range().start().magnitude();
        let fragment_end = fragment.range().end().magnitude();
        if fragment_start > union_end {
            active += i128::from(union_end) - i128::from(union_start);
            union_start = fragment_start;
            union_end = fragment_end;
        } else {
            union_end = union_end.max(fragment_end);
        }
        if fragment_end >= end.magnitude() {
            end = fragment.range().end();
        }
    }
    active += i128::from(union_end) - i128::from(union_start);
    let elapsed = i128::from(end.magnitude()) - i128::from(start.magnitude());
    let active = i64::try_from(active).map_err(|_| EpisodeError::MagnitudeOverflow)?;
    let elapsed = i64::try_from(elapsed).map_err(|_| EpisodeError::MagnitudeOverflow)?;
    let boundary = end
        .magnitude()
        .saturating_add(plan.formation.stitch_tolerance().magnitude());
    let within_settling = horizon.is_some_and(|point| point.magnitude() <= boundary);
    let finality = if contains_provisional || within_settling {
        ComparisonFinality::Provisional
    } else {
        ComparisonFinality::Final
    };
    let envelope = TemporalRange::new(start, end).map_err(|error| EpisodeError::Normalization {
        record_id: fragments[0].record_id().to_owned(),
        cause: EpisodeNormalizationFailure::InvalidTemporalRange(error),
    })?;
    let id = identity::create(
        &group.window_name,
        &group.key,
        group.source.as_deref(),
        group.partition.as_deref(),
        group.axis,
        &fragments,
        &finality,
    );
    Episode::new(
        id,
        group.window_name.clone(),
        group.key.clone(),
        group.source.clone(),
        group.partition.clone(),
        envelope,
        fragments,
        finality,
        active,
        elapsed,
    )
}

fn map_normalization_failure(failure: WindowNormalizationFailure) -> EpisodeNormalizationFailure {
    match failure {
        WindowNormalizationFailure::FutureWindowExcluded { .. } => {
            unreachable!("future windows are omitted before typed error mapping")
        }
        WindowNormalizationFailure::MissingTimestamp { actual, policy } => {
            EpisodeNormalizationFailure::MissingTimestamp { actual, policy }
        }
        WindowNormalizationFailure::TemporalAxisMismatch { expected, actual } => {
            EpisodeNormalizationFailure::TemporalAxisMismatch { expected, actual }
        }
        WindowNormalizationFailure::OpenWindowWithoutPolicy => {
            EpisodeNormalizationFailure::OpenWindowWithoutPolicy
        }
        WindowNormalizationFailure::InvalidRangeDuration { start, horizon } => {
            EpisodeNormalizationFailure::InvalidRangeDuration { start, horizon }
        }
        WindowNormalizationFailure::InvalidTemporalRange { error } => {
            EpisodeNormalizationFailure::InvalidTemporalRange(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        ClosedWindow, ComparisonNormalizationPolicy, ComparisonScope, ComparisonSelector,
        EpisodeError, EpisodeNormalizationFailure, OpenWindow, TemporalPoint, TemporalRange,
        WindowHistory, WindowRecordId,
    };

    use super::TemporalTolerance;

    fn closed(id: &str, start: i64, end: i64, source: &str) -> ClosedWindow {
        ClosedWindow {
            id: WindowRecordId::new(id).unwrap(),
            window_name: "Offline".to_owned(),
            key: "device-1".to_owned(),
            range: TemporalRange::positions(start, end).unwrap(),
            known_at: None,
            source: Some(source.to_owned()),
            partition: Some("north".to_owned()),
            segments: Vec::new(),
            tags: Vec::new(),
            boundary_reason: None,
            boundary_changes: Vec::new(),
        }
    }

    #[test]
    fn formation_stitches_exact_threshold_and_counts_union_not_envelope() {
        let history = WindowHistory::from_records(
            [
                closed("a", 0, 5, "provider-a"),
                closed("b", 3, 8, "provider-a"),
                closed("c", 10, 12, "provider-a"),
            ],
            [],
        )
        .unwrap();

        let set = history
            .form_episodes("outages")
            .from(ComparisonSelector::for_source("provider-a"))
            .scope(ComparisonScope::window("Offline"))
            .stitch_gaps_up_to(TemporalTolerance::processing_positions(2).unwrap())
            .run()
            .unwrap();

        let episode = &set.episodes()[0];
        assert_eq!(set.episodes().len(), 1);
        assert_eq!(episode.fragments().len(), 3);
        assert_eq!(episode.active_magnitude(), 10);
        assert_eq!(episode.elapsed_magnitude(), 12);
        assert_eq!(episode.internal_gap_magnitude(), 2);
        assert_eq!(episode.finality(), &crate::ComparisonFinality::Final);
    }

    #[test]
    fn formation_keeps_sources_separate_and_applies_strict_after_settling() {
        let history = WindowHistory::from_records(
            [
                closed("a", 0, 5, "provider-a"),
                closed("b", 0, 5, "provider-b"),
            ],
            [],
        )
        .unwrap();
        let builder = history
            .form_episodes("outages")
            .from(ComparisonSelector::serializable("all", "all"))
            .scope(ComparisonScope::window("Offline"))
            .stitch_gaps_up_to(TemporalTolerance::processing_positions(2).unwrap());

        let boundary = builder.run_live(TemporalPoint::position(7)).unwrap();
        assert_eq!(boundary.episodes().len(), 2);
        assert!(
            boundary
                .episodes()
                .iter()
                .all(|episode| episode.finality() == &crate::ComparisonFinality::Provisional)
        );

        let settled = builder.run_live(TemporalPoint::position(8)).unwrap();
        assert!(
            settled
                .episodes()
                .iter()
                .all(|episode| episode.finality() == &crate::ComparisonFinality::Final)
        );
    }

    #[test]
    fn timestamp_clocks_are_hard_group_boundaries() {
        let make = |id: &str, clock: &str| ClosedWindow {
            id: WindowRecordId::new(id).unwrap(),
            window_name: "Offline".to_owned(),
            key: "device-1".to_owned(),
            range: TemporalRange::new(
                TemporalPoint::timestamp_ticks_with_clock(0, clock),
                TemporalPoint::timestamp_ticks_with_clock(5, clock),
            )
            .unwrap(),
            known_at: None,
            source: Some("provider-a".to_owned()),
            partition: None,
            segments: Vec::new(),
            tags: Vec::new(),
            boundary_reason: None,
            boundary_changes: Vec::new(),
        };
        let history =
            WindowHistory::from_records([make("a", "utc"), make("b", "gps")], []).unwrap();
        let set = history
            .form_episodes("outages")
            .from(ComparisonSelector::for_source("provider-a"))
            .scope(ComparisonScope::window("Offline").on_event_time())
            .normalization(ComparisonNormalizationPolicy::event_time())
            .stitch_gaps_up_to(TemporalTolerance::timestamp_ticks(1).unwrap())
            .run()
            .unwrap();

        assert_eq!(set.episodes().len(), 2);
    }

    #[test]
    fn timestamp_horizon_rejects_a_different_clock() {
        let record = ClosedWindow {
            id: WindowRecordId::new("a").unwrap(),
            window_name: "Offline".to_owned(),
            key: "device-1".to_owned(),
            range: TemporalRange::new(
                TemporalPoint::timestamp_ticks_with_clock(0, "gps"),
                TemporalPoint::timestamp_ticks_with_clock(5, "gps"),
            )
            .unwrap(),
            known_at: None,
            source: Some("provider-a".to_owned()),
            partition: None,
            segments: Vec::new(),
            tags: Vec::new(),
            boundary_reason: None,
            boundary_changes: Vec::new(),
        };
        let history = WindowHistory::from_records([record], []).unwrap();
        let error = history
            .form_episodes("outages")
            .from(ComparisonSelector::for_source("provider-a"))
            .scope(ComparisonScope::window("Offline").on_event_time())
            .normalization(ComparisonNormalizationPolicy::event_time())
            .stitch_gaps_up_to(TemporalTolerance::timestamp_ticks(1).unwrap())
            .run_live(TemporalPoint::timestamp_ticks_with_clock(8, "utc"))
            .unwrap_err();

        assert!(matches!(error, EpisodeError::HorizonClockMismatch { .. }));
    }

    #[test]
    fn run_live_preserves_closed_only_policy_and_returns_typed_failure() {
        let open = OpenWindow {
            id: WindowRecordId::new("open-a").unwrap(),
            window_name: "Offline".to_owned(),
            key: "device-1".to_owned(),
            start: TemporalPoint::position(0),
            known_at: None,
            source: Some("provider-a".to_owned()),
            partition: None,
            segments: Vec::new(),
            tags: Vec::new(),
        };
        let history = WindowHistory::from_records([], [open]).unwrap();
        let mut policy = ComparisonNormalizationPolicy::require_closed();
        policy.require_closed_windows = true;
        let error = history
            .form_episodes("outages")
            .from(ComparisonSelector::for_source("provider-a"))
            .scope(ComparisonScope::window("Offline"))
            .normalization(policy)
            .run_live(TemporalPoint::position(5))
            .unwrap_err();

        assert!(matches!(
            error,
            EpisodeError::Normalization {
                cause: EpisodeNormalizationFailure::OpenWindowWithoutPolicy,
                ..
            }
        ));
    }

    #[test]
    fn identity_distinguishes_processing_positions_from_clockless_timestamps() {
        let position_history =
            WindowHistory::from_records([closed("same-id", 0, 5, "provider-a")], []).unwrap();
        let timestamp_record = ClosedWindow {
            id: WindowRecordId::new("same-id").unwrap(),
            window_name: "Offline".to_owned(),
            key: "device-1".to_owned(),
            range: TemporalRange::new(
                TemporalPoint::timestamp_ticks(0),
                TemporalPoint::timestamp_ticks(5),
            )
            .unwrap(),
            known_at: None,
            source: Some("provider-a".to_owned()),
            partition: Some("north".to_owned()),
            segments: Vec::new(),
            tags: Vec::new(),
            boundary_reason: None,
            boundary_changes: Vec::new(),
        };
        let timestamp_history = WindowHistory::from_records([timestamp_record], []).unwrap();
        let position = position_history
            .form_episodes("outages")
            .from(ComparisonSelector::for_source("provider-a"))
            .scope(ComparisonScope::window("Offline"))
            .run()
            .unwrap();
        let timestamp = timestamp_history
            .form_episodes("outages")
            .from(ComparisonSelector::for_source("provider-a"))
            .scope(ComparisonScope::window("Offline").on_event_time())
            .normalization(ComparisonNormalizationPolicy::event_time())
            .run()
            .unwrap();

        assert_ne!(position.episodes()[0].id(), timestamp.episodes()[0].id());
    }
}
