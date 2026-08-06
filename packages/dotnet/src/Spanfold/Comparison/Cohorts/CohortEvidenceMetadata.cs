namespace Spanfold.Comparison;

/// <summary>
/// Describes evidence for one cohort-aligned segment.
/// </summary>
/// <remarks>
/// Cohort evidence explains why a cohort selector was considered active or
/// inactive over a segment. Results carry this typed representation directly;
/// string extension metadata remains available as a compatibility projection.
/// </remarks>
public sealed record CohortEvidenceMetadata
{
    private readonly string? rawValue;

    /// <summary>
    /// Creates cohort evidence metadata.
    /// </summary>
    /// <param name="segmentIndex">The aligned segment index that emitted the evidence.</param>
    /// <param name="rule">The cohort activity rule name.</param>
    /// <param name="requiredCount">The number of active members required by the rule.</param>
    /// <param name="activeCount">The number of active members observed on the segment.</param>
    /// <param name="isActive">Whether the cohort was active on the segment.</param>
    /// <param name="activeSources">The active source identities represented as stable strings.</param>
    public CohortEvidenceMetadata(
        int segmentIndex,
        string rule,
        int requiredCount,
        int activeCount,
        bool isActive,
        IEnumerable<string> activeSources)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(rule);
        ArgumentNullException.ThrowIfNull(activeSources);

        SegmentIndex = segmentIndex;
        Rule = rule;
        RequiredCount = requiredCount;
        ActiveCount = activeCount;
        IsActive = isActive;
        ActiveSources = activeSources.ToArray();
    }

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
        : this(segmentIndex, rule, requiredCount, activeCount, isActive, activeSources)
    {
        ArgumentNullException.ThrowIfNull(rawValue);
        this.rawValue = rawValue;
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
    public string RawValue => this.rawValue ?? CohortEvidenceMetadataCompatibilityProjection.SerializeValue(this);
}
