using Spanfold.Testing;

namespace Spanfold.Tests.Comparison;

public sealed class ComparisonRevisionTests
{
    public static TheoryData<ComparisonPlanDifference> SemanticPlanDifferences =>
        new(Enum.GetValues<ComparisonPlanDifference>());

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

    [Fact]
    public void Between_SameSemanticPlanAtDifferentRunHorizons_ReturnsRevision()
    {
        var builder = new WindowHistoryFixtureBuilder()
            .AddOpenWindow("Offline", "device-1", 0, source: "target")
            .AddOpenWindow("Offline", "device-1", 2, source: "against")
            .Build()
            .Compare("Live overlap")
            .Target("target", selector => selector.Source("target"))
            .Against("against", selector => selector.Source("against"))
            .Within(scope => scope.Window("Offline"))
            .Using(comparators => comparators.Overlap());
        var previous = builder.RunLive(TemporalPoint.ForPosition(5));
        var current = builder.RunLive(TemporalPoint.ForPosition(9));

        var revision = ComparisonRevision.Between(previous, current);

        Assert.NotEmpty(revision.Rows);
    }

    [Theory]
    [MemberData(nameof(SemanticPlanDifferences))]
    public void Between_DifferentSemanticPlansWithSameRows_ThrowsArgumentException(
        ComparisonPlanDifference difference)
    {
        var (previousPlan, currentPlan) = CreateDifferentPlans(difference);
        var previous = CreateResult(previousPlan);
        var current = CreateResult(currentPlan);

        var exception = Assert.Throws<ArgumentException>(() =>
            ComparisonRevision.Between(previous, current));

        Assert.Equal("current", exception.ParamName);
        Assert.StartsWith(
            "Comparison revisions require snapshots produced by compatible comparison plans.",
            exception.Message);
    }

    [Fact]
    public void Between_RuntimeSelectorsSharingPredicate_AreCompatible()
    {
        Func<WindowRecord, bool> predicate = static window => window.Source is not null;
        var selector = ComparisonSelector.RuntimeOnly("runtime", "has a source", predicate);
        var previous = CreateResult(CreatePlan(selector));
        var current = CreateResult(CreatePlan(selector));

        var revision = ComparisonRevision.Between(previous, current);

        Assert.Empty(revision.Rows);
    }

    [Fact]
    public void Between_RuntimeSelectorsWithDifferentPredicates_ThrowsArgumentException()
    {
        var previousSelector = ComparisonSelector.RuntimeOnly(
            "runtime",
            "same display description",
            static window => window.Source is not null);
        var currentSelector = ComparisonSelector.RuntimeOnly(
            "runtime",
            "same display description",
            static window => window.Source is not null);
        var previous = CreateResult(CreatePlan(previousSelector));
        var current = CreateResult(CreatePlan(currentSelector));

        var exception = Assert.Throws<ArgumentException>(() =>
            ComparisonRevision.Between(previous, current));

        Assert.Equal("current", exception.ParamName);
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

    private static ComparisonResult CreateResult(ComparisonPlan plan)
    {
        var row = new ComparisonRowFinality(
            new ComparisonRowReference(ComparisonRowKind.Overlap, "same-row"),
            ComparisonFinality.Final,
            "Stable test row.");
        return new ComparisonResult(plan, [], rowFinalities: [row]);
    }

    private static (ComparisonPlan Previous, ComparisonPlan Current) CreateDifferentPlans(
        ComparisonPlanDifference difference)
    {
        var previousTarget = ComparisonSelector.ForSource("target");
        var currentTarget = difference == ComparisonPlanDifference.Selector
            ? ComparisonSelector.ForSources(["target"])
            : previousTarget;
        var previousScope = ComparisonScope.Window("Offline");
        var currentScope = difference == ComparisonPlanDifference.Scope
            ? ComparisonScope.All()
            : previousScope;
        var previousNormalization = ComparisonNormalizationPolicy.Default;
        var currentNormalization = difference == ComparisonPlanDifference.Normalization
            ? previousNormalization with
            {
                OpenWindowPolicy = ComparisonOpenWindowPolicy.ClipToHorizon,
                OpenWindowHorizon = TemporalPoint.ForPosition(10)
            }
            : previousNormalization;
        var previousComparators = difference == ComparisonPlanDifference.Tolerance
            ? new[] { "lead-lag:Opened:ProcessingPosition:1" }
            : ["overlap"];
        var currentComparators = difference switch
        {
            ComparisonPlanDifference.Comparator => ["coverage"],
            ComparisonPlanDifference.Tolerance => ["lead-lag:Opened:ProcessingPosition:2"],
            _ => previousComparators
        };

        return (
            CreatePlan(previousTarget, previousScope, previousNormalization, previousComparators),
            CreatePlan(currentTarget, currentScope, currentNormalization, currentComparators));
    }

    private static ComparisonPlan CreatePlan(ComparisonSelector target)
    {
        return CreatePlan(
            target,
            ComparisonScope.Window("Offline"),
            ComparisonNormalizationPolicy.Default,
            ["overlap"]);
    }

    private static ComparisonPlan CreatePlan(
        ComparisonSelector target,
        ComparisonScope scope,
        ComparisonNormalizationPolicy normalization,
        IEnumerable<string> comparators)
    {
        return new ComparisonPlan(
            "Shared display name",
            target,
            [ComparisonSelector.ForSource("against")],
            scope,
            normalization,
            comparators);
    }

    public enum ComparisonPlanDifference
    {
        Selector,
        Scope,
        Normalization,
        Comparator,
        Tolerance
    }
}
