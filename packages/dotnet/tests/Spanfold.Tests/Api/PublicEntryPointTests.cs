using Spanfold;

namespace Spanfold.Tests.Api;

public sealed class PublicEntryPointTests
{
    [Fact]
    public void ForReturnsBuilderForEventType()
    {
        var builder = EventPipeline.For<SampleEvent>();

        Assert.IsType<EventPipelineBuilder<SampleEvent>>(builder);
    }

    private sealed record SampleEvent;
}
