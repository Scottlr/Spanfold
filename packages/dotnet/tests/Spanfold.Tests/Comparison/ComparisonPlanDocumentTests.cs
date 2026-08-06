using System.Text.Json.Nodes;

using Spanfold.Artifacts.Comparison;
using Spanfold.Testing;

namespace Spanfold.Tests.Comparison;

public sealed class ComparisonPlanDocumentTests
{
    [Fact]
    public void PortableRoundTrip_NestedSelectors_CompileAndRun()
    {
        var target = ComparisonSelector.ForSource("provider-a")
            .And(ComparisonSelector.ForKey(42))
            .Or(ComparisonSelector.ForSource("fallback"))
            .WithName("target-provider");
        var against = ComparisonSelector.ForSource("provider-b")
            .Or(ComparisonSelector.ForSource("provider-c"))
            .And(ComparisonSelector.ForKey(42))
            .WithName("comparison-providers");
        var plan = new ComparisonPlan(
            "Portable nested plan",
            target,
            [against],
            ComparisonScope.Window("DeviceOffline").Tag("verified", true),
            ComparisonNormalizationPolicy.Default,
            ["overlap"]);
        var history = new WindowHistoryFixtureBuilder()
            .AddClosedWindow(
                "DeviceOffline",
                42,
                1,
                5,
                source: "provider-a",
                tags: [new WindowTag("verified", true)])
            .AddClosedWindow(
                "DeviceOffline",
                42,
                3,
                7,
                source: "provider-b",
                tags: [new WindowTag("verified", true)])
            .Build();

        var json = ComparisonPlanDocument.FromPlan(plan).WriteJson();
        var parsed = ComparisonPlanDocument.Parse(json);
        var compiled = parsed.Compile();
        var result = Run(history, compiled);

        Assert.Equal(json, parsed.WriteJson());
        Assert.Equal("target-provider", compiled.Target!.Value.Name);
        Assert.Equal("comparison-providers", Assert.Single(compiled.Against).Name);
        Assert.Single(result.OverlapRows);
    }

    [Fact]
    public void PortableRoundTrip_CohortSelector_PreservesActivity()
    {
        var cohort = ComparisonSelector.ForCohortSources(
                ["provider-b", "provider-c"],
                CohortActivity.AtLeast(2))
            .WithName("comparison-cohort");
        var plan = new ComparisonPlan(
            "Portable cohort plan",
            ComparisonSelector.ForSource("provider-a"),
            [cohort],
            ComparisonScope.All(),
            ComparisonNormalizationPolicy.Default,
            ["overlap"]);

        var compiled = ComparisonPlanDocument.Parse(
                ComparisonPlanDocument.FromPlan(plan).WriteJson())
            .Compile();

        var compiledCohort = Assert.Single(compiled.Against);
        Assert.Equal("comparison-cohort", compiledCohort.Name);
        Assert.Equal("at-least", compiledCohort.CohortActivity!.Name);
        Assert.Equal(2, compiledCohort.CohortActivity.Count);
        Assert.Equal(["provider-b", "provider-c"], compiledCohort.CohortSources);
    }

    [Theory]
    [InlineData(true)]
    [InlineData(false)]
    public void PortableRoundTrip_ComposedCohort_PreservesActivityAndPopulation(bool cohortFirst)
    {
        var cohort = ComparisonSelector.ForCohortSources(
            ["provider-b", "provider-c"],
            CohortActivity.AtLeast(2));
        var narrowing = ComparisonSelector.ForWindowName("DeviceOffline");
        var composed = cohortFirst
            ? cohort.And(narrowing)
            : narrowing.And(cohort);
        var plan = new ComparisonPlan(
            "Portable composed cohort plan",
            ComparisonSelector.ForSource("provider-a"),
            [composed],
            ComparisonScope.All(),
            ComparisonNormalizationPolicy.Default,
            ["overlap"]);

        var compiled = ComparisonPlanDocument.Parse(
                ComparisonPlanDocument.FromPlan(plan).WriteJson())
            .Compile();

        var compiledCohort = Assert.Single(compiled.Against);
        Assert.Equal("at-least", compiledCohort.CohortActivity!.Name);
        Assert.Equal(2, compiledCohort.CohortActivity.Count);
        Assert.Equal(["provider-b", "provider-c"], compiledCohort.CohortSources);
    }

    [Fact]
    public void Compile_PortableCohortOrComposition_FailsClosed()
    {
        var root = CreateComposedCohortDocument();
        root["against"]![0]!["descriptor"]!["kind"] = "or";

        var exception = Assert.Throws<InvalidDataException>(() =>
            ComparisonPlanDocument.Parse(root.ToJsonString()).Compile());

        Assert.Contains("invalid cohort composition", exception.Message);
    }

    [Fact]
    public void Compile_PortableTwoCohortComposition_FailsClosed()
    {
        var root = CreateComposedCohortDocument();
        var children = root["against"]![0]!["descriptor"]!["children"]!.AsArray();
        children[1] = children[0]!.DeepClone();

        var exception = Assert.Throws<InvalidDataException>(() =>
            ComparisonPlanDocument.Parse(root.ToJsonString()).Compile());

        Assert.Contains("invalid cohort composition", exception.Message);
    }

    [Fact]
    public void Parse_UnsupportedSchemaVersion_FailsClosed()
    {
        var json = ComparisonPlanDocument.FromPlan(CreateSimplePlan()).WriteJson()
            .Replace("\"schemaVersion\": 0", "\"schemaVersion\": 1", StringComparison.Ordinal);

        var exception = Assert.Throws<InvalidDataException>(() => ComparisonPlanDocument.Parse(json));

        Assert.Contains("schemaVersion", exception.Message);
    }

    [Fact]
    public void Compile_UnknownSelectorKind_FailsClosed()
    {
        var json = ComparisonPlanDocument.FromPlan(CreateSimplePlan()).WriteJson()
            .Replace("\"kind\": \"source\"", "\"kind\": \"future-selector\"", StringComparison.Ordinal);
        var document = ComparisonPlanDocument.Parse(json);

        var exception = Assert.Throws<InvalidDataException>(() => document.Compile());

        Assert.Contains("future-selector", exception.Message);
    }

    private static ComparisonPlan CreateSimplePlan()
    {
        return new ComparisonPlan(
            "Portable plan",
            ComparisonSelector.ForSource("provider-a"),
            [ComparisonSelector.ForSource("provider-b")],
            ComparisonScope.All(),
            ComparisonNormalizationPolicy.Default,
            ["overlap"]);
    }

    private static JsonObject CreateComposedCohortDocument()
    {
        var cohort = ComparisonSelector.ForCohortSources(
            ["provider-b", "provider-c"],
            CohortActivity.AtLeast(2));
        var plan = new ComparisonPlan(
            "Portable composed cohort plan",
            ComparisonSelector.ForSource("provider-a"),
            [cohort.And(ComparisonSelector.ForWindowName("DeviceOffline"))],
            ComparisonScope.All(),
            ComparisonNormalizationPolicy.Default,
            ["overlap"]);
        var json = ComparisonPlanDocument.FromPlan(plan).WriteJson();

        return JsonNode.Parse(json)!.AsObject();
    }

    private static ComparisonResult Run(WindowHistory history, ComparisonPlan plan)
    {
        var builder = history.Compare(plan.Name)
            .Target(plan.Target!.Value.Name, _ => plan.Target.Value);

        for (var index = 0; index < plan.Against.Count; index++)
        {
            var selector = plan.Against[index];
            builder.Against(selector.Name, _ => selector);
        }

        builder = builder
            .Within(_ => plan.Scope!)
            .Using(_ => BuildComparators(plan.Comparators));

        return builder.Run();
    }

    private static ComparisonComparatorBuilder BuildComparators(IReadOnlyList<string> comparators)
    {
        var builder = new ComparisonComparatorBuilder();
        for (var index = 0; index < comparators.Count; index++)
        {
            builder.Declaration(comparators[index]);
        }

        return builder;
    }
}
