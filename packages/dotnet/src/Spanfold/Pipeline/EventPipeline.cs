using Spanfold.Internal.Definitions;
using Spanfold.Internal.Runtime;

namespace Spanfold;

/// <summary>
/// Processes events through configured windows and roll-ups.
/// </summary>
/// <typeparam name="TEvent">The event type consumed by the pipeline.</typeparam>
public sealed class EventPipeline<TEvent>
{
    private readonly WindowRuntime<TEvent>[] runtimes;
    private readonly Action<WindowEmission<TEvent>>[] emissionCallbacks;
    private readonly Dictionary<string, WindowCallbackSet<TEvent>> windowCallbacks;
    private readonly Func<TEvent, DateTimeOffset>? eventTimeSelector;
    private long processingPosition;

    internal EventPipeline(
        IReadOnlyList<WindowDefinition<TEvent>> windows,
        IReadOnlyList<Action<WindowEmission<TEvent>>> emissionCallbacks,
        bool recordWindows,
        Func<TEvent, DateTimeOffset>? eventTimeSelector)
    {
        Windows = windows;
        Metadata = new EventPipelineMetadata(
            typeof(TEvent),
            CreateWindowMetadata(windows));
        this.emissionCallbacks = emissionCallbacks.ToArray();
        this.windowCallbacks = CreateWindowCallbackMap(windows);
        this.eventTimeSelector = eventTimeSelector;
        History = new WindowHistory(
            recordWindows,
            windows.ToDictionary(
                static window => window.Name,
                static window => window.KeyComparer,
                StringComparer.Ordinal));
        this.runtimes = new WindowRuntime<TEvent>[windows.Count];

        for (var i = 0; i < this.runtimes.Length; i++)
        {
            this.runtimes[i] = new WindowRuntime<TEvent>(windows[i]);
        }
    }

    internal IReadOnlyList<WindowDefinition<TEvent>> Windows { get; }

    /// <summary>
    /// Gets metadata describing the configured event type and window hierarchy.
    /// </summary>
    public EventPipelineMetadata Metadata { get; }

    /// <summary>
    /// Gets window history recorded by the pipeline.
    /// </summary>
    public WindowHistory History { get; }

    /// <summary>
    /// Ingests one event without source or partition context.
    /// </summary>
    /// <param name="event">The event to process.</param>
    /// <returns>The emissions produced by the event.</returns>
    public IngestionResult<TEvent> Ingest(TEvent @event)
    {
        return Ingest(@event, source: null, partition: null);
    }

    /// <summary>
    /// Ingests one event with source context.
    /// </summary>
    /// <param name="event">The event to process.</param>
    /// <param name="source">Optional source identity to attach to emissions.</param>
    /// <returns>The emissions produced by the event.</returns>
    public IngestionResult<TEvent> Ingest(TEvent @event, object? source)
    {
        return Ingest(@event, source, partition: null);
    }

    /// <summary>
    /// Ingests one event with source and partition context.
    /// </summary>
    /// <param name="event">The event to process.</param>
    /// <param name="source">Optional source identity to attach to emissions.</param>
    /// <param name="partition">Optional partition identity for independent runtime state.</param>
    /// <returns>The emissions produced by the event.</returns>
    public IngestionResult<TEvent> Ingest(TEvent @event, object? source, object? partition)
    {
        this.processingPosition++;
        List<WindowEmission<TEvent>>? emissions = null;

        foreach (var runtime in this.runtimes)
        {
            runtime.Ingest(@event, source, partition, ref emissions);
        }

        var result = new IngestionResult<TEvent>(
            emissions is null ? [] : emissions.ToArray());

        History.Record(
            result.Emissions,
            this.processingPosition,
            this.eventTimeSelector?.Invoke(@event));

        foreach (var emission in result.Emissions)
        {
            InvokeWindowCallbacks(emission);

            foreach (var callback in this.emissionCallbacks)
            {
                callback(emission);
            }
        }

        return result;
    }

    /// <summary>
    /// Ingests events sequentially and returns all emissions in processing order.
    /// </summary>
    /// <param name="events">The events to process.</param>
    /// <returns>The flattened emissions produced by the batch.</returns>
    public IngestionResult<TEvent> IngestMany(IEnumerable<TEvent> events)
    {
        ArgumentNullException.ThrowIfNull(events);

        List<WindowEmission<TEvent>>? emissions = null;

        foreach (var @event in events)
        {
            var result = Ingest(@event);
            if (!result.HasEmissions)
            {
                continue;
            }

            emissions ??= [];
            foreach (var emission in result.Emissions)
            {
                emissions.Add(emission);
            }
        }

        return new IngestionResult<TEvent>(
            emissions is null ? [] : emissions.ToArray());
    }

    private static IReadOnlyList<WindowMetadata> CreateWindowMetadata(
        IReadOnlyList<WindowDefinition<TEvent>> windows)
    {
        var metadata = new WindowMetadata[windows.Count];

        for (var i = 0; i < metadata.Length; i++)
        {
            metadata[i] = CreateWindowMetadata(windows[i]);
        }

        return metadata;
    }

    private static WindowMetadata CreateWindowMetadata(WindowNodeDefinition<TEvent> node)
    {
        var rollUps = new WindowMetadata[node.RollUps.Count];

        for (var i = 0; i < rollUps.Length; i++)
        {
            rollUps[i] = CreateWindowMetadata(node.RollUps[i]);
        }

        return new WindowMetadata(node.Name, rollUps);
    }

    private static Dictionary<string, WindowCallbackSet<TEvent>> CreateWindowCallbackMap(
        IReadOnlyList<WindowDefinition<TEvent>> windows)
    {
        var callbacks = new Dictionary<string, WindowCallbackSet<TEvent>>(StringComparer.Ordinal);

        foreach (var window in windows)
        {
            AddWindowCallbacks(window, callbacks);
        }

        return callbacks;
    }

    private static void AddWindowCallbacks(
        WindowNodeDefinition<TEvent> node,
        Dictionary<string, WindowCallbackSet<TEvent>> callbacks)
    {
        if (node.Callbacks.HasCallbacks)
        {
            callbacks.Add(node.Name, node.Callbacks.Copy());
        }

        foreach (var rollUp in node.RollUps)
        {
            AddWindowCallbacks(rollUp, callbacks);
        }
    }

    private void InvokeWindowCallbacks(WindowEmission<TEvent> emission)
    {
        if (!this.windowCallbacks.TryGetValue(emission.WindowName, out var callbacks))
        {
            return;
        }

        var selectedCallbacks = emission.Kind == WindowTransitionKind.Opened
            ? callbacks.Opened
            : callbacks.Closed;

        foreach (var callback in selectedCallbacks)
        {
            callback(emission);
        }
    }
}
