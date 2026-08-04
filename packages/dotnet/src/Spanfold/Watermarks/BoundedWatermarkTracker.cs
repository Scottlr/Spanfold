namespace Spanfold.Watermarks;

/// <summary>Coordinates caller-driven event-time progress and bounded late corrections per lane.</summary>
/// <remarks>
/// This single-writer in-memory primitive does not infer distributed source completeness,
/// advance idle lanes, persist state, or schedule progress. Accepted revision identities
/// remain correction-eligible for one additional allowed-lateness interval behind the
/// watermark. Callers own lane lifecycle and should remove lanes they no longer need.
/// </remarks>
public sealed class BoundedWatermarkTracker
{
    private readonly TimeSpan allowedLateness;
    private readonly Dictionary<string, LaneState> lanes = new(StringComparer.Ordinal);

    /// <summary>Creates a tracker with a nonnegative bounded lateness interval.</summary>
    /// <param name="allowedLateness">The interval subtracted from progress to derive each lane watermark.</param>
    public BoundedWatermarkTracker(TimeSpan allowedLateness)
    {
        if (allowedLateness < TimeSpan.Zero)
        {
            throw new ArgumentOutOfRangeException(
                nameof(allowedLateness),
                allowedLateness,
                "Allowed lateness must be nonnegative.");
        }

        this.allowedLateness = allowedLateness;
    }

    /// <summary>Gets the configured allowed-lateness interval.</summary>
    public TimeSpan AllowedLateness => this.allowedLateness;

    /// <summary>Reports monotonic event-time progress and releases eligible buffered events.</summary>
    public WatermarkAdvance AdvanceLane(string laneId, DateTimeOffset eventTimeProgress)
    {
        ValidateId(laneId, nameof(laneId));

        var lane = GetOrCreateLane(laneId);
        var normalizedProgress = eventTimeProgress.ToUniversalTime();
        if (lane.Progress.HasValue && normalizedProgress <= lane.Progress.Value)
        {
            return new WatermarkAdvance(
                laneId,
                lane.Progress.Value,
                lane.Watermark!.Value,
                advanced: false,
                released: []);
        }

        lane.Progress = normalizedProgress;
        lane.Watermark = SubtractOrMinimum(normalizedProgress, this.allowedLateness);

        var released = ReleaseBuffered(laneId, lane);
        EvictExpiredRevisions(lane);

        return new WatermarkAdvance(
            laneId,
            lane.Progress.Value,
            lane.Watermark.Value,
            advanced: true,
            released);
    }

    /// <summary>Evaluates one stable event revision against its lane's event-time progress.</summary>
    public WatermarkDecision Observe(
        string laneId,
        string eventId,
        string revisionId,
        DateTimeOffset eventTime)
    {
        ValidateId(laneId, nameof(laneId));
        ValidateId(eventId, nameof(eventId));
        ValidateId(revisionId, nameof(revisionId));

        var lane = GetOrCreateLane(laneId);
        var pending = new PendingRevision(eventId, revisionId, eventTime.ToUniversalTime());

        if (!lane.Progress.HasValue || pending.EventTime > lane.Progress.Value)
        {
            lane.Buffered.Add(pending);
            return CreateDecision(laneId, pending, WatermarkDecisionKind.Buffered, lane.Watermark);
        }

        return DecideAtOrBehindProgress(laneId, lane, pending);
    }

    /// <summary>Removes all progress, buffered events, and retained revisions for a lane.</summary>
    /// <returns><see langword="true" /> when the lane existed.</returns>
    public bool RemoveLane(string laneId)
    {
        ValidateId(laneId, nameof(laneId));
        return this.lanes.Remove(laneId);
    }

    private static void ValidateId(string value, string parameterName)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(value, parameterName);
    }

    private LaneState GetOrCreateLane(string laneId)
    {
        if (!this.lanes.TryGetValue(laneId, out var lane))
        {
            lane = new LaneState();
            this.lanes.Add(laneId, lane);
        }

        return lane;
    }

    private IReadOnlyList<WatermarkDecision> ReleaseBuffered(string laneId, LaneState lane)
    {
        var eligible = lane.Buffered
            .Where(pending => pending.EventTime <= lane.Progress!.Value)
            .OrderBy(static pending => pending.EventTime)
            .ThenBy(static pending => pending.EventId, StringComparer.Ordinal)
            .ThenBy(static pending => pending.RevisionId, StringComparer.Ordinal)
            .ToArray();

        if (eligible.Length == 0)
        {
            return [];
        }

        var eligibleSet = eligible.ToHashSet();
        lane.Buffered.RemoveAll(eligibleSet.Contains);

        var released = new WatermarkDecision[eligible.Length];
        for (var index = 0; index < eligible.Length; index++)
        {
            released[index] = DecideAtOrBehindProgress(laneId, lane, eligible[index]);
        }

        return released;
    }

    private void EvictExpiredRevisions(LaneState lane)
    {
        var correctionCutoff = SubtractOrMinimum(lane.Watermark!.Value, this.allowedLateness);
        var expiredEventIds = lane.Accepted
            .Where(entry => entry.Value.EventTime < correctionCutoff)
            .Select(static entry => entry.Key)
            .ToArray();

        foreach (var eventId in expiredEventIds)
        {
            lane.Accepted.Remove(eventId);
        }
    }

    private WatermarkDecision DecideAtOrBehindProgress(
        string laneId,
        LaneState lane,
        PendingRevision pending)
    {
        if (pending.EventTime >= lane.Watermark!.Value)
        {
            return Accept(laneId, lane, pending);
        }

        var correctionCutoff = SubtractOrMinimum(lane.Watermark.Value, this.allowedLateness);
        lane.Accepted.TryGetValue(pending.EventId, out var accepted);
        var canCorrect = pending.EventTime >= correctionCutoff && accepted is not null;

        if (!canCorrect)
        {
            return CreateDecision(laneId, pending, WatermarkDecisionKind.Rejected, lane.Watermark);
        }

        return Accept(laneId, lane, pending, accepted);
    }

    private static WatermarkDecision Accept(
        string laneId,
        LaneState lane,
        PendingRevision pending,
        AcceptedRevision? accepted = null)
    {
        accepted ??= lane.Accepted.GetValueOrDefault(pending.EventId);
        if (accepted?.RevisionId == pending.RevisionId)
        {
            return CreateDecision(
                laneId,
                pending,
                WatermarkDecisionKind.Accepted,
                lane.Watermark);
        }

        if (accepted is not null && accepted.RevisionId != pending.RevisionId)
        {
            var replacement = CreateReference(laneId, pending);
            var retraction = new WatermarkRevisionReference(laneId, pending.EventId, accepted.RevisionId);
            lane.Accepted[pending.EventId] = new AcceptedRevision(pending.RevisionId, pending.EventTime);

            return new WatermarkDecision(
                replacement,
                pending.EventTime,
                WatermarkDecisionKind.Corrected,
                lane.Watermark,
                new WatermarkCorrection(replacement, retraction));
        }

        lane.Accepted[pending.EventId] = new AcceptedRevision(
            pending.RevisionId,
            pending.EventTime);
        return CreateDecision(laneId, pending, WatermarkDecisionKind.Accepted, lane.Watermark);
    }

    private static WatermarkDecision CreateDecision(
        string laneId,
        PendingRevision pending,
        WatermarkDecisionKind kind,
        DateTimeOffset? watermark)
    {
        return new WatermarkDecision(CreateReference(laneId, pending), pending.EventTime, kind, watermark);
    }

    private static WatermarkRevisionReference CreateReference(string laneId, PendingRevision pending)
    {
        return new WatermarkRevisionReference(laneId, pending.EventId, pending.RevisionId);
    }

    private static DateTimeOffset SubtractOrMinimum(DateTimeOffset instant, TimeSpan duration)
    {
        return instant.UtcTicks < duration.Ticks ? DateTimeOffset.MinValue : instant - duration;
    }

    private sealed class LaneState
    {
        public DateTimeOffset? Progress { get; set; }
        public DateTimeOffset? Watermark { get; set; }
        public List<PendingRevision> Buffered { get; } = [];
        public Dictionary<string, AcceptedRevision> Accepted { get; } = new(StringComparer.Ordinal);
    }

    private sealed record PendingRevision(string EventId, string RevisionId, DateTimeOffset EventTime);
    private sealed record AcceptedRevision(string RevisionId, DateTimeOffset EventTime);
}
