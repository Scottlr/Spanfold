using System.Runtime.CompilerServices;
using System.Text.Json;

using ArtifactFixtureRunner = Spanfold.Artifacts.Comparison.ComparisonFixtureRunner;
using TestingFixtureRunner = Spanfold.Testing.ContractFixtureRunner;

namespace Spanfold.Tests.Comparison;

public sealed class ContractFixtureTests
{
    [Fact]
    public void ContractFixturesExecutePlans()
    {
        foreach (var fixturePath in FixturePaths())
        {
            using var fixture = JsonDocument.Parse(File.ReadAllText(fixturePath));
            var result = ArtifactFixtureRunner.Run(fixture.RootElement);

            Assert.NotNull(result);
            Assert.Equal(
                fixture.RootElement.GetProperty("expected").GetProperty("isValid").GetBoolean(),
                result.IsValid);
        }
    }

    [Fact]
    public void FixtureResultsMatchExpectedJson()
    {
        foreach (var fixturePath in FixturePaths())
        {
            using var fixture = JsonDocument.Parse(File.ReadAllText(fixturePath));
            using var exported = JsonDocument.Parse(ArtifactFixtureRunner.Run(fixture.RootElement).ExportJson());

            AssertExpectedDiagnostics(fixture.RootElement, exported.RootElement);
            AssertExpectedSummaries(fixture.RootElement, exported.RootElement);
            AssertExpectedRows(fixture.RootElement, exported.RootElement);
        }
    }

    [Fact]
    public void InvalidFixturePlansProduceExpectedDiagnostics()
    {
        var invalidFixturePaths = FixturePaths()
            .Where(path =>
            {
                using var fixture = JsonDocument.Parse(File.ReadAllText(path));
                return !fixture.RootElement.GetProperty("expected").GetProperty("isValid").GetBoolean();
            })
            .ToArray();

        Assert.NotEmpty(invalidFixturePaths);
        foreach (var fixturePath in invalidFixturePaths)
        {
            using var fixture = JsonDocument.Parse(File.ReadAllText(fixturePath));
            using var exported = JsonDocument.Parse(ArtifactFixtureRunner.Run(fixture.RootElement).ExportJson());

            AssertExpectedDiagnostics(fixture.RootElement, exported.RootElement);
        }
    }

    [Fact]
    public void TestingFacade_ValidFixture_MatchesArtifactsOwner()
    {
        using var fixture = JsonDocument.Parse(File.ReadAllText(FixturePaths()[0]));

        var ownerResult = ArtifactFixtureRunner.Run(fixture.RootElement).ExportJson();
        var facadeResult = TestingFixtureRunner.Run(fixture.RootElement).ExportJson();

        Assert.Equal(ownerResult, facadeResult);
    }

    private static void AssertExpectedDiagnostics(JsonElement fixture, JsonElement exported)
    {
        var expected = fixture.GetProperty("expected").GetProperty("diagnostics").EnumerateArray()
            .Select(static code => code.GetString())
            .ToArray();
        var actual = exported.GetProperty("diagnostics").EnumerateArray()
            .Select(static diagnostic => diagnostic.GetProperty("code").GetString())
            .ToArray();

        Assert.Equal(expected, actual);
    }

    private static void AssertExpectedSummaries(JsonElement fixture, JsonElement exported)
    {
        var expected = fixture.GetProperty("expected").GetProperty("summaries").EnumerateArray().ToArray();
        var actual = exported.GetProperty("comparatorSummaries").EnumerateArray().ToArray();

        Assert.Equal(expected.Length, actual.Length);
        for (var i = 0; i < expected.Length; i++)
        {
            Assert.Equal(expected[i].GetProperty("comparatorName").GetString(), actual[i].GetProperty("comparatorName").GetString());
            Assert.Equal(expected[i].GetProperty("rowCount").GetInt32(), actual[i].GetProperty("rowCount").GetInt32());
        }
    }

    private static void AssertExpectedRows(JsonElement fixture, JsonElement exported)
    {
        var expectedRows = fixture.GetProperty("expected").GetProperty("rows");
        var actualRows = exported.GetProperty("rows");

        AssertRangeRows(expectedRows, actualRows, "overlap");
        AssertRangeRows(expectedRows, actualRows, "residual");
    }

    private static void AssertRangeRows(JsonElement expectedRows, JsonElement actualRows, string rowType)
    {
        if (!expectedRows.TryGetProperty(rowType, out var expected))
        {
            return;
        }

        var actual = actualRows.GetProperty(rowType).EnumerateArray().ToArray();
        var expectedArray = expected.EnumerateArray().ToArray();
        Assert.Equal(expectedArray.Length, actual.Length);

        for (var i = 0; i < expectedArray.Length; i++)
        {
            var expectedRow = expectedArray[i];
            var actualRow = actual[i];
            Assert.Equal(expectedRow.GetProperty("start").GetInt64(), actualRow.GetProperty("range").GetProperty("start").GetProperty("position").GetInt64());
            Assert.Equal(expectedRow.GetProperty("end").GetInt64(), actualRow.GetProperty("range").GetProperty("end").GetProperty("position").GetInt64());

            if (expectedRow.TryGetProperty("targetRecordCount", out var targetCount))
            {
                Assert.Equal(targetCount.GetInt32(), actualRow.GetProperty("targetRecordIds").GetArrayLength());
            }

            if (expectedRow.TryGetProperty("againstRecordCount", out var againstCount))
            {
                Assert.Equal(againstCount.GetInt32(), actualRow.GetProperty("againstRecordIds").GetArrayLength());
            }
        }
    }

    private static string[] FixturePaths([CallerFilePath] string callerFilePath = "")
    {
        return Directory.GetFiles(
                Path.Combine(Path.GetDirectoryName(callerFilePath)!, "Fixtures"),
                "*.json")
            .OrderBy(static path => path, StringComparer.Ordinal)
            .ToArray();
    }
}
