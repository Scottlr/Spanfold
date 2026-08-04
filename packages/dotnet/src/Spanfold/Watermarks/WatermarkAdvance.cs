namespace Spanfold.Watermarks;

/// <summary>Contains the result of reporting event-time progress for one lane.</summary>
public sealed class WatermarkAdvance
{
    internal WatermarkAdvance(
        string laneId,
        DateTimeOffset progress,
        DateTimeOffset watermark,
        bool advanced,
        IReadOnlyList<WatermarkDecision> released)
    {
        LaneId = laneId;
        Progress = progress;
        Watermark = watermark;
        Advanced = advanced;
        Released = Array.AsReadOnly(released.ToArray());
    }

    /// <summary>Gets the stable lane identifier.</summary>
    public string LaneId { get; }
    /// <summary>Gets the greatest event-time progress reported for the lane.</summary>
    public DateTimeOffset Progress { get; }
    /// <summary>Gets the monotonic watermark derived from progress and allowed lateness.</summary>
    public DateTimeOffset Watermark { get; }
    /// <summary>Gets whether this report advanced lane progress.</summary>
    public bool Advanced { get; }
    /// <summary>Gets buffered event decisions released by this advancement.</summary>
    public IReadOnlyList<WatermarkDecision> Released { get; }
}
