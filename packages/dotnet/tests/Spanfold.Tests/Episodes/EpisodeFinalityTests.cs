using Spanfold;

namespace Spanfold.Tests.Episodes;

public sealed class EpisodeFinalityTests
{
    [Fact]
    public void HistoricalClosedEpisodeIsFinal()
    {
        var episode = Assert.Single(Form(History(new ClosedWindow(
            "State", "device-1", 0, 5, Source: "provider-a"))).Run().Episodes);

        Assert.Equal(ComparisonFinality.Final, episode.Finality);
        Assert.Equal(TemporalRangeEndStatus.Closed, episode.Envelope.EndStatus);
    }

    [Fact]
    public void OpenFragmentAtLiveHorizonIsProvisional()
    {
        var history = WindowHistory.FromRecords(
            [],
            [new OpenWindow("State", "device-1", 3, Source: "provider-a")]);

        var result = Form(history).RunLive(TemporalPoint.ForPosition(10));

        var episode = Assert.Single(result.Episodes);
        Assert.Equal(ComparisonFinality.Provisional, episode.Finality);
        Assert.Equal(TemporalRangeEndStatus.OpenAtHorizon, episode.Envelope.EndStatus);
        Assert.Equal(TemporalPoint.ForPosition(10), result.EvaluationHorizon);
        Assert.Equal(result.EvaluationHorizon, result.Plan.Normalization.OpenWindowHorizon);
    }

    [Fact]
    public void ClosedEpisodeSettlesStrictlyAfterToleranceBoundary()
    {
        var history = History(new ClosedWindow(
            "State", "device-1", 0, 5, Source: "provider-a"));

        var atBoundary = Form(history)
            .StitchGapsUpTo(2)
            .RunLive(TemporalPoint.ForPosition(7));
        var afterBoundary = Form(history)
            .StitchGapsUpTo(2)
            .RunLive(TemporalPoint.ForPosition(8));

        Assert.Equal(ComparisonFinality.Provisional, Assert.Single(atBoundary.Episodes).Finality);
        Assert.Equal(ComparisonFinality.Final, Assert.Single(afterBoundary.Episodes).Finality);
        Assert.Equal(TemporalRangeEndStatus.Closed, Assert.Single(atBoundary.Episodes).Envelope.EndStatus);
    }

    [Fact]
    public void KnownAtClipsActiveClosedRecordAndBecomesEvaluationHorizon()
    {
        var history = History(new ClosedWindow(
            "State", "device-1", 0, 10, Source: "provider-a"));

        var result = Form(history)
            .Normalize(normalization => normalization.KnownAtPosition(5))
            .Run();

        var episode = Assert.Single(result.Episodes);
        Assert.Equal(ComparisonFinality.Provisional, episode.Finality);
        Assert.Equal(5, episode.ActiveMagnitude);
        Assert.Equal(TemporalRangeEndStatus.OpenAtHorizon, episode.Envelope.EndStatus);
        Assert.Equal(TemporalPoint.ForPosition(5), result.EvaluationHorizon);
    }

    [Fact]
    public void KnownAtExcludesFutureRecords()
    {
        var history = History(
            new ClosedWindow("State", "device-1", 0, 2, Source: "provider-a"),
            new ClosedWindow("State", "device-1", 8, 10, Source: "provider-a"));

        var result = Form(history)
            .Normalize(normalization => normalization.KnownAtPosition(5))
            .Run();

        var episode = Assert.Single(result.Episodes);
        Assert.Single(episode.Fragments);
        Assert.Equal(0, episode.Envelope.Start.Position);
    }

    [Fact]
    public void EventTimePlanRejectsKnownAtPolicy()
    {
        var history = History(new ClosedWindow(
            "State",
            "device-1",
            0,
            1,
            Source: "provider-a",
            StartTime: DateTimeOffset.UnixEpoch,
            EndTime: DateTimeOffset.UnixEpoch.AddMinutes(1),
            TimestampClock: "exchange"));

        var builder = Form(history)
            .Within(scope => scope.Window("State", TemporalAxis.Timestamp))
            .Normalize(normalization => normalization.OnEventTime().KnownAtPosition(1));

        Assert.Throws<InvalidOperationException>(() => builder.Build());
    }

    [Fact]
    public void RunLiveRejectsConfiguredKnownAtOrOpenWindowHorizon()
    {
        var history = History(new ClosedWindow(
            "State", "device-1", 0, 5, Source: "provider-a"));
        var knownAt = Form(history)
            .Normalize(normalization => normalization.KnownAtPosition(5));
        var configuredHorizon = Form(history)
            .Normalize(normalization => normalization.ClipOpenWindowsTo(TemporalPoint.ForPosition(6)));

        Assert.Throws<InvalidOperationException>(() => knownAt.RunLive(TemporalPoint.ForPosition(6)));
        Assert.Throws<InvalidOperationException>(() => configuredHorizon.RunLive(TemporalPoint.ForPosition(7)));
    }

    private static EpisodeFormationBuilder Form(WindowHistory history)
    {
        return history.FormEpisodes("State episodes")
            .From(selector => selector.Source("provider-a"))
            .Within(scope => scope.Window("State"));
    }

    private static WindowHistory History(params ClosedWindow[] records)
    {
        return WindowHistory.FromRecords(records, []);
    }
}
