using Spanfold;

namespace Spanfold.Tests.Recording;

public sealed class EventTimeWindowRecordingTests
{
    [Fact]
    public void WindowTimestampsComeFromOpeningAndClosingEvents()
    {
        var openedAt = new DateTimeOffset(2026, 4, 12, 10, 0, 0, TimeSpan.Zero);
        var closedAt = new DateTimeOffset(2026, 4, 12, 10, 5, 0, TimeSpan.Zero);
        var pipeline = EventPipeline
            .For<PriceTick>()
            .RecordWindows()
            .WithEventTime(tick => tick.Timestamp)
            .Window(
                "SelectionSuspension",
                key: tick => tick.SelectionId,
                isActive: tick => tick.Price == 0m)
            .Build();

        pipeline.Ingest(new PriceTick("selection-1", 0m, openedAt));
        pipeline.Ingest(new PriceTick("selection-1", 1.01m, closedAt));

        var window = Assert.Single(pipeline.History.ClosedWindows);
        Assert.Equal(openedAt, window.StartTime);
        Assert.Equal(closedAt, window.EndTime);
        Assert.Equal(1, window.StartPosition);
        Assert.Equal(2, window.EndPosition);
    }

    [Fact]
    public void WindowTimestampsAreEmptyWhenEventTimeIsNotConfigured()
    {
        var pipeline = EventPipeline
            .For<PriceTick>()
            .RecordWindows()
            .Window(
                "SelectionSuspension",
                key: tick => tick.SelectionId,
                isActive: tick => tick.Price == 0m)
            .Build();

        pipeline.Ingest(new PriceTick("selection-1", 0m, DateTimeOffset.UtcNow));
        pipeline.Ingest(new PriceTick("selection-1", 1.01m, DateTimeOffset.UtcNow));

        var window = Assert.Single(pipeline.History.ClosedWindows);
        Assert.Null(window.StartTime);
        Assert.Null(window.EndTime);
    }

    [Fact]
    public void ConfiguredTimestampClockFlowsThroughHistoryAndSnapshots()
    {
        var openedAt = new DateTimeOffset(2026, 4, 12, 10, 0, 0, TimeSpan.Zero);
        var closedAt = openedAt.AddMinutes(5);
        var pipeline = EventPipeline
            .For<PriceTick>()
            .RecordWindows()
            .WithEventTime(tick => tick.Timestamp, "exchange-clock")
            .Window(
                "SelectionSuspension",
                key: tick => tick.SelectionId,
                isActive: tick => tick.Price == 0m)
            .Build();

        pipeline.Ingest(new PriceTick("selection-1", 0m, openedAt));
        pipeline.Ingest(new PriceTick("selection-1", 1.01m, closedAt));

        var window = Assert.Single(pipeline.History.ClosedWindows);
        Assert.Equal("exchange-clock", window.TimestampClock);
        var snapshot = pipeline.History.SnapshotAt(
            TemporalPoint.ForTimestamp(closedAt, "exchange-clock"));
        Assert.Single(snapshot.Records);
    }

    [Fact]
    public void EventTimeSelectorIsRequired()
    {
        var builder = EventPipeline.For<PriceTick>();

        Assert.Throws<ArgumentNullException>(() => builder.WithEventTime(null!));
        Assert.Throws<ArgumentException>(() => builder.WithEventTime(tick => tick.Timestamp, " "));
    }

    private sealed record PriceTick(
        string SelectionId,
        decimal Price,
        DateTimeOffset Timestamp);
}
