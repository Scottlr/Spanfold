using Spanfold.Comparison;
using System.Globalization;
using System.Text;
using System.Text.Json;

using Spanfold;

namespace Spanfold.Artifacts.Internal;

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

    internal static string ExportPortableJson(ComparisonPlan plan)
    {
        EnsureExportable(plan);

        using var stream = new MemoryStream();
        using (var writer = CreateWriter(stream, indented: true))
        {
            WritePlanEnvelope(writer, plan, plan.Validate(), includeDescriptors: true);
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

        foreach (var entry in result.OverlapRowsWithFinality())
        {
            yield return ExportRowLine(entry, ComparisonRowKind.Overlap, WriteOverlapRowFields);
        }

        foreach (var entry in result.ResidualRowsWithFinality())
        {
            yield return ExportRowLine(entry, ComparisonRowKind.Residual, WriteResidualRowFields);
        }

        foreach (var entry in result.MissingRowsWithFinality())
        {
            yield return ExportRowLine(entry, ComparisonRowKind.Missing, WriteMissingRowFields);
        }

        foreach (var entry in result.CoverageRowsWithFinality())
        {
            yield return ExportRowLine(entry, ComparisonRowKind.Coverage, WriteCoverageRowFields);
        }

        foreach (var entry in result.GapRowsWithFinality())
        {
            yield return ExportRowLine(entry, ComparisonRowKind.Gap, WriteGapRowFields);
        }

        foreach (var entry in result.SymmetricDifferenceRowsWithFinality())
        {
            yield return ExportRowLine(entry, ComparisonRowKind.SymmetricDifference, WriteSymmetricDifferenceRowFields);
        }

        foreach (var entry in result.ContainmentRowsWithFinality())
        {
            yield return ExportRowLine(entry, ComparisonRowKind.Containment, WriteContainmentRowFields);
        }

        foreach (var entry in result.LeadLagRowsWithFinality())
        {
            yield return ExportRowLine(entry, ComparisonRowKind.LeadLag, WriteLeadLagRowFields);
        }

        foreach (var entry in result.AsOfRowsWithFinality())
        {
            yield return ExportRowLine(entry, ComparisonRowKind.AsOf, WriteAsOfRowFields);
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
        IReadOnlyList<ComparisonPlanDiagnostic> diagnostics,
        bool includeDescriptors = false)
    {
        writer.WriteStartObject();
        writer.WriteString("schema", PlanSchema);
        writer.WriteNumber("schemaVersion", SchemaVersion);
        writer.WriteString("artifact", "plan");
        WritePlanFields(writer, plan, diagnostics, includeDescriptors);
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
        if (result.RecordEvidence.Any(static evidence =>
                evidence.Segments.Count > 0
                || evidence.Tags.Count > 0
                || evidence.BoundaryReason.HasValue
                || evidence.BoundaryChanges.Count > 0))
        {
            WriteRecordEvidence(writer, result.RecordEvidence);
        }
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

    private static void WriteRecordEvidence(
        Utf8JsonWriter writer,
        IReadOnlyList<WindowRecordEvidence> evidence)
    {
        writer.WritePropertyName("recordEvidence");
        writer.WriteStartArray();
        for (var i = 0; i < evidence.Count; i++)
        {
            var item = evidence[i];
            writer.WriteStartObject();
            writer.WriteString("id", item.Id.Value);
            writer.WriteString("windowName", item.WindowName);
            WriteSegments(writer, "segments", item.Segments);
            WriteTags(writer, "tags", item.Tags);
            if (item.BoundaryReason.HasValue)
            {
                writer.WriteString("boundaryReason", item.BoundaryReason.Value.ToString());
            }
            else
            {
                writer.WriteNull("boundaryReason");
            }

            writer.WritePropertyName("boundaryChanges");
            writer.WriteStartArray();
            for (var j = 0; j < item.BoundaryChanges.Count; j++)
            {
                writer.WriteStartObject();
                writer.WriteString("segmentName", item.BoundaryChanges[j].SegmentName);
                WriteObjectValue(writer, "previousValue", item.BoundaryChanges[j].PreviousValue);
                WriteObjectValue(writer, "currentValue", item.BoundaryChanges[j].CurrentValue);
                writer.WriteEndObject();
            }

            writer.WriteEndArray();
            writer.WriteEndObject();
        }

        writer.WriteEndArray();
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
        writer.WriteStringValue("Use the rows and rowFinalities inside fullResult for row-level analysis; they are the single canonical result representation.");
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
        WriteResultLine(writer, result);
        WriteLlmRowReferences(writer, result);
        writer.WriteEndArray();
        writer.WriteEndObject();
    }

    private static void WriteLlmRowReferences(Utf8JsonWriter writer, ComparisonResult result)
    {
        WriteLlmRowReferences(writer, ComparisonRowKind.Overlap, result.OverlapRowsWithFinality());
        WriteLlmRowReferences(writer, ComparisonRowKind.Residual, result.ResidualRowsWithFinality());
        WriteLlmRowReferences(writer, ComparisonRowKind.Missing, result.MissingRowsWithFinality());
        WriteLlmRowReferences(writer, ComparisonRowKind.Coverage, result.CoverageRowsWithFinality());
        WriteLlmRowReferences(writer, ComparisonRowKind.Gap, result.GapRowsWithFinality());
        WriteLlmRowReferences(writer, ComparisonRowKind.SymmetricDifference, result.SymmetricDifferenceRowsWithFinality());
        WriteLlmRowReferences(writer, ComparisonRowKind.Containment, result.ContainmentRowsWithFinality());
        WriteLlmRowReferences(writer, ComparisonRowKind.LeadLag, result.LeadLagRowsWithFinality());
        WriteLlmRowReferences(writer, ComparisonRowKind.AsOf, result.AsOfRowsWithFinality());
    }

    private static void WriteLlmRowReferences<T>(
        Utf8JsonWriter writer,
        ComparisonRowKind kind,
        IEnumerable<ComparisonRowWithFinality<T>> rows)
    {
        foreach (var entry in rows)
        {
            writer.WriteStartObject();
            writer.WriteString("artifact", "row-reference");
            writer.WriteString("rowType", kind.ToArtifactLabel());
            writer.WriteString("rowId", entry.Metadata.RowId);
            writer.WriteString("finality", entry.Metadata.Finality.ToString());
            writer.WriteString("reason", entry.Metadata.Reason);
            writer.WriteNumber("version", entry.Metadata.Version);
            WriteNullableString(writer, "supersedesRowId", entry.Metadata.SupersedesRowId);
            writer.WriteEndObject();
        }
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

    private static string ExportRowLine<T>(
        ComparisonRowWithFinality<T> entry,
        ComparisonRowKind kind,
        Action<Utf8JsonWriter, T> writeFields)
    {
        return ExportJsonLine(writer =>
        {
            WriteRowEnvelopeStart(writer, kind, entry.Metadata);
            writeFields(writer, entry.Row);
            writer.WriteEndObject();
        });
    }

    private static void WriteRowEnvelopeStart(
        Utf8JsonWriter writer,
        ComparisonRowKind kind,
        ComparisonRowFinality metadata)
    {
        writer.WriteStartObject();
        writer.WriteString("schema", RowSchema);
        writer.WriteNumber("schemaVersion", SchemaVersion);
        writer.WriteString("artifact", "result-row");
        writer.WriteString("rowType", kind.ToArtifactLabel());
        writer.WriteString("rowId", metadata.RowId);
        writer.WriteString("finality", metadata.Finality.ToString());
        writer.WriteString("reason", metadata.Reason);
        writer.WriteNumber("version", metadata.Version);
        WriteNullableString(writer, "supersedesRowId", metadata.SupersedesRowId);
    }

    private static void WritePlanFields(
        Utf8JsonWriter writer,
        ComparisonPlan plan,
        IReadOnlyList<ComparisonPlanDiagnostic> diagnostics,
        bool includeDescriptors = false)
    {
        writer.WriteString("name", plan.Name);
        writer.WriteBoolean("isStrict", plan.IsStrict);
        writer.WriteBoolean("isSerializable", plan.IsSerializable);
        writer.WritePropertyName("target");
        if (plan.Target.HasValue)
        {
            WriteSelector(writer, plan.Target.Value, includeDescriptors);
        }
        else
        {
            writer.WriteNullValue();
        }

        writer.WritePropertyName("against");
        writer.WriteStartArray();
        for (var i = 0; i < plan.Against.Count; i++)
        {
            WriteSelector(writer, plan.Against[i], includeDescriptors);
        }

        writer.WriteEndArray();
        writer.WritePropertyName("scope");
        WriteScope(writer, plan.Scope);
        writer.WritePropertyName("normalization");
        WriteNormalization(writer, plan.Normalization);
        WriteStringArray(writer, "comparators", plan.Comparators);
        WriteDiagnostics(writer, "diagnostics", diagnostics);
    }

    private static void WriteSelector(Utf8JsonWriter writer, ComparisonSelector selector, bool includeDescriptor = false)
    {
        writer.WriteStartObject();
        writer.WriteString("name", selector.Name);
        writer.WriteString("description", selector.Description);
        writer.WriteBoolean("isSerializable", selector.IsSerializable);
        if (includeDescriptor && selector.Descriptor is { } descriptor)
        {
            WriteSelectorDescriptor(writer, descriptor);
        }
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

    private static void WriteSelectorDescriptor(
        Utf8JsonWriter writer,
        ComparisonSelectorDescriptor descriptor)
    {
        writer.WritePropertyName("descriptor");
        writer.WriteStartObject();
        writer.WriteString("kind", descriptor.Kind);
        if (descriptor.Value is not null)
        {
            WriteObjectValue(writer, "value", descriptor.Value);
        }

        if (descriptor.Values.Count > 0)
        {
            writer.WriteStartArray("values");
            for (var i = 0; i < descriptor.Values.Count; i++)
            {
                WriteObjectValue(writer, descriptor.Values[i]);
            }

            writer.WriteEndArray();
        }

        WriteNullableNumber(writer, "startPosition", descriptor.StartPosition);
        WriteNullableNumber(writer, "endPosition", descriptor.EndPosition);
        WriteNullableTimestamp(writer, "startTime", descriptor.StartTime);
        WriteNullableTimestamp(writer, "endTime", descriptor.EndTime);
        WriteNullableString(writer, "clock", descriptor.Clock);
        WriteNullableString(writer, "activity", descriptor.Activity);
        if (descriptor.Count.HasValue)
        {
            writer.WriteNumber("count", descriptor.Count.Value);
        }

        if (descriptor.Children.Count > 0)
        {
            writer.WriteStartArray("children");
            for (var i = 0; i < descriptor.Children.Count; i++)
            {
                WriteSelectorDescriptor(writer, descriptor.Children[i]);
            }

            writer.WriteEndArray();
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
        writer.WriteString("timeAxis", policy.TimeAxis.ToString());
        writer.WriteString("openWindowPolicy", policy.OpenWindowPolicy.ToString());
        writer.WritePropertyName("openWindowHorizon");
        WritePoint(writer, policy.OpenWindowHorizon);
        writer.WriteString("nullTimestampPolicy", policy.NullTimestampPolicy.ToString());
        writer.WritePropertyName("knownAt");
        WritePoint(writer, policy.KnownAt);
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
        writer.WritePropertyName("rows");
        writer.WriteStartObject();
        writer.WritePropertyName("overlap");
        writer.WriteStartArray();
        foreach (var entry in result.OverlapRowsWithFinality())
        {
            WriteRowObject(writer, entry.Metadata, () => WriteOverlapRowFields(writer, entry.Row));
        }

        writer.WriteEndArray();
        writer.WritePropertyName("residual");
        writer.WriteStartArray();
        foreach (var entry in result.ResidualRowsWithFinality())
        {
            WriteRowObject(writer, entry.Metadata, () => WriteResidualRowFields(writer, entry.Row));
        }

        writer.WriteEndArray();
        writer.WritePropertyName("missing");
        writer.WriteStartArray();
        foreach (var entry in result.MissingRowsWithFinality())
        {
            WriteRowObject(writer, entry.Metadata, () => WriteMissingRowFields(writer, entry.Row));
        }

        writer.WriteEndArray();
        writer.WritePropertyName("coverage");
        writer.WriteStartArray();
        foreach (var entry in result.CoverageRowsWithFinality())
        {
            WriteRowObject(writer, entry.Metadata, () => WriteCoverageRowFields(writer, entry.Row));
        }

        writer.WriteEndArray();
        writer.WritePropertyName("gap");
        writer.WriteStartArray();
        foreach (var entry in result.GapRowsWithFinality())
        {
            WriteRowObject(writer, entry.Metadata, () => WriteGapRowFields(writer, entry.Row));
        }

        writer.WriteEndArray();
        writer.WritePropertyName("symmetricDifference");
        writer.WriteStartArray();
        foreach (var entry in result.SymmetricDifferenceRowsWithFinality())
        {
            WriteRowObject(writer, entry.Metadata, () => WriteSymmetricDifferenceRowFields(writer, entry.Row));
        }

        writer.WriteEndArray();
        writer.WritePropertyName("containment");
        writer.WriteStartArray();
        foreach (var entry in result.ContainmentRowsWithFinality())
        {
            WriteRowObject(writer, entry.Metadata, () => WriteContainmentRowFields(writer, entry.Row));
        }

        writer.WriteEndArray();
        writer.WritePropertyName("leadLag");
        writer.WriteStartArray();
        foreach (var entry in result.LeadLagRowsWithFinality())
        {
            WriteRowObject(writer, entry.Metadata, () => WriteLeadLagRowFields(writer, entry.Row));
        }

        writer.WriteEndArray();
        writer.WritePropertyName("asOf");
        writer.WriteStartArray();
        foreach (var entry in result.AsOfRowsWithFinality())
        {
            WriteRowObject(writer, entry.Metadata, () => WriteAsOfRowFields(writer, entry.Row));
        }

        writer.WriteEndArray();
        writer.WriteEndObject();
    }

    private static void WriteRowObject(
        Utf8JsonWriter writer,
        ComparisonRowFinality metadata,
        Action writeFields)
    {
        writer.WriteStartObject();
        writer.WriteString("rowId", metadata.RowId);
        writer.WriteString("finality", metadata.Finality.ToString());
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
        WriteIds(writer, "boundaryRecordIds", row.BoundaryRecordIds);
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
        if (window.TimestampClock is not null)
        {
            writer.WriteString("timestampClock", window.TimestampClock);
        }
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

    private static void WriteSegments(
        Utf8JsonWriter writer,
        string propertyName,
        IReadOnlyList<WindowSegment> segments)
    {
        writer.WritePropertyName(propertyName);
        writer.WriteStartArray();
        for (var i = 0; i < segments.Count; i++)
        {
            writer.WriteStartObject();
            writer.WriteString("name", segments[i].Name);
            WriteObjectValue(writer, "value", segments[i].Value);
            if (segments[i].ParentName is null)
            {
                writer.WriteNull("parentName");
            }
            else
            {
                writer.WriteString("parentName", segments[i].ParentName);
            }

            writer.WriteEndObject();
        }

        writer.WriteEndArray();
    }

    private static void WriteTags(
        Utf8JsonWriter writer,
        string propertyName,
        IReadOnlyList<WindowTag> tags)
    {
        writer.WritePropertyName(propertyName);
        writer.WriteStartArray();
        for (var i = 0; i < tags.Count; i++)
        {
            writer.WriteStartObject();
            writer.WriteString("name", tags[i].Name);
            WriteObjectValue(writer, "value", tags[i].Value);
            writer.WriteEndObject();
        }

        writer.WriteEndArray();
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
            byte[] bytes => Convert.ToHexString(bytes),
            DateTime dateTime => dateTime.ToUniversalTime().ToString("O", CultureInfo.InvariantCulture),
            DateTimeOffset dateTimeOffset => dateTimeOffset.ToUniversalTime().ToString("O", CultureInfo.InvariantCulture),
            TimeSpan timeSpan => timeSpan.ToString("c", CultureInfo.InvariantCulture),
            Guid guid => guid.ToString("D"),
            IFormattable formattable => formattable.ToString(null, CultureInfo.InvariantCulture),
            _ => value.ToString() ?? string.Empty
        };
    }
}
