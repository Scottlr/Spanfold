using System.Runtime.CompilerServices;
using System.Text.Json;

using Spanfold.Cli;

namespace Spanfold.Tests.Cli;

public sealed class SpanfoldCliTests
{
    [Fact]
    public void ValidatePlanAcceptsValidFixture()
    {
        var (exitCode, output, error) = Run("validate-plan", FixturePath("basic-overlap.json"));

        Assert.Equal(0, exitCode);
        Assert.Contains("\"isValid\":true", output);
        Assert.Equal(string.Empty, error);
    }

    [Fact]
    public void CompareRunsFixtureAndWritesJson()
    {
        var (exitCode, output, error) = Run("compare", FixturePath("basic-overlap.json"), "--format", "json");

        Assert.Equal(0, exitCode);
        Assert.Contains("\"schema\": \"spanfold.comparison.result\"", output);
        Assert.Contains("\"overlap\"", output);
        Assert.Equal(string.Empty, error);
    }

    [Fact]
    public void CompareRunsFixtureAndWritesLlmContext()
    {
        var (exitCode, output, error) = Run("compare", FixturePath("basic-overlap.json"), "--format", "llm-context");

        Assert.Equal(0, exitCode);
        Assert.Equal(string.Empty, error);
        using var document = JsonDocument.Parse(output);
        Assert.Equal("spanfold.comparison.llm-context", document.RootElement.GetProperty("schema").GetString());
        Assert.Equal("spanfold.comparison.result", document.RootElement.GetProperty("fullResult").GetProperty("schema").GetString());
    }

    [Fact]
    public void CompareRunsSegmentedFixtureWithCohortPlan()
    {
        var (exitCode, output, error) = Run("compare", FixturePath("cohort-any-residual.json"), "--format", "json");

        Assert.Equal(0, exitCode);
        Assert.Equal(string.Empty, error);
        using var document = JsonDocument.Parse(output);
        var against = Assert.Single(document.RootElement.GetProperty("plan").GetProperty("against").EnumerateArray());
        Assert.Equal("cohort", against.GetProperty("name").GetString());
        Assert.Equal("any", against.GetProperty("cohort").GetProperty("activity").GetString());
    }

    [Fact]
    public void CompareRunsFixtureWithSegmentAndTagScope()
    {
        var (exitCode, output, error) = Run("compare", FixturePath("segmented-residual.json"), "--format", "json");

        Assert.Equal(0, exitCode);
        Assert.Equal(string.Empty, error);
        using var document = JsonDocument.Parse(output);
        var scope = document.RootElement.GetProperty("plan").GetProperty("scope");
        Assert.Equal(2, scope.GetProperty("segmentFilters").GetArrayLength());
        Assert.Single(scope.GetProperty("tagFilters").EnumerateArray());
    }

    [Fact]
    public void CompareHonorsEveryAgainstSourceInFixturePlan()
    {
        var fixturePath = TempFixturePath();
        try
        {
            File.WriteAllText(fixturePath, """
                {
                  "schema": "spanfold.contract-fixture",
                  "schemaVersion": 1,
                  "name": "multi-source-overlap",
                  "windows": [
                    {
                      "windowName": "DeviceOffline",
                      "key": "device-1",
                      "source": "provider-a",
                      "startPosition": 1,
                      "endPosition": 5
                    },
                    {
                      "windowName": "DeviceOffline",
                      "key": "device-1",
                      "source": "provider-b",
                      "startPosition": 3,
                      "endPosition": 7
                    },
                    {
                      "windowName": "DeviceOffline",
                      "key": "device-1",
                      "source": "provider-c",
                      "startPosition": 2,
                      "endPosition": 4
                    }
                  ],
                  "plan": {
                    "name": "Provider QA",
                    "targetSource": "provider-a",
                    "againstSources": [ "provider-b", "provider-c" ],
                    "scopeWindow": "DeviceOffline",
                    "comparators": [ "overlap" ],
                    "strict": false
                  }
                }
                """);

            var (exitCode, output, error) = Run("compare", fixturePath, "--format", "json");

            Assert.Equal(0, exitCode);
            Assert.Equal(string.Empty, error);
            using var document = JsonDocument.Parse(output);
            var against = document.RootElement
                .GetProperty("plan")
                .GetProperty("against");
            Assert.Equal(2, against.GetArrayLength());
            Assert.Equal("source:provider-b", against[0].GetProperty("name").GetString());
            Assert.Equal("source:provider-c", against[1].GetProperty("name").GetString());
        }
        finally
        {
            File.Delete(fixturePath);
        }
    }

    [Fact]
    public void ComparePreservesAdvancedComparatorDeclarationsFromFixturePlan()
    {
        var fixturePath = TempFixturePath();
        try
        {
            File.WriteAllText(fixturePath, """
                {
                  "schema": "spanfold.contract-fixture",
                  "schemaVersion": 1,
                  "name": "advanced-comparators",
                  "windows": [
                    {
                      "windowName": "DeviceOffline",
                      "key": "device-1",
                      "source": "provider-a",
                      "startPosition": 1,
                      "endPosition": 5
                    },
                    {
                      "windowName": "DeviceOffline",
                      "key": "device-1",
                      "source": "provider-b",
                      "startPosition": 1,
                      "endPosition": 5
                    }
                  ],
                  "plan": {
                    "name": "Advanced Provider QA",
                    "targetSource": "provider-a",
                    "againstSources": [ "provider-b" ],
                    "scopeWindow": "DeviceOffline",
                    "comparators": [ "containment", "lead-lag:Start:ProcessingPosition:5" ],
                    "strict": false
                  }
                }
                """);

            var (exitCode, output, error) = Run("compare", fixturePath, "--format", "json");

            Assert.Equal(0, exitCode);
            Assert.Equal(string.Empty, error);
            using var document = JsonDocument.Parse(output);
            var comparators = document.RootElement
                .GetProperty("plan")
                .GetProperty("comparators");
            Assert.Equal("containment", comparators[0].GetString());
            Assert.Equal("lead-lag:Start:ProcessingPosition:5", comparators[1].GetString());
        }
        finally
        {
            File.Delete(fixturePath);
        }
    }

    [Fact]
    public void CompareRunsLiveFixtureWhenPlanHasHorizon()
    {
        var fixturePath = TempFixturePath();
        try
        {
            File.WriteAllText(fixturePath, """
                {
                  "schema": "spanfold.contract-fixture",
                  "schemaVersion": 1,
                  "name": "live-open-window",
                  "windows": [
                    {
                      "windowName": "DeviceOffline",
                      "key": "device-1",
                      "source": "provider-a",
                      "startPosition": 1,
                      "endPosition": null
                    }
                  ],
                  "plan": {
                    "name": "Live Provider QA",
                    "targetSource": "provider-a",
                    "againstSources": [ "provider-b" ],
                    "scopeWindow": "DeviceOffline",
                    "comparators": [ "residual" ],
                    "strict": false,
                    "liveHorizonPosition": 10
                  }
                }
                """);

            var (exitCode, output, error) = Run("compare", fixturePath, "--format", "json");

            Assert.Equal(0, exitCode);
            Assert.Equal(string.Empty, error);
            using var document = JsonDocument.Parse(output);
            Assert.Equal(10, document.RootElement
                .GetProperty("evaluationHorizon")
                .GetProperty("position")
                .GetInt64());
            var finality = Assert.Single(document.RootElement.GetProperty("rowFinalities").EnumerateArray());
            Assert.Equal("Provisional", finality.GetProperty("finality").GetString());
        }
        finally
        {
            File.Delete(fixturePath);
        }
    }

    [Fact]
    public void ExplainRunsFixtureAndWritesMarkdown()
    {
        var (exitCode, output, error) = Run("explain", FixturePath("basic-overlap.json"));

        Assert.Equal(0, exitCode);
        Assert.Contains("# Comparison Explain: Provider QA", output);
        Assert.Equal(string.Empty, error);
    }

    [Fact]
    public void AuditWritesArtifactBundleAndManifest()
    {
        var directory = TempDirectoryPath();
        try
        {
            var (exitCode, output, error) = Run("audit", FixturePath("basic-overlap.json"), "--out", directory);

            Assert.Equal(0, exitCode);
            Assert.Equal(string.Empty, error);
            Assert.True(File.Exists(Path.Combine(directory, "comparison.json")));
            Assert.True(File.Exists(Path.Combine(directory, "comparison.md")));
            Assert.True(File.Exists(Path.Combine(directory, "comparison.html")));
            Assert.True(File.Exists(Path.Combine(directory, "comparison.llm.json")));
            Assert.True(File.Exists(Path.Combine(directory, "manifest.json")));
            Assert.Equal(output, File.ReadAllText(Path.Combine(directory, "manifest.json")));

            using var document = JsonDocument.Parse(output);
            Assert.Equal("spanfold.audit.bundle", document.RootElement.GetProperty("schema").GetString());
            Assert.Equal("audit-bundle", document.RootElement.GetProperty("artifact").GetString());
            Assert.True(document.RootElement.GetProperty("isValid").GetBoolean());
            Assert.Equal(1, document.RootElement.GetProperty("rowCounts").GetProperty("overlap").GetInt32());
            Assert.Equal("comparison.llm.json", document.RootElement
                .GetProperty("artifacts")
                .GetProperty("llmContext")
                .GetString());
        }
        finally
        {
            if (Directory.Exists(directory))
            {
                Directory.Delete(directory, recursive: true);
            }
        }
    }

    [Fact]
    public void AuditWindowsWritesBundleFromJsonLinesWithoutFixturePlan()
    {
        var windowsPath = TempFixturePath();
        var directory = TempDirectoryPath();
        try
        {
            File.WriteAllText(windowsPath, """
                {"key":"device-1","source":"provider-a","startPosition":1,"endPosition":5}
                {"key":"device-1","source":"provider-b","startPosition":3,"endPosition":7}
                """);

            var (exitCode, output, error) = Run(
                "audit-windows",
                windowsPath,
                "--window",
                "DeviceOffline",
                "--target",
                "provider-a",
                "--against",
                "provider-b",
                "--out",
                directory);

            Assert.Equal(0, exitCode);
            Assert.Equal(string.Empty, error);
            Assert.True(File.Exists(Path.Combine(directory, "comparison.llm.json")));
            using var document = JsonDocument.Parse(output);
            Assert.Equal("spanfold.audit.bundle", document.RootElement.GetProperty("schema").GetString());
            Assert.Equal("Spanfold Window Audit", document.RootElement.GetProperty("planName").GetString());
            Assert.Equal(1, document.RootElement.GetProperty("rowCounts").GetProperty("overlap").GetInt32());
            Assert.Equal(1, document.RootElement.GetProperty("rowCounts").GetProperty("residual").GetInt32());
        }
        finally
        {
            File.Delete(windowsPath);
            if (Directory.Exists(directory))
            {
                Directory.Delete(directory, recursive: true);
            }
        }
    }

    [Fact]
    public void InvalidInputReturnsNonZeroExitCodeAndDiagnosticOutput()
    {
        var (exitCode, output, error) = Run("compare", "missing-fixture.json");

        Assert.Equal(2, exitCode);
        Assert.Equal(string.Empty, output);
        Assert.Contains("\"error\":", error);
    }

    [Fact]
    public void UnknownCommandReturnsDiagnosticBeforeReadingFixture()
    {
        var (exitCode, output, error) = Run("inspect", "missing-fixture.json");

        Assert.Equal(2, exitCode);
        Assert.Equal(string.Empty, output);
        Assert.Contains("Unknown command: inspect", error);
    }

    [Fact]
    public void UnknownOptionReturnsDiagnosticBeforeReadingFixture()
    {
        var (exitCode, output, error) = Run("compare", "missing-fixture.json", "--formta", "json");

        Assert.Equal(2, exitCode);
        Assert.Equal(string.Empty, output);
        Assert.Contains("Unknown option: --formta", error);
    }

    [Fact]
    public void InvalidFixtureShapeReturnsReadableDiagnostic()
    {
        var fixturePath = TempFixturePath();
        try
        {
            File.WriteAllText(fixturePath, """
                {
                  "schema": "spanfold.contract-fixture",
                  "schemaVersion": 1,
                  "windows": []
                }
                """);

            var (exitCode, output, error) = Run("validate-plan", fixturePath);

            Assert.Equal(2, exitCode);
            Assert.Equal(string.Empty, output);
            Assert.Contains("missing required property", error);
            Assert.Contains("plan", error);
        }
        finally
        {
            File.Delete(fixturePath);
        }
    }

    private static (int ExitCode, string Output, string Error) Run(params string[] args)
    {
        using var output = new StringWriter();
        using var error = new StringWriter();
        var exitCode = SpanfoldCli.Run(args, output, error);
        return (exitCode, output.ToString(), error.ToString());
    }

    private static string FixturePath(string name, [CallerFilePath] string callerFilePath = "")
    {
        return Path.Combine(
            Path.GetDirectoryName(callerFilePath)!,
            "..",
            "Comparison",
            "Fixtures",
            name);
    }

    private static string TempFixturePath()
    {
        return Path.Combine(Path.GetTempPath(), "spanfold-cli-" + Guid.NewGuid().ToString("N") + ".json");
    }

    private static string TempDirectoryPath()
    {
        return Path.Combine(Path.GetTempPath(), "spanfold-cli-" + Guid.NewGuid().ToString("N"));
    }
}
