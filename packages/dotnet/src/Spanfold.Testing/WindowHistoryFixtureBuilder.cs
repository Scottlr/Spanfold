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
        return WindowHistory.CreateFixture(this.closedWindows, this.openWindows);
    }
}
