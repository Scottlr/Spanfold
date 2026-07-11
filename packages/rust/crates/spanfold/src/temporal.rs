use std::cmp::Ordering;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Temporal axis used by a point or range.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum TemporalAxis {
    /// Monotonic ingestion or processing position.
    ProcessingPosition,
    /// Event timestamp represented as ticks.
    Timestamp,
}

/// A typed temporal point.
///
/// Timestamp points are comparable only when their clock identities match.
/// Processing positions have no clock identity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemporalPoint {
    axis: TemporalAxis,
    magnitude: i64,
    clock: Option<String>,
}

impl TemporalPoint {
    /// Creates a processing-position point.
    #[must_use]
    pub fn position(position: i64) -> Self {
        Self {
            axis: TemporalAxis::ProcessingPosition,
            magnitude: position,
            clock: None,
        }
    }

    /// Creates a timestamp point from opaque ticks.
    ///
    /// The unit and epoch are part of the clock contract supplied by the
    /// caller; values from different contracts must use different clock IDs.
    #[must_use]
    pub fn timestamp_ticks(ticks: i64) -> Self {
        Self {
            axis: TemporalAxis::Timestamp,
            magnitude: ticks,
            clock: None,
        }
    }

    /// Creates a timestamp point from ticks and a stable clock identity.
    /// The clock ID identifies the unit/epoch contract; Spanfold does not
    /// reinterpret or convert ticks.
    #[must_use]
    pub fn timestamp_ticks_with_clock(ticks: i64, clock: impl Into<String>) -> Self {
        Self {
            axis: TemporalAxis::Timestamp,
            magnitude: ticks,
            clock: Some(clock.into()),
        }
    }

    /// Returns the point axis.
    #[must_use]
    pub const fn axis(&self) -> TemporalAxis {
        self.axis
    }

    /// Returns the point magnitude.
    #[must_use]
    pub const fn magnitude(&self) -> i64 {
        self.magnitude
    }

    /// Returns the point clock identity, when any.
    #[must_use]
    pub fn clock(&self) -> Option<&str> {
        self.clock.as_deref()
    }

    /// Returns whether two points belong to the same temporal domain.
    #[must_use]
    pub fn is_compatible_with(&self, other: &Self) -> bool {
        self.axis == other.axis
            && (self.axis != TemporalAxis::Timestamp || self.clock == other.clock)
    }

    /// Compares two points after checking axis and clock compatibility.
    pub fn try_cmp(&self, other: &Self) -> Result<Ordering, TemporalPointError> {
        if self.axis != other.axis {
            return Err(TemporalPointError::AxisMismatch {
                left: self.axis,
                right: other.axis,
            });
        }
        if self.axis == TemporalAxis::Timestamp && self.clock != other.clock {
            return Err(TemporalPointError::ClockMismatch {
                left: self.clock.clone(),
                right: other.clock.clone(),
            });
        }
        Ok(self.magnitude.cmp(&other.magnitude))
    }
}

/// Error returned when temporal points are compared across incompatible domains.
#[derive(Clone, Debug, Eq, PartialEq, Error)]
pub enum TemporalPointError {
    /// Points use different temporal axes.
    #[error("temporal point axis mismatch: left={left:?}, right={right:?}")]
    AxisMismatch {
        /// Left axis.
        left: TemporalAxis,
        /// Right axis.
        right: TemporalAxis,
    },
    /// Timestamp points use different clocks.
    #[error("temporal point clock mismatch: left={left:?}, right={right:?}")]
    ClockMismatch {
        /// Left clock.
        left: Option<String>,
        /// Right clock.
        right: Option<String>,
    },
}

/// A half-open temporal range, `[start, end)`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct TemporalRange {
    start: TemporalPoint,
    end: TemporalPoint,
}

impl TemporalRange {
    /// Creates a half-open temporal range.
    ///
    /// The start and end points must share an axis and timestamp clock, and
    /// `start <= end` within that domain.
    pub fn new(start: TemporalPoint, end: TemporalPoint) -> Result<Self, TemporalRangeError> {
        if start.axis() != end.axis() {
            return Err(TemporalRangeError::AxisMismatch {
                start: start.axis(),
                end: end.axis(),
            });
        }

        if start.axis() == TemporalAxis::Timestamp && start.clock != end.clock {
            return Err(TemporalRangeError::ClockMismatch {
                start: start.clock.clone(),
                end: end.clock.clone(),
            });
        }

        if start.magnitude > end.magnitude {
            return Err(TemporalRangeError::EndBeforeStart { start, end });
        }

        if end.magnitude.checked_sub(start.magnitude).is_none() {
            return Err(TemporalRangeError::MagnitudeOverflow { start, end });
        }

        Ok(Self { start, end })
    }

    /// Creates a processing-position range.
    pub fn positions(start: i64, end: i64) -> Result<Self, TemporalRangeError> {
        Self::new(TemporalPoint::position(start), TemporalPoint::position(end))
    }

    /// Returns the inclusive start point.
    #[must_use]
    pub fn start(&self) -> TemporalPoint {
        self.start.clone()
    }

    /// Returns the exclusive end point.
    #[must_use]
    pub fn end(&self) -> TemporalPoint {
        self.end.clone()
    }

    /// Returns the non-negative range magnitude.
    #[must_use]
    pub fn magnitude(&self) -> i64 {
        // `new` and validated deserialization reject an overflowing duration.
        self.end
            .magnitude
            .checked_sub(self.start.magnitude)
            .expect("validated temporal range magnitude")
    }
}

impl<'de> Deserialize<'de> for TemporalRange {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawRange {
            start: TemporalPoint,
            end: TemporalPoint,
        }

        let raw = RawRange::deserialize(deserializer)?;
        Self::new(raw.start, raw.end).map_err(serde::de::Error::custom)
    }
}

/// Temporal range construction error.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum TemporalRangeError {
    /// Start and end use different temporal axes.
    #[error("temporal range axis mismatch: start={start:?}, end={end:?}")]
    AxisMismatch {
        /// Start axis.
        start: TemporalAxis,
        /// End axis.
        end: TemporalAxis,
    },
    /// Timestamp endpoints use different clocks.
    #[error("temporal range clock mismatch: start={start:?}, end={end:?}")]
    ClockMismatch {
        /// Start clock.
        start: Option<String>,
        /// End clock.
        end: Option<String>,
    },
    /// End point is before start point.
    #[error("temporal range end is before start: start={start:?}, end={end:?}")]
    EndBeforeStart {
        /// Start point.
        start: TemporalPoint,
        /// End point.
        end: TemporalPoint,
    },
    /// The range duration cannot be represented as an `i64`.
    #[error("temporal range magnitude overflows i64: start={start:?}, end={end:?}")]
    MagnitudeOverflow {
        /// Start point.
        start: TemporalPoint,
        /// End point.
        end: TemporalPoint,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn position_ranges_are_half_open_and_have_magnitude() {
        let range = TemporalRange::positions(10, 14).expect("valid range");

        assert_eq!(range.start(), TemporalPoint::position(10));
        assert_eq!(range.end(), TemporalPoint::position(14));
        assert_eq!(range.magnitude(), 4);
    }

    #[test]
    fn timestamp_points_can_carry_clock_identity() {
        let point = TemporalPoint::timestamp_ticks_with_clock(10, "provider");

        assert_eq!(point.axis(), TemporalAxis::Timestamp);
        assert_eq!(point.magnitude(), 10);
        assert_eq!(point.clock(), Some("provider"));
    }

    #[test]
    fn ranges_reject_mixed_axes() {
        let error = TemporalRange::new(
            TemporalPoint::position(1),
            TemporalPoint::timestamp_ticks(2),
        )
        .expect_err("mixed axes should fail");

        assert!(matches!(error, TemporalRangeError::AxisMismatch { .. }));
    }

    #[test]
    fn ranges_reject_mixed_clocks_and_overflowing_magnitude() {
        let error = TemporalRange::new(
            TemporalPoint::timestamp_ticks_with_clock(1, "provider-a"),
            TemporalPoint::timestamp_ticks_with_clock(2, "provider-b"),
        )
        .expect_err("mixed clocks should fail");
        assert!(matches!(error, TemporalRangeError::ClockMismatch { .. }));

        let error = TemporalRange::new(
            TemporalPoint::position(i64::MIN),
            TemporalPoint::position(i64::MAX),
        )
        .expect_err("overflowing magnitude should fail");
        assert!(matches!(
            error,
            TemporalRangeError::MagnitudeOverflow { .. }
        ));
    }

    #[test]
    fn points_do_not_order_across_domains() {
        assert!(matches!(
            TemporalPoint::position(1).try_cmp(&TemporalPoint::timestamp_ticks(1)),
            Err(TemporalPointError::AxisMismatch { .. })
        ));
        assert!(matches!(
            TemporalPoint::timestamp_ticks_with_clock(1, "a")
                .try_cmp(&TemporalPoint::timestamp_ticks_with_clock(1, "b")),
            Err(TemporalPointError::ClockMismatch { .. })
        ));
    }
}
