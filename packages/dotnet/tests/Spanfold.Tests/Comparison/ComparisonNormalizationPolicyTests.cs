using Spanfold;

namespace Spanfold.Tests.Comparison;

public sealed class ComparisonNormalizationPolicyTests
{
    [Fact]
    public void DefaultPolicyUsesClosedHalfOpenProcessingPositionRanges()
    {
        var policy = ComparisonNormalizationPolicy.Default;

        Assert.Equal(TemporalAxis.ProcessingPosition, policy.TimeAxis);
        Assert.Equal(ComparisonOpenWindowPolicy.RequireClosed, policy.OpenWindowPolicy);
        Assert.Null(policy.OpenWindowHorizon);
        Assert.Equal(ComparisonNullTimestampPolicy.Reject, policy.NullTimestampPolicy);
    }

    [Fact]
    public void BuilderCanSelectEventTimeAndMissingTimestampPolicy()
    {
        var history = EventPipeline.For<DeviceSignal>().RecordWindows().Build().History;

        var plan = history.Compare("Event Time QA")
            .Target("provider-a", s => s.Source("provider-a"))
            .Against("provider-b", s => s.Source("provider-b"))
            .Within(s => s.Window("DeviceOffline"))
            .Normalize(n => n.OnEventTime().ExcludeMissingEventTime())
            .Using(c => c.Overlap())
            .Build();

        Assert.Equal(TemporalAxis.Timestamp, plan.Normalization.TimeAxis);
        Assert.Equal(ComparisonNullTimestampPolicy.Exclude, plan.Normalization.NullTimestampPolicy);
    }

    [Fact]
    public void BuilderCanSelectEventTimeScope()
    {
        var scope = new ComparisonScopeBuilder()
            .Window("DeviceOffline", TemporalAxis.Timestamp);

        Assert.Equal("DeviceOffline", scope.WindowName);
        Assert.Equal(TemporalAxis.Timestamp, scope.TimeAxis);
    }

    [Fact]
    public void BuilderCanClipOpenWindowsToHorizon()
    {
        var horizon = TemporalPoint.ForPosition(100);
        var history = EventPipeline.For<DeviceSignal>().RecordWindows().Build().History;

        var plan = history.Compare("Live QA")
            .Target("provider-a", s => s.Source("provider-a"))
            .Against("provider-b", s => s.Source("provider-b"))
            .Within(s => s.Window("DeviceOffline"))
            .Normalize(n => n.ClipOpenWindowsTo(horizon))
            .Using(c => c.Overlap())
            .Build();

        Assert.Equal(ComparisonOpenWindowPolicy.ClipToHorizon, plan.Normalization.OpenWindowPolicy);
        Assert.Equal(horizon, plan.Normalization.OpenWindowHorizon);
    }

    private sealed record DeviceSignal(string DeviceId, bool IsOnline);
}
