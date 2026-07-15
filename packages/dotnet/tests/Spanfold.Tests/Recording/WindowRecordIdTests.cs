using Spanfold;

namespace Spanfold.Tests.Recording;

public sealed class WindowRecordIdTests
{
    [Fact]
    public void SameClosedWindowDataProducesSameId()
    {
        var first = new ClosedWindow(
            "DeviceOffline",
            "device-1",
            StartPosition: 10,
            EndPosition: 20,
            Source: "provider-a",
            Partition: "partition-1",
            StartTime: new DateTimeOffset(2026, 4, 18, 10, 0, 0, TimeSpan.Zero),
            EndTime: new DateTimeOffset(2026, 4, 18, 10, 1, 0, TimeSpan.Zero));
        var second = new ClosedWindow(
            "DeviceOffline",
            "device-1",
            StartPosition: 10,
            EndPosition: 20,
            Source: "provider-a",
            Partition: "partition-1",
            StartTime: new DateTimeOffset(2026, 4, 18, 10, 0, 0, TimeSpan.Zero),
            EndTime: new DateTimeOffset(2026, 4, 18, 10, 1, 0, TimeSpan.Zero));

        Assert.Equal(first.Id, second.Id);
    }

    [Fact]
    public void DifferentPositionProducesDifferentId()
    {
        var first = new ClosedWindow("DeviceOffline", "device-1", StartPosition: 10, EndPosition: 20);
        var second = new ClosedWindow("DeviceOffline", "device-1", StartPosition: 10, EndPosition: 21);

        Assert.NotEqual(first.Id, second.Id);
    }

    [Fact]
    public void OpenAndClosedWindowsDoNotCollide()
    {
        var open = new OpenWindow("DeviceOffline", "device-1", StartPosition: 10);
        var closed = new ClosedWindow("DeviceOffline", "device-1", StartPosition: 10, EndPosition: 20);

        Assert.NotEqual(open.Id, closed.Id);
    }

    [Fact]
    public void SameReplayProducesSameRecordedIds()
    {
        var first = BuildOfflineHistory();
        var second = BuildOfflineHistory();

        Assert.Equal(
            Assert.Single(first.ClosedWindows).Id,
            Assert.Single(second.ClosedWindows).Id);
    }

    [Fact]
    public void IdStringIsStableHex()
    {
        var id = new OpenWindow("DeviceOffline", "device-1", StartPosition: 10).Id.ToString();

        Assert.Equal(64, id.Length);
        Assert.Matches("^[0-9a-f]+$", id);
    }

    [Fact]
    public void UnsupportedIdentityValuesAreRejected()
    {
        Assert.Throws<ArgumentException>(() =>
            new OpenWindow("DeviceOffline", new UnsupportedIdentity(), StartPosition: 10));
    }

    [Fact]
    public void ReadingIdDoesNotChangeRecordEqualityOrHashCode()
    {
        var first = new OpenWindow("DeviceOffline", "device-1", StartPosition: 10);
        var second = new OpenWindow("DeviceOffline", "device-1", StartPosition: 10);
        var before = first.GetHashCode();

        _ = first.Id;

        Assert.Equal(second, first);
        Assert.Equal(before, first.GetHashCode());
    }

    [Fact]
    public void InvalidWindowRangesAreRejectedAtConstruction()
    {
        Assert.Throws<ArgumentException>(() =>
            new ClosedWindow("DeviceOffline", "device-1", StartPosition: 5, EndPosition: 4));

        Assert.Throws<ArgumentException>(() =>
            new ClosedWindow(
                "DeviceOffline",
                "device-1",
                StartPosition: 1,
                EndPosition: 5,
                StartTime: DateTimeOffset.UnixEpoch,
                EndTime: DateTimeOffset.UnixEpoch.AddSeconds(-1)));
    }

    private static WindowHistory BuildOfflineHistory()
    {
        var pipeline = EventPipeline
            .For<DeviceSignal>()
            .RecordWindows()
            .TrackWindow(
                "DeviceOffline",
                signal => signal.DeviceId,
                signal => !signal.IsOnline);

        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: false), source: "provider-a");
        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: true), source: "provider-a");

        return pipeline.History;
    }

    private sealed record DeviceSignal(string DeviceId, bool IsOnline);

    private sealed class UnsupportedIdentity
    {
        public override string ToString() => "same text for every instance";
    }
}
