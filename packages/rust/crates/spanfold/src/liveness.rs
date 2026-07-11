use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{TemporalAxis, TemporalPoint};

/// Stable identity for one tracked source lane.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct LaneKey {
    /// Source or lane identifier.
    pub lane: String,
    /// Optional partition identifier.
    pub partition: Option<String>,
}

impl LaneKey {
    /// Creates an unpartitioned lane key.
    #[must_use]
    pub fn new(lane: impl Into<String>) -> Self {
        Self {
            lane: lane.into(),
            partition: None,
        }
    }

    /// Creates a partition-aware lane key.
    #[must_use]
    pub fn with_partition(lane: impl Into<String>, partition: impl Into<String>) -> Self {
        Self {
            lane: lane.into(),
            partition: Some(partition.into()),
        }
    }
}

impl From<&str> for LaneKey {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for LaneKey {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

/// A deterministic lane liveness state change.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct LaneLivenessSignal {
    /// Source or lane identifier.
    pub lane: String,
    /// Optional partition identifier.
    pub partition: Option<String>,
    /// Whether this signal represents silence instead of recovery/alive state.
    #[serde(rename = "isSilent")]
    pub is_silent: bool,
    /// Point where the liveness transition occurred.
    #[serde(rename = "occurredAt")]
    pub occurred_at: TemporalPoint,
    /// Point where the lane was evaluated.
    #[serde(rename = "evaluatedAt")]
    pub evaluated_at: TemporalPoint,
    /// Silence threshold magnitude on the same axis as the tracked points.
    #[serde(rename = "silenceThresholdMagnitude")]
    pub silence_threshold_magnitude: i64,
}

impl<'de> Deserialize<'de> for LaneLivenessSignal {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawSignal {
            lane: String,
            partition: Option<String>,
            #[serde(rename = "isSilent")]
            is_silent: bool,
            #[serde(rename = "occurredAt")]
            occurred_at: TemporalPoint,
            #[serde(rename = "evaluatedAt")]
            evaluated_at: TemporalPoint,
            #[serde(rename = "silenceThresholdMagnitude")]
            silence_threshold_magnitude: i64,
        }

        let raw = RawSignal::deserialize(deserializer)?;
        if raw.lane.trim().is_empty() {
            return Err(serde::de::Error::custom("lane cannot be empty"));
        }
        if raw.silence_threshold_magnitude <= 0 {
            return Err(serde::de::Error::custom(
                "silence threshold must be greater than zero",
            ));
        }
        if !raw.occurred_at.is_compatible_with(&raw.evaluated_at)
            || !matches!(
                raw.occurred_at.try_cmp(&raw.evaluated_at),
                Ok(std::cmp::Ordering::Less | std::cmp::Ordering::Equal)
            )
        {
            return Err(serde::de::Error::custom(
                "liveness signal points must share a domain and occurredAt must not be after evaluatedAt",
            ));
        }
        Ok(Self {
            lane: raw.lane,
            partition: raw.partition,
            is_silent: raw.is_silent,
            occurred_at: raw.occurred_at,
            evaluated_at: raw.evaluated_at,
            silence_threshold_magnitude: raw.silence_threshold_magnitude,
        })
    }
}

impl LaneLivenessSignal {
    fn new(
        key: &LaneKey,
        is_silent: bool,
        occurred_at: TemporalPoint,
        evaluated_at: TemporalPoint,
        silence_threshold_magnitude: i64,
    ) -> Self {
        Self {
            lane: key.lane.clone(),
            partition: key.partition.clone(),
            is_silent,
            occurred_at,
            evaluated_at,
            silence_threshold_magnitude,
        }
    }
}

#[derive(Clone, Debug)]
struct LaneState {
    key: LaneKey,
    started_at: TemporalPoint,
    last_observed_at: Option<TemporalPoint>,
    has_reported_state: bool,
    is_silent: bool,
}

impl LaneState {
    fn new(key: LaneKey, started_at: TemporalPoint) -> Self {
        Self {
            key,
            started_at,
            last_observed_at: None,
            has_reported_state: false,
            is_silent: false,
        }
    }
}

/// Liveness tracker construction and evaluation errors.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum LaneLivenessError {
    /// At least one lane must be tracked.
    #[error("at least one lane must be tracked")]
    EmptyLanes,
    /// Silence thresholds must be positive.
    #[error("silence threshold must be greater than zero: {threshold}")]
    NonPositiveThreshold {
        /// Invalid threshold magnitude.
        threshold: i64,
    },
    /// Tracked lane identities must be unique.
    #[error("tracked lanes must be unique: {lane:?}")]
    DuplicateLane {
        /// Duplicate lane key.
        lane: LaneKey,
    },
    /// Observations/checks must use the same temporal axis as the tracker.
    #[error("liveness point axis mismatch: expected={expected:?}, actual={actual:?}")]
    AxisMismatch {
        /// Expected temporal axis.
        expected: TemporalAxis,
        /// Actual temporal axis.
        actual: TemporalAxis,
    },
    /// Observations cannot precede tracker start.
    #[error("observation cannot be earlier than tracker start")]
    ObservationBeforeStart,
    /// Per-lane observations must be monotonic.
    #[error("observation cannot be earlier than the lane's previous observation: {lane:?}")]
    ObservationMovedBackwards {
        /// Lane that moved backwards.
        lane: LaneKey,
    },
    /// Check horizons cannot precede tracker start.
    #[error("liveness horizon cannot be earlier than tracker start")]
    HorizonBeforeStart,
    /// Check horizons must be monotonic.
    #[error("liveness horizon cannot move backwards")]
    HorizonMovedBackwards,
    /// Observed lane is not tracked.
    #[error("lane is not tracked by this liveness tracker: {lane:?}")]
    UnknownLane {
        /// Unknown lane key.
        lane: LaneKey,
    },
    /// Timestamp points use different clocks.
    #[error("liveness point clock mismatch")]
    ClockMismatch,
    /// Observations cannot arrive before the last evaluation horizon.
    #[error("observation cannot precede the last liveness check")]
    ObservationBeforeLastCheck,
    /// Silence threshold arithmetic overflowed.
    #[error("liveness silence horizon overflowed")]
    ArithmeticOverflow,
}

/// Deterministic heartbeat/silence tracker for a fixed set of lanes.
///
/// The tracker owns no timers, background tasks, persistence, or IO. Call
/// [`observe`](Self::observe) when a lane reports and [`check`](Self::check) at
/// explicit horizons. Returned signals can be ingested into a normal Spanfold
/// pipeline to record silence windows.
#[derive(Clone, Debug)]
pub struct LaneLivenessTracker {
    started_at: TemporalPoint,
    last_check_at: TemporalPoint,
    silence_threshold_magnitude: i64,
    lanes: BTreeMap<LaneKey, LaneState>,
}

impl LaneLivenessTracker {
    /// Creates a tracker for explicit lane keys.
    pub fn new<I>(
        started_at: TemporalPoint,
        silence_threshold_magnitude: i64,
        lanes: I,
    ) -> Result<Self, LaneLivenessError>
    where
        I: IntoIterator<Item = LaneKey>,
    {
        if silence_threshold_magnitude <= 0 {
            return Err(LaneLivenessError::NonPositiveThreshold {
                threshold: silence_threshold_magnitude,
            });
        }

        let mut states = BTreeMap::new();
        for lane in lanes {
            if states.contains_key(&lane) {
                return Err(LaneLivenessError::DuplicateLane { lane });
            }
            states.insert(lane.clone(), LaneState::new(lane, started_at.clone()));
        }

        if states.is_empty() {
            return Err(LaneLivenessError::EmptyLanes);
        }

        Ok(Self {
            started_at: started_at.clone(),
            last_check_at: started_at,
            silence_threshold_magnitude,
            lanes: states,
        })
    }

    /// Creates a tracker for unpartitioned lanes.
    pub fn for_lanes<I, S>(
        started_at: TemporalPoint,
        silence_threshold_magnitude: i64,
        lanes: I,
    ) -> Result<Self, LaneLivenessError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self::new(
            started_at,
            silence_threshold_magnitude,
            lanes.into_iter().map(|lane| LaneKey::new(lane.into())),
        )
    }

    /// Returns the tracker start point.
    #[must_use]
    pub fn started_at(&self) -> TemporalPoint {
        self.started_at.clone()
    }

    /// Returns the configured silence threshold magnitude.
    #[must_use]
    pub const fn silence_threshold_magnitude(&self) -> i64 {
        self.silence_threshold_magnitude
    }

    /// Records an observation for an unpartitioned lane.
    pub fn observe(
        &mut self,
        lane: impl Into<String>,
        observed_at: TemporalPoint,
    ) -> Result<Vec<LaneLivenessSignal>, LaneLivenessError> {
        self.observe_key(LaneKey::new(lane.into()), observed_at)
    }

    /// Records an observation for a partitioned lane.
    pub fn observe_partition(
        &mut self,
        lane: impl Into<String>,
        partition: impl Into<String>,
        observed_at: TemporalPoint,
    ) -> Result<Vec<LaneLivenessSignal>, LaneLivenessError> {
        self.observe_key(
            LaneKey::with_partition(lane.into(), partition.into()),
            observed_at,
        )
    }

    /// Records an observation for an explicit lane key.
    pub fn observe_key(
        &mut self,
        lane: LaneKey,
        observed_at: TemporalPoint,
    ) -> Result<Vec<LaneLivenessSignal>, LaneLivenessError> {
        self.ensure_compatible(&observed_at)?;
        if matches!(
            observed_at.try_cmp(&self.started_at),
            Ok(std::cmp::Ordering::Less)
        ) {
            return Err(LaneLivenessError::ObservationBeforeStart);
        }
        if matches!(
            observed_at.try_cmp(&self.last_check_at),
            Ok(std::cmp::Ordering::Less)
        ) {
            return Err(LaneLivenessError::ObservationBeforeLastCheck);
        }

        let Some(state) = self.lanes.get_mut(&lane) else {
            return Err(LaneLivenessError::UnknownLane { lane });
        };
        if state
            .last_observed_at
            .as_ref()
            .is_some_and(|last_observed_at| observed_at.magnitude() < last_observed_at.magnitude())
        {
            return Err(LaneLivenessError::ObservationMovedBackwards { lane });
        }

        state.last_observed_at = Some(observed_at.clone());
        if !state.has_reported_state || state.is_silent {
            state.has_reported_state = true;
            state.is_silent = false;
            return Ok(vec![LaneLivenessSignal::new(
                &state.key,
                false,
                observed_at.clone(),
                observed_at,
                self.silence_threshold_magnitude,
            )]);
        }

        Ok(Vec::new())
    }

    /// Evaluates all tracked lanes at an explicit horizon.
    pub fn check(
        &mut self,
        horizon: TemporalPoint,
    ) -> Result<Vec<LaneLivenessSignal>, LaneLivenessError> {
        self.ensure_compatible(&horizon)?;
        if matches!(
            horizon.try_cmp(&self.started_at),
            Ok(std::cmp::Ordering::Less)
        ) {
            return Err(LaneLivenessError::HorizonBeforeStart);
        }
        if matches!(
            horizon.try_cmp(&self.last_check_at),
            Ok(std::cmp::Ordering::Less)
        ) {
            return Err(LaneLivenessError::HorizonMovedBackwards);
        }

        self.last_check_at = horizon.clone();
        let threshold = self.silence_threshold_magnitude;
        let mut signals = Vec::new();
        for state in self.lanes.values_mut() {
            let silence_started_at = add_magnitude(
                state
                    .last_observed_at
                    .clone()
                    .unwrap_or_else(|| state.started_at.clone()),
                threshold,
            )?;
            if state.is_silent
                || matches!(
                    horizon.try_cmp(&silence_started_at),
                    Ok(std::cmp::Ordering::Less)
                )
            {
                continue;
            }

            state.has_reported_state = true;
            state.is_silent = true;
            signals.push(LaneLivenessSignal::new(
                &state.key,
                true,
                silence_started_at,
                horizon.clone(),
                threshold,
            ));
        }
        Ok(signals)
    }

    fn ensure_compatible(&self, point: &TemporalPoint) -> Result<(), LaneLivenessError> {
        let expected = self.started_at.axis();
        let actual = point.axis();
        if expected != actual {
            return Err(LaneLivenessError::AxisMismatch { expected, actual });
        }
        if !self.started_at.is_compatible_with(point) {
            return Err(LaneLivenessError::ClockMismatch);
        }
        Ok(())
    }
}

fn add_magnitude(point: TemporalPoint, magnitude: i64) -> Result<TemporalPoint, LaneLivenessError> {
    let value = point
        .magnitude()
        .checked_add(magnitude)
        .ok_or(LaneLivenessError::ArithmeticOverflow)?;
    match point.axis() {
        TemporalAxis::ProcessingPosition => Ok(TemporalPoint::position(value)),
        TemporalAxis::Timestamp => Ok(match point.clock() {
            Some(clock) => TemporalPoint::timestamp_ticks_with_clock(value, clock),
            None => TemporalPoint::timestamp_ticks(value),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_observation_emits_alive_once() {
        let started_at = TemporalPoint::timestamp_ticks(100);
        let mut tracker =
            LaneLivenessTracker::for_lanes(started_at.clone(), 30, ["lane-a"]).expect("tracker");

        let first = tracker
            .observe("lane-a", TemporalPoint::timestamp_ticks(105))
            .expect("first observation");
        let second = tracker
            .observe("lane-a", TemporalPoint::timestamp_ticks(110))
            .expect("second observation");

        assert_eq!(first.len(), 1);
        assert_eq!(first[0].lane, "lane-a");
        assert!(!first[0].is_silent);
        assert_eq!(first[0].occurred_at, TemporalPoint::timestamp_ticks(105));
        assert!(second.is_empty());
    }

    #[test]
    fn check_emits_silence_once_when_lane_expires() {
        let started_at = TemporalPoint::timestamp_ticks(100);
        let mut tracker =
            LaneLivenessTracker::for_lanes(started_at.clone(), 30, ["lane-a"]).expect("tracker");
        tracker
            .observe("lane-a", TemporalPoint::timestamp_ticks(105))
            .expect("observation");

        let early = tracker
            .check(TemporalPoint::timestamp_ticks(134))
            .expect("early check");
        let expired = tracker
            .check(TemporalPoint::timestamp_ticks(140))
            .expect("expired check");
        let repeated = tracker
            .check(TemporalPoint::timestamp_ticks(150))
            .expect("repeated check");

        assert!(early.is_empty());
        assert_eq!(expired.len(), 1);
        assert!(expired[0].is_silent);
        assert_eq!(expired[0].occurred_at, TemporalPoint::timestamp_ticks(135));
        assert_eq!(expired[0].evaluated_at, TemporalPoint::timestamp_ticks(140));
        assert!(repeated.is_empty());
    }

    #[test]
    fn observation_after_silence_emits_recovery() {
        let started_at = TemporalPoint::timestamp_ticks(100);
        let mut tracker =
            LaneLivenessTracker::for_lanes(started_at.clone(), 30, ["lane-a"]).expect("tracker");

        tracker
            .observe("lane-a", started_at)
            .expect("first observation");
        tracker
            .check(TemporalPoint::timestamp_ticks(131))
            .expect("silence check");
        let recovery = tracker
            .observe("lane-a", TemporalPoint::timestamp_ticks(145))
            .expect("recovery");

        assert_eq!(recovery.len(), 1);
        assert!(!recovery[0].is_silent);
        assert_eq!(recovery[0].occurred_at, TemporalPoint::timestamp_ticks(145));
    }

    #[test]
    fn check_can_emit_silence_for_lane_that_never_reported() {
        let started_at = TemporalPoint::timestamp_ticks(100);
        let mut tracker =
            LaneLivenessTracker::for_lanes(started_at.clone(), 30, ["lane-a"]).expect("tracker");

        let signal = tracker
            .check(TemporalPoint::timestamp_ticks(140))
            .expect("check")
            .remove(0);

        assert!(signal.is_silent);
        assert_eq!(signal.occurred_at, TemporalPoint::timestamp_ticks(130));
        assert_eq!(signal.evaluated_at, TemporalPoint::timestamp_ticks(140));
    }

    #[test]
    fn tracker_rejects_unknown_lane() {
        let started_at = TemporalPoint::timestamp_ticks(100);
        let mut tracker =
            LaneLivenessTracker::for_lanes(started_at.clone(), 30, ["lane-a"]).expect("tracker");

        let error = tracker
            .observe("lane-b", started_at)
            .expect_err("unknown lane should fail");

        assert!(matches!(error, LaneLivenessError::UnknownLane { .. }));
    }

    #[test]
    fn partitioned_lanes_are_tracked_independently() {
        let started_at = TemporalPoint::position(0);
        let mut tracker = LaneLivenessTracker::new(
            started_at,
            10,
            [
                LaneKey::with_partition("provider-a", "partition-1"),
                LaneKey::with_partition("provider-a", "partition-2"),
            ],
        )
        .expect("tracker");

        tracker
            .observe_partition("provider-a", "partition-1", TemporalPoint::position(1))
            .expect("partition-1 observation");
        let signals = tracker.check(TemporalPoint::position(10)).expect("check");

        assert_eq!(signals.len(), 1);
        assert_eq!(signals[0].partition.as_deref(), Some("partition-2"));
    }
}
