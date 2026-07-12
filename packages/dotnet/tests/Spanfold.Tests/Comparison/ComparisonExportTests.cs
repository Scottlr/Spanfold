using System.Text.Json;

using Spanfold;

namespace Spanfold.Tests.Comparison;

public sealed class ComparisonExportTests
{
    [Fact]
    public void PlanExportProducesByteStableJson()
    {
        var plan = CreatePlan();

        var first = plan.ExportJson();
        var second = plan.ExportJson();

        Assert.Equal(first, second);
        using var document = JsonDocument.Parse(first);
        Assert.Equal("spanfold.comparison.plan", document.RootElement.GetProperty("schema").GetString());
        Assert.Equal(0, document.RootElement.GetProperty("schemaVersion").GetInt32());
        Assert.Equal("Provider QA", document.RootElement.GetProperty("name").GetString());
        Assert.Equal(JsonValueKind.Array, document.RootElement.GetProperty("diagnostics").ValueKind);
    }

    [Fact]
    public void ResultExportProducesByteStableJsonWithEmptyCollections()
    {
        var result = new ComparisonResult(CreatePlan(), []);

        var first = result.ExportJson();
        var second = result.ExportJson();

        Assert.Equal(first, second);
        using var document = JsonDocument.Parse(first);
        var root = document.RootElement;

        Assert.Equal("spanfold.comparison.result", root.GetProperty("schema").GetString());
        Assert.Equal(JsonValueKind.Null, root.GetProperty("prepared").ValueKind);
        Assert.Equal(JsonValueKind.Null, root.GetProperty("aligned").ValueKind);
        Assert.Equal(0, root.GetProperty("rows").GetProperty("overlap").GetArrayLength());
        Assert.Equal(0, root.GetProperty("rows").GetProperty("residual").GetArrayLength());
        Assert.Equal(0, root.GetProperty("rows").GetProperty("missing").GetArrayLength());
        Assert.Equal(0, root.GetProperty("rows").GetProperty("coverage").GetArrayLength());
        Assert.Equal(0, root.GetProperty("rows").GetProperty("gap").GetArrayLength());
        Assert.Equal(0, root.GetProperty("rows").GetProperty("symmetricDifference").GetArrayLength());
        Assert.Equal(0, root.GetProperty("rows").GetProperty("containment").GetArrayLength());
        Assert.Equal(0, root.GetProperty("rows").GetProperty("leadLag").GetArrayLength());
        Assert.Equal(0, root.GetProperty("rows").GetProperty("asOf").GetArrayLength());
        Assert.Equal(0, root.GetProperty("leadLagSummaries").GetArrayLength());
    }

    [Fact]
    public void ResultMarkdownExportContainsDiagnosticsAndRowCounts()
    {
        var result = CreateResult(
            new ComparisonPlanDiagnostic(
                ComparisonPlanValidationCode.UnknownComparator,
                "Comparator 'shape' is not registered.",
                "comparators[1]",
                ComparisonPlanDiagnosticSeverity.Warning));

        var markdown = result.ExportMarkdown();

        Assert.Contains("diagnostic[0]: Warning UnknownComparator", markdown);
        Assert.Contains("overlap rows: 1", markdown);
        Assert.Contains("coverage rows: 0", markdown);
    }

    [Fact]
    public void NonExportablePlanFailsWithDiagnostics()
    {
        var plan = new ComparisonPlan(
            "Runtime selector QA",
            ComparisonSelector.RuntimeOnly("dynamic-target", "uses a delegate"),
            [ComparisonSelector.ForSource("provider-b")],
            ComparisonScope.Window("DeviceOffline"),
            ComparisonNormalizationPolicy.Default,
            ["overlap"],
            ComparisonOutputOptions.Default);

        var exception = Assert.Throws<ComparisonExportException>(() => plan.ExportJson());

        Assert.Contains("runtime-only selectors", exception.Message);
        Assert.Contains(exception.Diagnostics, diagnostic =>
            diagnostic.Code == ComparisonPlanValidationCode.NonSerializableSelector);
    }

    [Fact]
    public void ResultJsonLinesStreamsSummaryAndRows()
    {
        var result = CreateResult();

        var lines = result.ExportJsonLines().ToArray();

        Assert.Equal(2, lines.Length);
        using var summary = JsonDocument.Parse(lines[0]);
        using var row = JsonDocument.Parse(lines[1]);

        Assert.Equal("result-summary", summary.RootElement.GetProperty("artifact").GetString());
        Assert.Equal("result-row", row.RootElement.GetProperty("artifact").GetString());
        Assert.Equal("overlap[0]", row.RootElement.GetProperty("rowId").GetString());
        Assert.Equal("Final", row.RootElement.GetProperty("finality").GetString());
        Assert.Equal(1, row.RootElement.GetProperty("version").GetInt32());
    }

    [Fact]
    public void ResultDebugHtmlExportProducesStableVisualDocument()
    {
        var result = CreateResult();

        var first = result.ExportDebugHtml();
        var second = result.ExportDebugHtml();

        Assert.Equal(first, second);
        Assert.Contains("<!doctype html>", first);
        Assert.Contains("Provider QA", first);
        Assert.Contains("Window Timeline", first);
        Assert.Contains("Aligned Segments", first);
        Assert.Contains("source:provider-a", first);
        Assert.Contains("target only", first);
        Assert.Contains("comparison only", first);
        Assert.DoesNotContain("<script", first, StringComparison.OrdinalIgnoreCase);
    }

    [Fact]
    public void ResultDebugHtmlExportWritesFileAndCreatesDirectory()
    {
        var result = CreateResult();
        var directory = Path.Combine(Path.GetTempPath(), "spanfold-debug-" + Guid.NewGuid().ToString("N"));
        var path = Path.Combine(directory, "comparison.html");

        try
        {
            result.ExportDebugHtml(path);

            Assert.True(File.Exists(path));
            Assert.Contains("Rows And Finality", File.ReadAllText(path));
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
    public void ResultLlmContextExportProducesStableAgentDocument()
    {
        var result = CreateResult();

        var first = result.ExportLlmContext();
        var second = result.ExportLlmContext();

        Assert.Equal(first, second);
        using var document = JsonDocument.Parse(first);
        var root = document.RootElement;

        Assert.Equal("spanfold.comparison.llm-context", root.GetProperty("schema").GetString());
        Assert.Equal("llm-context", root.GetProperty("artifact").GetString());
        Assert.Contains("fullResult", root.GetProperty("analysisInstructions")[0].GetString());
        Assert.Equal("Provider QA", root.GetProperty("summary").GetProperty("planName").GetString());
        Assert.Equal(1, root.GetProperty("summary").GetProperty("rowCounts").GetProperty("overlap").GetInt32());
        Assert.Contains("overlap rows: 1", root.GetProperty("resultMarkdown").GetString());
        Assert.Equal("spanfold.comparison.result", root.GetProperty("fullResult").GetProperty("schema").GetString());
        Assert.Equal(2, root.GetProperty("rowDocuments").GetArrayLength());
        Assert.Equal("result-summary", root.GetProperty("rowDocuments")[0].GetProperty("artifact").GetString());
        Assert.Equal("overlap[0]", root.GetProperty("rowDocuments")[1].GetProperty("rowId").GetString());
    }

    [Fact]
    public void ResultLlmContextExportWritesFileAndCreatesDirectory()
    {
        var result = CreateResult();
        var directory = Path.Combine(Path.GetTempPath(), "spanfold-llm-" + Guid.NewGuid().ToString("N"));
        var path = Path.Combine(directory, "comparison.llm.json");

        try
        {
            result.ExportLlmContext(path);

            Assert.True(File.Exists(path));
            Assert.Contains("spanfold.comparison.llm-context", File.ReadAllText(path));
        }
        finally
        {
            if (Directory.Exists(directory))
            {
                Directory.Delete(directory, recursive: true);
            }
        }
    }

    private static ComparisonPlan CreatePlan()
    {
        return new ComparisonPlan(
            "Provider QA",
            ComparisonSelector.ForSource("provider-a"),
            [ComparisonSelector.ForSource("provider-b")],
            ComparisonScope.Window("DeviceOffline"),
            ComparisonNormalizationPolicy.Default,
            ["overlap"],
            ComparisonOutputOptions.Default);
    }

    private static ComparisonResult CreateResult(params ComparisonPlanDiagnostic[] diagnostics)
    {
        var target = new ClosedWindow("DeviceOffline", "device-1", StartPosition: 1, EndPosition: 5, Source: "provider-a");
        var against = new ClosedWindow("DeviceOffline", "device-1", StartPosition: 3, EndPosition: 7, Source: "provider-b");
        var plan = CreatePlan();
        var prepared = new PreparedComparison(
            plan,
            diagnostics,
            [target, against],
            [],
            [
                new NormalizedWindowRecord(
                    target,
                    target.Id,
                    "source:provider-a",
                    ComparisonSide.Target,
                    TemporalRange.Closed(
                        TemporalPoint.ForPosition(target.StartPosition),
                        TemporalPoint.ForPosition(target.EndPosition!.Value))),
                new NormalizedWindowRecord(
                    against,
                    against.Id,
                    "source:provider-b",
                    ComparisonSide.Against,
                    TemporalRange.Closed(
                        TemporalPoint.ForPosition(against.StartPosition),
                        TemporalPoint.ForPosition(against.EndPosition!.Value)))
            ]);
        var aligned = prepared.Align();
        var overlap = Assert.Single(
            aligned.Segments,
            static segment => segment.TargetRecordIds.Count == 1 && segment.AgainstRecordIds.Count == 1);

        return new ComparisonResult(
            plan,
            diagnostics,
            prepared,
            aligned,
            [new ComparatorSummary("overlap", 1)],
            [
                new OverlapRow(
                    overlap.WindowName,
                    overlap.Key,
                    overlap.Partition,
                    overlap.Range,
                    overlap.TargetRecordIds,
                    overlap.AgainstRecordIds)
            ]);
    }
}
