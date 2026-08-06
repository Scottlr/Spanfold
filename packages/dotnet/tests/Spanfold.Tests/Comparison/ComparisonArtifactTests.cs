using System.Globalization;

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
}
