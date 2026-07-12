using Spanfold.Testing;

namespace Spanfold.Tests.TestingPackage;

public sealed class SpanfoldTestingPackageConsumerTests
{
    [Fact]
    public void ConsumerCanAssertComparisonResult()
    {
        var history = new WindowHistoryFixtureBuilder()
            .AddClosedWindow("DeviceOffline", "device-1", 1, 5, source: "provider-a")
            .AddClosedWindow("DeviceOffline", "device-1", 3, 7, source: "provider-b")
            .Build();

        var result = history.Compare("Provider QA")
            .Target("provider-a", selector => selector.Source("provider-a"))
            .Against("provider-b", selector => selector.Source("provider-b"))
            .Within(scope => scope.Window("DeviceOffline"))
            .Using(comparators => comparators.Overlap())
            .Run();

        SpanfoldAssert.IsValid(result);
        SpanfoldAssert.HasNoDiagnostics(result);
        SpanfoldAssert.HasRowCount(result, "overlap", 1);
    }

    [Fact]
    public void SnapshotHelperNormalizesRecordIds()
    {
        const string firstId = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        const string secondId = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

        SpanfoldSnapshot.AssertEqual(
            "ids: <record-id:1> <record-id:2>\n",
            "ids: " + firstId + " " + secondId,
            normalizeUnlabeledHex: true);
    }

    [Fact]
    public void SnapshotHelperPreservesUnrelatedHexTokensByDefault()
    {
        const string first = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        const string second = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

        Assert.Throws<SpanfoldAssertionException>(() =>
            SpanfoldSnapshot.AssertEqual("hash: " + first, "hash: " + second));
    }

    [Fact]
    public void VirtualClockProducesDeterministicHorizons()
    {
        var clock = new VirtualComparisonClock(initialPosition: 5);

        Assert.Equal(TemporalPoint.ForPosition(5), clock.Horizon);
        Assert.Equal(TemporalPoint.ForPosition(8), clock.AdvanceBy(3));
        Assert.Throws<ArgumentOutOfRangeException>(() => clock.AdvanceTo(7));
    }

    [Fact]
    public void FixtureBuilderCanCreateOpenWindowHistories()
    {
        var history = new WindowHistoryFixtureBuilder()
            .AddOpenWindow("DeviceOffline", "device-1", 1, source: "provider-a")
            .Build();

        var result = history.Compare("Live fixture QA")
            .Target("provider-a", selector => selector.Source("provider-a"))
            .Against("provider-b", selector => selector.Source("provider-b"))
            .Within(scope => scope.Window("DeviceOffline"))
            .Using(comparators => comparators.Residual())
            .RunLive(TemporalPoint.ForPosition(10));

        Assert.Single(history.OpenWindows);
        Assert.Single(result.ResidualRows);
        Assert.True(result.HasProvisionalRows());
    }

    [Fact]
    public void FixtureBuilderCanCreateSegmentedWindowHistories()
    {
        var history = new WindowHistoryFixtureBuilder()
            .AddClosedWindow(
                "DeviceOffline",
                "device-1",
                1,
                5,
                source: "source-a",
                segments: [new WindowSegment("lifecycle", "Incident")])
            .Build();

        var result = history.Compare("Segmented fixture QA")
            .Target("source-a", selector => selector.Source("source-a"))
            .Against("source-b", selector => selector.Source("source-b"))
            .Within(scope => scope.Window("DeviceOffline").Segment("lifecycle", "Incident"))
            .Using(comparators => comparators.Residual())
            .Run();

        Assert.Single(result.ResidualRows);
    }

    [Fact]
    public void FixtureBuilderCanCreateSegmentedTaggedWindowHistories()
    {
        var history = new WindowHistoryFixtureBuilder()
            .AddClosedWindow(
                "DeviceOffline",
                "device-1",
                1,
                5,
                window => window
                    .Source("source-a")
                    .Segment("lifecycle", "Incident")
                    .Segment("stage", "Escalated", parentName: "lifecycle")
                    .Tag("fleet", "critical"))
            .Build();

        var result = history.Compare("Fixture segment tag QA")
            .Target("source-a", selector => selector.Source("source-a"))
            .Against("source-b", selector => selector.Source("source-b"))
            .Within(scope => scope
                .Window("DeviceOffline")
                .Segment("lifecycle", "Incident")
                .Segment("stage", "Escalated")
                .Tag("fleet", "critical"))
            .Using(comparators => comparators.Residual())
            .Run();

        var window = Assert.Single(history.Windows);
        Assert.Equal("critical", Assert.Single(window.Tags).Value);
        Assert.Equal("lifecycle", window.Segments[1].ParentName);
        Assert.Single(result.ResidualRows);
    }

    [Fact]
    public void FixtureBuilderCanKeepTwoOpenSegmentsForSameKey()
    {
        var history = new WindowHistoryFixtureBuilder()
            .AddOpenWindow(
                "DeviceOffline",
                "device-1",
                1,
                source: "source-a",
                segments: [new WindowSegment("lifecycle", "Normal")])
            .AddOpenWindow(
                "DeviceOffline",
                "device-1",
                2,
                source: "source-a",
                segments: [new WindowSegment("lifecycle", "Incident")])
            .Build();

        Assert.Equal(2, history.OpenWindows.Count);
    }

    [Fact]
    public void FixtureBuilderCanCreateOpenSegmentedTaggedWindows()
    {
        var history = new WindowHistoryFixtureBuilder()
            .AddOpenWindow(
                "DeviceOffline",
                "device-1",
                1,
                window => window
                    .Source("source-a")
                    .Segment("lifecycle", "Incident")
                    .Tag("fleet", "critical"))
            .Build();

        var result = history.Compare("Live segmented fixture QA")
            .Target("source-a", selector => selector.Source("source-a"))
            .Against("source-b", selector => selector.Source("source-b"))
            .Within(scope => scope
                .Window("DeviceOffline")
                .Segment("lifecycle", "Incident")
                .Tag("fleet", "critical"))
            .Using(comparators => comparators.Residual())
            .RunLive(TemporalPoint.ForPosition(10));

        Assert.Single(history.OpenWindows);
        Assert.Single(result.ResidualRows);
        Assert.True(result.HasProvisionalRows());
    }

    [Fact]
    public void PackageAssertionsThrowFrameworkNeutralException()
    {
        var result = new ComparisonResult(
            new ComparisonPlan(
                "Invalid",
                ComparisonSelector.ForSource("provider-a"),
                [ComparisonSelector.ForSource("provider-b")],
                ComparisonScope.Window("DeviceOffline"),
                ComparisonNormalizationPolicy.Default,
                ["overlap"],
                ComparisonOutputOptions.Default),
            [new ComparisonPlanDiagnostic(
                ComparisonPlanValidationCode.Unknown,
                "Invalid",
                "result",
                ComparisonPlanDiagnosticSeverity.Error)]);

        Assert.Throws<SpanfoldAssertionException>(() => SpanfoldAssert.IsValid(result));
    }
}
