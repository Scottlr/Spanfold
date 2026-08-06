using Spanfold;

namespace Spanfold.Tests.Comparison;

public sealed class ComparisonSelectorTests
{
    [Fact]
    public void SourceSelectorMatchesExpectedWindows()
    {
        var selector = ComparisonSelector.ForSource("provider-a");

        Assert.True(selector.Matches(ClosedWindow(source: "provider-a")));
        Assert.False(selector.Matches(ClosedWindow(source: "provider-b")));
        Assert.True(selector.IsSerializable);
    }

    [Fact]
    public void WindowNameSelectorMatchesExpectedWindows()
    {
        var selector = ComparisonSelector.ForWindowName("DeviceOffline");

        Assert.True(selector.Matches(ClosedWindow(windowName: "DeviceOffline")));
        Assert.False(selector.Matches(ClosedWindow(windowName: "DeviceDegraded")));
    }

    [Fact]
    public void CombinedAndSelectorRequiresBothSelectors()
    {
        var selector = ComparisonSelector
            .ForWindowName("DeviceOffline")
            .And(ComparisonSelector.ForSource("provider-a"));

        Assert.True(selector.Matches(ClosedWindow(windowName: "DeviceOffline", source: "provider-a")));
        Assert.False(selector.Matches(ClosedWindow(windowName: "DeviceOffline", source: "provider-b")));
        Assert.False(selector.Matches(ClosedWindow(windowName: "DeviceDegraded", source: "provider-a")));
        Assert.True(selector.IsSerializable);
    }

    [Fact]
    public void CombinedOrSelectorAllowsEitherSelector()
    {
        var selector = ComparisonSelector
            .ForSource("provider-a")
            .Or(ComparisonSelector.ForSource("provider-b"));

        Assert.True(selector.Matches(ClosedWindow(source: "provider-a")));
        Assert.True(selector.Matches(ClosedWindow(source: "provider-b")));
        Assert.False(selector.Matches(ClosedWindow(source: "provider-c")));
    }

    [Theory]
    [InlineData(true)]
    [InlineData(false)]
    public void CohortAndNarrowingSelectorPreservesCohortSemantics(bool cohortFirst)
    {
        var cohort = ComparisonSelector.ForCohortSources(
            ["provider-a", "provider-b"],
            CohortActivity.AtLeast(2));
        var narrowing = ComparisonSelector.ForWindowName("DeviceOffline");

        var selector = cohortFirst
            ? cohort.And(narrowing)
            : narrowing.And(cohort);

        Assert.Equal(cohort.CohortActivity, selector.CohortActivity);
        Assert.Equal(cohort.CohortSources, selector.CohortSources);
        Assert.True(selector.Matches(ClosedWindow(windowName: "DeviceOffline", source: "provider-a")));
        Assert.False(selector.Matches(ClosedWindow(windowName: "DeviceDegraded", source: "provider-a")));
    }

    [Fact]
    public void CohortCompositionRejectsAmbiguousOperators()
    {
        var first = ComparisonSelector.ForCohortSources(
            ["provider-a", "provider-b"],
            CohortActivity.AtLeast(2));
        var second = ComparisonSelector.ForCohortSources(
            ["provider-c", "provider-d"],
            CohortActivity.Any());
        var narrowing = ComparisonSelector.ForWindowName("DeviceOffline");

        Assert.Throws<InvalidOperationException>(() => first.Or(narrowing));
        Assert.Throws<InvalidOperationException>(() => narrowing.Or(first));
        Assert.Throws<InvalidOperationException>(() => first.And(second));
    }

    [Fact]
    public void RuntimeSelectorMatchesByDelegateAndIsNotSerializable()
    {
        var selector = ComparisonSelector.RuntimeOnly(
            "long-window",
            "window duration is longer than ten positions",
            window => window.EndPosition - window.StartPosition > 10);

        Assert.True(selector.Matches(ClosedWindow(start: 0, end: 12)));
        Assert.False(selector.Matches(ClosedWindow(start: 0, end: 5)));
        Assert.False(selector.IsSerializable);
    }

    [Fact]
    public void PositionRangeSelectorUsesHalfOpenStartPositionRange()
    {
        var selector = ComparisonSelector.ForPositionRange(10, 20);

        Assert.False(selector.Matches(ClosedWindow(start: 9)));
        Assert.True(selector.Matches(ClosedWindow(start: 10)));
        Assert.True(selector.Matches(ClosedWindow(start: 19)));
        Assert.False(selector.Matches(ClosedWindow(start: 20)));
    }

    [Fact]
    public void TimeRangeSelectorUsesHalfOpenStartTimeRange()
    {
        var start = new DateTimeOffset(2026, 4, 18, 10, 0, 0, TimeSpan.Zero);
        var end = start.AddMinutes(10);
        var selector = ComparisonSelector.ForTimeRange(start, end);

        Assert.True(selector.Matches(ClosedWindow(startTime: start)));
        Assert.True(selector.Matches(ClosedWindow(startTime: start.AddMinutes(5))));
        Assert.False(selector.Matches(ClosedWindow(startTime: end)));
        Assert.False(selector.Matches(ClosedWindow(startTime: null)));
    }

    private static ClosedWindow ClosedWindow(
        string windowName = "DeviceOffline",
        object? source = null,
        long start = 10,
        long end = 20,
        DateTimeOffset? startTime = null)
    {
        return new ClosedWindow(
            windowName,
            "device-1",
            start,
            end,
            source,
            Partition: null,
            startTime,
            EndTime: null);
    }
}
