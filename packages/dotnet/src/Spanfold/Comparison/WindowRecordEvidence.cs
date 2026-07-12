namespace Spanfold;

/// <summary>
/// Preserves source metadata needed to interpret comparison record IDs.
/// </summary>
public sealed class WindowRecordEvidence
{
    internal WindowRecordEvidence(WindowRecord window)
    {
        Id = window.Id;
        WindowName = window.WindowName;
        Segments = Array.AsReadOnly(window.Segments.ToArray());
        Tags = Array.AsReadOnly(window.Tags.ToArray());
        BoundaryReason = window.BoundaryReason;
        BoundaryChanges = Array.AsReadOnly(window.BoundaryChanges.ToArray());
    }

    /// <summary>Gets the recorded window identity.</summary>
    public WindowRecordId Id { get; }

    /// <summary>Gets the source window name.</summary>
    public string WindowName { get; }

    /// <summary>Gets analytical segment values attached to the source window.</summary>
    public IReadOnlyList<WindowSegment> Segments { get; }

    /// <summary>Gets descriptive tags attached to the source window.</summary>
    public IReadOnlyList<WindowTag> Tags { get; }

    /// <summary>Gets the source boundary reason, when the window is closed.</summary>
    public WindowBoundaryReason? BoundaryReason { get; }

    /// <summary>Gets segment changes that caused the source window to close.</summary>
    public IReadOnlyList<WindowBoundaryChange> BoundaryChanges { get; }
}
