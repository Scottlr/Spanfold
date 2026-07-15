using Spanfold;

namespace Spanfold.Tests.Runtime;

public sealed class SegmentHierarchyValidationTests
{
    [Fact]
    public void NestedSegmentDefinitionIsValid()
    {
        var pipeline = EventPipeline
            .For<DeviceStateChanged>()
            .RecordWindows()
            .TrackWindow("DeviceOffline", window => window
                .Key(update => update.DeviceId)
                .ActiveWhen(update => update.IsOffline)
                .Segment("lifecycle", lifecycle => lifecycle
                    .Value(update => update.Lifecycle)
                    .Child("stage", stage => stage.Value(update => update.Stage))));

        pipeline.Ingest(new DeviceStateChanged("device-1", IsOffline: true, "Incident", "Escalated"));

        var open = Assert.Single(pipeline.History.OpenWindows);
        Assert.Collection(
            open.Segments,
            segment =>
            {
                Assert.Equal("lifecycle", segment.Name);
                Assert.Null(segment.ParentName);
            },
            segment =>
            {
                Assert.Equal("stage", segment.Name);
                Assert.Equal("lifecycle", segment.ParentName);
            });
    }

    [Fact]
    public void DuplicateSegmentNameAcrossHierarchyIsRejected()
    {
        var exception = Assert.Throws<InvalidOperationException>(() =>
            EventPipeline
                .For<DeviceStateChanged>()
                .TrackWindow("DeviceOffline", window => window
                    .Key(update => update.DeviceId)
                    .ActiveWhen(update => update.IsOffline)
                    .Segment("lifecycle", lifecycle => lifecycle
                        .Value(update => update.Lifecycle)
                        .Child("lifecycle", stage => stage.Value(update => update.Stage)))));

        Assert.Contains("has already been configured", exception.Message);
    }

    [Fact]
    public void DuplicateSiblingSegmentNameIsRejected()
    {
        var exception = Assert.Throws<InvalidOperationException>(() =>
            EventPipeline
                .For<DeviceStateChanged>()
                .TrackWindow("DeviceOffline", window => window
                    .Key(update => update.DeviceId)
                    .ActiveWhen(update => update.IsOffline)
                    .Segment("lifecycle", lifecycle => lifecycle
                        .Value(update => update.Lifecycle)
                        .Child("stage", stage => stage.Value(update => update.Stage))
                        .Child("stage", stage => stage.Value(update => update.Stage)))));

        Assert.Contains("has already been configured", exception.Message);
    }

    [Fact]
    public void ChildSegmentWithoutValueSelectorIsRejected()
    {
        var exception = Assert.Throws<InvalidOperationException>(() =>
            EventPipeline
                .For<DeviceStateChanged>()
                .TrackWindow("DeviceOffline", window => window
                    .Key(update => update.DeviceId)
                    .ActiveWhen(update => update.IsOffline)
                    .Segment("lifecycle", lifecycle => lifecycle
                        .Value(update => update.Lifecycle)
                        .Child("stage", _ => { }))));

        Assert.Contains("must configure a value selector", exception.Message);
    }

    private sealed record DeviceStateChanged(
        string DeviceId,
        bool IsOffline,
        string Lifecycle,
        string Stage);
}
