using Spanfold;

namespace Spanfold.Tests.Runtime;

public sealed class AtomicIngestionTests
{
    [Fact]
    public void LaterSelectorFailureRollsBackEarlierRuntimeAndPosition()
    {
        var pipeline = EventPipeline
            .For<ObservedEvent>()
            .RecordWindows()
            .Window("First", key: item => item.Key, isActive: item => item.IsActive)
            .Window(
                "Second",
                key: item => item.Key,
                isActive: item => item.ShouldThrow
                    ? throw new InvalidOperationException("selector failed")
                    : false)
            .Build();

        Assert.Throws<InvalidOperationException>(() =>
            pipeline.Ingest(new ObservedEvent("item-1", IsActive: true, ShouldThrow: true)));
        Assert.Empty(pipeline.History.Windows);

        var result = pipeline.Ingest(
            new ObservedEvent("item-1", IsActive: true, ShouldThrow: false));

        var opened = Assert.Single(result.Emissions);
        Assert.Equal("First", opened.WindowName);
        Assert.Equal(1, Assert.Single(pipeline.History.OpenWindows).StartPosition);
    }

    private sealed record ObservedEvent(string Key, bool IsActive, bool ShouldThrow);
}
