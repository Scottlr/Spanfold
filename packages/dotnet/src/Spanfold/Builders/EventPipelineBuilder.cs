using Spanfold.Internal.Definitions;

namespace Spanfold;

/// <summary>
/// Configures windows and options for a Spanfold event pipeline.
/// </summary>
/// <typeparam name="TEvent">The event type consumed by the pipeline.</typeparam>
public sealed class EventPipelineBuilder<TEvent>
{
    private readonly List<WindowDefinition<TEvent>> windows;
    private readonly HashSet<string> windowNames;
    private readonly List<Action<WindowEmission<TEvent>>> emissionCallbacks;
    private readonly PipelineBuilderOptions<TEvent> options;

    internal EventPipelineBuilder()
    {
        this.windows = [];
        this.windowNames = new HashSet<string>(StringComparer.Ordinal);
        this.emissionCallbacks = [];
        this.options = new PipelineBuilderOptions<TEvent>();
    }

    /// <summary>
    /// Adds a state-driven source window.
    /// </summary>
    /// <typeparam name="TKey">The key type used to track independent window state.</typeparam>
    /// <param name="name">The unique window name.</param>
    /// <param name="key">Selects the logical key from each event.</param>
    /// <param name="isActive">Returns true when the keyed window should be active.</param>
    /// <param name="configure">Source window options.</param>
    /// <returns>A builder positioned at the newly added window.</returns>
    public WindowPipelineBuilder<TEvent> Window<TKey>(
        string name,
        Func<TEvent, TKey> key,
        Func<TEvent, bool> isActive,
        Action<WindowOptions<TEvent, TKey>> configure)
        where TKey : notnull
    {
        ArgumentNullException.ThrowIfNull(configure);

        return Window(name, key, isActive, comparer: null, configure);
    }

    /// <summary>
    /// Adds a state-driven source window.
    /// </summary>
    /// <typeparam name="TKey">The key type used to track independent window state.</typeparam>
    /// <param name="name">The unique window name.</param>
    /// <param name="key">Selects the logical key from each event.</param>
    /// <param name="isActive">Returns true when the keyed window should be active.</param>
    /// <param name="comparer">Optional comparer for source window keys.</param>
    /// <param name="configure">Optional source window options.</param>
    /// <returns>A builder positioned at the newly added window.</returns>
    public WindowPipelineBuilder<TEvent> Window<TKey>(
        string name,
        Func<TEvent, TKey> key,
        Func<TEvent, bool> isActive,
        IEqualityComparer<TKey>? comparer = null,
        Action<WindowOptions<TEvent, TKey>>? configure = null)
        where TKey : notnull
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(name);
        ArgumentNullException.ThrowIfNull(key);
        ArgumentNullException.ThrowIfNull(isActive);
        ThrowIfNameExists(name, this.windowNames);

        var definition = new WindowDefinition<TEvent, TKey>(
            name,
            key,
            isActive,
            comparer);
        var options = new WindowOptions<TEvent, TKey>(definition.Callbacks);
        configure?.Invoke(options);
        options.ApplyTo(definition);
        this.windows.Add(definition);

        return new WindowPipelineBuilder<TEvent>(
            this.windows,
            this.windowNames,
            this.emissionCallbacks,
            this.options,
            definition);
    }

    /// <summary>
    /// Adds a reusable source window definition.
    /// </summary>
    /// <typeparam name="TWindow">The reusable window definition type.</typeparam>
    /// <returns>A builder positioned at the newly added window.</returns>
    public WindowPipelineBuilder<TEvent> Window<TWindow>()
        where TWindow : IWindowDefinition<TEvent>, new()
    {
        var definition = CreateWindowDefinition<TWindow>();
        ThrowIfNameExists(definition.Name, this.windowNames);
        this.windows.Add(definition);

        return new WindowPipelineBuilder<TEvent>(
            this.windows,
            this.windowNames,
            this.emissionCallbacks,
            this.options,
            definition);
    }

    /// <summary>
    /// Adds a state-driven source window using the full definition builder.
    /// </summary>
    /// <param name="name">The default window name.</param>
    /// <param name="configure">Configures the window definition.</param>
    /// <returns>A builder positioned at the newly added window.</returns>
    public WindowPipelineBuilder<TEvent> Window(
        string name,
        Action<WindowDefinitionBuilder<TEvent>> configure)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(name);
        ArgumentNullException.ThrowIfNull(configure);

        var builder = new WindowDefinitionBuilder<TEvent>(name);
        configure(builder);
        var definition = builder.Build();
        ThrowIfNameExists(definition.Name, this.windowNames);
        this.windows.Add(definition);

        return new WindowPipelineBuilder<TEvent>(
            this.windows,
            this.windowNames,
            this.emissionCallbacks,
            this.options,
            definition);
    }

    /// <summary>
    /// Builds a pipeline for a single state-driven source window.
    /// </summary>
    /// <typeparam name="TKey">The key type used to track independent window state.</typeparam>
    /// <param name="name">The unique window name.</param>
    /// <param name="key">Selects the logical key from each event.</param>
    /// <param name="isActive">Returns true when the keyed window should be active.</param>
    /// <param name="configure">Source window options.</param>
    /// <returns>A pipeline ready to ingest events.</returns>
    public EventPipeline<TEvent> TrackWindow<TKey>(
        string name,
        Func<TEvent, TKey> key,
        Func<TEvent, bool> isActive,
        Action<WindowOptions<TEvent, TKey>> configure)
        where TKey : notnull
    {
        ArgumentNullException.ThrowIfNull(configure);

        return TrackWindow(name, key, isActive, comparer: null, configure);
    }

    /// <summary>
    /// Builds a pipeline for a single state-driven source window.
    /// </summary>
    /// <typeparam name="TKey">The key type used to track independent window state.</typeparam>
    /// <param name="name">The unique window name.</param>
    /// <param name="key">Selects the logical key from each event.</param>
    /// <param name="isActive">Returns true when the keyed window should be active.</param>
    /// <param name="comparer">Optional comparer for source window keys.</param>
    /// <param name="configure">Optional source window options.</param>
    /// <returns>A pipeline ready to ingest events.</returns>
    public EventPipeline<TEvent> TrackWindow<TKey>(
        string name,
        Func<TEvent, TKey> key,
        Func<TEvent, bool> isActive,
        IEqualityComparer<TKey>? comparer = null,
        Action<WindowOptions<TEvent, TKey>>? configure = null)
        where TKey : notnull
    {
        return Window(name, key, isActive, comparer, configure).Build();
    }

    /// <summary>
    /// Builds a pipeline for a single reusable source window definition.
    /// </summary>
    /// <typeparam name="TWindow">The reusable window definition type.</typeparam>
    /// <returns>A pipeline ready to ingest events.</returns>
    public EventPipeline<TEvent> TrackWindow<TWindow>()
        where TWindow : IWindowDefinition<TEvent>, new()
    {
        return Window<TWindow>().Build();
    }

    /// <summary>
    /// Builds a pipeline for a single source window using the full definition builder.
    /// </summary>
    /// <param name="name">The default window name.</param>
    /// <param name="configure">Configures the window definition.</param>
    /// <returns>A pipeline ready to ingest events.</returns>
    public EventPipeline<TEvent> TrackWindow(
        string name,
        Action<WindowDefinitionBuilder<TEvent>> configure)
    {
        return Window(name, configure).Build();
    }

    /// <summary>
    /// Registers a synchronous callback invoked for each emitted transition.
    /// </summary>
    /// <param name="callback">The callback to invoke in emission order.</param>
    /// <returns>The current builder.</returns>
    public EventPipelineBuilder<TEvent> OnEmission(Action<WindowEmission<TEvent>> callback)
    {
        ArgumentNullException.ThrowIfNull(callback);

        this.emissionCallbacks.Add(callback);
        return this;
    }

    /// <summary>
    /// Enables processing-position window history for opened and closed windows.
    /// </summary>
    /// <returns>The current builder.</returns>
    public EventPipelineBuilder<TEvent> RecordWindows()
    {
        this.options.RecordWindows = true;
        return this;
    }

    /// <summary>
    /// Configures event timestamps for window history.
    /// </summary>
    /// <param name="eventTime">Selects the event timestamp.</param>
    /// <returns>The current builder.</returns>
    public EventPipelineBuilder<TEvent> WithEventTime(
        Func<TEvent, DateTimeOffset> eventTime)
    {
        ArgumentNullException.ThrowIfNull(eventTime);

        this.options.EventTimeSelector = eventTime;
        this.options.EventTimeClock = null;
        return this;
    }

    /// <summary>
    /// Configures event timestamps and the clock identity used to compare them.
    /// </summary>
    /// <param name="eventTime">Selects the event timestamp.</param>
    /// <param name="clock">Identifies the clock that produced the timestamps.</param>
    /// <returns>The current builder.</returns>
    public EventPipelineBuilder<TEvent> WithEventTime(
        Func<TEvent, DateTimeOffset> eventTime,
        string clock)
    {
        ArgumentNullException.ThrowIfNull(eventTime);
        ArgumentException.ThrowIfNullOrWhiteSpace(clock);

        this.options.EventTimeSelector = eventTime;
        this.options.EventTimeClock = clock;
        return this;
    }

    /// <summary>
    /// Builds an event pipeline from the configured windows and options.
    /// </summary>
    /// <returns>A pipeline ready to ingest events.</returns>
    public EventPipeline<TEvent> Build()
    {
        return new EventPipeline<TEvent>(
            this.windows.ToArray(),
            this.emissionCallbacks.ToArray(),
            this.options.RecordWindows,
            this.options.EventTimeSelector,
            this.options.EventTimeClock);
    }

    internal static void ThrowIfNameExists(string name, HashSet<string> windowNames)
    {
        if (!windowNames.Add(name))
        {
            throw new InvalidOperationException(
                $"A window named '{name}' has already been configured.");
        }
    }

    internal static WindowDefinition<TEvent> CreateWindowDefinition<TWindow>()
        where TWindow : IWindowDefinition<TEvent>, new()
    {
        var builder = new WindowDefinitionBuilder<TEvent>(typeof(TWindow).Name);
        new TWindow().Define(builder);

        return builder.Build();
    }

    internal static RollUpDefinition<TEvent> CreateRollUpDefinition<TRollUp>()
        where TRollUp : IRollUpDefinition<TEvent>, new()
    {
        var builder = new RollUpDefinitionBuilder<TEvent>(typeof(TRollUp).Name);
        new TRollUp().Define(builder);

        return builder.Build();
    }
}
