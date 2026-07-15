namespace Spanfold.Comparison;

/// <summary>
/// Structured data describing how a serializable selector can be reconstructed.
/// </summary>
public sealed record ComparisonSelectorDescriptor
{
    internal ComparisonSelectorDescriptor(
        string kind,
        object? value = null,
        IReadOnlyList<object>? values = null,
        long? startPosition = null,
        long? endPosition = null,
        DateTimeOffset? startTime = null,
        DateTimeOffset? endTime = null,
        string? clock = null,
        string? activity = null,
        int? count = null,
        IReadOnlyList<ComparisonSelectorDescriptor>? children = null)
    {
        Kind = kind;
        Value = value;
        Values = values is null ? [] : Array.AsReadOnly(values.ToArray());
        StartPosition = startPosition;
        EndPosition = endPosition;
        StartTime = startTime;
        EndTime = endTime;
        Clock = clock;
        Activity = activity;
        Count = count;
        Children = children is null ? [] : Array.AsReadOnly(children.ToArray());
    }

    /// <summary>Gets the selector operation kind.</summary>
    public string Kind { get; }
    /// <summary>Gets the single operand, when applicable.</summary>
    public object? Value { get; }
    /// <summary>Gets multiple operands, when applicable.</summary>
    public IReadOnlyList<object> Values { get; }
    /// <summary>Gets the inclusive processing-position range start.</summary>
    public long? StartPosition { get; }
    /// <summary>Gets the exclusive processing-position range end.</summary>
    public long? EndPosition { get; }
    /// <summary>Gets the inclusive timestamp range start.</summary>
    public DateTimeOffset? StartTime { get; }
    /// <summary>Gets the exclusive timestamp range end.</summary>
    public DateTimeOffset? EndTime { get; }
    /// <summary>Gets the timestamp clock identity, when applicable.</summary>
    public string? Clock { get; }
    /// <summary>Gets the cohort activity rule, when applicable.</summary>
    public string? Activity { get; }
    /// <summary>Gets the cohort count, when applicable.</summary>
    public int? Count { get; }
    /// <summary>Gets child descriptors for composite selectors.</summary>
    public IReadOnlyList<ComparisonSelectorDescriptor> Children { get; }
}
