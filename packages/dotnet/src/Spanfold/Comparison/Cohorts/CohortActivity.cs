namespace Spanfold.Comparison;

/// <summary>
/// Describes how a cohort is considered active.
/// </summary>
/// <remarks>
/// Cohort activity rules collapse member windows into one derived comparison
/// lane before comparators run. This avoids pairwise overcounting when asking
/// whether a target was unmatched by the cohort as a whole.
/// </remarks>
public sealed record CohortActivity
{
    private CohortActivity(string name, int? count = null)
    {
        Name = name;
        Count = count;
    }

    /// <summary>
    /// Gets the activity rule name.
    /// </summary>
    public string Name { get; }

    /// <summary>
    /// Gets the configured count for threshold activity rules.
    /// </summary>
    public int? Count { get; }

    /// <summary>
    /// Creates an activity rule where any active member makes the cohort active.
    /// </summary>
    /// <returns>An any-member activity rule.</returns>
    public static CohortActivity Any()
    {
        return new CohortActivity("any");
    }

    /// <summary>
    /// Creates an activity rule where every declared member must be active.
    /// </summary>
    /// <returns>An all-member activity rule.</returns>
    public static CohortActivity All()
    {
        return new CohortActivity("all");
    }

    /// <summary>
    /// Creates an activity rule where no declared member may be active.
    /// </summary>
    /// <returns>A no-active-members activity rule.</returns>
    public static CohortActivity None()
    {
        return new CohortActivity("none");
    }

    /// <summary>
    /// Creates an activity rule where at least a configured number of members must be active.
    /// </summary>
    /// <param name="count">The required number of active members.</param>
    /// <returns>An at-least activity rule.</returns>
    /// <exception cref="ArgumentOutOfRangeException">Thrown when <paramref name="count" /> is less than one.</exception>
    public static CohortActivity AtLeast(int count)
    {
        if (count < 1)
        {
            throw new ArgumentOutOfRangeException(nameof(count), count, "At-least cohort count must be greater than zero.");
        }

        return new CohortActivity("at-least", count);
    }

    /// <summary>
    /// Creates an activity rule where no more than a configured number of members may be active.
    /// </summary>
    /// <param name="count">The maximum number of active members.</param>
    /// <returns>An at-most activity rule.</returns>
    /// <exception cref="ArgumentOutOfRangeException">Thrown when <paramref name="count" /> is less than zero.</exception>
    public static CohortActivity AtMost(int count)
    {
        if (count < 0)
        {
            throw new ArgumentOutOfRangeException(nameof(count), count, "At-most cohort count cannot be negative.");
        }

        return new CohortActivity("at-most", count);
    }

    /// <summary>
    /// Creates an activity rule where exactly a configured number of members must be active.
    /// </summary>
    /// <param name="count">The required number of active members.</param>
    /// <returns>An exact-count activity rule.</returns>
    /// <exception cref="ArgumentOutOfRangeException">Thrown when <paramref name="count" /> is less than zero.</exception>
    public static CohortActivity Exactly(int count)
    {
        if (count < 0)
        {
            throw new ArgumentOutOfRangeException(nameof(count), count, "Exact cohort count cannot be negative.");
        }

        return new CohortActivity("exactly", count);
    }

    internal int RequiredActiveCount(int memberCount)
    {
        return Name switch
        {
            "any" => 1,
            "all" => memberCount,
            "none" => 0,
            "at-least" => Count!.Value,
            "at-most" => Count!.Value,
            "exactly" => Count!.Value,
            _ => throw new InvalidOperationException("Unknown cohort activity rule.")
        };
    }

    internal bool IsActive(int activeCount, int memberCount)
    {
        return Name switch
        {
            "any" => activeCount >= 1,
            "all" => activeCount == memberCount,
            "none" => activeCount == 0,
            "at-least" => activeCount >= Count!.Value,
            "at-most" => activeCount <= Count!.Value,
            "exactly" => activeCount == Count!.Value,
            _ => throw new InvalidOperationException("Unknown cohort activity rule.")
        };
    }
}
