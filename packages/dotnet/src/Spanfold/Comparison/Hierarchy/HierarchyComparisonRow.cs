namespace Spanfold.Comparison;

/// <summary>
/// Describes one parent/child temporal contribution segment.
/// </summary>
/// <param name="Kind">The hierarchy explanation kind.</param>
/// <param name="Source">The optional source identity shared by the row.</param>
/// <param name="Partition">The optional partition identity shared by the row.</param>
/// <param name="Range">The temporal range of the row.</param>
/// <param name="ParentRecordIds">The active parent record IDs.</param>
/// <param name="ChildRecordIds">The active child contribution record IDs.</param>
public sealed record HierarchyComparisonRow(
    HierarchyComparisonRowKind Kind,
    object? Source,
    object? Partition,
    TemporalRange Range,
    IReadOnlyList<WindowRecordId> ParentRecordIds,
    IReadOnlyList<WindowRecordId> ChildRecordIds)
{
    /// <summary>Gets active parent record IDs.</summary>
    public IReadOnlyList<WindowRecordId> ParentRecordIds { get; } = Array.AsReadOnly(ParentRecordIds.ToArray());

    /// <summary>Gets active child contribution record IDs.</summary>
    public IReadOnlyList<WindowRecordId> ChildRecordIds { get; } = Array.AsReadOnly(ChildRecordIds.ToArray());
}
