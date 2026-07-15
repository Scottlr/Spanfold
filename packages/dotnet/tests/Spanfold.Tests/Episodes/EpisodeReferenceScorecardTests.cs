using Spanfold;

namespace Spanfold.Tests.Episodes;

public sealed class EpisodeReferenceScorecardTests
{
    [Fact]
    public void OneToOneProducesPerfectReferenceScorecard()
    {
        var scorecard = Compare(Target(0, 5), Detection(1, 4)).Run().AsReference();

        AssertScorecard(scorecard, 1, 1, 0, 1, 1, 0, 1, 1, 1);
    }

    [Fact]
    public void MissedReferenceAndUnexpectedDetectionProduceDefinedZeroRates()
    {
        var scorecard = Compare(Target(0, 2), Detection(5, 7)).Run().AsReference();

        AssertScorecard(scorecard, 1, 0, 1, 1, 0, 1, 0, 0, 0);
    }

    [Fact]
    public void PartialDetectionUsesSideSpecificDenominatorsForF1()
    {
        var scorecard = Compare(
            Target(0, 2, "matched"),
            Detection(0, 2, "matched"),
            Target(10, 12, "missed"),
            Detection(20, 22, "unexpected")).Run().AsReference();

        AssertScorecard(scorecard, 2, 1, 1, 2, 1, 1, 0.5, 0.5, 0.5);
    }

    [Fact]
    public void SplitCountsOneReferenceAndEveryMatchedDetection()
    {
        var scorecard = Compare(
            Target(0, 10),
            Detection(0, 4),
            Detection(6, 10)).Run().AsReference();

        AssertScorecard(scorecard, 1, 1, 0, 2, 2, 0, 1, 1, 1);
    }

    [Fact]
    public void MergeCountsEveryDetectedReferenceAndOneDetection()
    {
        var scorecard = Compare(
            Target(0, 4),
            Target(6, 10),
            Detection(0, 10)).Run().AsReference();

        AssertScorecard(scorecard, 2, 2, 0, 1, 1, 0, 1, 1, 1);
    }

    [Fact]
    public void ComplexCountsEpisodesBySideRatherThanEdges()
    {
        var result = Compare(
            Target(0, 4),
            Target(6, 10),
            Detection(0, 6),
            Detection(7, 10)).Run();
        var beforeKinds = result.Relations.Select(relation => relation.Kind).ToArray();

        var scorecard = result.AsReference();

        AssertScorecard(scorecard, 2, 2, 0, 2, 2, 0, 1, 1, 1);
        Assert.Equal(beforeKinds, result.Relations.Select(relation => relation.Kind));
    }

    [Fact]
    public void EmptyReferenceLeavesRecallAndF1Undefined()
    {
        var scorecard = Compare(Detection(0, 2)).Run().AsReference();

        Assert.Equal(0, scorecard.ReferenceEpisodeCount);
        Assert.Equal(1, scorecard.DetectionEpisodeCount);
        Assert.Null(scorecard.Recall);
        Assert.Equal(0, scorecard.Precision);
        Assert.Null(scorecard.F1Score);
    }

    [Fact]
    public void EmptyDetectionLeavesPrecisionAndF1Undefined()
    {
        var scorecard = Compare(Target(0, 2)).Run().AsReference();

        Assert.Equal(1, scorecard.ReferenceEpisodeCount);
        Assert.Equal(0, scorecard.DetectionEpisodeCount);
        Assert.Equal(0, scorecard.Recall);
        Assert.Null(scorecard.Precision);
        Assert.Null(scorecard.F1Score);
    }

    [Fact]
    public void EmptyBothSidesLeavesEveryRateUndefined()
    {
        var scorecard = Compare().Run().AsReference();

        Assert.Equal(0, scorecard.ReferenceEpisodeCount);
        Assert.Equal(0, scorecard.DetectionEpisodeCount);
        Assert.Null(scorecard.Recall);
        Assert.Null(scorecard.Precision);
        Assert.Null(scorecard.F1Score);
    }

    private static void AssertScorecard(
        EpisodeReferenceScorecard scorecard,
        int references,
        int detectedReferences,
        int missedReferences,
        int detections,
        int matchedDetections,
        int unexpectedDetections,
        double? recall,
        double? precision,
        double? f1)
    {
        Assert.Equal(references, scorecard.ReferenceEpisodeCount);
        Assert.Equal(detectedReferences, scorecard.DetectedReferenceEpisodeCount);
        Assert.Equal(missedReferences, scorecard.MissedReferenceEpisodeCount);
        Assert.Equal(detections, scorecard.DetectionEpisodeCount);
        Assert.Equal(matchedDetections, scorecard.MatchedDetectionEpisodeCount);
        Assert.Equal(unexpectedDetections, scorecard.UnexpectedDetectionEpisodeCount);
        Assert.Equal(recall, scorecard.Recall);
        Assert.Equal(precision, scorecard.Precision);
        Assert.Equal(f1, scorecard.F1Score);
    }

    private static EpisodeComparisonBuilder Compare(params ClosedWindow[] records)
    {
        return WindowHistory.FromRecords(records, [])
            .CompareEpisodes("Detector evaluation")
            .Target("reference", selector => selector.Source("reference"))
            .Against("detection", selector => selector.Source("detection"))
            .Within(scope => scope.Window("State"));
    }

    private static ClosedWindow Target(long start, long end, string key = "device-1")
    {
        return new ClosedWindow("State", key, start, end, Source: "reference");
    }

    private static ClosedWindow Detection(long start, long end, string key = "device-1")
    {
        return new ClosedWindow("State", key, start, end, Source: "detection");
    }
}
