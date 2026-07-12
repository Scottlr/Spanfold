using Spanfold.Internal.Definitions;
using Spanfold.Internal.Keys;

namespace Spanfold.Internal.Runtime;

internal sealed class WindowRuntime<TEvent>
{
    private readonly WindowDefinition<TEvent> definition;
    private readonly Dictionary<RuntimeStateKey, ActiveWindowState> activeKeys;
    private readonly RollUpRuntime<TEvent>[] rollUps;

    public WindowRuntime(WindowDefinition<TEvent> definition)
    {
        this.definition = definition;
        this.activeKeys = new Dictionary<RuntimeStateKey, ActiveWindowState>(
            new RuntimeStateKeyComparer(definition.KeyComparer));
        this.rollUps = new RollUpRuntime<TEvent>[definition.RollUps.Count];

        for (var i = 0; i < this.rollUps.Length; i++)
        {
            this.rollUps[i] = new RollUpRuntime<TEvent>(
                definition.RollUps[i],
                definition.KeyComparer);
        }
    }

    public void Ingest(
        TEvent @event,
        object? source,
        object? partition,
        ref List<WindowEmission<TEvent>>? emissions)
    {
        var key = this.definition.GetKey(@event);
        var isActive = this.definition.IsActive(@event);
        var stateKey = new RuntimeStateKey(key, source, partition);
        var wasActive = this.activeKeys.TryGetValue(stateKey, out var previousState);
        var changed = isActive != wasActive;
        var currentSegments = isActive ? this.definition.GetSegments(@event) : [];
        var currentTags = isActive ? this.definition.GetTags(@event) : [];
        var segmentChanged = isActive
            && wasActive
            && previousState is not null
            && !SegmentsEqual(previousState.Segments, currentSegments);
        var observedRollUps = false;

        if (changed && isActive)
        {
            this.activeKeys[stateKey] = new ActiveWindowState(currentSegments, currentTags);
            AddEmission(
                ref emissions,
                new WindowEmission<TEvent>(
                    this.definition.Name,
                    key,
                    @event,
                    WindowTransitionKind.Opened,
                    source,
                    partition,
                    currentSegments,
                    currentTags));
        }
        else if (changed && previousState is not null)
        {
            this.activeKeys.Remove(stateKey);
            AddEmission(
                ref emissions,
                new WindowEmission<TEvent>(
                    this.definition.Name,
                    key,
                    @event,
                    WindowTransitionKind.Closed,
                    source,
                    partition,
                    previousState.Segments,
                    previousState.Tags,
                    WindowBoundaryReason.ActivePredicateEnded));
        }
        else if (segmentChanged && previousState is not null)
        {
            var boundaryChanges = GetSegmentChanges(previousState.Segments, currentSegments);
            AddEmission(
                ref emissions,
                new WindowEmission<TEvent>(
                    this.definition.Name,
                    key,
                    @event,
                    WindowTransitionKind.Closed,
                    source,
                    partition,
                    previousState.Segments,
                    previousState.Tags,
                    WindowBoundaryReason.SegmentChanged,
                    boundaryChanges));
            this.activeKeys[stateKey] = new ActiveWindowState(currentSegments, currentTags);
            AddEmission(
                ref emissions,
                new WindowEmission<TEvent>(
                    this.definition.Name,
                    key,
                    @event,
                    WindowTransitionKind.Opened,
                    source,
                    partition,
                    currentSegments,
                    currentTags));
            changed = true;
            ObserveRollUpSegmentTransitions(
                @event,
                source,
                partition,
                key,
                previousState.Segments,
                previousState.Tags,
                currentSegments,
                currentTags,
                ref emissions);
            observedRollUps = true;
        }

        if (!observedRollUps)
        {
            var observedState = isActive
                ? new ActiveWindowState(currentSegments, currentTags)
                : previousState ?? new ActiveWindowState(Segments: [], Tags: []);
            ObserveRollUps(
                @event,
                source,
                partition,
                key,
                isActive,
                changed,
                observedState.Segments,
                observedState.Tags,
                ref emissions);
        }
    }

    public void TrimInactiveState()
    {
        foreach (var rollUp in this.rollUps)
        {
            rollUp.TrimInactiveState();
        }
    }

    private void ObserveRollUps(
        TEvent @event,
        object? source,
        object? partition,
        object key,
        bool childIsActive,
        bool childChanged,
        IReadOnlyList<WindowSegment> segments,
        IReadOnlyList<WindowTag> tags,
        ref List<WindowEmission<TEvent>>? emissions)
    {
        foreach (var rollUp in this.rollUps)
        {
            rollUp.ObserveChild(
                @event,
                source,
                partition,
                key,
                childIsActive,
                childChanged,
                segments,
                tags,
                ref emissions);
        }
    }

    private void ObserveRollUpSegmentTransitions(
        TEvent @event,
        object? source,
        object? partition,
        object key,
        IReadOnlyList<WindowSegment> previousSegments,
        IReadOnlyList<WindowTag> previousTags,
        IReadOnlyList<WindowSegment> currentSegments,
        IReadOnlyList<WindowTag> currentTags,
        ref List<WindowEmission<TEvent>>? emissions)
    {
        foreach (var rollUp in this.rollUps)
        {
            rollUp.ObserveChildSegmentTransition(
                @event,
                source,
                partition,
                key,
                previousSegments,
                previousTags,
                currentSegments,
                currentTags,
                ref emissions);
        }
    }

    internal static void AddEmission(
        ref List<WindowEmission<TEvent>>? emissions,
        WindowEmission<TEvent> emission)
    {
        emissions ??= [];
        emissions.Add(emission);
    }

    private static bool SegmentsEqual(
        IReadOnlyList<WindowSegment> left,
        IReadOnlyList<WindowSegment> right)
    {
        if (left.Count != right.Count)
        {
            return false;
        }

        for (var i = 0; i < left.Count; i++)
        {
            if (!string.Equals(left[i].Name, right[i].Name, StringComparison.Ordinal)
                || !string.Equals(left[i].ParentName, right[i].ParentName, StringComparison.Ordinal)
                || !EqualityComparer<object?>.Default.Equals(left[i].Value, right[i].Value))
            {
                return false;
            }
        }

        return true;
    }

    private static IReadOnlyList<WindowBoundaryChange> GetSegmentChanges(
        IReadOnlyList<WindowSegment> previous,
        IReadOnlyList<WindowSegment> current)
    {
        var count = Math.Max(previous.Count, current.Count);
        var changes = new List<WindowBoundaryChange>();

        for (var i = 0; i < count; i++)
        {
            var previousSegment = i < previous.Count ? previous[i] : null;
            var currentSegment = i < current.Count ? current[i] : null;
            var name = previousSegment?.Name ?? currentSegment?.Name ?? string.Empty;

            if (previousSegment is null || currentSegment is null)
            {
                changes.Add(new WindowBoundaryChange(
                    name,
                    previousSegment?.Value,
                    currentSegment?.Value));
                continue;
            }

            if (!string.Equals(previousSegment.Name, currentSegment.Name, StringComparison.Ordinal)
                || !string.Equals(previousSegment.ParentName, currentSegment.ParentName, StringComparison.Ordinal)
                || !EqualityComparer<object?>.Default.Equals(previousSegment.Value, currentSegment.Value))
            {
                changes.Add(new WindowBoundaryChange(
                    name,
                    previousSegment.Value,
                    currentSegment.Value));
            }
        }

        return changes.ToArray();
    }
}
