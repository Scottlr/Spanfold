namespace Spanfold.Sequences;

/// <summary>
/// Starts ordered sequence analysis over recorded window history.
/// </summary>
public static class WindowHistorySequenceExtensions
{
    /// <summary>
    /// Starts a bounded ordered sequence over named window families.
    /// </summary>
    /// <param name="history">The recorded history to analyse.</param>
    /// <param name="name">The analytical name for the sequence.</param>
    /// <returns>A sequence builder.</returns>
    public static WindowSequenceBuilder MatchSequence(this WindowHistory history, string name)
    {
        ArgumentNullException.ThrowIfNull(history);
        ArgumentException.ThrowIfNullOrWhiteSpace(name);

        return new WindowSequenceBuilder(history, name);
    }
}
