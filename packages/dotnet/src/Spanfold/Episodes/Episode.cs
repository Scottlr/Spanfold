namespace Spanfold.Episodes;

/// <summary>
/// Represents one stitched occurrence while preserving its source fragments.
/// </summary>
public sealed record Episode
{
    internal Episode(
        EpisodeId id,
        string windowName,
        object key,
        object? source,
        object? partition,
        TemporalRange envelope,
        IReadOnlyList<EpisodeFragment> fragments,
        ComparisonFinality finality,
        long activeMagnitude,
        long elapsedMagnitude,
        long internalGapMagnitude)
    {
        Id = id;
        WindowName = windowName;
        Key = key;
        Source = source;
        Partition = partition;
        Envelope = envelope;
        Fragments = Array.AsReadOnly(fragments.ToArray());
        Finality = finality;
        ActiveMagnitude = activeMagnitude;
        ElapsedMagnitude = elapsedMagnitude;
        InternalGapMagnitude = internalGapMagnitude;
    }

    /// <summary>Gets the deterministic episode identifier.</summary>
    public EpisodeId Id { get; }

    /// <summary>Gets the configured window family.</summary>
    public string WindowName { get; }

    /// <summary>Gets the representative logical key.</summary>
    public object Key { get; }

    /// <summary>Gets the source identity shared by the fragments.</summary>
    public object? Source { get; }

    /// <summary>Gets the partition identity shared by the fragments.</summary>
    public object? Partition { get; }

    /// <summary>Gets the temporal axis used by the episode.</summary>
    public TemporalAxis TimeAxis => Envelope.Axis;

    /// <summary>Gets the elapsed extent from the first start to the last end.</summary>
    public TemporalRange Envelope { get; }

    /// <summary>Gets the ordered normalized source fragments.</summary>
    public IReadOnlyList<EpisodeFragment> Fragments { get; }

    /// <summary>Gets whether the episode can still change at its evaluation horizon.</summary>
    public ComparisonFinality Finality { get; }

    /// <summary>Gets the union magnitude of active fragment ranges.</summary>
    public long ActiveMagnitude { get; }

    /// <summary>Gets the episode-envelope magnitude.</summary>
    public long ElapsedMagnitude { get; }

    /// <summary>Gets the elapsed magnitude not covered by active fragments.</summary>
    public long InternalGapMagnitude { get; }
}
