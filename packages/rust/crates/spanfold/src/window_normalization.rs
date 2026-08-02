use std::cmp::Ordering;

use crate::{
    ClosedWindow, ComparisonNullTimestampPolicy, ComparisonScope, OpenWindow, OpenWindowPolicy,
    TemporalAxis, TemporalPoint, TemporalRange, TemporalRangeError, WindowHistory, WindowRecord,
};

/// Borrowed raw history candidate in the deterministic comparison/formation order.
#[derive(Clone, Copy, Debug)]
pub(crate) enum RawWindowRef<'a> {
    /// Closed history record.
    Closed(&'a ClosedWindow),
    /// Open history record.
    Open(&'a OpenWindow),
}

impl RawWindowRef<'_> {
    /// Returns the stable record identifier.
    pub(crate) fn record_id(&self) -> &str {
        match self {
            Self::Closed(window) => window.id.as_str(),
            Self::Open(window) => window.id.as_str(),
        }
    }

    /// Returns the window family name.
    pub(crate) fn window_name(&self) -> &str {
        match self {
            Self::Closed(window) => &window.window_name,
            Self::Open(window) => &window.window_name,
        }
    }

    /// Returns the logical key.
    pub(crate) fn key(&self) -> &str {
        match self {
            Self::Closed(window) => &window.key,
            Self::Open(window) => &window.key,
        }
    }

    /// Returns the optional source/lane.
    pub(crate) fn source(&self) -> Option<&str> {
        match self {
            Self::Closed(window) => window.source.as_deref(),
            Self::Open(window) => window.source.as_deref(),
        }
    }

    /// Returns the optional partition.
    pub(crate) fn partition(&self) -> Option<&str> {
        match self {
            Self::Closed(window) => window.partition.as_deref(),
            Self::Open(window) => window.partition.as_deref(),
        }
    }

    /// Returns the scalar start magnitude used by history ordering.
    pub(crate) fn start_position(&self) -> i64 {
        self.start_point().magnitude()
    }

    /// Returns the temporal start point.
    pub(crate) fn start_point(&self) -> TemporalPoint {
        match self {
            Self::Closed(window) => window.range.start(),
            Self::Open(window) => window.start.clone(),
        }
    }

    /// Returns the scalar closed end magnitude used by history ordering.
    pub(crate) fn end_position(&self) -> Option<i64> {
        self.end_point().map(|point| point.magnitude())
    }

    /// Returns the closed end point, when present.
    pub(crate) fn end_point(&self) -> Option<TemporalPoint> {
        match self {
            Self::Closed(window) => Some(window.range.end()),
            Self::Open(_) => None,
        }
    }

    /// Returns the explicit availability point, when present.
    pub(crate) fn known_at_point(&self) -> Option<TemporalPoint> {
        match self {
            Self::Closed(window) => window.known_at.clone(),
            Self::Open(window) => window.known_at.clone(),
        }
    }

    /// Returns the scalar availability magnitude, when present.
    pub(crate) fn known_at_position(&self) -> Option<i64> {
        self.known_at_point().map(|point| point.magnitude())
    }

    /// Returns captured segments.
    pub(crate) fn segments(&self) -> &[crate::WindowSegment] {
        match self {
            Self::Closed(window) => &window.segments,
            Self::Open(window) => &window.segments,
        }
    }

    /// Returns captured tags.
    pub(crate) fn tags(&self) -> &[crate::WindowTag] {
        match self {
            Self::Closed(window) => &window.tags,
            Self::Open(window) => &window.tags,
        }
    }

    /// Returns whether the source record is open.
    pub(crate) fn is_open(&self) -> bool {
        matches!(self, Self::Open(_))
    }

    /// Materializes the borrowed candidate as the neutral history record type.
    pub(crate) fn to_window_record(self) -> WindowRecord {
        match self {
            Self::Closed(window) => WindowRecord::Closed((*window).clone()),
            Self::Open(window) => WindowRecord::Open((*window).clone()),
        }
    }
}

/// Returns all history records in the comparison/formation order.
#[must_use]
pub(crate) fn ordered_candidates(history: &WindowHistory) -> Vec<RawWindowRef<'_>> {
    let mut candidates = history
        .closed_windows()
        .iter()
        .map(RawWindowRef::Closed)
        .collect::<Vec<_>>();
    candidates.extend(history.open_windows().iter().map(RawWindowRef::Open));

    candidates.sort_by(|left, right| {
        (
            left.window_name(),
            left.key(),
            left.source().unwrap_or(""),
            left.partition().unwrap_or(""),
            left.start_position(),
            left.end_position().unwrap_or(i64::MAX),
            left.record_id(),
        )
            .cmp(&(
                right.window_name(),
                right.key(),
                right.source().unwrap_or(""),
                right.partition().unwrap_or(""),
                right.start_position(),
                right.end_position().unwrap_or(i64::MAX),
                right.record_id(),
            ))
    });
    candidates
}

/// Private normalization request shared by analytical consumers.
pub(crate) struct WindowNormalizationRequest<'a> {
    /// Scope used for neutral evidence membership.
    pub(crate) scope: &'a ComparisonScope,
    /// Requested temporal axis.
    pub(crate) time_axis: TemporalAxis,
    /// Availability horizon for known-at filtering.
    pub(crate) known_at: Option<&'a TemporalPoint>,
    /// Missing-timestamp policy.
    pub(crate) null_timestamp_policy: ComparisonNullTimestampPolicy,
    /// Whether open windows must be closed.
    pub(crate) require_closed: bool,
    /// Open-window handling policy.
    pub(crate) open_window_policy: OpenWindowPolicy,
    /// Effective evaluation horizon used for open-window clipping.
    pub(crate) evaluation_horizon: Option<&'a TemporalPoint>,
}

/// Neutral reason why a raw candidate could not produce normalized evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum WindowNormalizationFailure {
    /// The candidate was not available at the requested known-at point.
    FutureWindowExcluded {
        /// Candidate availability point used by the eligibility check.
        available_at: TemporalPoint,
        /// Requested known-at point.
        known_at: TemporalPoint,
    },
    /// A candidate uses the wrong non-timestamp temporal axis.
    TemporalAxisMismatch {
        /// Requested axis.
        expected: TemporalAxis,
        /// Candidate axis.
        actual: TemporalAxis,
    },
    /// A timestamp-axis request received a position-only candidate.
    MissingTimestamp {
        /// Candidate axis.
        actual: TemporalAxis,
        /// Policy governing the comparison outcome.
        policy: ComparisonNullTimestampPolicy,
    },
    /// An open candidate had no permitted clipping horizon.
    OpenWindowWithoutPolicy,
    /// An open-window horizon was incompatible with or earlier than its start.
    InvalidRangeDuration {
        /// Candidate start point.
        start: TemporalPoint,
        /// Configured clipping horizon.
        horizon: TemporalPoint,
    },
    /// The source range failed temporal-domain or duration validation.
    InvalidTemporalRange {
        /// Underlying neutral range error.
        error: TemporalRangeError,
    },
}

/// Neutral normalized evidence retained for a comparison or future formation consumer.
#[derive(Clone, Debug)]
pub(crate) struct NormalizedWindowEvidence<'a> {
    /// Original borrowed history candidate.
    pub(crate) candidate: RawWindowRef<'a>,
    /// Bounded half-open range.
    pub(crate) range: TemporalRange,
    /// Whether the range depends on an open-window clip.
    pub(crate) is_provisional: bool,
}

/// Tests whether a raw candidate belongs to the configured neutral scope.
#[must_use]
pub(crate) fn matches_scope(candidate: RawWindowRef<'_>, scope: &ComparisonScope) -> bool {
    scope
        .window_name
        .as_deref()
        .is_none_or(|name| candidate.window_name() == name)
        && scope
            .key
            .as_deref()
            .is_none_or(|key| candidate.key() == key)
        && scope
            .partition
            .as_deref()
            .is_none_or(|partition| candidate.partition() == Some(partition))
        && scope.segment_filters.iter().all(|filter| {
            candidate
                .segments()
                .iter()
                .any(|item| item.name == filter.name && item.value == filter.value)
        })
        && scope.tag_filters.iter().all(|filter| {
            candidate
                .tags()
                .iter()
                .any(|item| item.name == filter.name && item.value == filter.value)
        })
}

/// Normalizes one raw history candidate without comparison-side selector policy.
pub(crate) fn normalize_window<'a>(
    candidate: RawWindowRef<'a>,
    request: &WindowNormalizationRequest<'_>,
) -> Result<Option<NormalizedWindowEvidence<'a>>, WindowNormalizationFailure> {
    let known_at_point = candidate.known_at_point().unwrap_or_else(|| {
        candidate
            .end_point()
            .unwrap_or_else(|| candidate.start_point())
    });
    if let Some(known_at) = request.known_at
        && !matches!(
            known_at_point.try_cmp(known_at),
            Ok(Ordering::Less | Ordering::Equal)
        )
    {
        return Err(WindowNormalizationFailure::FutureWindowExcluded {
            available_at: known_at_point,
            known_at: known_at.clone(),
        });
    }

    if !matches_scope(candidate, request.scope) {
        return Ok(None);
    }

    let start_point = candidate.start_point();
    if start_point.axis() != request.time_axis {
        if request.time_axis == TemporalAxis::Timestamp {
            return Err(WindowNormalizationFailure::MissingTimestamp {
                actual: start_point.axis(),
                policy: request.null_timestamp_policy,
            });
        }
        return Err(WindowNormalizationFailure::TemporalAxisMismatch {
            expected: request.time_axis,
            actual: start_point.axis(),
        });
    }

    let (end_point, is_provisional) = match candidate.end_point() {
        Some(end) => (end, false),
        None => match (
            request.require_closed,
            request.open_window_policy,
            request.evaluation_horizon,
        ) {
            (true, _, _) | (false, OpenWindowPolicy::RequireClosed, _) => {
                return Err(WindowNormalizationFailure::OpenWindowWithoutPolicy);
            }
            (false, OpenWindowPolicy::ClipToHorizon, Some(horizon))
                if horizon.is_compatible_with(&start_point)
                    && matches!(
                        horizon.try_cmp(&start_point),
                        Ok(Ordering::Greater | Ordering::Equal)
                    ) =>
            {
                (horizon.clone(), true)
            }
            (false, OpenWindowPolicy::ClipToHorizon, Some(horizon)) => {
                return Err(WindowNormalizationFailure::InvalidRangeDuration {
                    start: start_point,
                    horizon: horizon.clone(),
                });
            }
            (false, OpenWindowPolicy::ClipToHorizon, None) => {
                return Err(WindowNormalizationFailure::OpenWindowWithoutPolicy);
            }
        },
    };

    let range = TemporalRange::new(start_point, end_point)
        .map_err(|error| WindowNormalizationFailure::InvalidTemporalRange { error })?;

    Ok(Some(NormalizedWindowEvidence {
        candidate,
        range,
        is_provisional,
    }))
}
