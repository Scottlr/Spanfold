using Spanfold;

namespace Spanfold.Tests.Comparison;

public sealed class SegmentAlignmentTests
{
    [Fact]
    public void AlignSplitsOverlappingWindowsIntoDeterministicSegments()
    {
        var target = new ClosedWindow("DeviceOffline", "device-1", StartPosition: 1, EndPosition: 5, Source: "provider-a");
        var against = new ClosedWindow("DeviceOffline", "device-1", StartPosition: 3, EndPosition: 7, Source: "provider-b");
        var prepared = Prepared(target, against);

        var aligned = prepared.Align();

        Assert.Collection(
            aligned.Segments,
            first =>
            {
                Assert.Equal(1, first.Range.Start.Position);
                Assert.Equal(3, first.Range.End!.Value.Position);
                Assert.Single(first.TargetRecordIds);
                Assert.Empty(first.AgainstRecordIds);
            },
            second =>
            {
                Assert.Equal(3, second.Range.Start.Position);
                Assert.Equal(5, second.Range.End!.Value.Position);
                Assert.Single(second.TargetRecordIds);
                Assert.Single(second.AgainstRecordIds);
            },
            third =>
            {
                Assert.Equal(5, third.Range.Start.Position);
                Assert.Equal(7, third.Range.End!.Value.Position);
                Assert.Empty(third.TargetRecordIds);
                Assert.Single(third.AgainstRecordIds);
            });
    }

    [Fact]
    public void TouchingWindowsCreateAdjacentNonOverlappingSegments()
    {
        var target = new ClosedWindow("DeviceOffline", "device-1", StartPosition: 1, EndPosition: 3, Source: "provider-a");
        var against = new ClosedWindow("DeviceOffline", "device-1", StartPosition: 3, EndPosition: 5, Source: "provider-b");
        var prepared = Prepared(target, against);

        var aligned = prepared.Align();

        Assert.Collection(
            aligned.Segments,
            first =>
            {
                Assert.Equal(1, first.Range.Start.Position);
                Assert.Equal(3, first.Range.End!.Value.Position);
                Assert.Single(first.TargetRecordIds);
                Assert.Empty(first.AgainstRecordIds);
            },
            second =>
            {
                Assert.Equal(3, second.Range.Start.Position);
                Assert.Equal(5, second.Range.End!.Value.Position);
                Assert.Empty(second.TargetRecordIds);
                Assert.Single(second.AgainstRecordIds);
            });
    }

    [Fact]
    public void AlignmentKeepsScopesSeparate()
    {
        var first = new ClosedWindow("DeviceOffline", "device-1", StartPosition: 1, EndPosition: 5, Source: "provider-a");
        var second = new ClosedWindow("DeviceOffline", "device-2", StartPosition: 3, EndPosition: 7, Source: "provider-b");
        var prepared = Prepared(first, second);

        var aligned = prepared.Align();

        Assert.Equal(2, aligned.Segments.Count);
        Assert.Contains(aligned.Segments, segment => Equals(segment.Key, "device-1"));
        Assert.Contains(aligned.Segments, segment => Equals(segment.Key, "device-2"));
    }

    private static PreparedComparison Prepared(ClosedWindow target, ClosedWindow against)
    {
        var plan = new ComparisonPlan(
            "Provider QA",
            ComparisonSelector.ForSource("provider-a"),
            [ComparisonSelector.ForSource("provider-b")],
            ComparisonScope.Window("DeviceOffline"),
            ComparisonNormalizationPolicy.Default,
            ["overlap"]
            );

        return new PreparedComparison(
            plan,
            [],
            [target, against],
            [],
            [
                new NormalizedWindowRecord(
                    target,
                    target.Id,
                    "provider-a",
                    ComparisonSide.Target,
                    TemporalRange.Closed(TemporalPoint.ForPosition(target.StartPosition), TemporalPoint.ForPosition(target.EndPosition!.Value))),
                new NormalizedWindowRecord(
                    against,
                    against.Id,
                    "provider-b",
                    ComparisonSide.Against,
                    TemporalRange.Closed(TemporalPoint.ForPosition(against.StartPosition), TemporalPoint.ForPosition(against.EndPosition!.Value)))
            ]);
    }
}
