using Spanfold.Testing;

namespace Spanfold.Tests.Comparison;

public sealed class ComparisonRevisionTests
{
    [Fact]
    public void RevisionIncludesRowsCoverageAndAssessmentChanges()
    {
        var previous = CreateResult(5);
        var current = CreateResult(9);
        var specification = AssessmentSpecification.Create(
            "coverage",
            rules => rules.MinimumCoverage(0.8));

        var revision = ComparisonRevision.Between(
            previous,
            current,
            previous.Assess(specification),
            current.Assess(specification));

        Assert.NotEmpty(revision.Rows);
        var coverage = Assert.Single(revision.CoverageSummaries);
        Assert.Equal(0.5, coverage.PreviousCoverageRatio);
        Assert.Equal(0.9, coverage.CurrentCoverageRatio);
        var assessment = Assert.Single(revision.AssessmentViolations);
        Assert.Equal(ComparisonRevisionKind.Retracted, assessment.Kind);
    }

    private static ComparisonResult CreateResult(long coveredUntil)
    {
        return new WindowHistoryFixtureBuilder()
            .AddClosedWindow("Offline", "device-1", 0, 10, source: "target")
            .AddClosedWindow("Offline", "device-1", 0, coveredUntil, source: "against")
            .Build()
            .Compare("coverage")
            .Target("target", selector => selector.Source("target"))
            .Against("against", selector => selector.Source("against"))
            .Within(scope => scope.Window("Offline"))
            .Using(comparators => comparators.Coverage())
            .Run();
    }
}
