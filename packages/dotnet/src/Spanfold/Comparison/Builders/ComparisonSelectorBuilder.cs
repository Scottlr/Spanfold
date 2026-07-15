namespace Spanfold.Comparison;

/// <summary>
/// Builds selectors for a comparison plan.
/// </summary>
public sealed class ComparisonSelectorBuilder
{
    /// <summary>
    /// Selects windows by configured window name.
    /// </summary>
    /// <param name="windowName">The configured window name.</param>
    /// <returns>A window-name selector.</returns>
    public ComparisonSelector WindowName(string windowName)
    {
        return ComparisonSelector.ForWindowName(windowName);
    }

    /// <summary>
    /// Selects windows by key.
    /// </summary>
    /// <param name="key">The window key.</param>
    /// <returns>A key selector.</returns>
    public ComparisonSelector Key(object key)
    {
        return ComparisonSelector.ForKey(key);
    }

    /// <summary>
    /// Selects windows by source identity.
    /// </summary>
    /// <param name="source">The source identity.</param>
    /// <returns>A source selector.</returns>
    public ComparisonSelector Source(object source)
    {
        return ComparisonSelector.ForSource(source);
    }

    /// <summary>
    /// Selects windows from any of several source identities.
    /// </summary>
    /// <param name="sources">The source identities.</param>
    /// <returns>A multi-source selector.</returns>
    public ComparisonSelector Sources(params object[] sources)
    {
        return ComparisonSelector.ForSources(sources);
    }

    /// <summary>
    /// Selects windows by partition identity.
    /// </summary>
    /// <param name="partition">The partition identity.</param>
    /// <returns>A partition selector.</returns>
    public ComparisonSelector Partition(object partition)
    {
        return ComparisonSelector.ForPartition(partition);
    }

    /// <summary>
    /// Selects windows by start processing position.
    /// </summary>
    /// <param name="startInclusive">The inclusive start position.</param>
    /// <param name="endExclusive">The optional exclusive end position.</param>
    /// <returns>A processing-position range selector.</returns>
    public ComparisonSelector PositionRange(long startInclusive, long? endExclusive = null)
    {
        return ComparisonSelector.ForPositionRange(startInclusive, endExclusive);
    }

    /// <summary>
    /// Selects windows by start timestamp.
    /// </summary>
    /// <param name="startInclusive">The inclusive start timestamp.</param>
    /// <param name="endExclusive">The optional exclusive end timestamp.</param>
    /// <param name="clock">Optional identity of the timestamp clock.</param>
    /// <returns>A timestamp range selector.</returns>
    public ComparisonSelector TimeRange(
        DateTimeOffset startInclusive,
        DateTimeOffset? endExclusive = null,
        string? clock = null)
    {
        return ComparisonSelector.ForTimeRange(startInclusive, endExclusive, clock);
    }

    /// <summary>
    /// Selects windows with a runtime-only predicate.
    /// </summary>
    /// <param name="name">The selector name.</param>
    /// <param name="description">A readable selector description.</param>
    /// <param name="predicate">The runtime predicate.</param>
    /// <returns>A runtime-only selector.</returns>
    public ComparisonSelector Runtime(
        string name,
        string description,
        Func<WindowRecord, bool> predicate)
    {
        return ComparisonSelector.RuntimeOnly(name, description, predicate);
    }
}
