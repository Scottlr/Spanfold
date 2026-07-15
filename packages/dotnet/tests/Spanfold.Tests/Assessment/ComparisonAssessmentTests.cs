using Spanfold.Testing;

namespace Spanfold.Tests.Assessment;

public sealed class ComparisonAssessmentTests
{
    [Fact]
    public void CoverageAndResidualRulesUseAuthoritativeResultData()
    {
        var result = CreateCoverageResult();
        var specification = AssessmentSpecification.Create(
            "provider-parity",
            rules => rules
                .MinimumCoverage(0.9)
                .MaximumResidualMagnitude(1, AssessmentAggregation.Total));

        var assessment = result.Assess(specification);

        Assert.False(assessment.Passed);
        Assert.Collection(
            assessment.Violations,
            violation =>
            {
                Assert.Equal("coverage.below-minimum", violation.Code);
                Assert.Equal(0.8, violation.Actual);
                Assert.NotEmpty(violation.Evidence);
            },
            violation =>
            {
                Assert.Equal("residual.total-above-maximum", violation.Code);
                Assert.Equal(2d, violation.Actual);
                Assert.Single(violation.Evidence);
            });
    }

    [Fact]
    public void FinalRowRuleLinksEveryProvisionalRow()
    {
        var history = new WindowHistoryFixtureBuilder()
            .AddOpenWindow("DeviceOffline", "device-1", 1, source: "provider-a")
            .AddOpenWindow("DeviceOffline", "device-1", 1, source: "provider-b")
            .Build();
        var result = history.Compare("live")
            .Target("provider-a", selector => selector.Source("provider-a"))
            .Against("provider-b", selector => selector.Source("provider-b"))
            .Within(scope => scope.Window("DeviceOffline"))
            .Using(comparators => comparators.Overlap())
            .RunLive(TemporalPoint.ForPosition(5));
        var specification = AssessmentSpecification.Create(
            "final-only",
            rules => rules.RequireFinalRows());

        var assessment = result.Assess(specification);

        var violation = Assert.Single(assessment.Violations);
        Assert.Equal("row.provisional", violation.Code);
        Assert.Equal(result.RowFinalities[0].Reference, Assert.Single(violation.Evidence));
    }

    [Fact]
    public void SuitePassesOnlyWhenEverySpecificationPasses()
    {
        var result = CreateCoverageResult();
        var suite = new AssessmentSuite(
            "release",
            [
                AssessmentSpecification.Create("lenient", rules => rules.MinimumCoverage(0.75)),
                AssessmentSpecification.Create("strict", rules => rules.MinimumCoverage(0.95))
            ]);

        var evaluated = suite.Evaluate(result);

        Assert.False(evaluated.Passed);
        Assert.True(evaluated.Assessments[0].Passed);
        Assert.False(evaluated.Assessments[1].Passed);
    }

    private static ComparisonResult CreateCoverageResult()
    {
        var history = new WindowHistoryFixtureBuilder()
            .AddClosedWindow("DeviceOffline", "device-1", 0, 10, source: "provider-a")
            .AddClosedWindow("DeviceOffline", "device-1", 0, 8, source: "provider-b")
            .Build();
        return history.Compare("coverage")
            .Target("provider-a", selector => selector.Source("provider-a"))
            .Against("provider-b", selector => selector.Source("provider-b"))
            .Within(scope => scope.Window("DeviceOffline"))
            .Using(comparators => comparators.Coverage().Residual())
            .Run();
    }
}
