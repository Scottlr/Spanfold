namespace Spanfold.Comparison;

/// <summary>
/// Describes parsed evidence for one cohort-aligned segment.
/// </summary>
/// <remarks>
/// Cohort evidence explains why a cohort selector was considered active or
/// inactive over a segment. It is derived from comparison extension metadata
/// and is intended for diagnostics, export readers, and debug tooling.
/// </remarks>
public sealed record CohortEvidenceMetadata
{
    /// <summary>
    /// Creates parsed cohort evidence metadata.
    /// </summary>
    /// <param name="segmentIndex">The aligned segment index that emitted the evidence.</param>
    /// <param name="rule">The cohort activity rule name.</param>
    /// <param name="requiredCount">The number of active members required by the rule.</param>
    /// <param name="activeCount">The number of active members observed on the segment.</param>
    /// <param name="isActive">Whether the cohort was active on the segment.</param>
    /// <param name="activeSources">The active source identities represented as stable strings.</param>
    /// <param name="rawValue">The raw extension metadata value.</param>
    public CohortEvidenceMetadata(
        int segmentIndex,
        string rule,
        int requiredCount,
        int activeCount,
        bool isActive,
        IEnumerable<string> activeSources,
        string rawValue)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(rule);
        ArgumentNullException.ThrowIfNull(activeSources);
        ArgumentNullException.ThrowIfNull(rawValue);

        SegmentIndex = segmentIndex;
        Rule = rule;
        RequiredCount = requiredCount;
        ActiveCount = activeCount;
        IsActive = isActive;
        ActiveSources = activeSources.ToArray();
        RawValue = rawValue;
    }

    /// <summary>
    /// Gets the aligned segment index that emitted the evidence.
    /// </summary>
    public int SegmentIndex { get; }

    /// <summary>
    /// Gets the cohort activity rule name.
    /// </summary>
    public string Rule { get; }

    /// <summary>
    /// Gets the number of active members required by the rule.
    /// </summary>
    public int RequiredCount { get; }

    /// <summary>
    /// Gets the number of active members observed on the segment.
    /// </summary>
    public int ActiveCount { get; }

    /// <summary>
    /// Gets whether the cohort was active on the segment.
    /// </summary>
    public bool IsActive { get; }

    /// <summary>
    /// Gets the active source identities represented as stable strings.
    /// </summary>
    public IReadOnlyList<string> ActiveSources { get; }

    /// <summary>
    /// Gets the raw extension metadata value.
    /// </summary>
    public string RawValue { get; }
}
