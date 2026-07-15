namespace Spanfold.Comparison;

/// <summary>
/// Describes a tag value required by a window history query or comparison scope.
/// </summary>
/// <param name="Name">The tag name.</param>
/// <param name="Value">The required tag value.</param>
public sealed record WindowTagFilter(
    string Name,
    object? Value);
