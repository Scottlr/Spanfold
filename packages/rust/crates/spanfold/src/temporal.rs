use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Temporal axis used by a point or range.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum TemporalAxis {
    /// Monotonic ingestion or processing position.
    ProcessingPosition,
    /// Event timestamp represented as ticks.
    Timestamp,
}

/// A typed temporal point.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct TemporalPoint {
    axis: TemporalAxis,
    magnitude: i64,
    #[serde(skip_deserializing)]
    clock: Option<&'static str>,
}

impl TemporalPoint {
    /// Creates a processing-position point.
    #[must_use]
    pub const fn position(position: i64) -> Self {
        Self {
            axis: TemporalAxis::ProcessingPosition,
            magnitude: position,
            clock: None,
        }
    }

    /// Creates a timestamp point from ticks.
    #[must_use]
    pub const fn timestamp_ticks(ticks: i64) -> Self {
        Self {
            axis: TemporalAxis::Timestamp,
            magnitude: ticks,
            clock: None,
        }
    }

    /// Creates a timestamp point from ticks and a stable clock identity.
    #[must_use]
    pub const fn timestamp_ticks_with_clock(ticks: i64, clock: &'static str) -> Self {
        Self {
            axis: TemporalAxis::Timestamp,
            magnitude: ticks,
            clock: Some(clock),
        }
    }

    /// Returns the point axis.
    #[must_use]
    pub const fn axis(self) -> TemporalAxis {
        self.axis
    }

    /// Returns the point magnitude.
    #[must_use]
    pub const fn magnitude(self) -> i64 {
        self.magnitude
    }

    /// Returns the point clock identity, when any.
    #[must_use]
    pub const fn clock(self) -> Option<&'static str> {
        self.clock
    }
}

/// A half-open temporal range, `[start, end)`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TemporalRange {
    start: TemporalPoint,
    end: TemporalPoint,
}

impl TemporalRange {
    /// Creates a half-open temporal range.
    ///
    /// The start and end points must share an axis, and `start <= end`.
    pub fn new(start: TemporalPoint, end: TemporalPoint) -> Result<Self, TemporalRangeError> {
        if start.axis() != end.axis() {
            return Err(TemporalRangeError::AxisMismatch {
                start: start.axis(),
                end: end.axis(),
            });
        }

        if start > end {
            return Err(TemporalRangeError::EndBeforeStart { start, end });
        }

        Ok(Self { start, end })
    }

    /// Creates a processing-position range.
    pub fn positions(start: i64, end: i64) -> Result<Self, TemporalRangeError> {
        Self::new(TemporalPoint::position(start), TemporalPoint::position(end))
    }

    /// Returns the inclusive start point.
    #[must_use]
    pub const fn start(self) -> TemporalPoint {
        self.start
    }

    /// Returns the exclusive end point.
    #[must_use]
    pub const fn end(self) -> TemporalPoint {
        self.end
    }

    /// Returns the non-negative range magnitude.
    #[must_use]
    pub fn magnitude(self) -> i64 {
        self.end.magnitude() - self.start.magnitude()
    }
}

/// Temporal range construction error.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum TemporalRangeError {
    /// Start and end use different temporal axes.
    #[error("temporal range axis mismatch: start={start:?}, end={end:?}")]
    AxisMismatch {
        /// Start axis.
        start: TemporalAxis,
        /// End axis.
        end: TemporalAxis,
    },
    /// End point is before start point.
    #[error("temporal range end is before start: start={start:?}, end={end:?}")]
    EndBeforeStart {
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
    fn ranges_reject_end_before_start() {
        let error = TemporalRange::positions(5, 3).expect_err("reversed range should fail");

        assert!(matches!(error, TemporalRangeError::EndBeforeStart { .. }));
    }
}
