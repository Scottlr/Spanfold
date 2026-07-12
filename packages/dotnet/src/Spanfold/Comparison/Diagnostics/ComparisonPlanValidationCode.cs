namespace Spanfold;

/// <summary>
/// Identifies a validation diagnostic produced by a comparison plan.
/// </summary>
public enum ComparisonPlanValidationCode
{
    /// <summary>
    /// Indicates an unspecified validation issue.
    /// </summary>
    Unknown = 0,

    /// <summary>
    /// Indicates that the plan name is missing.
    /// </summary>
    MissingName = 1,

    /// <summary>
    /// Indicates that the target selector is missing.
    /// </summary>
    MissingTarget = 2,

    /// <summary>
    /// Indicates that the plan has no comparison selectors.
    /// </summary>
    MissingAgainst = 3,

    /// <summary>
    /// Indicates that the plan has no comparator declarations.
    /// </summary>
    MissingComparator = 4,

    /// <summary>
    /// Indicates that a selector cannot be exported as a plan descriptor.
    /// </summary>
    NonSerializableSelector = 5,

    /// <summary>
    /// Indicates that the plan has no temporal scope.
    /// </summary>
    MissingScope = 6,

    /// <summary>
    /// Indicates that a plan mixes incompatible temporal axes.
    /// </summary>
    MixedTimeAxes = 7,

    /// <summary>
    /// Indicates that open windows require an explicit normalization policy.
    /// </summary>
    OpenWindowsWithoutPolicy = 8,

    /// <summary>
    /// Indicates that a timestamp comparison encountered a missing event time.
    /// </summary>
    MissingEventTime = 9,

    /// <summary>
    /// Indicates that a range has zero or negative duration.
    /// </summary>
    InvalidRangeDuration = 10,

    /// <summary>
    /// Indicates that a recorded window was clipped by policy.
    /// </summary>
    ClippedWindow = 11,

    /// <summary>
    /// Indicates that a comparator declaration is not registered.
    /// </summary>
    UnknownComparator = 12,

    /// <summary>
    /// Indicates that an as-of lookup had multiple equally eligible matches.
    /// </summary>
    AmbiguousAsOfMatch = 13,

    /// <summary>
    /// Indicates that hierarchy comparison could not find expected parent or child lineage records.
    /// </summary>
    MissingLineage = 14,

    /// <summary>
    /// Indicates that a recorded window was unavailable at the configured known-at point.
    /// </summary>
    FutureWindowExcluded = 15,

    /// <summary>
    /// Indicates that known-at requires processing-position availability information.
    /// </summary>
    KnownAtRequiresProcessingPosition = 16,

    /// <summary>
    /// Indicates that runtime criticism found a non-serializable plan.
    /// </summary>
    RuntimeNonSerializablePlan = 17,

    /// <summary>
    /// Indicates that runtime criticism found a selector or scope that is likely too broad.
    /// </summary>
    BroadSelector = 18,

    /// <summary>
    /// Indicates that runtime criticism found possible future leakage.
    /// </summary>
    FutureLeakageRisk = 19,

    /// <summary>
    /// Indicates that live finality was requested without an evaluation horizon.
    /// </summary>
    LiveFinalityWithoutHorizon = 20,

    /// <summary>
    /// Indicates that runtime criticism found open durations without a bounded policy.
    /// </summary>
    UnboundedOpenDuration = 21,

    /// <summary>
    /// Indicates that runtime criticism found timestamp points from incompatible clocks.
    /// </summary>
    MixedClockRisk = 22,

    /// <summary>
    /// Indicates that hierarchy co-activity includes open windows without a bounded horizon.
    /// </summary>
    HierarchyOpenWindowsWithoutHorizon = 23
}
