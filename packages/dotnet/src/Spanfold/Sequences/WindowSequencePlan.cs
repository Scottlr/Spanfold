namespace Spanfold.Sequences;

/// <summary>
/// Describes a bounded ordered sequence over named window families.
/// </summary>
public sealed class WindowSequencePlan
{
    internal WindowSequencePlan(string name, IReadOnlyList<string> steps, long? maximumGap)
    {
        Name = name;
        Steps = Array.AsReadOnly(steps.ToArray());
        MaximumGap = maximumGap;
    }

    /// <summary>Gets the analytical sequence name.</summary>
    public string Name { get; }

    /// <summary>Gets the ordered named window-family steps.</summary>
    public IReadOnlyList<string> Steps { get; }

    /// <summary>Gets the inclusive maximum inactive gap between consecutive steps.</summary>
    public long? MaximumGap { get; }
}
