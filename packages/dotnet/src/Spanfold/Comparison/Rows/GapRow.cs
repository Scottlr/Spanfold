namespace Spanfold;

/// <summary>
/// Describes an uncovered temporal space between observed comparison segments.
/// </summary>
/// <remarks>
/// A gap differs from a residual row: residual means the target is active
/// without comparison coverage, while a gap means neither side is active inside
/// the observed scope envelope.
/// </remarks>
/// <param name="WindowName">The configured window name.</param>
/// <param name="Key">The logical window key.</param>
/// <param name="Partition">The optional partition identity.</param>
/// <param name="Range">The uncovered temporal range.</param>
/// <param name="BoundaryRecordIds">The records bounding this gap.</param>
public sealed record GapRow(
    string WindowName,
    object Key,
    object? Partition,
    TemporalRange Range,
    IReadOnlyList<WindowRecordId>? BoundaryRecordIds = null)
{
    /// <summary>
    /// Gets the records bounding this gap and determining its live finality.
    /// </summary>
    public IReadOnlyList<WindowRecordId> BoundaryRecordIds { get; } = BoundaryRecordIds?.ToArray() ?? [];
}
