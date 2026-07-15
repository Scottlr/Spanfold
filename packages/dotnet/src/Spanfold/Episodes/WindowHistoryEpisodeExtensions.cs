using Spanfold.Internal.Episodes;

namespace Spanfold.Episodes;

/// <summary>
/// Starts episode-formation workflows over recorded window history.
/// </summary>
public static class WindowHistoryEpisodeExtensions
{
    /// <summary>
    /// Starts a staged episode-formation workflow.
    /// </summary>
    /// <param name="history">The recorded window history.</param>
    /// <param name="name">The analytical name for the episode set.</param>
    /// <returns>An episode-formation builder.</returns>
    public static EpisodeFormationBuilder FormEpisodes(this WindowHistory history, string name)
    {
        ArgumentNullException.ThrowIfNull(history);
        ArgumentException.ThrowIfNullOrWhiteSpace(name);

        return new EpisodeFormationBuilder(history, name);
    }
}
