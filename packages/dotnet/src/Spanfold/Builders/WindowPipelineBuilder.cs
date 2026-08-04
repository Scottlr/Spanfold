using Spanfold.Internal.Definitions;

namespace Spanfold;

/// <summary>
/// Configures a pipeline after a source window or roll-up has been added.
/// </summary>
/// <typeparam name="TEvent">The event type consumed by the pipeline.</typeparam>
public sealed class WindowPipelineBuilder<TEvent>
{
    private readonly HashSet<string> windowNames;
    private readonly List<Action<WindowEmission<TEvent>>> emissionCallbacks;
    private readonly PipelineBuilderOptions<TEvent> options;

    internal WindowPipelineBuilder(
        List<WindowDefinition<TEvent>> windows,
        HashSet<string> windowNames,
        List<Action<WindowEmission<TEvent>>> emissionCallbacks,
        PipelineBuilderOptions<TEvent> options,
        WindowNodeDefinition<TEvent> currentWindow)
    {
        Windows = windows;
        this.windowNames = windowNames;
        this.emissionCallbacks = emissionCallbacks;
        this.options = options;
        CurrentWindow = currentWindow;
    }

    internal List<WindowDefinition<TEvent>> Windows { get; }

    internal WindowNodeDefinition<TEvent> CurrentWindow { get; }

    /// <summary>
    /// Adds a parent roll-up for the current window.
    /// </summary>
    /// <typeparam name="TKey">The parent key type.</typeparam>
    /// <param name="name">The unique roll-up window name.</param>
    /// <param name="key">Selects the parent key from each event.</param>
    /// <param name="isActive">Returns true when the parent should be active for its children.</param>
    /// <returns>A builder positioned at the newly added roll-up.</returns>
    public WindowPipelineBuilder<TEvent> RollUp<TKey>(
        string name,
        Func<TEvent, TKey> key,
        Func<ChildActivityView, bool> isActive)
        where TKey : notnull
    {
        return AddRollUp(name, key, isActive, configureSegments: null);
    }

    /// <summary>
    /// Adds a parent roll-up for the current window with segment projection options.
    /// </summary>
    /// <typeparam name="TKey">The parent key type.</typeparam>
    /// <param name="name">The unique roll-up window name.</param>
    /// <param name="key">Selects the parent key from each event.</param>
    /// <param name="isActive">Returns true when the parent should be active for its children.</param>
    /// <param name="configureSegments">Configures which child segment dimensions are preserved.</param>
    /// <returns>A builder positioned at the newly added roll-up.</returns>
    public WindowPipelineBuilder<TEvent> RollUp<TKey>(
        string name,
        Func<TEvent, TKey> key,
        Func<ChildActivityView, bool> isActive,
        Action<RollUpSegmentProjectionBuilder> configureSegments)
        where TKey : notnull
    {
        ArgumentNullException.ThrowIfNull(configureSegments);

        return AddRollUp(name, key, isActive, configureSegments);
    }

    private WindowPipelineBuilder<TEvent> AddRollUp<TKey>(
        string name,
        Func<TEvent, TKey> key,
        Func<ChildActivityView, bool> isActive,
        Action<RollUpSegmentProjectionBuilder>? configureSegments)
        where TKey : notnull
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(name);
        ArgumentNullException.ThrowIfNull(key);
        ArgumentNullException.ThrowIfNull(isActive);
        EventPipelineBuilder<TEvent>.ThrowIfNameExists(name, this.windowNames);

        var segmentProjection = RollUpSegmentProjection.PreserveAll;
        if (configureSegments is not null)
        {
            var segmentBuilder = new RollUpSegmentProjectionBuilder();
            configureSegments(segmentBuilder);
            segmentProjection = segmentBuilder.Build();
        }

        var definition = new RollUpDefinition<TEvent, TKey>(
            name,
            key,
            isActive,
            segmentProjection);
        CurrentWindow.RollUps.Add(definition);

        return new WindowPipelineBuilder<TEvent>(
            Windows,
            this.windowNames,
            this.emissionCallbacks,
            this.options,
            definition);
    }

    /// <summary>
    /// Adds a reusable roll-up window definition for the current window.
    /// </summary>
    /// <typeparam name="TRollUp">The reusable roll-up definition type.</typeparam>
    /// <returns>A builder positioned at the newly added roll-up.</returns>
    public WindowPipelineBuilder<TEvent> RollUp<TRollUp>()
        where TRollUp : IRollUpDefinition<TEvent>, new()
    {
        var definition = EventPipelineBuilder<TEvent>.CreateRollUpDefinition<TRollUp>();
        EventPipelineBuilder<TEvent>.ThrowIfNameExists(definition.Name, this.windowNames);
        CurrentWindow.RollUps.Add(definition);

        return new WindowPipelineBuilder<TEvent>(
            Windows,
            this.windowNames,
            this.emissionCallbacks,
            this.options,
            definition);
    }

    /// <summary>
    /// Registers a synchronous callback invoked for each emitted transition.
    /// </summary>
    /// <param name="callback">The callback to invoke in emission order.</param>
    /// <returns>The current builder.</returns>
    public WindowPipelineBuilder<TEvent> OnEmission(Action<WindowEmission<TEvent>> callback)
    {
        ArgumentNullException.ThrowIfNull(callback);

        this.emissionCallbacks.Add(callback);
        return this;
    }

    /// <summary>
    /// Enables processing-position window history for opened and closed windows.
    /// </summary>
    /// <returns>The current builder.</returns>
    public WindowPipelineBuilder<TEvent> RecordWindows()
    {
        this.options.RecordWindows = true;
        return this;
    }

    /// <summary>
    /// Configures event timestamps for window history.
    /// </summary>
    /// <param name="eventTime">Selects the event timestamp.</param>
    /// <returns>The current builder.</returns>
    public WindowPipelineBuilder<TEvent> WithEventTime(
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
    public WindowPipelineBuilder<TEvent> WithEventTime(
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
    /// Adds another independent source window.
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
    /// Adds another independent source window.
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
        EventPipelineBuilder<TEvent>.ThrowIfNameExists(name, this.windowNames);

        var definition = new WindowDefinition<TEvent, TKey>(
            name,
            key,
            isActive,
            comparer);
        var options = new WindowOptions<TEvent, TKey>(definition.Callbacks);
        configure?.Invoke(options);
        options.ApplyTo(definition);
        Windows.Add(definition);

        return new WindowPipelineBuilder<TEvent>(
            Windows,
            this.windowNames,
            this.emissionCallbacks,
            this.options,
            definition);
    }

    /// <summary>
    /// Adds another reusable source window definition.
    /// </summary>
    /// <typeparam name="TWindow">The reusable window definition type.</typeparam>
    /// <returns>A builder positioned at the newly added window.</returns>
    public WindowPipelineBuilder<TEvent> Window<TWindow>()
        where TWindow : IWindowDefinition<TEvent>, new()
    {
        var definition = EventPipelineBuilder<TEvent>.CreateWindowDefinition<TWindow>();
        EventPipelineBuilder<TEvent>.ThrowIfNameExists(definition.Name, this.windowNames);
        Windows.Add(definition);

        return new WindowPipelineBuilder<TEvent>(
            Windows,
            this.windowNames,
            this.emissionCallbacks,
            this.options,
            definition);
    }

    /// <summary>
    /// Adds another state-driven source window using the full definition builder.
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
        EventPipelineBuilder<TEvent>.ThrowIfNameExists(definition.Name, this.windowNames);
        Windows.Add(definition);

        return new WindowPipelineBuilder<TEvent>(
            Windows,
            this.windowNames,
            this.emissionCallbacks,
            this.options,
            definition);
    }

    /// <summary>
    /// Builds an event pipeline from the configured windows and options.
    /// </summary>
    /// <returns>A pipeline ready to ingest events.</returns>
    public EventPipeline<TEvent> Build()
    {
        return new EventPipeline<TEvent>(
            Windows.ToArray(),
            this.emissionCallbacks.ToArray(),
            this.options.RecordWindows,
            this.options.EventTimeSelector,
            this.options.EventTimeClock);
    }
}
