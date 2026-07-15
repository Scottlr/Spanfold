namespace Spanfold.Episodes;

/// <summary>
/// Preserves one normalized source window inside an episode.
/// </summary>
/// <param name="Window">The source recorded window.</param>
/// <param name="Range">The normalized effective range.</param>
/// <param name="Finality">Whether the normalized fragment is final.</param>
public sealed record EpisodeFragment(
    WindowRecord Window,
    TemporalRange Range,
    ComparisonFinality Finality)
{
    /// <summary>Gets the deterministic source-window identifier.</summary>
    public WindowRecordId RecordId => Window.Id;
}
