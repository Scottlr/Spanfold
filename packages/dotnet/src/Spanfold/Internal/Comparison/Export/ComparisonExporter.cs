using System.Globalization;
using System.Text;
using System.Text.Json;

using Spanfold;

namespace Spanfold.Internal.Comparison;

internal static class ComparisonExporter
{
    private const string PlanSchema = "spanfold.comparison.plan";
    private const string ResultSchema = "spanfold.comparison.result";
    private const string RowSchema = "spanfold.comparison.result-row";
    private const string LlmContextSchema = "spanfold.comparison.llm-context";
    private const int SchemaVersion = 0;

    internal static string ExportJson(ComparisonPlan plan)
    {
        EnsureExportable(plan);

        using var stream = new MemoryStream();
        using (var writer = CreateWriter(stream, indented: true))
        {
            WritePlanEnvelope(writer, plan, plan.Validate());
        }

        return Encoding.UTF8.GetString(stream.ToArray());
    }

    internal static string ExportJson(ComparisonResult result)
    {
        EnsureExportable(result.Plan);

        using var stream = new MemoryStream();
        using (var writer = CreateWriter(stream, indented: true))
        {
            WriteResultEnvelope(writer, result);
        }

        return Encoding.UTF8.GetString(stream.ToArray());
    }

    internal static IEnumerable<string> ExportJsonLines(ComparisonResult result)
    {
        EnsureExportable(result.Plan);

        yield return ExportJsonLine(writer => WriteResultLine(writer, result));

        for (var i = 0; i < result.OverlapRows.Count; i++)
        {
            var row = result.OverlapRows[i];
            yield return ExportJsonLine(writer =>
            {
                WriteRowEnvelopeStart(writer, "overlap", i);
                WriteOverlapRowFields(writer, row);
                writer.WriteEndObject();
            });
        }

        for (var i = 0; i < result.ResidualRows.Count; i++)
        {
            var row = result.ResidualRows[i];
            yield return ExportJsonLine(writer =>
            {
                WriteRowEnvelopeStart(writer, "residual", i);
                WriteResidualRowFields(writer, row);
                writer.WriteEndObject();
            });
        }

        for (var i = 0; i < result.MissingRows.Count; i++)
        {
            var row = result.MissingRows[i];
            yield return ExportJsonLine(writer =>
            {
                WriteRowEnvelopeStart(writer, "missing", i);
                WriteMissingRowFields(writer, row);
                writer.WriteEndObject();
            });
        }

        for (var i = 0; i < result.CoverageRows.Count; i++)
        {
            var row = result.CoverageRows[i];
            yield return ExportJsonLine(writer =>
            {
                WriteRowEnvelopeStart(writer, "coverage", i);
                WriteCoverageRowFields(writer, row);
                writer.WriteEndObject();
            });
        }

        for (var i = 0; i < result.GapRows.Count; i++)
        {
            var row = result.GapRows[i];
            yield return ExportJsonLine(writer =>
            {
                WriteRowEnvelopeStart(writer, "gap", i);
                WriteGapRowFields(writer, row);
                writer.WriteEndObject();
            });
        }

        for (var i = 0; i < result.SymmetricDifferenceRows.Count; i++)
        {
            var row = result.SymmetricDifferenceRows[i];
            yield return ExportJsonLine(writer =>
            {
                WriteRowEnvelopeStart(writer, "symmetric-difference", i);
                WriteSymmetricDifferenceRowFields(writer, row);
                writer.WriteEndObject();
            });
        }

        for (var i = 0; i < result.ContainmentRows.Count; i++)
        {
            var row = result.ContainmentRows[i];
            yield return ExportJsonLine(writer =>
            {
                WriteRowEnvelopeStart(writer, "containment", i);
                WriteContainmentRowFields(writer, row);
                writer.WriteEndObject();
            });
        }

        for (var i = 0; i < result.LeadLagRows.Count; i++)
        {
            var row = result.LeadLagRows[i];
            yield return ExportJsonLine(writer =>
            {
                WriteRowEnvelopeStart(writer, "lead-lag", i);
                WriteLeadLagRowFields(writer, row);
                writer.WriteEndObject();
            });
        }

        for (var i = 0; i < result.AsOfRows.Count; i++)
        {
            var row = result.AsOfRows[i];
            yield return ExportJsonLine(writer =>
            {
                WriteRowEnvelopeStart(writer, "asof", i);
                WriteAsOfRowFields(writer, row);
                writer.WriteEndObject();
            });
        }
    }

    internal static string ExportDebugHtml(ComparisonResult result)
    {
        return ComparisonDebugHtmlExporter.Export(result);
    }

    internal static string ExportLlmContext(ComparisonResult result)
    {
        EnsureExportable(result.Plan);

        using var stream = new MemoryStream();
        using (var writer = CreateWriter(stream, indented: true))
        {
            WriteLlmContextEnvelope(writer, result);
        }

        return Encoding.UTF8.GetString(stream.ToArray());
    }

    private static void EnsureExportable(ComparisonPlan plan)
    {
        if (plan.IsSerializable)
        {
            return;
        }

        var diagnostics = plan.Validate()
            .Where(static diagnostic => diagnostic.Code == ComparisonPlanValidationCode.NonSerializableSelector)
            .ToArray();

        if (diagnostics.Length == 0)
        {
            diagnostics =
            [
                new ComparisonPlanDiagnostic(
                    ComparisonPlanValidationCode.NonSerializableSelector,
                    "Comparison plan contains runtime-only selectors and cannot be exported as portable data.",
                    "selectors",
                    ComparisonPlanDiagnosticSeverity.Error)
            ];
        }

        throw new ComparisonExportException(
            "Comparison plan contains runtime-only selectors and cannot be exported as portable data.",
            diagnostics);
    }

    private static Utf8JsonWriter CreateWriter(Stream stream, bool indented)
    {
        return new Utf8JsonWriter(stream, new JsonWriterOptions
        {
            Indented = indented
        });
    }

    private static string ExportJsonLine(Action<Utf8JsonWriter> write)
    {
        using var stream = new MemoryStream();
        using (var writer = CreateWriter(stream, indented: false))
        {
            write(writer);
        }

        return Encoding.UTF8.GetString(stream.ToArray());
    }

    private static void WritePlanEnvelope(
        Utf8JsonWriter writer,
        ComparisonPlan plan,
        IReadOnlyList<ComparisonPlanDiagnostic> diagnostics)
    {
        writer.WriteStartObject();
        writer.WriteString("schema", PlanSchema);
        writer.WriteNumber("schemaVersion", SchemaVersion);
        writer.WriteString("artifact", "plan");
        WritePlanFields(writer, plan, diagnostics);
        writer.WriteEndObject();
    }

    private static void WriteResultEnvelope(Utf8JsonWriter writer, ComparisonResult result)
    {
        writer.WriteStartObject();
        writer.WriteString("schema", ResultSchema);
        writer.WriteNumber("schemaVersion", SchemaVersion);
        writer.WriteString("artifact", "result");
        writer.WriteBoolean("isValid", result.IsValid);
        writer.WritePropertyName("knownAt");
        WritePoint(writer, result.KnownAt);
        writer.WritePropertyName("evaluationHorizon");
        WritePoint(writer, result.EvaluationHorizon);
        writer.WritePropertyName("plan");
        writer.WriteStartObject();
        WritePlanFields(writer, result.Plan, result.Diagnostics);
        writer.WriteEndObject();
        WriteDiagnostics(writer, "diagnostics", result.Diagnostics);
        WritePrepared(writer, result.Prepared);
        WriteAligned(writer, result.Aligned);
        WriteComparatorSummaries(writer, result.ComparatorSummaries);
        WriteRows(writer, result);
        WriteRowFinalities(writer, result.RowFinalities);
        WriteExtensionMetadata(writer, result.ExtensionMetadata);
        WriteCoverageSummaries(writer, result.CoverageSummaries);
        WriteLeadLagSummaries(writer, result.LeadLagSummaries);
        writer.WriteEndObject();
    }

    private static void WriteResultLine(Utf8JsonWriter writer, ComparisonResult result)
    {
        writer.WriteStartObject();
        writer.WriteString("schema", RowSchema);
        writer.WriteNumber("schemaVersion", SchemaVersion);
        writer.WriteString("artifact", "result-summary");
        writer.WriteString("planName", result.Plan.Name);
        writer.WriteBoolean("isValid", result.IsValid);
        writer.WritePropertyName("knownAt");
        WritePoint(writer, result.KnownAt);
        writer.WritePropertyName("evaluationHorizon");
        WritePoint(writer, result.EvaluationHorizon);
        writer.WriteNumber("diagnosticCount", result.Diagnostics.Count);
        writer.WriteNumber("overlapRowCount", result.OverlapRows.Count);
        writer.WriteNumber("residualRowCount", result.ResidualRows.Count);
        writer.WriteNumber("missingRowCount", result.MissingRows.Count);
        writer.WriteNumber("coverageRowCount", result.CoverageRows.Count);
        writer.WriteNumber("gapRowCount", result.GapRows.Count);
        writer.WriteNumber("symmetricDifferenceRowCount", result.SymmetricDifferenceRows.Count);
        writer.WriteNumber("containmentRowCount", result.ContainmentRows.Count);
        writer.WriteNumber("leadLagRowCount", result.LeadLagRows.Count);
        writer.WriteNumber("asOfRowCount", result.AsOfRows.Count);
        writer.WriteEndObject();
    }

    private static void WriteLlmContextEnvelope(Utf8JsonWriter writer, ComparisonResult result)
    {
        writer.WriteStartObject();
        writer.WriteString("schema", LlmContextSchema);
        writer.WriteNumber("schemaVersion", SchemaVersion);
        writer.WriteString("artifact", "llm-context");
        writer.WriteString("purpose", "Portable comparison context for LLMs, coding agents, CI triage, and support handoff.");
        writer.WriteStartArray("analysisInstructions");
        writer.WriteStringValue("Treat fullResult as the source of truth for exact fields, ranges, windows, segments, tags, diagnostics, summaries, and row evidence.");
        writer.WriteStringValue("Use resultMarkdown for a concise natural-language orientation before drilling into fullResult.");
        writer.WriteStringValue("Use rowDocuments when chunking or streaming row-level analysis; rowDocuments[0] is the result summary and later entries are individual comparison rows.");
        writer.WriteStringValue("Preserve rowId, recordIds, window ids, temporal ranges, knownAt, evaluationHorizon, and finality metadata when citing evidence.");
        writer.WriteStringValue("Do not infer missing source data from absence alone; check diagnostics, normalization, excluded windows, and row finalities first.");
        writer.WriteEndArray();
        writer.WriteStartObject("summary");
        WriteLlmSummaryFields(writer, result);
        writer.WriteEndObject();
        writer.WriteString("resultMarkdown", result.Explain(ComparisonExplanationFormat.Markdown));
        writer.WritePropertyName("fullResult");
        WriteResultEnvelope(writer, result);
        writer.WriteStartArray("rowDocuments");
        foreach (var line in ExportJsonLines(result))
        {
            using var document = JsonDocument.Parse(line);
            document.RootElement.WriteTo(writer);
        }

        writer.WriteEndArray();
        writer.WriteEndObject();
    }

    private static void WriteLlmSummaryFields(Utf8JsonWriter writer, ComparisonResult result)
    {
        writer.WriteString("planName", result.Plan.Name);
        writer.WriteBoolean("isValid", result.IsValid);
        writer.WritePropertyName("knownAt");
        WritePoint(writer, result.KnownAt);
        writer.WritePropertyName("evaluationHorizon");
        WritePoint(writer, result.EvaluationHorizon);
        writer.WriteNumber("diagnosticCount", result.Diagnostics.Count);
        writer.WriteNumber("selectedWindowCount", result.Prepared?.SelectedWindows.Count ?? 0);
        writer.WriteNumber("excludedWindowCount", result.Prepared?.ExcludedWindows.Count ?? 0);
        writer.WriteNumber("normalizedWindowCount", result.Prepared?.NormalizedWindows.Count ?? 0);
        writer.WriteNumber("alignedSegmentCount", result.Aligned?.Segments.Count ?? 0);
        writer.WriteStartObject("rowCounts");
        writer.WriteNumber("overlap", result.OverlapRows.Count);
        writer.WriteNumber("residual", result.ResidualRows.Count);
        writer.WriteNumber("missing", result.MissingRows.Count);
        writer.WriteNumber("coverage", result.CoverageRows.Count);
        writer.WriteNumber("gap", result.GapRows.Count);
        writer.WriteNumber("symmetricDifference", result.SymmetricDifferenceRows.Count);
        writer.WriteNumber("containment", result.ContainmentRows.Count);
        writer.WriteNumber("leadLag", result.LeadLagRows.Count);
        writer.WriteNumber("asOf", result.AsOfRows.Count);
        writer.WriteEndObject();
    }

    private static void WriteRowEnvelopeStart(Utf8JsonWriter writer, string rowType, int index)
    {
        writer.WriteStartObject();
        writer.WriteString("schema", RowSchema);
        writer.WriteNumber("schemaVersion", SchemaVersion);
        writer.WriteString("artifact", "result-row");
        writer.WriteString("rowType", rowType);
        writer.WriteString("rowId", rowType + "[" + index.ToString(CultureInfo.InvariantCulture) + "]");
    }

    private static void WritePlanFields(
        Utf8JsonWriter writer,
        ComparisonPlan plan,
        IReadOnlyList<ComparisonPlanDiagnostic> diagnostics)
    {
        writer.WriteString("name", plan.Name);
        writer.WriteBoolean("isStrict", plan.IsStrict);
        writer.WriteBoolean("isSerializable", plan.IsSerializable);
        writer.WritePropertyName("target");
        if (plan.Target.HasValue)
        {
            WriteSelector(writer, plan.Target.Value);
        }
        else
        {
            writer.WriteNullValue();
        }

        writer.WritePropertyName("against");
        writer.WriteStartArray();
        for (var i = 0; i < plan.Against.Count; i++)
        {
            WriteSelector(writer, plan.Against[i]);
        }

        writer.WriteEndArray();
        writer.WritePropertyName("scope");
        WriteScope(writer, plan.Scope);
        writer.WritePropertyName("normalization");
        WriteNormalization(writer, plan.Normalization);
        WriteStringArray(writer, "comparators", plan.Comparators);
        writer.WritePropertyName("output");
        WriteOutput(writer, plan.Output);
        WriteDiagnostics(writer, "diagnostics", diagnostics);
    }

    private static void WriteSelector(Utf8JsonWriter writer, ComparisonSelector selector)
    {
        writer.WriteStartObject();
        writer.WriteString("name", selector.Name);
        writer.WriteString("description", selector.Description);
        writer.WriteBoolean("isSerializable", selector.IsSerializable);
        if (selector.CohortActivity is not null)
        {
            writer.WritePropertyName("cohort");
            writer.WriteStartObject();
            writer.WriteString("activity", selector.CohortActivity.Name);
            if (selector.CohortActivity.Count.HasValue)
            {
                writer.WriteNumber("count", selector.CohortActivity.Count.Value);
            }

            writer.WritePropertyName("sources");
            writer.WriteStartArray();
            for (var i = 0; i < selector.CohortSources.Count; i++)
            {
                WriteObjectValue(writer, selector.CohortSources[i]);
            }

            writer.WriteEndArray();
            writer.WriteEndObject();
        }

        writer.WriteEndObject();
    }

    private static void WriteScope(Utf8JsonWriter writer, ComparisonScope? scope)
    {
        if (scope is null)
        {
            writer.WriteNullValue();
            return;
        }

        writer.WriteStartObject();
        WriteNullableString(writer, "windowName", scope.WindowName);
        writer.WriteString("timeAxis", scope.TimeAxis.ToString());
        if (scope.SegmentFilters.Count > 0)
        {
            writer.WritePropertyName("segmentFilters");
            writer.WriteStartArray();
            for (var i = 0; i < scope.SegmentFilters.Count; i++)
            {
                writer.WriteStartObject();
                writer.WriteString("name", scope.SegmentFilters[i].Name);
                WriteObjectValue(writer, "value", scope.SegmentFilters[i].Value);
                writer.WriteEndObject();
            }

            writer.WriteEndArray();
        }

        if (scope.TagFilters.Count > 0)
        {
            writer.WritePropertyName("tagFilters");
            writer.WriteStartArray();
            for (var i = 0; i < scope.TagFilters.Count; i++)
            {
                writer.WriteStartObject();
                writer.WriteString("name", scope.TagFilters[i].Name);
                WriteObjectValue(writer, "value", scope.TagFilters[i].Value);
                writer.WriteEndObject();
            }

            writer.WriteEndArray();
        }

        writer.WriteEndObject();
    }

    private static void WriteNormalization(Utf8JsonWriter writer, ComparisonNormalizationPolicy policy)
    {
        writer.WriteStartObject();
        writer.WriteBoolean("requireClosedWindows", policy.RequireClosedWindows);
        writer.WriteBoolean("useHalfOpenRanges", policy.UseHalfOpenRanges);
        writer.WriteString("timeAxis", policy.TimeAxis.ToString());
        writer.WriteString("openWindowPolicy", policy.OpenWindowPolicy.ToString());
        writer.WritePropertyName("openWindowHorizon");
        WritePoint(writer, policy.OpenWindowHorizon);
        writer.WriteString("nullTimestampPolicy", policy.NullTimestampPolicy.ToString());
        writer.WriteBoolean("coalesceAdjacentWindows", policy.CoalesceAdjacentWindows);
        writer.WriteString("duplicateWindowPolicy", policy.DuplicateWindowPolicy.ToString());
        writer.WritePropertyName("knownAt");
        WritePoint(writer, policy.KnownAt);
        writer.WriteEndObject();
    }

    private static void WriteOutput(Utf8JsonWriter writer, ComparisonOutputOptions output)
    {
        writer.WriteStartObject();
        writer.WriteBoolean("includeAlignedSegments", output.IncludeAlignedSegments);
        writer.WriteBoolean("includeExplainData", output.IncludeExplainData);
        writer.WriteEndObject();
    }

    private static void WriteDiagnostics(
        Utf8JsonWriter writer,
        string propertyName,
        IReadOnlyList<ComparisonPlanDiagnostic> diagnostics)
    {
        writer.WritePropertyName(propertyName);
        writer.WriteStartArray();
        for (var i = 0; i < diagnostics.Count; i++)
        {
            var diagnostic = diagnostics[i];
            writer.WriteStartObject();
            writer.WriteString("code", diagnostic.Code.ToString());
            writer.WriteString("message", diagnostic.Message);
            writer.WriteString("path", diagnostic.Path);
            writer.WriteString("severity", diagnostic.Severity.ToString());
            writer.WriteEndObject();
        }

        writer.WriteEndArray();
    }

    private static void WritePrepared(Utf8JsonWriter writer, PreparedComparison? prepared)
    {
        writer.WritePropertyName("prepared");
        if (prepared is null)
        {
            writer.WriteNullValue();
            return;
        }

        writer.WriteStartObject();
        writer.WritePropertyName("selectedWindows");
        writer.WriteStartArray();
        for (var i = 0; i < prepared.SelectedWindows.Count; i++)
        {
            WriteWindow(writer, prepared.SelectedWindows[i]);
        }

        writer.WriteEndArray();
        writer.WritePropertyName("excludedWindows");
        writer.WriteStartArray();
        for (var i = 0; i < prepared.ExcludedWindows.Count; i++)
        {
            var excluded = prepared.ExcludedWindows[i];
            writer.WriteStartObject();
            writer.WriteString("recordId", excluded.Window.Id.ToString());
            writer.WriteString("reason", excluded.Reason);
            WriteNullableString(writer, "diagnosticCode", excluded.DiagnosticCode?.ToString());
            writer.WritePropertyName("window");
            WriteWindow(writer, excluded.Window);
            writer.WriteEndObject();
        }

        writer.WriteEndArray();
        writer.WritePropertyName("normalizedWindows");
        writer.WriteStartArray();
        for (var i = 0; i < prepared.NormalizedWindows.Count; i++)
        {
            var normalized = prepared.NormalizedWindows[i];
            writer.WriteStartObject();
            writer.WriteString("recordId", normalized.RecordId.ToString());
            writer.WriteString("selectorName", normalized.SelectorName);
            writer.WriteString("side", normalized.Side.ToString());
            writer.WritePropertyName("range");
            WriteRange(writer, normalized.Range);
            writer.WritePropertyName("window");
            WriteWindow(writer, normalized.Window);
            writer.WriteEndObject();
        }

        writer.WriteEndArray();
        writer.WriteEndObject();
    }

    private static void WriteAligned(Utf8JsonWriter writer, AlignedComparison? aligned)
    {
        writer.WritePropertyName("aligned");
        if (aligned is null)
        {
            writer.WriteNullValue();
            return;
        }

        writer.WriteStartObject();
        writer.WritePropertyName("segments");
        writer.WriteStartArray();
        for (var i = 0; i < aligned.Segments.Count; i++)
        {
            var segment = aligned.Segments[i];
            writer.WriteStartObject();
            writer.WriteString("segmentId", "segment[" + i.ToString(CultureInfo.InvariantCulture) + "]");
            writer.WriteString("windowName", segment.WindowName);
            WriteObjectValue(writer, "key", segment.Key);
            WriteObjectValue(writer, "partition", segment.Partition);
            writer.WritePropertyName("range");
            WriteRange(writer, segment.Range);
            WriteIds(writer, "targetRecordIds", segment.TargetRecordIds);
            WriteIds(writer, "againstRecordIds", segment.AgainstRecordIds);
            writer.WriteEndObject();
        }

        writer.WriteEndArray();
        writer.WriteEndObject();
    }

    private static void WriteComparatorSummaries(
        Utf8JsonWriter writer,
        IReadOnlyList<ComparatorSummary> summaries)
    {
        writer.WritePropertyName("comparatorSummaries");
        writer.WriteStartArray();
        for (var i = 0; i < summaries.Count; i++)
        {
            var summary = summaries[i];
            writer.WriteStartObject();
            writer.WriteString("comparatorName", summary.ComparatorName);
            writer.WriteNumber("rowCount", summary.RowCount);
            writer.WriteEndObject();
        }

        writer.WriteEndArray();
    }

    private static void WriteRows(Utf8JsonWriter writer, ComparisonResult result)
    {
        var finalities = result.RowFinalities.ToDictionary(
            static finality => finality.RowType + "\n" + finality.RowId,
            static finality => finality.Finality,
            StringComparer.Ordinal);
        writer.WritePropertyName("rows");
        writer.WriteStartObject();
        writer.WritePropertyName("overlap");
        writer.WriteStartArray();
        for (var i = 0; i < result.OverlapRows.Count; i++)
        {
            WriteRowObject(writer, finalities, "overlap", i, () => WriteOverlapRowFields(writer, result.OverlapRows[i]));
        }

        writer.WriteEndArray();
        writer.WritePropertyName("residual");
        writer.WriteStartArray();
        for (var i = 0; i < result.ResidualRows.Count; i++)
        {
            WriteRowObject(writer, finalities, "residual", i, () => WriteResidualRowFields(writer, result.ResidualRows[i]));
        }

        writer.WriteEndArray();
        writer.WritePropertyName("missing");
        writer.WriteStartArray();
        for (var i = 0; i < result.MissingRows.Count; i++)
        {
            WriteRowObject(writer, finalities, "missing", i, () => WriteMissingRowFields(writer, result.MissingRows[i]));
        }

        writer.WriteEndArray();
        writer.WritePropertyName("coverage");
        writer.WriteStartArray();
        for (var i = 0; i < result.CoverageRows.Count; i++)
        {
            WriteRowObject(writer, finalities, "coverage", i, () => WriteCoverageRowFields(writer, result.CoverageRows[i]));
        }

        writer.WriteEndArray();
        writer.WritePropertyName("gap");
        writer.WriteStartArray();
        for (var i = 0; i < result.GapRows.Count; i++)
        {
            WriteRowObject(writer, finalities, "gap", i, () => WriteGapRowFields(writer, result.GapRows[i]));
        }

        writer.WriteEndArray();
        writer.WritePropertyName("symmetricDifference");
        writer.WriteStartArray();
        for (var i = 0; i < result.SymmetricDifferenceRows.Count; i++)
        {
            WriteRowObject(writer, finalities, "symmetricDifference", i, () => WriteSymmetricDifferenceRowFields(writer, result.SymmetricDifferenceRows[i]));
        }

        writer.WriteEndArray();
        writer.WritePropertyName("containment");
        writer.WriteStartArray();
        for (var i = 0; i < result.ContainmentRows.Count; i++)
        {
            WriteRowObject(writer, finalities, "containment", i, () => WriteContainmentRowFields(writer, result.ContainmentRows[i]));
        }

        writer.WriteEndArray();
        writer.WritePropertyName("leadLag");
        writer.WriteStartArray();
        for (var i = 0; i < result.LeadLagRows.Count; i++)
        {
            WriteRowObject(writer, finalities, "leadLag", i, () => WriteLeadLagRowFields(writer, result.LeadLagRows[i]));
        }

        writer.WriteEndArray();
        writer.WritePropertyName("asOf");
        writer.WriteStartArray();
        for (var i = 0; i < result.AsOfRows.Count; i++)
        {
            WriteRowObject(writer, finalities, "asOf", i, () => WriteAsOfRowFields(writer, result.AsOfRows[i]));
        }

        writer.WriteEndArray();
        writer.WriteEndObject();
    }

    private static void WriteRowObject(
        Utf8JsonWriter writer,
        IReadOnlyDictionary<string, ComparisonFinality> finalities,
        string rowType,
        int index,
        Action writeFields)
    {
        var rowId = rowType + "[" + index.ToString(CultureInfo.InvariantCulture) + "]";
        writer.WriteStartObject();
        writer.WriteString("rowId", rowId);
        writer.WriteString(
            "finality",
            finalities.TryGetValue(rowType + "\n" + rowId, out var finality)
                ? finality.ToString()
                : ComparisonFinality.Final.ToString());
        writeFields();
        writer.WriteEndObject();
    }

    private static void WriteRowFinalities(Utf8JsonWriter writer, IReadOnlyList<ComparisonRowFinality> finalities)
    {
        writer.WritePropertyName("rowFinalities");
        writer.WriteStartArray();
        for (var i = 0; i < finalities.Count; i++)
        {
            var finality = finalities[i];
            writer.WriteStartObject();
            writer.WriteString("rowType", finality.RowType);
            writer.WriteString("rowId", finality.RowId);
            writer.WriteString("finality", finality.Finality.ToString());
            writer.WriteString("reason", finality.Reason);
            writer.WriteNumber("version", finality.Version);
            WriteNullableString(writer, "supersedesRowId", finality.SupersedesRowId);
            writer.WriteEndObject();
        }

        writer.WriteEndArray();
    }

    private static void WriteExtensionMetadata(Utf8JsonWriter writer, IReadOnlyList<ComparisonExtensionMetadata> metadata)
    {
        writer.WritePropertyName("extensionMetadata");
        writer.WriteStartArray();
        for (var i = 0; i < metadata.Count; i++)
        {
            var item = metadata[i];
            writer.WriteStartObject();
            writer.WriteString("extensionId", item.ExtensionId);
            writer.WriteString("key", item.Key);
            writer.WriteString("value", item.Value);
            writer.WriteEndObject();
        }

        writer.WriteEndArray();
    }

    private static void WriteOverlapRowFields(Utf8JsonWriter writer, OverlapRow row)
    {
        WriteCommonRowFields(writer, row.WindowName, row.Key, row.Partition, row.Range);
        WriteIds(writer, "targetRecordIds", row.TargetRecordIds);
        WriteIds(writer, "againstRecordIds", row.AgainstRecordIds);
    }

    private static void WriteResidualRowFields(Utf8JsonWriter writer, ResidualRow row)
    {
        WriteCommonRowFields(writer, row.WindowName, row.Key, row.Partition, row.Range);
        WriteIds(writer, "targetRecordIds", row.TargetRecordIds);
    }

    private static void WriteMissingRowFields(Utf8JsonWriter writer, MissingRow row)
    {
        WriteCommonRowFields(writer, row.WindowName, row.Key, row.Partition, row.Range);
        WriteIds(writer, "againstRecordIds", row.AgainstRecordIds);
    }

    private static void WriteCoverageRowFields(Utf8JsonWriter writer, CoverageRow row)
    {
        WriteCommonRowFields(writer, row.WindowName, row.Key, row.Partition, row.Range);
        writer.WriteNumber("targetMagnitude", row.TargetMagnitude);
        writer.WriteNumber("coveredMagnitude", row.CoveredMagnitude);
        WriteIds(writer, "targetRecordIds", row.TargetRecordIds);
        WriteIds(writer, "againstRecordIds", row.AgainstRecordIds);
    }

    private static void WriteGapRowFields(Utf8JsonWriter writer, GapRow row)
    {
        WriteCommonRowFields(writer, row.WindowName, row.Key, row.Partition, row.Range);
    }

    private static void WriteSymmetricDifferenceRowFields(Utf8JsonWriter writer, SymmetricDifferenceRow row)
    {
        WriteCommonRowFields(writer, row.WindowName, row.Key, row.Partition, row.Range);
        writer.WriteString("side", row.Side.ToString());
        WriteIds(writer, "targetRecordIds", row.TargetRecordIds);
        WriteIds(writer, "againstRecordIds", row.AgainstRecordIds);
    }

    private static void WriteContainmentRowFields(Utf8JsonWriter writer, ContainmentRow row)
    {
        WriteCommonRowFields(writer, row.WindowName, row.Key, row.Partition, row.Range);
        writer.WriteString("status", row.Status.ToString());
        WriteIds(writer, "targetRecordIds", row.TargetRecordIds);
        WriteIds(writer, "containerRecordIds", row.ContainerRecordIds);
    }

    private static void WriteLeadLagRowFields(Utf8JsonWriter writer, LeadLagRow row)
    {
        writer.WriteString("windowName", row.WindowName);
        WriteObjectValue(writer, "key", row.Key);
        WriteObjectValue(writer, "partition", row.Partition);
        writer.WriteString("transition", row.Transition.ToString());
        writer.WriteString("axis", row.Axis.ToString());
        writer.WritePropertyName("targetPoint");
        WritePoint(writer, row.TargetPoint);
        writer.WritePropertyName("comparisonPoint");
        WritePoint(writer, row.ComparisonPoint);
        WriteNullableNumber(writer, "deltaMagnitude", row.DeltaMagnitude);
        writer.WriteNumber("toleranceMagnitude", row.ToleranceMagnitude);
        writer.WriteBoolean("isWithinTolerance", row.IsWithinTolerance);
        writer.WriteString("direction", row.Direction.ToString());
        writer.WriteString("targetRecordId", row.TargetRecordId.ToString());
        WriteNullableString(writer, "comparisonRecordId", row.ComparisonRecordId?.ToString());
    }

    private static void WriteAsOfRowFields(Utf8JsonWriter writer, AsOfRow row)
    {
        writer.WriteString("windowName", row.WindowName);
        WriteObjectValue(writer, "key", row.Key);
        WriteObjectValue(writer, "partition", row.Partition);
        writer.WriteString("axis", row.Axis.ToString());
        writer.WriteString("direction", row.Direction.ToString());
        writer.WritePropertyName("targetPoint");
        WritePoint(writer, row.TargetPoint);
        writer.WritePropertyName("matchedPoint");
        WritePoint(writer, row.MatchedPoint);
        WriteNullableNumber(writer, "distanceMagnitude", row.DistanceMagnitude);
        writer.WriteNumber("toleranceMagnitude", row.ToleranceMagnitude);
        writer.WriteString("status", row.Status.ToString());
        writer.WriteString("targetRecordId", row.TargetRecordId.ToString());
        WriteNullableString(writer, "matchedRecordId", row.MatchedRecordId?.ToString());
    }

    private static void WriteCommonRowFields(
        Utf8JsonWriter writer,
        string windowName,
        object key,
        object? partition,
        TemporalRange range)
    {
        writer.WriteString("windowName", windowName);
        WriteObjectValue(writer, "key", key);
        WriteObjectValue(writer, "partition", partition);
        writer.WritePropertyName("range");
        WriteRange(writer, range);
    }

    private static void WriteCoverageSummaries(
        Utf8JsonWriter writer,
        IReadOnlyList<CoverageSummary> summaries)
    {
        writer.WritePropertyName("coverageSummaries");
        writer.WriteStartArray();
        for (var i = 0; i < summaries.Count; i++)
        {
            var summary = summaries[i];
            writer.WriteStartObject();
            writer.WriteString("windowName", summary.WindowName);
            WriteObjectValue(writer, "key", summary.Key);
            WriteObjectValue(writer, "partition", summary.Partition);
            writer.WriteNumber("targetMagnitude", summary.TargetMagnitude);
            writer.WriteNumber("coveredMagnitude", summary.CoveredMagnitude);
            writer.WriteNumber("coverageRatio", summary.CoverageRatio);
            writer.WriteEndObject();
        }

        writer.WriteEndArray();
    }

    private static void WriteLeadLagSummaries(
        Utf8JsonWriter writer,
        IReadOnlyList<LeadLagSummary> summaries)
    {
        writer.WritePropertyName("leadLagSummaries");
        writer.WriteStartArray();
        for (var i = 0; i < summaries.Count; i++)
        {
            var summary = summaries[i];
            writer.WriteStartObject();
            writer.WriteString("transition", summary.Transition.ToString());
            writer.WriteString("axis", summary.Axis.ToString());
            writer.WriteNumber("toleranceMagnitude", summary.ToleranceMagnitude);
            writer.WriteNumber("rowCount", summary.RowCount);
            writer.WriteNumber("targetLeadCount", summary.TargetLeadCount);
            writer.WriteNumber("targetLagCount", summary.TargetLagCount);
            writer.WriteNumber("equalCount", summary.EqualCount);
            writer.WriteNumber("missingComparisonCount", summary.MissingComparisonCount);
            writer.WriteNumber("outsideToleranceCount", summary.OutsideToleranceCount);
            WriteNullableNumber(writer, "minimumDeltaMagnitude", summary.MinimumDeltaMagnitude);
            WriteNullableNumber(writer, "maximumDeltaMagnitude", summary.MaximumDeltaMagnitude);
            writer.WriteEndObject();
        }

        writer.WriteEndArray();
    }

    private static void WriteWindow(Utf8JsonWriter writer, WindowRecord window)
    {
        writer.WriteStartObject();
        writer.WriteString("recordId", window.Id.ToString());
        writer.WriteString("windowName", window.WindowName);
        WriteObjectValue(writer, "key", window.Key);
        WriteObjectValue(writer, "source", window.Source);
        WriteObjectValue(writer, "partition", window.Partition);
        writer.WriteNumber("startPosition", window.StartPosition);
        WriteNullableNumber(writer, "endPosition", window.EndPosition);
        WriteNullableTimestamp(writer, "startTime", window.StartTime);
        WriteNullableTimestamp(writer, "endTime", window.EndTime);
        writer.WriteBoolean("isClosed", window.IsClosed);
        writer.WriteEndObject();
    }

    private static void WriteRange(Utf8JsonWriter writer, TemporalRange range)
    {
        writer.WriteStartObject();
        writer.WriteString("axis", range.Axis.ToString());
        writer.WritePropertyName("start");
        WritePoint(writer, range.Start);
        writer.WritePropertyName("end");
        WritePoint(writer, range.End);
        writer.WriteString("endStatus", range.EndStatus.ToString());
        writer.WriteEndObject();
    }

    private static void WritePoint(Utf8JsonWriter writer, TemporalPoint? point)
    {
        if (!point.HasValue)
        {
            writer.WriteNullValue();
            return;
        }

        var value = point.Value;
        writer.WriteStartObject();
        writer.WriteString("axis", value.Axis.ToString());
        if (value.Axis == TemporalAxis.ProcessingPosition)
        {
            writer.WriteNumber("position", value.Position);
        }
        else if (value.Axis == TemporalAxis.Timestamp)
        {
            writer.WriteString("timestamp", value.Timestamp.ToUniversalTime().ToString("O", CultureInfo.InvariantCulture));
            WriteNullableString(writer, "clock", value.Clock);
        }

        writer.WriteEndObject();
    }

    private static void WriteObjectValue(Utf8JsonWriter writer, string propertyName, object? value)
    {
        writer.WritePropertyName(propertyName);
        WriteObjectValue(writer, value);
    }

    private static void WriteObjectValue(Utf8JsonWriter writer, object? value)
    {
        if (value is null)
        {
            writer.WriteNullValue();
            return;
        }

        writer.WriteStartObject();
        writer.WriteString("type", value.GetType().FullName);
        writer.WriteString("value", StableObjectText(value));
        writer.WriteEndObject();
    }

    private static void WriteStringArray(
        Utf8JsonWriter writer,
        string propertyName,
        IReadOnlyList<string> values)
    {
        writer.WritePropertyName(propertyName);
        writer.WriteStartArray();
        for (var i = 0; i < values.Count; i++)
        {
            writer.WriteStringValue(values[i]);
        }

        writer.WriteEndArray();
    }

    private static void WriteIds(
        Utf8JsonWriter writer,
        string propertyName,
        IReadOnlyList<WindowRecordId> ids)
    {
        writer.WritePropertyName(propertyName);
        writer.WriteStartArray();
        for (var i = 0; i < ids.Count; i++)
        {
            writer.WriteStringValue(ids[i].ToString());
        }

        writer.WriteEndArray();
    }

    private static void WriteNullableString(Utf8JsonWriter writer, string propertyName, string? value)
    {
        writer.WritePropertyName(propertyName);
        if (value is null)
        {
            writer.WriteNullValue();
            return;
        }

        writer.WriteStringValue(value);
    }

    private static void WriteNullableNumber(Utf8JsonWriter writer, string propertyName, long? value)
    {
        writer.WritePropertyName(propertyName);
        if (value.HasValue)
        {
            writer.WriteNumberValue(value.Value);
            return;
        }

        writer.WriteNullValue();
    }

    private static void WriteNullableTimestamp(Utf8JsonWriter writer, string propertyName, DateTimeOffset? value)
    {
        writer.WritePropertyName(propertyName);
        if (value.HasValue)
        {
            writer.WriteStringValue(value.Value.ToUniversalTime().ToString("O", CultureInfo.InvariantCulture));
            return;
        }

        writer.WriteNullValue();
    }

    private static string StableObjectText(object value)
    {
        return value switch
        {
            IFormattable formattable => formattable.ToString(null, CultureInfo.InvariantCulture),
            _ => value.ToString() ?? string.Empty
        };
    }
}
