namespace Spanfold.Episodes;

/// <summary>
/// Identifies an episode deterministically for the same normalized .NET input.
/// </summary>
/// <param name="Value">The lowercase SHA-256 identity value.</param>
public readonly record struct EpisodeId(string Value)
{
    /// <summary>Returns the identity value.</summary>
    /// <returns>The identity value.</returns>
    public override string ToString()
    {
        return Value;
    }
}
