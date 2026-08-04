using Spanfold.Internal.Definitions;
using Spanfold.Internal.Keys;

namespace Spanfold.Internal.Runtime;

internal sealed class WindowRuntime<TEvent>
{
    private readonly WindowDefinition<TEvent> definition;
    private readonly Dictionary<RuntimeStateKey, ActiveWindowState> activeKeys;
    private readonly Dictionary<RuntimeStateKey, int> pendingConfirmations;
    private readonly RollUpRuntime<TEvent>[] rollUps;

    public WindowRuntime(WindowDefinition<TEvent> definition)
    {
        this.definition = definition;
        this.activeKeys = new Dictionary<RuntimeStateKey, ActiveWindowState>(
            new RuntimeStateKeyComparer(definition.KeyComparer));
        this.pendingConfirmations = new Dictionary<RuntimeStateKey, int>(
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
        RuntimeMutationJournal journal,
        ref List<WindowEmission<TEvent>>? emissions)
    {
        var key = this.definition.GetKey(@event);
        var stateKey = new RuntimeStateKey(key, source, partition);
        var wasActive = this.activeKeys.TryGetValue(stateKey, out var previousState);

        if (!wasActive)
        {
            ObserveInactive(
                @event,
                source,
                partition,
                key,
                stateKey,
                journal,
                ref emissions);
            return;
        }

        ObserveActive(
            @event,
            source,
            partition,
            key,
            stateKey,
            previousState!,
            journal,
            ref emissions);
    }

    public void TrimInactiveState()
    {
        foreach (var rollUp in this.rollUps)
        {
            rollUp.TrimInactiveState();
        }
    }

    private void ObserveInactive(
        TEvent @event,
        object? source,
        object? partition,
        object key,
        RuntimeStateKey stateKey,
        RuntimeMutationJournal journal,
        ref List<WindowEmission<TEvent>>? emissions)
    {
        if (!this.definition.IsActive(@event))
        {
            journal.Remove(this.pendingConfirmations, stateKey);
            ObserveRollUps(
                @event,
                source,
                partition,
                key,
                childIsActive: false,
                childChanged: false,
                segments: [],
                tags: [],
                journal,
                ref emissions);
            return;
        }

        var confirmationCount = IncrementConfirmation(stateKey, journal);
        if (confirmationCount < this.definition.EnterConfirmationCount)
        {
            return;
        }

        journal.Remove(this.pendingConfirmations, stateKey);
        var currentSegments = this.definition.GetSegments(@event);
        var currentTags = this.definition.GetTags(@event);
        journal.Set(this.activeKeys, stateKey, new ActiveWindowState(currentSegments, currentTags));
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
        ObserveRollUps(
            @event,
            source,
            partition,
            key,
            childIsActive: true,
            childChanged: true,
            currentSegments,
            currentTags,
            journal,
            ref emissions);
    }

    private void ObserveActive(
        TEvent @event,
        object? source,
        object? partition,
        object key,
        RuntimeStateKey stateKey,
        ActiveWindowState previousState,
        RuntimeMutationJournal journal,
        ref List<WindowEmission<TEvent>>? emissions)
    {
        if (this.definition.ShouldExit(@event))
        {
            var confirmationCount = IncrementConfirmation(stateKey, journal);
            if (confirmationCount < this.definition.ExitConfirmationCount)
            {
                return;
            }

            journal.Remove(this.pendingConfirmations, stateKey);
            journal.Remove(this.activeKeys, stateKey);
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
            ObserveRollUps(
                @event,
                source,
                partition,
                key,
                childIsActive: false,
                childChanged: true,
                previousState.Segments,
                previousState.Tags,
                journal,
                ref emissions);
            return;
        }

        journal.Remove(this.pendingConfirmations, stateKey);
        var currentSegments = this.definition.GetSegments(@event);
        var currentTags = this.definition.GetTags(@event);
        var segmentChanged = !SegmentsEqual(previousState.Segments, currentSegments);
        var observedRollUps = false;

        if (segmentChanged)
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
            journal.Set(this.activeKeys, stateKey, new ActiveWindowState(currentSegments, currentTags));
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
            ObserveRollUpSegmentTransitions(
                @event,
                source,
                partition,
                key,
                previousState.Segments,
                previousState.Tags,
                currentSegments,
                currentTags,
                journal,
                ref emissions);
            observedRollUps = true;
        }

        if (!observedRollUps)
        {
            ObserveRollUps(
                @event,
                source,
                partition,
                key,
                childIsActive: true,
                childChanged: false,
                currentSegments,
                currentTags,
                journal,
                ref emissions);
        }
    }

    private int IncrementConfirmation(
        RuntimeStateKey stateKey,
        RuntimeMutationJournal journal)
    {
        var current = this.pendingConfirmations.GetValueOrDefault(stateKey);
        var next = checked(current + 1);
        journal.Set(this.pendingConfirmations, stateKey, next);
        return next;
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
        RuntimeMutationJournal journal,
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
                journal,
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
        RuntimeMutationJournal journal,
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
                journal,
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
