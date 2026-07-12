using System.Globalization;
using System.Text;

using Spanfold;

namespace Spanfold.Internal.Comparison;

internal static class ComparisonExplainer
{
    internal static string Explain(ComparisonPlan plan, ComparisonExplanationFormat format)
    {
        var writer = new ExplainWriter(format);
        writer.Title("Comparison Explain: " + plan.Name);
        AppendPlan(writer, plan, plan.Validate());
        return writer.ToString();
    }

    internal static string Explain(PreparedComparison prepared, ComparisonExplanationFormat format)
    {
        var writer = new ExplainWriter(format);
        writer.Title("Comparison Explain: " + prepared.Plan.Name);
        AppendPlan(writer, prepared.Plan, prepared.Diagnostics);
        AppendPreparation(writer, prepared);
        return writer.ToString();
    }

    internal static string Explain(AlignedComparison aligned, ComparisonExplanationFormat format)
    {
        var writer = new ExplainWriter(format);
        writer.Title("Comparison Explain: " + aligned.Prepared.Plan.Name);
        AppendPlan(writer, aligned.Prepared.Plan, aligned.Prepared.Diagnostics);
        AppendPreparation(writer, aligned.Prepared);
        AppendAlignment(writer, aligned);
        return writer.ToString();
    }

    internal static string Explain(ComparisonResult result, ComparisonExplanationFormat format)
    {
        var writer = new ExplainWriter(format);
        writer.Title("Comparison Explain: " + result.Plan.Name);
        AppendPlan(writer, result.Plan, result.Diagnostics);

        if (!result.Plan.Output.IncludeExplainData)
        {
            writer.Section("Preparation");
            writer.Item("status", "omitted by output options");
        }
        else if (result.Prepared is null)
        {
            writer.Section("Preparation");
            writer.Item("status", "not produced");
        }
        else
        {
            AppendPreparation(writer, result.Prepared);
        }

        if (!result.Plan.Output.IncludeAlignedSegments)
        {
            writer.Section("Alignment");
            writer.Item("status", "omitted by output options");
        }
        else if (result.Aligned is null)
        {
            writer.Section("Alignment");
            writer.Item("status", "not produced");
        }
        else
        {
            AppendAlignment(writer, result.Aligned);
        }

        AppendResult(writer, result);
        return writer.ToString();
    }

    private static void AppendPlan(
        ExplainWriter writer,
        ComparisonPlan plan,
        IReadOnlyList<ComparisonPlanDiagnostic> diagnostics)
    {
        writer.Section("Plan");
        writer.Item("name", plan.Name);
        writer.Item("strict", FormatBool(plan.IsStrict));
        writer.Item("serializable", FormatBool(plan.IsSerializable));
        writer.Item("target", plan.Target.HasValue ? FormatSelector(plan.Target.Value) : "<missing>");
        writer.Item("against", plan.Against.Count.ToString(CultureInfo.InvariantCulture));

        for (var i = 0; i < plan.Against.Count; i++)
        {
            writer.Item("against[" + i.ToString(CultureInfo.InvariantCulture) + "]", FormatSelector(plan.Against[i]));
        }

        writer.Item("scope", FormatScope(plan.Scope));
        writer.Item("normalization", FormatNormalization(plan.Normalization));
        writer.Item("comparators", plan.Comparators.Count == 0 ? "<none>" : string.Join(", ", plan.Comparators));
        writer.Item("output", FormatOutput(plan.Output));
        AppendDiagnostics(writer, diagnostics);
    }

    private static void AppendPreparation(ExplainWriter writer, PreparedComparison prepared)
    {
        writer.Section("Preparation");
        writer.Item("selected windows", prepared.SelectedWindows.Count.ToString(CultureInfo.InvariantCulture));
        writer.Item("excluded windows", prepared.ExcludedWindows.Count.ToString(CultureInfo.InvariantCulture));
        writer.Item("normalized windows", prepared.NormalizedWindows.Count.ToString(CultureInfo.InvariantCulture));

        for (var i = 0; i < prepared.SelectedWindows.Count; i++)
        {
            writer.Item("selected[" + i.ToString(CultureInfo.InvariantCulture) + "]", FormatWindow(prepared.SelectedWindows[i]));
        }

        for (var i = 0; i < prepared.ExcludedWindows.Count; i++)
        {
            var excluded = prepared.ExcludedWindows[i];
            var reason = excluded.DiagnosticCode.HasValue
                ? excluded.Reason + " code=" + excluded.DiagnosticCode.Value
                : excluded.Reason;

            writer.Item(
                "excluded[" + i.ToString(CultureInfo.InvariantCulture) + "]",
                "record=" + excluded.Window.Id + "; reason=" + reason);
        }

        for (var i = 0; i < prepared.NormalizedWindows.Count; i++)
        {
            var normalized = prepared.NormalizedWindows[i];
            writer.Item(
                "normalized[" + i.ToString(CultureInfo.InvariantCulture) + "]",
                "record=" + normalized.RecordId
                    + "; side=" + normalized.Side
                    + "; selector=" + normalized.SelectorName
                    + "; range=" + FormatRange(normalized.Range));
        }
    }

    private static void AppendAlignment(ExplainWriter writer, AlignedComparison aligned)
    {
        writer.Section("Alignment");
        writer.Item("segments", aligned.Segments.Count.ToString(CultureInfo.InvariantCulture));

        for (var i = 0; i < aligned.Segments.Count; i++)
        {
            var segment = aligned.Segments[i];
            writer.Item(
                "segment[" + i.ToString(CultureInfo.InvariantCulture) + "]",
                "window=" + segment.WindowName
                    + "; key=" + StableObjectValue(segment.Key)
                    + "; partition=" + StableObjectValue(segment.Partition)
                    + "; range=" + FormatRange(segment.Range)
                    + "; target=" + FormatIds(segment.TargetRecordIds)
                    + "; against=" + FormatIds(segment.AgainstRecordIds));
        }
    }

    private static void AppendResult(ExplainWriter writer, ComparisonResult result)
    {
        writer.Section("Result");
        writer.Item("valid", FormatBool(result.IsValid));
        writer.Item("evaluation horizon", result.EvaluationHorizon.HasValue ? FormatPoint(result.EvaluationHorizon.Value) : "<none>");
        writer.Item("summaries", result.ComparatorSummaries.Count.ToString(CultureInfo.InvariantCulture));
        writer.Item("overlap rows", result.OverlapRows.Count.ToString(CultureInfo.InvariantCulture));
        writer.Item("residual rows", result.ResidualRows.Count.ToString(CultureInfo.InvariantCulture));
        writer.Item("missing rows", result.MissingRows.Count.ToString(CultureInfo.InvariantCulture));
        writer.Item("coverage rows", result.CoverageRows.Count.ToString(CultureInfo.InvariantCulture));
        writer.Item("coverage summaries", result.CoverageSummaries.Count.ToString(CultureInfo.InvariantCulture));
        writer.Item("gap rows", result.GapRows.Count.ToString(CultureInfo.InvariantCulture));
        writer.Item("symmetric difference rows", result.SymmetricDifferenceRows.Count.ToString(CultureInfo.InvariantCulture));
        writer.Item("containment rows", result.ContainmentRows.Count.ToString(CultureInfo.InvariantCulture));
        writer.Item("lead lag rows", result.LeadLagRows.Count.ToString(CultureInfo.InvariantCulture));
        writer.Item("lead lag summaries", result.LeadLagSummaries.Count.ToString(CultureInfo.InvariantCulture));
        writer.Item("as of rows", result.AsOfRows.Count.ToString(CultureInfo.InvariantCulture));
        writer.Item("row finalities", result.RowFinalities.Count.ToString(CultureInfo.InvariantCulture));
        writer.Item("extension metadata", result.ExtensionMetadata.Count.ToString(CultureInfo.InvariantCulture));

        for (var i = 0; i < result.ComparatorSummaries.Count; i++)
        {
            var summary = result.ComparatorSummaries[i];
            writer.Item(
                "summary[" + i.ToString(CultureInfo.InvariantCulture) + "]",
                summary.ComparatorName + " rows=" + summary.RowCount.ToString(CultureInfo.InvariantCulture));
        }

        for (var i = 0; i < result.RowFinalities.Count; i++)
        {
            var finality = result.RowFinalities[i];
            writer.Item(
                "finality[" + i.ToString(CultureInfo.InvariantCulture) + "]",
                finality.RowId + "=" + finality.Finality + "; reason=" + finality.Reason);
        }

        for (var i = 0; i < result.ExtensionMetadata.Count; i++)
        {
            var item = result.ExtensionMetadata[i];
            writer.Item(
                "extensionMetadata[" + i.ToString(CultureInfo.InvariantCulture) + "]",
                item.ExtensionId + "." + item.Key + "=" + item.Value);
        }

        for (var i = 0; i < result.OverlapRows.Count; i++)
        {
            var row = result.OverlapRows[i];
            writer.Item(
                "overlap[" + i.ToString(CultureInfo.InvariantCulture) + "]",
                "window=" + row.WindowName
                    + "; key=" + StableObjectValue(row.Key)
                    + "; partition=" + StableObjectValue(row.Partition)
                    + "; range=" + FormatRange(row.Range)
                    + "; target=" + FormatIds(row.TargetRecordIds)
                    + "; against=" + FormatIds(row.AgainstRecordIds));
        }

        for (var i = 0; i < result.ResidualRows.Count; i++)
        {
            var row = result.ResidualRows[i];
            writer.Item(
                "residual[" + i.ToString(CultureInfo.InvariantCulture) + "]",
                "window=" + row.WindowName
                    + "; key=" + StableObjectValue(row.Key)
                    + "; partition=" + StableObjectValue(row.Partition)
                    + "; range=" + FormatRange(row.Range)
                    + "; target=" + FormatIds(row.TargetRecordIds));
        }

        for (var i = 0; i < result.MissingRows.Count; i++)
        {
            var row = result.MissingRows[i];
            writer.Item(
                "missing[" + i.ToString(CultureInfo.InvariantCulture) + "]",
                "window=" + row.WindowName
                    + "; key=" + StableObjectValue(row.Key)
                    + "; partition=" + StableObjectValue(row.Partition)
                    + "; range=" + FormatRange(row.Range)
                    + "; against=" + FormatIds(row.AgainstRecordIds));
        }

        for (var i = 0; i < result.CoverageRows.Count; i++)
        {
            var row = result.CoverageRows[i];
            writer.Item(
                "coverage[" + i.ToString(CultureInfo.InvariantCulture) + "]",
                "window=" + row.WindowName
                    + "; key=" + StableObjectValue(row.Key)
                    + "; partition=" + StableObjectValue(row.Partition)
                    + "; range=" + FormatRange(row.Range)
                    + "; targetMagnitude=" + row.TargetMagnitude.ToString("R", CultureInfo.InvariantCulture)
                    + "; coveredMagnitude=" + row.CoveredMagnitude.ToString("R", CultureInfo.InvariantCulture)
                    + "; target=" + FormatIds(row.TargetRecordIds)
                    + "; against=" + FormatIds(row.AgainstRecordIds));
        }

        for (var i = 0; i < result.GapRows.Count; i++)
        {
            var row = result.GapRows[i];
            writer.Item(
                "gap[" + i.ToString(CultureInfo.InvariantCulture) + "]",
                "window=" + row.WindowName
                    + "; key=" + StableObjectValue(row.Key)
                    + "; partition=" + StableObjectValue(row.Partition)
                    + "; range=" + FormatRange(row.Range));
        }

        for (var i = 0; i < result.SymmetricDifferenceRows.Count; i++)
        {
            var row = result.SymmetricDifferenceRows[i];
            writer.Item(
                "symmetricDifference[" + i.ToString(CultureInfo.InvariantCulture) + "]",
                "window=" + row.WindowName
                    + "; key=" + StableObjectValue(row.Key)
                    + "; partition=" + StableObjectValue(row.Partition)
                    + "; range=" + FormatRange(row.Range)
                    + "; side=" + row.Side
                    + "; target=" + FormatIds(row.TargetRecordIds)
                    + "; against=" + FormatIds(row.AgainstRecordIds));
        }

        for (var i = 0; i < result.ContainmentRows.Count; i++)
        {
            var row = result.ContainmentRows[i];
            writer.Item(
                "containment[" + i.ToString(CultureInfo.InvariantCulture) + "]",
                "window=" + row.WindowName
                    + "; key=" + StableObjectValue(row.Key)
                    + "; partition=" + StableObjectValue(row.Partition)
                    + "; range=" + FormatRange(row.Range)
                    + "; status=" + row.Status
                    + "; target=" + FormatIds(row.TargetRecordIds)
                    + "; container=" + FormatIds(row.ContainerRecordIds));
        }

        for (var i = 0; i < result.LeadLagRows.Count; i++)
        {
            var row = result.LeadLagRows[i];
            writer.Item(
                "leadLag[" + i.ToString(CultureInfo.InvariantCulture) + "]",
                "window=" + row.WindowName
                    + "; key=" + StableObjectValue(row.Key)
                    + "; partition=" + StableObjectValue(row.Partition)
                    + "; transition=" + row.Transition
                    + "; axis=" + row.Axis
                    + "; targetPoint=" + FormatPoint(row.TargetPoint)
                    + "; comparisonPoint=" + (row.ComparisonPoint.HasValue ? FormatPoint(row.ComparisonPoint.Value) : "<missing>")
                    + "; delta=" + (row.DeltaMagnitude?.ToString(CultureInfo.InvariantCulture) ?? "<missing>")
                    + "; tolerance=" + row.ToleranceMagnitude.ToString(CultureInfo.InvariantCulture)
                    + "; withinTolerance=" + FormatBool(row.IsWithinTolerance)
                    + "; direction=" + row.Direction
                    + "; target=" + row.TargetRecordId
                    + "; comparison=" + (row.ComparisonRecordId?.ToString() ?? "<missing>"));
        }

        for (var i = 0; i < result.LeadLagSummaries.Count; i++)
        {
            var summary = result.LeadLagSummaries[i];
            writer.Item(
                "leadLagSummary[" + i.ToString(CultureInfo.InvariantCulture) + "]",
                "transition=" + summary.Transition
                    + "; axis=" + summary.Axis
                    + "; tolerance=" + summary.ToleranceMagnitude.ToString(CultureInfo.InvariantCulture)
                    + "; rows=" + summary.RowCount.ToString(CultureInfo.InvariantCulture)
                    + "; targetLead=" + summary.TargetLeadCount.ToString(CultureInfo.InvariantCulture)
                    + "; targetLag=" + summary.TargetLagCount.ToString(CultureInfo.InvariantCulture)
                    + "; equal=" + summary.EqualCount.ToString(CultureInfo.InvariantCulture)
                    + "; missingComparison=" + summary.MissingComparisonCount.ToString(CultureInfo.InvariantCulture)
                    + "; outsideTolerance=" + summary.OutsideToleranceCount.ToString(CultureInfo.InvariantCulture)
                    + "; minDelta=" + (summary.MinimumDeltaMagnitude?.ToString(CultureInfo.InvariantCulture) ?? "<none>")
                    + "; maxDelta=" + (summary.MaximumDeltaMagnitude?.ToString(CultureInfo.InvariantCulture) ?? "<none>"));
        }

        for (var i = 0; i < result.AsOfRows.Count; i++)
        {
            var row = result.AsOfRows[i];
            writer.Item(
                "asOf[" + i.ToString(CultureInfo.InvariantCulture) + "]",
                "window=" + row.WindowName
                    + "; key=" + StableObjectValue(row.Key)
                    + "; partition=" + StableObjectValue(row.Partition)
                    + "; axis=" + row.Axis
                    + "; direction=" + row.Direction
                    + "; targetPoint=" + FormatPoint(row.TargetPoint)
                    + "; matchedPoint=" + (row.MatchedPoint.HasValue ? FormatPoint(row.MatchedPoint.Value) : "<missing>")
                    + "; distance=" + (row.DistanceMagnitude?.ToString(CultureInfo.InvariantCulture) ?? "<missing>")
                    + "; tolerance=" + row.ToleranceMagnitude.ToString(CultureInfo.InvariantCulture)
                    + "; status=" + row.Status
                    + "; target=" + row.TargetRecordId
                    + "; match=" + (row.MatchedRecordId?.ToString() ?? "<missing>"));
        }

        for (var i = 0; i < result.CoverageSummaries.Count; i++)
        {
            var summary = result.CoverageSummaries[i];
            writer.Item(
                "coverageSummary[" + i.ToString(CultureInfo.InvariantCulture) + "]",
                "window=" + summary.WindowName
                    + "; key=" + StableObjectValue(summary.Key)
                    + "; partition=" + StableObjectValue(summary.Partition)
                    + "; targetMagnitude=" + summary.TargetMagnitude.ToString("R", CultureInfo.InvariantCulture)
                    + "; coveredMagnitude=" + summary.CoveredMagnitude.ToString("R", CultureInfo.InvariantCulture)
                    + "; ratio=" + summary.CoverageRatio.ToString("R", CultureInfo.InvariantCulture));
        }
    }

    private static void AppendDiagnostics(
        ExplainWriter writer,
        IReadOnlyList<ComparisonPlanDiagnostic> diagnostics)
    {
        writer.Item("diagnostics", diagnostics.Count.ToString(CultureInfo.InvariantCulture));

        for (var i = 0; i < diagnostics.Count; i++)
        {
            var diagnostic = diagnostics[i];
            writer.Item(
                "diagnostic[" + i.ToString(CultureInfo.InvariantCulture) + "]",
                diagnostic.Severity
                    + " "
                    + diagnostic.Code
                    + " at "
                    + diagnostic.Path
                    + ": "
                    + diagnostic.Message);
        }
    }

    private static string FormatSelector(ComparisonSelector selector)
    {
        var value = selector.Name
            + " (" + selector.Description
            + "; serializable=" + FormatBool(selector.IsSerializable);

        if (selector.CohortActivity is not null)
        {
            value += "; cohort=" + selector.CohortActivity.Name;
            if (selector.CohortActivity.Count.HasValue)
            {
                value += ":" + selector.CohortActivity.Count.Value.ToString(CultureInfo.InvariantCulture);
            }
        }

        return value + ")";
    }

    private static string FormatScope(ComparisonScope? scope)
    {
        if (scope is null)
        {
            return "<missing>";
        }

        var parts = new List<string>
        {
            "window=" + (scope.WindowName ?? "<all>"),
            "axis=" + scope.TimeAxis
        };

        if (scope.SegmentFilters.Count > 0)
        {
            parts.Add("segments=" + string.Join(
                ", ",
                scope.SegmentFilters.Select(static filter => filter.Name + "=" + StableObjectValue(filter.Value))));
        }

        if (scope.TagFilters.Count > 0)
        {
            parts.Add("tags=" + string.Join(
                ", ",
                scope.TagFilters.Select(static filter => filter.Name + "=" + StableObjectValue(filter.Value))));
        }

        return string.Join("; ", parts);
    }

    private static string FormatNormalization(ComparisonNormalizationPolicy policy)
    {
        return "axis=" + policy.TimeAxis
            + "; openPolicy=" + policy.OpenWindowPolicy
            + "; horizon=" + (policy.OpenWindowHorizon.HasValue ? FormatPoint(policy.OpenWindowHorizon.Value) : "<none>")
            + "; nullTimestamp=" + policy.NullTimestampPolicy
            + "; knownAt=" + (policy.KnownAt.HasValue ? FormatPoint(policy.KnownAt.Value) : "<none>");
    }

    private static string FormatOutput(ComparisonOutputOptions output)
    {
        return "alignedSegments=" + FormatBool(output.IncludeAlignedSegments)
            + "; explainData=" + FormatBool(output.IncludeExplainData);
    }

    private static string FormatWindow(WindowRecord window)
    {
        return "record=" + window.Id
            + "; window=" + window.WindowName
            + "; key=" + StableObjectValue(window.Key)
            + "; source=" + StableObjectValue(window.Source)
            + "; partition=" + StableObjectValue(window.Partition)
            + "; startPosition=" + window.StartPosition.ToString(CultureInfo.InvariantCulture)
            + "; endPosition=" + (window.EndPosition?.ToString(CultureInfo.InvariantCulture) ?? "<open>")
            + "; startTime=" + FormatTimestamp(window.StartTime)
            + "; endTime=" + FormatTimestamp(window.EndTime);
    }

    private static string FormatRange(TemporalRange range)
    {
        return "[" + FormatPoint(range.Start)
            + ", " + (range.End.HasValue ? FormatPoint(range.End.Value) : "<open>")
            + ") endStatus=" + range.EndStatus;
    }

    private static string FormatPoint(TemporalPoint point)
    {
        return point.Axis switch
        {
            TemporalAxis.ProcessingPosition => "pos:" + point.Position.ToString(CultureInfo.InvariantCulture),
            TemporalAxis.Timestamp => "time:" + point.Timestamp.ToUniversalTime().ToString("O", CultureInfo.InvariantCulture)
                + (point.Clock is null ? string.Empty : " clock=" + point.Clock),
            _ => "unknown"
        };
    }

    private static string FormatTimestamp(DateTimeOffset? timestamp)
    {
        return timestamp?.ToUniversalTime().ToString("O", CultureInfo.InvariantCulture) ?? "<null>";
    }

    private static string FormatIds(IReadOnlyList<WindowRecordId> ids)
    {
        if (ids.Count == 0)
        {
            return "[]";
        }

        var builder = new StringBuilder();
        builder.Append('[');
        for (var i = 0; i < ids.Count; i++)
        {
            if (i > 0)
            {
                builder.Append(", ");
            }

            builder.Append(ids[i]);
        }

        builder.Append(']');
        return builder.ToString();
    }

    private static string StableObjectValue(object? value)
    {
        return value switch
        {
            null => "<null>",
            IFormattable formattable => value.GetType().FullName + ":" + formattable.ToString(null, CultureInfo.InvariantCulture),
            _ => value.GetType().FullName + ":" + value
        };
    }

    private static string FormatBool(bool value)
    {
        return value ? "true" : "false";
    }

    private sealed class ExplainWriter
    {
        private readonly StringBuilder builder = new();
        private readonly ComparisonExplanationFormat format;

        internal ExplainWriter(ComparisonExplanationFormat format)
        {
            this.format = format;
        }

        internal void Title(string value)
        {
            if (this.format == ComparisonExplanationFormat.Markdown)
            {
                this.builder.Append("# ");
            }

            this.builder.AppendLine(this.FormatValue(value));
        }

        internal void Section(string value)
        {
            this.builder.AppendLine();
            if (this.format == ComparisonExplanationFormat.Markdown)
            {
                this.builder.Append("## ");
            }

            this.builder.AppendLine(this.FormatValue(value));
        }

        internal void Item(string name, string value)
        {
            if (this.format == ComparisonExplanationFormat.Markdown)
            {
                this.builder.Append("- ");
            }

            this.builder
                .Append(name)
                .Append(": ")
                .AppendLine(this.FormatValue(value));
        }

        private string FormatValue(string value)
        {
            return this.format == ComparisonExplanationFormat.Markdown
                ? EscapeMarkdown(value)
                : value;
        }

        private static string EscapeMarkdown(string value)
        {
            var escaped = new StringBuilder(value.Length);
            foreach (var character in value)
            {
                switch (character)
                {
                    case '\\':
                        escaped.Append(@"\\");
                        break;
                    case '\r':
                        escaped.Append(@"\r");
                        break;
                    case '\n':
                        escaped.Append(@"\n");
                        break;
                    case '*':
                    case '_':
                    case '[':
                    case ']':
                    case '(':
                    case ')':
                    case '#':
                    case '|':
                    case '`':
                        escaped.Append('\\').Append(character);
                        break;
                    default:
                        escaped.Append(character);
                        break;
                }
            }

            return escaped.ToString();
        }

        public override string ToString()
        {
            return this.builder.ToString();
        }
    }
}
