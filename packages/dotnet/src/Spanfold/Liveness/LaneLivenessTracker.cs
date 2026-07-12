namespace Spanfold;

/// <summary>
/// Emits deterministic lane liveness state changes from explicit observations and horizon checks.
/// </summary>
/// <remarks>
/// The tracker does not own timers, scheduling, persistence, or distributed
/// heartbeat monitoring. Consumers call <see cref="Observe" /> when a lane
/// reports and <see cref="Check" /> at explicit horizons. Returned
/// <see cref="LaneLivenessSignal" /> values can be ingested into a normal Spanfold
/// pipeline to record silence windows.
/// </remarks>
public sealed class LaneLivenessTracker
{
    private readonly Dictionary<object, LaneState> lanes;
    private readonly TimeSpan silenceThreshold;
    private readonly DateTimeOffset startedAt;
    private DateTimeOffset lastCheckAt;

    /// <summary>
    /// Creates a lane liveness tracker.
    /// </summary>
    /// <param name="lanes">The known lanes to track.</param>
    /// <param name="startedAt">The timestamp where liveness tracking starts.</param>
    /// <param name="silenceThreshold">The duration after which a lane becomes silent.</param>
    public LaneLivenessTracker(
        IEnumerable<object> lanes,
        DateTimeOffset startedAt,
        TimeSpan silenceThreshold)
    {
        ArgumentNullException.ThrowIfNull(lanes);

        if (silenceThreshold <= TimeSpan.Zero)
        {
            throw new ArgumentOutOfRangeException(nameof(silenceThreshold), "Silence threshold must be greater than zero.");
        }

        this.startedAt = startedAt;
        this.lastCheckAt = startedAt;
        this.silenceThreshold = silenceThreshold;
        this.lanes = [];

        foreach (var lane in lanes)
        {
            ArgumentNullException.ThrowIfNull(lane);

            if (this.lanes.ContainsKey(lane))
            {
                throw new ArgumentException("Tracked lanes must be unique.", nameof(lanes));
            }

            this.lanes.Add(lane, new LaneState(lane, startedAt));
        }

        if (this.lanes.Count == 0)
        {
            throw new ArgumentException("At least one lane must be tracked.", nameof(lanes));
        }
    }

    /// <summary>
    /// Creates a lane liveness tracker for a fixed set of lanes.
    /// </summary>
    /// <param name="startedAt">The timestamp where liveness tracking starts.</param>
    /// <param name="silenceThreshold">The duration after which a lane becomes silent.</param>
    /// <param name="lanes">The known lanes to track.</param>
    /// <returns>A lane liveness tracker.</returns>
    public static LaneLivenessTracker ForLanes(
        DateTimeOffset startedAt,
        TimeSpan silenceThreshold,
        params object[] lanes)
    {
        return new LaneLivenessTracker(lanes, startedAt, silenceThreshold);
    }

    /// <summary>
    /// Records that a lane reported at a specific timestamp.
    /// </summary>
    /// <param name="lane">The lane that reported.</param>
    /// <param name="observedAt">When the lane reported.</param>
    /// <returns>Liveness state changes produced by the observation.</returns>
    public IReadOnlyList<LaneLivenessSignal> Observe(object lane, DateTimeOffset observedAt)
    {
        var state = GetLane(lane);

        if (observedAt < this.startedAt)
        {
            throw new ArgumentOutOfRangeException(nameof(observedAt), "Observation cannot be earlier than tracker start.");
        }

        if (observedAt < this.lastCheckAt)
        {
            throw new ArgumentOutOfRangeException(nameof(observedAt), "Observation cannot be earlier than the last evaluated horizon.");
        }

        if (state.LastObservedAt.HasValue && observedAt < state.LastObservedAt.Value)
        {
            throw new ArgumentOutOfRangeException(nameof(observedAt), "Observation cannot be earlier than the lane's previous observation.");
        }

        state.LastObservedAt = observedAt;

        if (!state.HasReportedState || state.IsSilent)
        {
            state.HasReportedState = true;
            state.IsSilent = false;
            return
            [
                new LaneLivenessSignal(
                    lane,
                    IsSilent: false,
                    observedAt,
                    observedAt,
                    this.silenceThreshold)
            ];
        }

        return [];
    }

    /// <summary>
    /// Evaluates all tracked lanes at an explicit horizon.
    /// </summary>
    /// <param name="horizon">The timestamp where liveness is evaluated.</param>
    /// <returns>Liveness state changes produced by the horizon check.</returns>
    public IReadOnlyList<LaneLivenessSignal> Check(DateTimeOffset horizon)
    {
        if (horizon < this.startedAt)
        {
            throw new ArgumentOutOfRangeException(nameof(horizon), "Liveness horizon cannot be earlier than tracker start.");
        }

        if (horizon < this.lastCheckAt)
        {
            throw new ArgumentOutOfRangeException(nameof(horizon), "Liveness horizon cannot move backwards.");
        }

        this.lastCheckAt = horizon;
        var signals = new List<LaneLivenessSignal>();

        foreach (var state in this.lanes.Values)
        {
            var silenceStartedAt = (state.LastObservedAt ?? state.StartedAt) + this.silenceThreshold;
            if (state.IsSilent || horizon < silenceStartedAt)
            {
                continue;
            }

            state.HasReportedState = true;
            state.IsSilent = true;
            signals.Add(new LaneLivenessSignal(
                state.Lane,
                IsSilent: true,
                silenceStartedAt,
                horizon,
                this.silenceThreshold));
        }

        return signals.ToArray();
    }

    private LaneState GetLane(object lane)
    {
        ArgumentNullException.ThrowIfNull(lane);

        if (!this.lanes.TryGetValue(lane, out var state))
        {
            throw new ArgumentException("Lane is not tracked by this liveness tracker.", nameof(lane));
        }

        return state;
    }

    private sealed class LaneState(object lane, DateTimeOffset startedAt)
    {
        public object Lane { get; } = lane;

        public DateTimeOffset StartedAt { get; } = startedAt;

        public DateTimeOffset? LastObservedAt { get; set; }

        public bool HasReportedState { get; set; }

        public bool IsSilent { get; set; }
    }
}
