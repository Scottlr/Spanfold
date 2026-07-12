using Spanfold.Internal.Definitions;
using Spanfold.Internal.Keys;

namespace Spanfold.Internal.Runtime;

internal sealed class RollUpRuntime<TEvent>
{
    private readonly RollUpDefinition<TEvent> definition;
    private readonly IEqualityComparer<object> childKeyComparer;
    private readonly Dictionary<RollUpStateKey, ParentState> parents;
    private readonly Dictionary<ChildIdentity, ChildMembership> childMemberships;
    private readonly RollUpRuntime<TEvent>[] rollUps;

    public RollUpRuntime(
        RollUpDefinition<TEvent> definition,
        IEqualityComparer<object> childKeyComparer)
    {
        this.definition = definition;
        this.childKeyComparer = childKeyComparer;
        this.parents = new Dictionary<RollUpStateKey, ParentState>(
            new RollUpStateKeyComparer(definition.KeyComparer));
        this.childMemberships = new Dictionary<ChildIdentity, ChildMembership>(
            new ChildIdentityComparer(childKeyComparer));
        this.rollUps = new RollUpRuntime<TEvent>[definition.RollUps.Count];

        for (var i = 0; i < this.rollUps.Length; i++)
        {
            this.rollUps[i] = new RollUpRuntime<TEvent>(
                definition.RollUps[i],
                definition.KeyComparer);
        }
    }

    public void ObserveChild(
        TEvent @event,
        object? source,
        object? partition,
        object childKey,
        bool childIsActive,
        bool childChanged,
        IReadOnlyList<WindowSegment> segments,
        IReadOnlyList<WindowTag> tags,
        RuntimeMutationJournal journal,
        ref List<WindowEmission<TEvent>>? emissions)
    {
        var projectedSegments = this.definition.SegmentProjection.Project(segments);
        var parentKey = this.definition.GetKey(@event);
        var parentStateKey = new RollUpStateKey(
            parentKey,
            source,
            partition,
            StableSegments(projectedSegments));

        var childIdentity = new ChildIdentity(childKey, source, partition);
        if (this.childMemberships.TryGetValue(childIdentity, out var previousMembership)
            && !StateKeysEqual(previousMembership.StateKey, parentStateKey))
        {
            RemoveChildFromPreviousParent(
                @event,
                source,
                partition,
                childKey,
                previousMembership,
                journal,
                ref emissions);
        }

        if (!this.parents.TryGetValue(parentStateKey, out var parent))
        {
            parent = new ParentState(this.childKeyComparer);
            journal.Set(this.parents, parentStateKey, parent);
        }

        var membershipChanged = !parent.Children.ContainsKey(childKey);
        journal.Set(parent.Children, childKey, childIsActive);
        journal.Set(this.childMemberships, childIdentity, new ChildMembership(
            parentStateKey,
            projectedSegments.ToArray(),
            tags.ToArray()));

        var parentChanged = false;

        if (!childChanged && !membershipChanged)
        {
            PropagateToParents(
                @event,
                source,
                partition,
                parentKey,
                parent.IsActive,
                parentChanged,
                projectedSegments,
                tags,
                journal,
                ref emissions);
            return;
        }

        var children = parent.ToChildActivityView();
        var isActive = this.definition.IsActive(children);

        if (isActive == parent.IsActive)
        {
            PropagateToParents(
                @event,
                source,
                partition,
                parentKey,
                parent.IsActive,
                parentChanged,
                projectedSegments,
                tags,
                journal,
                ref emissions);
            return;
        }

        SetParentActive(parent, isActive, journal);
        parentChanged = true;
        WindowRuntime<TEvent>.AddEmission(
            ref emissions,
            new WindowEmission<TEvent>(
                this.definition.Name,
                parentKey,
                @event,
                isActive ? WindowTransitionKind.Opened : WindowTransitionKind.Closed,
                source,
                partition,
                projectedSegments,
                tags,
                isActive ? null : WindowBoundaryReason.ActivePredicateEnded));

        PropagateToParents(
            @event,
            source,
            partition,
            parentKey,
            parent.IsActive,
            parentChanged,
            projectedSegments,
            tags,
            journal,
            ref emissions);
    }

    public void TrimInactiveState()
    {
        var inactiveKeys = this.parents
            .Where(static pair => !pair.Value.IsActive && pair.Value.Children.Values.All(static active => !active))
            .Select(static pair => pair.Key)
            .ToHashSet();

        foreach (var key in inactiveKeys)
        {
            this.parents.Remove(key);
        }

        if (inactiveKeys.Count > 0)
        {
            foreach (var child in this.childMemberships
                .Where(pair => inactiveKeys.Contains(pair.Value.StateKey))
                .Select(static pair => pair.Key)
                .ToArray())
            {
                this.childMemberships.Remove(child);
            }
        }

        foreach (var rollUp in this.rollUps)
        {
            rollUp.TrimInactiveState();
        }
    }

    public void ObserveChildSegmentTransition(
        TEvent @event,
        object? source,
        object? partition,
        object childKey,
        IReadOnlyList<WindowSegment> previousSegments,
        IReadOnlyList<WindowTag> previousTags,
        IReadOnlyList<WindowSegment> currentSegments,
        IReadOnlyList<WindowTag> currentTags,
        RuntimeMutationJournal journal,
        ref List<WindowEmission<TEvent>>? emissions)
    {
        var projectedPreviousSegments = this.definition.SegmentProjection.Project(previousSegments);
        var projectedCurrentSegments = this.definition.SegmentProjection.Project(currentSegments);

        if (!SegmentContextsEqual(projectedPreviousSegments, projectedCurrentSegments))
        {
            ObserveChild(
                @event,
                source,
                partition,
                childKey,
                childIsActive: false,
                childChanged: true,
                previousSegments,
                previousTags,
                journal,
                ref emissions);
            ObserveChild(
                @event,
                source,
                partition,
                childKey,
                childIsActive: true,
                childChanged: true,
                currentSegments,
                currentTags,
                journal,
                ref emissions);
            return;
        }

        var parentKey = this.definition.GetKey(@event);
        var parentStateKey = new RollUpStateKey(
            parentKey,
            source,
            partition,
            StableSegments(projectedCurrentSegments));

        var childIdentity = new ChildIdentity(childKey, source, partition);
        if (this.childMemberships.TryGetValue(childIdentity, out var previousMembership)
            && !StateKeysEqual(previousMembership.StateKey, parentStateKey))
        {
            RemoveChildFromPreviousParent(
                @event,
                source,
                partition,
                childKey,
                previousMembership,
                journal,
                ref emissions);
        }

        if (!this.parents.TryGetValue(parentStateKey, out var parent))
        {
            parent = new ParentState(this.childKeyComparer);
            journal.Set(this.parents, parentStateKey, parent);
        }

        journal.Set(parent.Children, childKey, true);
        journal.Set(this.childMemberships, childIdentity, new ChildMembership(
            parentStateKey,
            projectedCurrentSegments.ToArray(),
            currentTags.ToArray()));
        var children = parent.ToChildActivityView();
        var isActive = this.definition.IsActive(children);
        var parentChanged = isActive != parent.IsActive;

        if (parentChanged)
        {
            SetParentActive(parent, isActive, journal);
            WindowRuntime<TEvent>.AddEmission(
                ref emissions,
                new WindowEmission<TEvent>(
                    this.definition.Name,
                    parentKey,
                    @event,
                    isActive ? WindowTransitionKind.Opened : WindowTransitionKind.Closed,
                    source,
                    partition,
                    projectedCurrentSegments,
                    currentTags,
                    isActive ? null : WindowBoundaryReason.ActivePredicateEnded));
        }

        PropagateToParents(
            @event,
            source,
            partition,
            parentKey,
            parent.IsActive,
            parentChanged,
            projectedCurrentSegments,
            currentTags,
            journal,
            ref emissions);
    }

    private void RemoveChildFromPreviousParent(
        TEvent @event,
        object? source,
        object? partition,
        object childKey,
        ChildMembership previousMembership,
        RuntimeMutationJournal journal,
        ref List<WindowEmission<TEvent>>? emissions)
    {
        if (!this.parents.TryGetValue(previousMembership.StateKey, out var previousParent)
            || !journal.Remove(previousParent.Children, childKey))
        {
            return;
        }

        var isActive = this.definition.IsActive(previousParent.ToChildActivityView());
        var parentChanged = isActive != previousParent.IsActive;
        if (parentChanged)
        {
            SetParentActive(previousParent, isActive, journal);
            WindowRuntime<TEvent>.AddEmission(
                ref emissions,
                new WindowEmission<TEvent>(
                    this.definition.Name,
                    previousMembership.StateKey.Key,
                    @event,
                    isActive ? WindowTransitionKind.Opened : WindowTransitionKind.Closed,
                    source,
                    partition,
                    previousMembership.Segments,
                    previousMembership.Tags,
                    isActive ? null : WindowBoundaryReason.ActivePredicateEnded));
        }

        PropagateToParents(
            @event,
            source,
            partition,
            previousMembership.StateKey.Key,
            previousParent.IsActive,
            parentChanged,
            previousMembership.Segments,
            previousMembership.Tags,
            journal,
            ref emissions);
    }

    private bool StateKeysEqual(RollUpStateKey left, RollUpStateKey right)
    {
        return this.definition.KeyComparer.Equals(left.Key, right.Key)
            && EqualityComparer<object?>.Default.Equals(left.Source, right.Source)
            && EqualityComparer<object?>.Default.Equals(left.Partition, right.Partition)
            && Equals(left.SegmentContext, right.SegmentContext);
    }

    private void PropagateToParents(
        TEvent @event,
        object? source,
        object? partition,
        object parentKey,
        bool parentIsActive,
        bool parentChanged,
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
                parentKey,
                parentIsActive,
                parentChanged,
                segments,
                tags,
                journal,
                ref emissions);
        }
    }

    private static SegmentContext StableSegments(IReadOnlyList<WindowSegment> segments) => new(segments);

    private static void SetParentActive(
        ParentState parent,
        bool isActive,
        RuntimeMutationJournal journal)
    {
        var previous = parent.IsActive;
        journal.Record(() => parent.IsActive = previous);
        parent.IsActive = isActive;
    }

    private static bool SegmentContextsEqual(
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

    private sealed class ParentState
    {
        public ParentState(IEqualityComparer<object> childKeyComparer)
        {
            Children = new Dictionary<object, bool>(childKeyComparer);
        }

        public Dictionary<object, bool> Children { get; }

        public bool IsActive { get; set; }

        public ChildActivityView ToChildActivityView()
        {
            var activeCount = 0;

            foreach (var child in Children)
            {
                if (child.Value)
                {
                    activeCount++;
                }
            }

            return new ChildActivityView(activeCount, Children.Count);
        }
    }

    private readonly record struct ChildIdentity(
        object Key,
        object? Source,
        object? Partition);

    private sealed record ChildMembership(
        RollUpStateKey StateKey,
        IReadOnlyList<WindowSegment> Segments,
        IReadOnlyList<WindowTag> Tags);

    private sealed class ChildIdentityComparer : IEqualityComparer<ChildIdentity>
    {
        private readonly IEqualityComparer<object> keyComparer;

        public ChildIdentityComparer(IEqualityComparer<object> keyComparer)
        {
            this.keyComparer = keyComparer;
        }

        public bool Equals(ChildIdentity x, ChildIdentity y)
        {
            return this.keyComparer.Equals(x.Key, y.Key)
                && EqualityComparer<object?>.Default.Equals(x.Source, y.Source)
                && EqualityComparer<object?>.Default.Equals(x.Partition, y.Partition);
        }

        public int GetHashCode(ChildIdentity obj)
        {
            return HashCode.Combine(
                this.keyComparer.GetHashCode(obj.Key),
                obj.Source,
                obj.Partition);
        }
    }

    private readonly record struct RollUpStateKey(
        object Key,
        object? Source,
        object? Partition,
        SegmentContext SegmentContext);

    private sealed class RollUpStateKeyComparer : IEqualityComparer<RollUpStateKey>
    {
        private readonly IEqualityComparer<object> keyComparer;

        public RollUpStateKeyComparer(IEqualityComparer<object> keyComparer)
        {
            this.keyComparer = keyComparer;
        }

        public bool Equals(RollUpStateKey x, RollUpStateKey y)
        {
            return this.keyComparer.Equals(x.Key, y.Key)
                && EqualityComparer<object?>.Default.Equals(x.Source, y.Source)
                && EqualityComparer<object?>.Default.Equals(x.Partition, y.Partition)
                && Equals(x.SegmentContext, y.SegmentContext);
        }

        public int GetHashCode(RollUpStateKey obj)
        {
            return HashCode.Combine(
                this.keyComparer.GetHashCode(obj.Key),
                obj.Source,
                obj.Partition,
                obj.SegmentContext);
        }
    }
}
