namespace Spanfold.Comparison;

/// <summary>
/// Builds a comparison cohort declaration.
/// </summary>
public sealed class ComparisonCohortBuilder
{
    private readonly List<object> sources;
    private CohortActivity activity;

    /// <summary>
    /// Creates a cohort builder.
    /// </summary>
    public ComparisonCohortBuilder()
    {
        this.sources = [];
        this.activity = CohortActivity.Any();
    }

    /// <summary>
    /// Adds source identities to the cohort.
    /// </summary>
    /// <param name="sources">The source identities.</param>
    /// <returns>The current builder.</returns>
    public ComparisonCohortBuilder Sources(params object[] sources)
    {
        ArgumentNullException.ThrowIfNull(sources);

        for (var i = 0; i < sources.Length; i++)
        {
            ArgumentNullException.ThrowIfNull(sources[i]);
            this.sources.Add(sources[i]);
        }

        return this;
    }

    /// <summary>
    /// Sets the cohort activity rule.
    /// </summary>
    /// <param name="activity">The cohort activity rule.</param>
    /// <returns>The current builder.</returns>
    public ComparisonCohortBuilder Activity(CohortActivity activity)
    {
        ArgumentNullException.ThrowIfNull(activity);

        this.activity = activity;
        return this;
    }

    internal ComparisonSelector Build(string name)
    {
        if (this.sources.Count == 0)
        {
            throw new InvalidOperationException("Cohort must declare at least one source.");
        }

        var uniqueSources = new HashSet<object>();
        for (var i = 0; i < this.sources.Count; i++)
        {
            if (!uniqueSources.Add(this.sources[i]))
            {
                throw new InvalidOperationException("Cohort sources must be unique.");
            }
        }

        if (this.activity.Count.HasValue && this.activity.Count.Value > this.sources.Count)
        {
            throw new InvalidOperationException("Cohort activity count cannot exceed the number of declared sources.");
        }

        return ComparisonSelector.ForCohortSources(this.sources, this.activity).WithName(name);
    }
}
