namespace Spanfold.Comparison;

/// <summary>
/// Describes a segment value required by a window history query or comparison scope.
/// </summary>
/// <param name="Name">The segment dimension name.</param>
/// <param name="Value">The required segment value.</param>
public sealed record WindowSegmentFilter(
    string Name,
    object? Value);
