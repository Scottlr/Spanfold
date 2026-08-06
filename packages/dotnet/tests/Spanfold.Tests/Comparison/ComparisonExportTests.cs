using System.Text.Json;

using Spanfold;
using Spanfold.Testing;

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
            ["overlap"]
            );

        var exception = Assert.Throws<ComparisonExportException>(() => plan.ExportJson());

        Assert.Contains("runtime-only selectors", exception.Message);
        Assert.Contains(exception.Diagnostics, diagnostic =>
            diagnostic.Code == ComparisonPlanValidationCode.NonSerializableSelector);
    }

    [Fact]
    public void InvalidTemporalPlanFailsBeforePortableExport()
    {
        var plan = new ComparisonPlan(
            "Invalid temporal QA",
            ComparisonSelector.ForSource("provider-a"),
            [ComparisonSelector.ForSource("provider-b")],
            ComparisonScope.Window("DeviceOffline", TemporalAxis.Unknown),
            ComparisonNormalizationPolicy.Default with
            {
                TimeAxis = TemporalAxis.Unknown
            },
            ["overlap"]
            );

        var exception = Assert.Throws<ComparisonExportException>(() => plan.ExportPortableJson());

        Assert.Contains("invalid temporal configuration", exception.Message);
        Assert.Collection(
            exception.Diagnostics,
            scope =>
            {
                Assert.Equal(ComparisonPlanValidationCode.InvalidTemporalAxis, scope.Code);
                Assert.Equal("scope.timeAxis", scope.Path);
            },
            normalization =>
            {
                Assert.Equal(ComparisonPlanValidationCode.InvalidTemporalAxis, normalization.Code);
                Assert.Equal("normalization.timeAxis", normalization.Path);
            });
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
        Assert.StartsWith("overlap:", row.RootElement.GetProperty("rowId").GetString());
        Assert.Equal("Final", row.RootElement.GetProperty("finality").GetString());
        Assert.Equal(1, row.RootElement.GetProperty("version").GetInt32());
    }

    [Fact]
    public void TypedRowsUseTheStoredResultMetadataAssociation()
    {
        var result = CreateResultWithFinality([
            new ComparisonRowFinality(new ComparisonRowReference(ComparisonRowKind.Overlap, "overlap:stored"), ComparisonFinality.Provisional, "live", 3, "overlap:prior")
        ]);

        var entry = Assert.Single(result.OverlapRowsWithFinality());

        Assert.Same(result.OverlapRows[0], entry.Row);
        Assert.Equal("overlap:stored", entry.Metadata.RowId);
        Assert.Equal(ComparisonFinality.Provisional, entry.Metadata.Finality);
        Assert.Equal(3, entry.Metadata.Version);
        Assert.Equal("overlap:prior", entry.Metadata.SupersedesRowId);
    }

    [Fact]
    public void ResultExportsPreserveStoredIdentityAndFullJsonLinesFinality()
    {
        var result = CreateResultWithFinality([
            new ComparisonRowFinality(new ComparisonRowReference(ComparisonRowKind.Overlap, "overlap:stored"), ComparisonFinality.Provisional, "live", 3, "overlap:prior")
        ]);

        using var json = JsonDocument.Parse(result.ExportJson());
        var jsonRow = json.RootElement.GetProperty("rows").GetProperty("overlap")[0];
        Assert.Equal("overlap:stored", jsonRow.GetProperty("rowId").GetString());
        Assert.Equal("Provisional", jsonRow.GetProperty("finality").GetString());

        var lines = result.ExportJsonLines().ToArray();
        using var jsonLine = JsonDocument.Parse(lines[1]);
        Assert.Equal("overlap:stored", jsonLine.RootElement.GetProperty("rowId").GetString());
        Assert.Equal("Provisional", jsonLine.RootElement.GetProperty("finality").GetString());
        Assert.Equal("live", jsonLine.RootElement.GetProperty("reason").GetString());
        Assert.Equal(3, jsonLine.RootElement.GetProperty("version").GetInt32());
        Assert.Equal("overlap:prior", jsonLine.RootElement.GetProperty("supersedesRowId").GetString());

        using var llm = JsonDocument.Parse(result.ExportLlmContext());
        var llmRow = llm.RootElement.GetProperty("rowDocuments")[1];
        Assert.Equal("overlap:stored", llmRow.GetProperty("rowId").GetString());
        Assert.Equal("Provisional", llmRow.GetProperty("finality").GetString());
        Assert.Equal("live", llmRow.GetProperty("reason").GetString());
    }

    [Fact]
    public void TypedViewsAndExportsFailClosedOnMetadataLayoutCorruption()
    {
        var missing = CreateResultWithFinality([]);
        var missingException = Assert.Throws<ComparisonRowMetadataException>(
            () => missing.OverlapRowsWithFinality().ToArray());

        Assert.Equal(ComparisonRowKind.Overlap, missingException.Family);
        Assert.Equal(0, missingException.MetadataIndex);
        Assert.Equal(1, missingException.ExpectedCount);
        Assert.Equal(0, missingException.ActualCount);
        Assert.Null(missingException.ActualKind);
        Assert.Throws<ComparisonRowMetadataException>(() => missing.ExportJson());

        var wrongKind = CreateResultWithFinality([
            new ComparisonRowFinality(new ComparisonRowReference(ComparisonRowKind.Residual, "residual:wrong"), ComparisonFinality.Final, "closed")
        ]);
        var wrongKindException = Assert.Throws<ComparisonRowMetadataException>(
            () => wrongKind.ExportJsonLines().ToArray());

        Assert.Equal(ComparisonRowKind.Overlap, wrongKindException.Family);
        Assert.Equal(0, wrongKindException.MetadataIndex);
        Assert.Equal("residual", wrongKindException.ActualKind);

        var nullMetadata = CreateResultWithFinality([null!]);
        var nullMetadataException = Assert.Throws<ComparisonRowMetadataException>(
            () => nullMetadata.OverlapRowsWithFinality().ToArray());

        Assert.Equal(0, nullMetadataException.MetadataIndex);
        Assert.Null(nullMetadataException.ActualKind);
    }

    [Fact]
    public void RowKindsParseCanonicalLabelsAndRustJsonLinesAliases()
    {
        var cases = new Dictionary<string, ComparisonRowKind>
        {
            ["overlap"] = ComparisonRowKind.Overlap,
            ["residual"] = ComparisonRowKind.Residual,
            ["missing"] = ComparisonRowKind.Missing,
            ["coverage"] = ComparisonRowKind.Coverage,
            ["gap"] = ComparisonRowKind.Gap,
            ["symmetricDifference"] = ComparisonRowKind.SymmetricDifference,
            ["symmetric-difference"] = ComparisonRowKind.SymmetricDifference,
            ["containment"] = ComparisonRowKind.Containment,
            ["leadLag"] = ComparisonRowKind.LeadLag,
            ["lead-lag"] = ComparisonRowKind.LeadLag,
            ["asOf"] = ComparisonRowKind.AsOf,
            ["asof"] = ComparisonRowKind.AsOf
        };

        foreach (var (label, expected) in cases)
        {
            Assert.True(ComparisonRowKindExtensions.TryParseArtifactLabel(label, out var actual));
            Assert.Equal(expected, actual);
            Assert.Equal(label is "symmetric-difference" ? "symmetricDifference"
                : label is "lead-lag" ? "leadLag"
                : label is "asof" ? "asOf"
                : label, actual.ToArtifactLabel());
        }

        Assert.False(ComparisonRowKindExtensions.TryParseArtifactLabel("lead_lag", out _));
    }

    [Fact]
    public void AllTypedFamiliesShareAuthoritativeMetadataAcrossExports()
    {
        var result = CreateAllFamilyResult();

        Assert.NotEmpty(result.OverlapRowsWithFinality());
        Assert.NotEmpty(result.ResidualRowsWithFinality());
        Assert.NotEmpty(result.MissingRowsWithFinality());
        Assert.NotEmpty(result.CoverageRowsWithFinality());
        Assert.NotEmpty(result.GapRowsWithFinality());
        Assert.NotEmpty(result.SymmetricDifferenceRowsWithFinality());
        Assert.NotEmpty(result.ContainmentRowsWithFinality());
        Assert.NotEmpty(result.LeadLagRowsWithFinality());
        Assert.NotEmpty(result.AsOfRowsWithFinality());

        var expected = result.RowFinalities
            .Select(static metadata => (metadata.RowType, metadata.RowId, metadata.Finality.ToString()))
            .ToArray();
        var kinds = new[]
        {
            ComparisonRowKind.Overlap,
            ComparisonRowKind.Residual,
            ComparisonRowKind.Missing,
            ComparisonRowKind.Coverage,
            ComparisonRowKind.Gap,
            ComparisonRowKind.SymmetricDifference,
            ComparisonRowKind.Containment,
            ComparisonRowKind.LeadLag,
            ComparisonRowKind.AsOf
        };

        using var json = JsonDocument.Parse(result.ExportJson());
        var jsonAssociations = new List<(string, string, string)>();
        foreach (var kind in kinds)
        {
            foreach (var row in json.RootElement.GetProperty("rows").GetProperty(kind.ToArtifactLabel()).EnumerateArray())
            {
                jsonAssociations.Add((
                    kind.ToArtifactLabel(),
                    row.GetProperty("rowId").GetString()!,
                    row.GetProperty("finality").GetString()!));
            }
        }

        Assert.Equal(expected, jsonAssociations);

        var jsonLineAssociations = result.ExportJsonLines()
            .Skip(1)
            .Select(static line => JsonDocument.Parse(line).RootElement)
            .Select(static row => (
                row.GetProperty("rowType").GetString()!,
                row.GetProperty("rowId").GetString()!,
                row.GetProperty("finality").GetString()!))
            .ToArray();
        Assert.Equal(expected, jsonLineAssociations);

        using var llm = JsonDocument.Parse(result.ExportLlmContext());
        var llmAssociations = llm.RootElement.GetProperty("rowDocuments")
            .EnumerateArray()
            .Skip(1)
            .Select(static row => (
                row.GetProperty("rowType").GetString()!,
                row.GetProperty("rowId").GetString()!,
                row.GetProperty("finality").GetString()!))
            .ToArray();
        Assert.Equal(expected, llmAssociations);
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
        Assert.StartsWith("overlap:", root.GetProperty("rowDocuments")[1].GetProperty("rowId").GetString());
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
            ["overlap"]
            );
    }

    private static ComparisonResult CreateResult(params ComparisonPlanDiagnostic[] diagnostics)
    {
        return CreateResultWithFinality(null, diagnostics);
    }

    private static ComparisonResult CreateAllFamilyResult()
    {
        var history = new WindowHistoryFixtureBuilder()
            .AddClosedWindow("DeviceOffline", "device-1", 1, 5, window => window.Source("provider-a"))
            .AddClosedWindow("DeviceOffline", "device-1", 9, 11, window => window.Source("provider-a"))
            .AddClosedWindow("DeviceOffline", "device-1", 3, 7, window => window.Source("provider-b"))
            .AddClosedWindow("DeviceOffline", "device-1", 12, 13, window => window.Source("provider-b"))
            .Build();

        return history.Compare("All row families")
            .Target("provider-a", selector => selector.Source("provider-a"))
            .Against("provider-b", selector => selector.Source("provider-b"))
            .Within(scope => scope.Window("DeviceOffline"))
            .Using(comparators => comparators
                .Overlap()
                .Residual()
                .Missing()
                .Coverage()
                .Gap()
                .SymmetricDifference()
                .Containment()
                .LeadLag(LeadLagTransition.Start, TemporalAxis.ProcessingPosition, 100)
                .AsOf(AsOfDirection.Previous, TemporalAxis.ProcessingPosition, 100))
            .Run();
    }

    private static ComparisonResult CreateResultWithFinality(
        IReadOnlyList<ComparisonRowFinality>? rowFinalities,
        params ComparisonPlanDiagnostic[] diagnostics)
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
            ],
            rowFinalities: rowFinalities ??
            [
                new ComparisonRowFinality(new ComparisonRowReference(ComparisonRowKind.Overlap, "overlap:stored"), ComparisonFinality.Final, "closed")
            ]);
    }
}
