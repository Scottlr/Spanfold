using Spanfold;

namespace Spanfold.Tests.Temporal;

public sealed class TemporalRangeTests
{
    [Fact]
    public void ClosedPositionRangeExposesLength()
    {
        var range = TemporalRange.Closed(
            TemporalPoint.ForPosition(10),
            TemporalPoint.ForPosition(15));

        Assert.Equal(TemporalRangeEndStatus.Closed, range.EndStatus);
        Assert.Equal(TemporalAxis.ProcessingPosition, range.Axis);
        Assert.True(range.HasEnd);
        Assert.True(range.IsClosed);
        Assert.Equal(5, range.GetPositionLength());
    }

    [Fact]
    public void ExtremePositionRangeSaturatesLengthInsteadOfOverflowing()
    {
        var range = TemporalRange.Closed(
            TemporalPoint.ForPosition(long.MinValue),
            TemporalPoint.ForPosition(long.MaxValue));

        Assert.Equal(long.MaxValue, range.GetPositionLength());
    }

    [Fact]
    public void ClosedTimestampRangeExposesDuration()
    {
        var range = TemporalRange.Closed(
            TemporalPoint.ForTimestamp(new DateTimeOffset(2026, 4, 18, 12, 0, 0, TimeSpan.Zero), "event-time"),
            TemporalPoint.ForTimestamp(new DateTimeOffset(2026, 4, 18, 12, 5, 0, TimeSpan.Zero), "event-time"));

        Assert.Equal(TimeSpan.FromMinutes(5), range.GetTimeDuration());
    }

    [Fact]
    public void TouchingHalfOpenRangesDoNotOverlap()
    {
        var first = TemporalRange.Closed(
            TemporalPoint.ForPosition(10),
            TemporalPoint.ForPosition(20));
        var second = TemporalRange.Closed(
            TemporalPoint.ForPosition(20),
            TemporalPoint.ForPosition(30));

        Assert.False(first.Overlaps(second));
        Assert.False(second.Overlaps(first));
    }

    [Fact]
    public void OverlappingRangesReturnTrue()
    {
        var first = TemporalRange.Closed(
            TemporalPoint.ForPosition(10),
            TemporalPoint.ForPosition(21));
        var second = TemporalRange.Closed(
            TemporalPoint.ForPosition(20),
            TemporalPoint.ForPosition(30));

        Assert.True(first.Overlaps(second));
        Assert.True(second.Overlaps(first));
    }

    [Fact]
    public void EmptyRangeIsAllowedAndDoesNotContainStart()
    {
        var point = TemporalPoint.ForPosition(10);
        var range = TemporalRange.Closed(point, point);

        Assert.True(range.IsEmpty);
        Assert.Equal(0, range.GetPositionLength());
        Assert.False(range.Contains(point));
    }

    [Fact]
    public void OpenRangeHasNoDurationUntilClipped()
    {
        var range = TemporalRange.Open(TemporalPoint.ForPosition(10));

        Assert.False(range.HasEnd);
        Assert.Equal(TemporalRangeEndStatus.UnknownEnd, range.EndStatus);
        Assert.Throws<InvalidOperationException>(() => range.GetPositionLength());
        Assert.Throws<InvalidOperationException>(() => range.IsEmpty);
    }

    [Fact]
    public void EffectiveEndAllowsOpenRangeDuration()
    {
        var range = TemporalRange.WithEffectiveEnd(
            TemporalPoint.ForPosition(10),
            TemporalPoint.ForPosition(25),
            TemporalRangeEndStatus.OpenAtHorizon);

        Assert.True(range.HasEnd);
        Assert.False(range.IsClosed);
        Assert.Equal(15, range.GetPositionLength());
    }

    [Fact]
    public void MixedAxisBoundsAreRejected()
    {
        Assert.Throws<InvalidOperationException>(() => TemporalRange.Closed(
            TemporalPoint.ForPosition(10),
            TemporalPoint.ForTimestamp(new DateTimeOffset(2026, 4, 18, 12, 0, 0, TimeSpan.Zero))));
    }

    [Fact]
    public void TimestampBoundsWithDifferentClocksAreRejected()
    {
        Assert.Throws<InvalidOperationException>(() => TemporalRange.Closed(
            TemporalPoint.ForTimestamp(new DateTimeOffset(2026, 4, 18, 12, 0, 0, TimeSpan.Zero), "provider"),
            TemporalPoint.ForTimestamp(new DateTimeOffset(2026, 4, 18, 12, 1, 0, TimeSpan.Zero), "received")));
    }

    [Fact]
    public void EndBeforeStartIsRejected()
    {
        Assert.Throws<ArgumentException>(() => TemporalRange.Closed(
            TemporalPoint.ForPosition(20),
            TemporalPoint.ForPosition(10)));
    }

    [Fact]
    public void ContainsUsesHalfOpenSemantics()
    {
        var range = TemporalRange.Closed(
            TemporalPoint.ForPosition(10),
            TemporalPoint.ForPosition(20));

        Assert.True(range.Contains(TemporalPoint.ForPosition(10)));
        Assert.True(range.Contains(TemporalPoint.ForPosition(19)));
        Assert.False(range.Contains(TemporalPoint.ForPosition(20)));
    }
}
