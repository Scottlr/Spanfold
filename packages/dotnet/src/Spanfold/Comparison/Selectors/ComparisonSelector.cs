using Spanfold.Internal.Keys;

namespace Spanfold.Comparison;

/// <summary>
/// Describes a selection used by a window comparison plan.
/// </summary>
/// <remarks>
/// Prefer descriptor selectors such as <see cref="ForSource" /> and
/// <see cref="ForWindowName" /> for persistent plans. Runtime-only selectors
/// are useful locally, but cannot be exported as plan data.
/// </remarks>
public readonly record struct ComparisonSelector
{
    private readonly Func<WindowRecord, IEqualityComparer<object>, bool>? predicate;

    private ComparisonSelector(
        string name,
        string description,
        bool isSerializable,
        Func<WindowRecord, IEqualityComparer<object>, bool>? predicate,
        CohortActivity? cohortActivity = null,
        IReadOnlyList<object>? cohortSources = null,
        ComparisonSelectorDescriptor? descriptor = null)
    {
        Name = name;
        Description = description;
        IsSerializable = isSerializable;
        this.predicate = predicate;
        CohortActivity = cohortActivity;
        CohortSources = cohortSources is null
            ? []
            : Array.AsReadOnly(cohortSources.ToArray());
        Descriptor = descriptor;
    }

    /// <summary>
    /// Gets the selector name used in output and diagnostics.
    /// </summary>
    public string Name { get; }

    /// <summary>
    /// Gets a readable description of the selector.
    /// </summary>
    public string Description { get; }

    /// <summary>
    /// Gets whether the selector can be exported as plan data.
    /// </summary>
    public bool IsSerializable { get; }

    /// <summary>
    /// Gets the cohort activity rule when this selector represents a cohort.
    /// </summary>
    public CohortActivity? CohortActivity { get; }

    /// <summary>
    /// Gets the source identities that belong to this cohort selector.
    /// </summary>
    public IReadOnlyList<object> CohortSources { get; }

    /// <summary>Gets the structured executable descriptor, when available.</summary>
    public ComparisonSelectorDescriptor? Descriptor { get; }

    internal bool IsDefined => !string.IsNullOrWhiteSpace(Name)
        && !string.IsNullOrWhiteSpace(Description)
        && this.predicate is not null;

    internal object? RuntimeIdentity => IsSerializable ? null : this.predicate;

    /// <summary>
    /// Creates a selector for a configured window name.
    /// </summary>
    /// <param name="windowName">The configured window name.</param>
    /// <returns>A serializable window-name selector.</returns>
    public static ComparisonSelector ForWindowName(string windowName)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(windowName);

        return new ComparisonSelector(
            $"window:{windowName}",
            $"window name = {windowName}",
            isSerializable: true,
            (window, _) => string.Equals(window.WindowName, windowName, StringComparison.Ordinal),
            descriptor: new ComparisonSelectorDescriptor("windowName", value: windowName));
    }

    /// <summary>
    /// Creates a selector for a recorded window key.
    /// </summary>
    /// <param name="key">The recorded window key.</param>
    /// <returns>A serializable key selector.</returns>
    public static ComparisonSelector ForKey(object key)
    {
        ArgumentNullException.ThrowIfNull(key);
        EnsurePortableValue(key);

        return new ComparisonSelector(
            $"key:{key}",
            $"key = {key}",
            isSerializable: true,
            (window, keyComparer) => keyComparer.Equals(window.Key, key),
            descriptor: new ComparisonSelectorDescriptor("key", value: key));
    }

    /// <summary>
    /// Creates a selector for a source identity.
    /// </summary>
    /// <param name="source">The source identity.</param>
    /// <returns>A serializable source selector.</returns>
    public static ComparisonSelector ForSource(object source)
    {
        ArgumentNullException.ThrowIfNull(source);
        EnsurePortableValue(source);

        return new ComparisonSelector(
            $"source:{source}",
            $"source = {source}",
            isSerializable: true,
            (window, _) => EqualityComparer<object?>.Default.Equals(window.Source, source),
            descriptor: new ComparisonSelectorDescriptor("source", value: source));
    }

    /// <summary>
    /// Creates a selector for any of several source identities.
    /// </summary>
    /// <param name="sources">The source identities.</param>
    /// <returns>A serializable multi-source selector.</returns>
    public static ComparisonSelector ForSources(IEnumerable<object> sources)
    {
        return ForSourcesCore(sources, cohortActivity: null);
    }

    /// <summary>
    /// Creates a selector for a cohort of source identities.
    /// </summary>
    /// <param name="sources">The cohort source identities.</param>
    /// <param name="activity">The cohort activity rule.</param>
    /// <returns>A serializable cohort selector.</returns>
    public static ComparisonSelector ForCohortSources(
        IEnumerable<object> sources,
        CohortActivity activity)
    {
        ArgumentNullException.ThrowIfNull(activity);

        return ForSourcesCore(sources, activity);
    }

    private static ComparisonSelector ForSourcesCore(
        IEnumerable<object> sources,
        CohortActivity? cohortActivity)
    {
        ArgumentNullException.ThrowIfNull(sources);

        var orderedSources = sources.ToArray();
        if (orderedSources.Length == 0)
        {
            throw new ArgumentException("At least one source is required.", nameof(sources));
        }

        for (var i = 0; i < orderedSources.Length; i++)
        {
            ArgumentNullException.ThrowIfNull(orderedSources[i]);
            EnsurePortableValue(orderedSources[i]);
            for (var j = 0; j < i; j++)
            {
                if (EqualityComparer<object>.Default.Equals(orderedSources[i], orderedSources[j]))
                {
                    throw new ArgumentException("Cohort and multi-source selectors require unique source identities.", nameof(sources));
                }
            }
        }

        if (cohortActivity?.Count is { } count && count > orderedSources.Length)
        {
            throw new ArgumentOutOfRangeException(
                nameof(cohortActivity),
                count,
                "Cohort activity count cannot exceed the number of unique sources.");
        }

        return new ComparisonSelector(
            "sources:" + string.Join(",", orderedSources.Select(static source => source.ToString())),
            "source in [" + string.Join(", ", orderedSources.Select(static source => source.ToString())) + "]",
            isSerializable: true,
            (window, _) =>
            {
                for (var i = 0; i < orderedSources.Length; i++)
                {
                    if (EqualityComparer<object?>.Default.Equals(window.Source, orderedSources[i]))
                    {
                        return true;
                    }
                }

                return false;
            },
            cohortActivity,
            orderedSources,
            new ComparisonSelectorDescriptor(
                cohortActivity is null ? "sources" : "cohort",
                values: orderedSources,
                activity: cohortActivity?.Name,
                count: cohortActivity?.Count));
    }

    /// <summary>
    /// Creates a selector for a partition identity.
    /// </summary>
    /// <param name="partition">The partition identity.</param>
    /// <returns>A serializable partition selector.</returns>
    public static ComparisonSelector ForPartition(object partition)
    {
        ArgumentNullException.ThrowIfNull(partition);
        EnsurePortableValue(partition);

        return new ComparisonSelector(
            $"partition:{partition}",
            $"partition = {partition}",
            isSerializable: true,
            (window, _) => EqualityComparer<object?>.Default.Equals(window.Partition, partition),
            descriptor: new ComparisonSelectorDescriptor("partition", value: partition));
    }

    /// <summary>
    /// Creates a selector for windows whose start position is inside a half-open processing-position range.
    /// </summary>
    /// <param name="startInclusive">The inclusive start position.</param>
    /// <param name="endExclusive">The optional exclusive end position.</param>
    /// <returns>A serializable processing-position range selector.</returns>
    public static ComparisonSelector ForPositionRange(long startInclusive, long? endExclusive = null)
    {
        if (endExclusive.HasValue && endExclusive.Value < startInclusive)
        {
            throw new ArgumentException("Position range end cannot be earlier than the start.", nameof(endExclusive));
        }

        return new ComparisonSelector(
            $"position:{startInclusive}..{endExclusive?.ToString() ?? "*"}",
            $"start position in [{startInclusive}, {endExclusive?.ToString() ?? "*"})",
            isSerializable: true,
            (window, _) => window.StartPosition >= startInclusive
                && (!endExclusive.HasValue || window.StartPosition < endExclusive.Value),
            descriptor: new ComparisonSelectorDescriptor("positionRange", startPosition: startInclusive, endPosition: endExclusive));
    }

    /// <summary>
    /// Creates a selector for windows whose start timestamp is inside a half-open timestamp range.
    /// </summary>
    /// <param name="startInclusive">The inclusive start timestamp.</param>
    /// <param name="endExclusive">The optional exclusive end timestamp.</param>
    /// <param name="clock">Optional identity of the timestamp clock.</param>
    /// <returns>A serializable timestamp range selector.</returns>
    public static ComparisonSelector ForTimeRange(
        DateTimeOffset startInclusive,
        DateTimeOffset? endExclusive = null,
        string? clock = null)
    {
        if (endExclusive.HasValue && endExclusive.Value < startInclusive)
        {
            throw new ArgumentException("Time range end cannot be earlier than the start.", nameof(endExclusive));
        }

        if (clock is not null && string.IsNullOrWhiteSpace(clock))
        {
            throw new ArgumentException("Timestamp clock identity cannot be blank.", nameof(clock));
        }

        return new ComparisonSelector(
            $"time:{startInclusive:O}..{endExclusive?.ToString("O") ?? "*"}",
            $"start time in [{startInclusive:O}, {endExclusive?.ToString("O") ?? "*"})",
            isSerializable: true,
            (window, _) => window.StartTime.HasValue
                && (clock is null || string.Equals(window.TimestampClock, clock, StringComparison.Ordinal))
                && window.StartTime.Value >= startInclusive
                && (!endExclusive.HasValue || window.StartTime.Value < endExclusive.Value),
            descriptor: new ComparisonSelectorDescriptor(
                "timeRange",
                startTime: startInclusive,
                endTime: endExclusive,
                clock: clock));
    }

    /// <summary>
    /// Creates a runtime-only selector descriptor.
    /// </summary>
    /// <param name="name">The selector name.</param>
    /// <param name="description">A readable selector description.</param>
    /// <returns>A runtime-only comparison selector.</returns>
    public static ComparisonSelector RuntimeOnly(string name, string description)
    {
        return RuntimeOnly(name, description, static _ => true);
    }

    /// <summary>
    /// Creates a runtime-only selector backed by a delegate.
    /// </summary>
    /// <param name="name">The selector name.</param>
    /// <param name="description">A readable selector description.</param>
    /// <param name="predicate">The runtime predicate.</param>
    /// <returns>A runtime-only comparison selector.</returns>
    public static ComparisonSelector RuntimeOnly(
        string name,
        string description,
        Func<WindowRecord, bool> predicate)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(name);
        ArgumentException.ThrowIfNullOrWhiteSpace(description);
        ArgumentNullException.ThrowIfNull(predicate);

        return new ComparisonSelector(
            name,
            description,
            isSerializable: false,
            (window, _) => predicate(window));
    }

    /// <summary>
    /// Creates a copy of this selector with a different display name.
    /// </summary>
    /// <param name="name">The selector name.</param>
    /// <returns>A selector with the supplied name.</returns>
    public ComparisonSelector WithName(string name)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(name);

        return new ComparisonSelector(
            name,
            Description,
            IsSerializable,
            this.predicate,
            CohortActivity,
            CohortSources,
            Descriptor);
    }

    /// <summary>
    /// Creates a selector that requires both selectors to match.
    /// </summary>
    /// <param name="other">The selector to combine with this selector.</param>
    /// <returns>A combined selector.</returns>
    /// <exception cref="InvalidOperationException">Both selectors carry cohort semantics.</exception>
    public ComparisonSelector And(ComparisonSelector other)
    {
        var current = this;
        var cohort = GetSingleCohort(current, other);

        return new ComparisonSelector(
            $"{Name}&{other.Name}",
            $"({Description}) and ({other.Description})",
            IsSerializable && other.IsSerializable,
            (window, keyComparer) => current.Matches(window, keyComparer)
                && other.Matches(window, keyComparer),
            cohort?.CohortActivity,
            cohort?.CohortSources,
            descriptor: current.Descriptor is { } left && other.Descriptor is { } right
                ? new ComparisonSelectorDescriptor("and", children: [left, right])
                : null);
    }

    /// <summary>
    /// Creates a selector that allows either selector to match.
    /// </summary>
    /// <param name="other">The selector to combine with this selector.</param>
    /// <returns>A combined selector.</returns>
    /// <exception cref="InvalidOperationException">Either selector carries cohort semantics.</exception>
    public ComparisonSelector Or(ComparisonSelector other)
    {
        var current = this;
        if (current.CohortActivity is not null || other.CohortActivity is not null)
        {
            throw new InvalidOperationException("Cohort selectors cannot be combined with logical OR.");
        }

        return new ComparisonSelector(
            $"{Name}|{other.Name}",
            $"({Description}) or ({other.Description})",
            IsSerializable && other.IsSerializable,
            (window, keyComparer) => current.Matches(window, keyComparer)
                || other.Matches(window, keyComparer),
            descriptor: current.Descriptor is { } left && other.Descriptor is { } right
                ? new ComparisonSelectorDescriptor("or", children: [left, right])
                : null);
    }

    /// <summary>
    /// Determines whether the selector matches a recorded window.
    /// </summary>
    /// <param name="window">The recorded window to test.</param>
    /// <returns><see langword="true" /> when the selector matches.</returns>
    public bool Matches(WindowRecord window)
    {
        ArgumentNullException.ThrowIfNull(window);

        return Matches(window, EqualityComparer<object>.Default);
    }

    internal bool Matches(
        WindowRecord window,
        IEqualityComparer<object> keyComparer)
    {
        ArgumentNullException.ThrowIfNull(window);
        ArgumentNullException.ThrowIfNull(keyComparer);

        return this.predicate?.Invoke(window, keyComparer)
            ?? throw new InvalidOperationException("The comparison selector is uninitialized.");
    }

    private static void EnsurePortableValue(object value)
    {
        _ = CanonicalValueFormatter.Format(value);
    }

    private static ComparisonSelector? GetSingleCohort(
        ComparisonSelector left,
        ComparisonSelector right)
    {
        var leftIsCohort = left.CohortActivity is not null;
        var rightIsCohort = right.CohortActivity is not null;
        if (leftIsCohort && rightIsCohort)
        {
            throw new InvalidOperationException("Two cohort selectors cannot be combined.");
        }

        if (leftIsCohort)
        {
            return left;
        }

        return rightIsCohort ? right : null;
    }
}
