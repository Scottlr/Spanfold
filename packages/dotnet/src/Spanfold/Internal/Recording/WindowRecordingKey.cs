using Spanfold.Internal.Keys;

namespace Spanfold.Internal.Recording;

internal sealed record WindowRecordingKey(
    string WindowName,
    object Key,
    object? Source,
    object? Partition,
    SegmentContext? SegmentContext = null)
{
    // Kept for the fixture builder's reflection-based compatibility path.
    public WindowRecordingKey(
        string windowName,
        object key,
        object? source,
        object? partition,
        string segmentContext)
        : this(windowName, key, source, partition, (SegmentContext?)null)
    {
    }
}
