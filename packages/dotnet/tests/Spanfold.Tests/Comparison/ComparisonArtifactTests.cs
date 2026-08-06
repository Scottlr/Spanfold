using System.Globalization;

using Spanfold.Testing;

namespace Spanfold.Tests.Comparison;

public sealed class ComparisonArtifactTests
{
    public static TheoryData<ComparisonFinality> DefinedFinalities =>
        new(Enum.GetValues<ComparisonFinality>());

    [Theory]
    [MemberData(nameof(DefinedFinalities))]
    public void Parse_DefinedRowFinality_Parses(ComparisonFinality finality)
    {
        var json = CreateArtifactJson(finality.ToString());

        var artifact = ComparisonArtifact.Parse(json);

        Assert.Equal(finality, Assert.Single(artifact.RowMetadata).Finality);
    }

    [Theory]
    [MemberData(nameof(DefinedFinalities))]
    public void Parse_DefinedNumericRowFinality_Parses(ComparisonFinality finality)
    {
        var value = Convert.ToInt32(finality, CultureInfo.InvariantCulture)
            .ToString(CultureInfo.InvariantCulture);
        var json = CreateArtifactJson(value);

        var artifact = ComparisonArtifact.Parse(json);

        Assert.Equal(finality, Assert.Single(artifact.RowMetadata).Finality);
    }

    [Theory]
    [InlineData("2")]
    [InlineData("-1")]
    [InlineData("not-a-finality")]
    public void Parse_UndefinedOrMalformedRowFinality_ThrowsInvalidDataException(string finality)
    {
        var json = CreateArtifactJson(finality);

        var exception = Assert.Throws<InvalidDataException>(() => ComparisonArtifact.Parse(json));

        Assert.Equal(
            "The comparison artifact contains malformed row finality metadata.",
            exception.Message);
    }

    [Fact]
    public void Revision_LegacyArtifactsWithoutIdentity_PreservesCompatibleDiff()
    {
        var previous = ComparisonArtifact.Parse(CreateArtifactJson("Provisional"));
        var current = ComparisonArtifact.Parse(CreateArtifactJson("Final"));

        var revision = ComparisonArtifactRevision.Between(previous, current);

        Assert.Equal(ComparisonRevisionKind.Revised, Assert.Single(revision.Rows).Kind);
    }

    [Fact]
    public void Revision_SameSemanticPlanAtDifferentRunHorizons_ReturnsChanges()
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
        var previous = ComparisonArtifact.Parse(
            builder.RunLive(TemporalPoint.ForPosition(5)).ExportJson());
        var current = ComparisonArtifact.Parse(
            builder.RunLive(TemporalPoint.ForPosition(9)).ExportJson());

        var revision = ComparisonArtifactRevision.Between(previous, current);

        Assert.NotEmpty(revision.Rows);
    }

    [Theory]
    [MemberData(nameof(ComparisonRevisionTests.SemanticPlanDifferences), MemberType = typeof(ComparisonRevisionTests))]
    public void Revision_DifferentSemanticPlansWithSameRows_ThrowsArgumentException(
        ComparisonRevisionTests.ComparisonPlanDifference difference)
    {
        var (previousResult, currentResult) = CreateDifferentResults(difference);
        var previous = ComparisonArtifact.Parse(ExportWithSameRowMetadata(previousResult));
        var current = ComparisonArtifact.Parse(ExportWithSameRowMetadata(currentResult));

        var exception = Assert.Throws<ArgumentException>(() =>
            ComparisonArtifactRevision.Between(previous, current));

        Assert.Equal("current", exception.ParamName);
        Assert.StartsWith(
            "Comparison artifact revisions require artifacts produced by compatible comparison plans.",
            exception.Message);
    }

    private static string CreateArtifactJson(string finality)
    {
        return $$"""
            {
              "schema": "spanfold.comparison.result",
              "schemaVersion": 0,
              "plan": {
                "name": "Artifact finality test"
              },
              "isValid": true,
              "rowFinalities": [
                {
                  "rowType": "overlap",
                  "rowId": "overlap:test",
                  "finality": "{{finality}}",
                  "version": 1
                }
              ]
            }
            """;
    }

    private static (ComparisonResult Previous, ComparisonResult Current) CreateDifferentResults(
        ComparisonRevisionTests.ComparisonPlanDifference difference)
    {
        var previousTarget = ComparisonSelector.ForSource("target");
        var currentTarget = difference == ComparisonRevisionTests.ComparisonPlanDifference.Selector
            ? ComparisonSelector.ForSources(["target"])
            : previousTarget;
        var previousScope = ComparisonScope.Window("Offline");
        var currentScope = difference == ComparisonRevisionTests.ComparisonPlanDifference.Scope
            ? ComparisonScope.All()
            : previousScope;
        var previousNormalization = ComparisonNormalizationPolicy.Default;
        var currentNormalization = difference == ComparisonRevisionTests.ComparisonPlanDifference.Normalization
            ? previousNormalization with
            {
                OpenWindowPolicy = ComparisonOpenWindowPolicy.ClipToHorizon,
                OpenWindowHorizon = TemporalPoint.ForPosition(10)
            }
            : previousNormalization;
        var previousComparators = difference == ComparisonRevisionTests.ComparisonPlanDifference.Tolerance
            ? new[] { "lead-lag:Opened:ProcessingPosition:1" }
            : ["overlap"];
        var currentComparators = difference switch
        {
            ComparisonRevisionTests.ComparisonPlanDifference.Comparator => ["coverage"],
            ComparisonRevisionTests.ComparisonPlanDifference.Tolerance => ["lead-lag:Opened:ProcessingPosition:2"],
            _ => previousComparators
        };

        return (
            CreateResult(previousTarget, previousScope, previousNormalization, previousComparators),
            CreateResult(currentTarget, currentScope, currentNormalization, currentComparators));
    }

    private static ComparisonResult CreateResult(
        ComparisonSelector target,
        ComparisonScope scope,
        ComparisonNormalizationPolicy normalization,
        IEnumerable<string> comparators)
    {
        var plan = new ComparisonPlan(
            "Shared display name",
            target,
            [ComparisonSelector.ForSource("against")],
            scope,
            normalization,
            comparators);
        return new ComparisonResult(plan, []);
    }

    private static string ExportWithSameRowMetadata(ComparisonResult result)
    {
        const string rowFinalities = """
            "rowFinalities": [
                {
                  "rowType": "overlap",
                  "rowId": "same-row",
                  "finality": "Final",
                  "reason": "Stable test row.",
                  "version": 1,
                  "supersedesRowId": null
                }
              ]
            """;

        return result.ExportJson().Replace(
            "\"rowFinalities\": []",
            rowFinalities,
            StringComparison.Ordinal);
    }
}
