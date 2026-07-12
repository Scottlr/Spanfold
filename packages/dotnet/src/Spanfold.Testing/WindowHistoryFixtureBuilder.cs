using System.Collections;
using System.Reflection;

namespace Spanfold.Testing;

/// <summary>
/// Builds small window histories for comparison tests without running a full event pipeline.
/// </summary>
/// <remarks>
/// This helper is intended for concise contract and comparator tests. Prefer
/// normal Spanfold pipeline ingestion when the test is about runtime window
/// emission behavior.
/// </remarks>
public sealed class WindowHistoryFixtureBuilder
{
    private readonly List<ClosedWindow> closedWindows = [];
    private readonly List<OpenWindow> openWindows = [];

    /// <summary>
    /// Adds a closed window to the fixture history.
    /// </summary>
    /// <param name="windowName">The configured window name.</param>
    /// <param name="key">The window key.</param>
    /// <param name="startPosition">The inclusive start processing position.</param>
    /// <param name="endPosition">The exclusive end processing position.</param>
    /// <param name="source">The optional source identity.</param>
    /// <param name="partition">The optional partition identity.</param>
    /// <param name="segments">The optional analytical segment values.</param>
    /// <param name="tags">The optional descriptive tags.</param>
    /// <returns>This builder.</returns>
    public WindowHistoryFixtureBuilder AddClosedWindow(
        string windowName,
        object key,
        long startPosition,
        long endPosition,
        object? source = null,
        object? partition = null,
        IReadOnlyList<WindowSegment>? segments = null,
        IReadOnlyList<WindowTag>? tags = null)
    {
        this.closedWindows.Add(new ClosedWindow(
            windowName,
            key,
            startPosition,
            endPosition,
            source,
            partition,
            Segments: segments,
            Tags: tags));
        return this;
    }

    /// <summary>
    /// Adds a closed window to the fixture history using a window builder.
    /// </summary>
    /// <param name="windowName">The configured window name.</param>
    /// <param name="key">The window key.</param>
    /// <param name="startPosition">The inclusive start processing position.</param>
    /// <param name="endPosition">The exclusive end processing position.</param>
    /// <param name="configure">Configures source, partition, segments, and tags.</param>
    /// <returns>This builder.</returns>
    public WindowHistoryFixtureBuilder AddClosedWindow(
        string windowName,
        object key,
        long startPosition,
        long endPosition,
        Action<WindowHistoryFixtureWindowBuilder> configure)
    {
        ArgumentNullException.ThrowIfNull(configure);

        var builder = new WindowHistoryFixtureWindowBuilder();
        configure(builder);

        return AddClosedWindow(
            windowName,
            key,
            startPosition,
            endPosition,
            builder.SourceValue,
            builder.PartitionValue,
            builder.Segments,
            builder.Tags);
    }

    /// <summary>
    /// Adds an open window to the fixture history.
    /// </summary>
    /// <param name="windowName">The configured window name.</param>
    /// <param name="key">The window key.</param>
    /// <param name="startPosition">The inclusive start processing position.</param>
    /// <param name="source">The optional source identity.</param>
    /// <param name="partition">The optional partition identity.</param>
    /// <param name="segments">The optional analytical segment values.</param>
    /// <param name="tags">The optional descriptive tags.</param>
    /// <returns>This builder.</returns>
    public WindowHistoryFixtureBuilder AddOpenWindow(
        string windowName,
        object key,
        long startPosition,
        object? source = null,
        object? partition = null,
        IReadOnlyList<WindowSegment>? segments = null,
        IReadOnlyList<WindowTag>? tags = null)
    {
        this.openWindows.Add(new OpenWindow(
            windowName,
            key,
            startPosition,
            source,
            partition,
            Segments: segments,
            Tags: tags));
        return this;
    }

    /// <summary>
    /// Adds an open window to the fixture history using a window builder.
    /// </summary>
    /// <param name="windowName">The configured window name.</param>
    /// <param name="key">The window key.</param>
    /// <param name="startPosition">The inclusive start processing position.</param>
    /// <param name="configure">Configures source, partition, segments, and tags.</param>
    /// <returns>This builder.</returns>
    public WindowHistoryFixtureBuilder AddOpenWindow(
        string windowName,
        object key,
        long startPosition,
        Action<WindowHistoryFixtureWindowBuilder> configure)
    {
        ArgumentNullException.ThrowIfNull(configure);

        var builder = new WindowHistoryFixtureWindowBuilder();
        configure(builder);

        return AddOpenWindow(
            windowName,
            key,
            startPosition,
            builder.SourceValue,
            builder.PartitionValue,
            builder.Segments,
            builder.Tags);
    }

    /// <summary>
    /// Builds a Spanfold window history containing the configured windows.
    /// </summary>
    /// <returns>A window history fixture.</returns>
    public WindowHistory Build()
    {
        var constructor = typeof(WindowHistory).GetConstructor(
            BindingFlags.Instance | BindingFlags.NonPublic,
            binder: null,
            [typeof(bool)],
            modifiers: null)
            ?? throw new InvalidOperationException("Spanfold history constructor was not found.");
        var history = (WindowHistory)constructor.Invoke([true]);
        var field = typeof(WindowHistory).GetField(
            "closedWindows",
            BindingFlags.Instance | BindingFlags.NonPublic)
            ?? throw new InvalidOperationException("Spanfold closed-window storage was not found.");
        var closed = (List<ClosedWindow>)field.GetValue(history)!;

        for (var i = 0; i < this.closedWindows.Count; i++)
        {
            closed.Add(this.closedWindows[i]);
        }

        AddOpenWindows(history);

        return history;
    }

    private void AddOpenWindows(WindowHistory history)
    {
        if (this.openWindows.Count == 0)
        {
            return;
        }

        var keyType = typeof(WindowHistory).Assembly.GetType("Spanfold.Internal.Recording.WindowRecordingKey")
            ?? throw new InvalidOperationException("Spanfold window recording key type was not found.");
        var field = typeof(WindowHistory).GetField(
            "openWindows",
            BindingFlags.Instance | BindingFlags.NonPublic)
            ?? throw new InvalidOperationException("Spanfold open-window storage was not found.");
        var open = (IDictionary)field.GetValue(history)!;
        var segmentContextType = typeof(WindowHistory).Assembly.GetType("Spanfold.Internal.Keys.SegmentContext")
            ?? throw new InvalidOperationException("Spanfold segment context type was not found.");

        for (var i = 0; i < this.openWindows.Count; i++)
        {
            var window = this.openWindows[i];
            var segmentContext = Activator.CreateInstance(
                segmentContextType,
                BindingFlags.Instance | BindingFlags.NonPublic | BindingFlags.Public,
                binder: null,
                args: [window.Segments],
                culture: null)
                ?? throw new InvalidOperationException("Spanfold segment context could not be created.");
            var key = Activator.CreateInstance(
                keyType,
                window.WindowName,
                window.Key,
                window.Source,
                window.Partition,
                segmentContext)
                ?? throw new InvalidOperationException("Spanfold window recording key could not be created.");
            open.Add(key, window);
        }
    }

}
