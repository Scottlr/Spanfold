using System.Globalization;

using Spanfold.Comparison;

namespace Spanfold.Assessment;

/// <summary>
/// Evaluates portable acceptance specifications over comparison snapshots.
/// </summary>
public static class ComparisonAssessmentExtensions
{
    /// <summary>
    /// Evaluates a comparison result without changing its execution-validity semantics.
    /// </summary>
    public static ComparisonAssessment Assess(
        this ComparisonResult result,
        AssessmentSpecification specification)
    {
        ArgumentNullException.ThrowIfNull(result);
        ArgumentNullException.ThrowIfNull(specification);

        var violations = new List<AssessmentViolation>();
        if (!result.IsValid)
        {
            violations.Add(new AssessmentViolation(
                "$comparison",
                "comparison.invalid",
                "The comparison contains error diagnostics and cannot satisfy an assessment."));
        }

        for (var index = 0; index < specification.Rules.Count; index++)
        {
            EvaluateRule(result, specification.Rules[index], violations);
        }

        return new ComparisonAssessment(specification, violations);
    }

    private static void EvaluateRule(
        ComparisonResult result,
        AssessmentRule rule,
        List<AssessmentViolation> violations)
    {
        switch (rule)
        {
            case MinimumCoverageRule coverage:
                EvaluateMinimumCoverage(result, coverage, violations);
                break;
            case MaximumResidualMagnitudeRule residual:
                EvaluateMagnitude(
                    residual,
                    result.ResidualRowsWithFinality()
                        .Select(static row => new RowMagnitude(row.Metadata.Reference, GetMagnitude(row.Row.Range))),
                    residual.MaximumMagnitude,
                    residual.Aggregation,
                    "residual",
                    violations);
                break;
            case MaximumGapMagnitudeRule gap:
                EvaluateMagnitude(
                    gap,
                    result.GapRowsWithFinality()
                        .Select(static row => new RowMagnitude(row.Metadata.Reference, GetMagnitude(row.Row.Range))),
                    gap.MaximumMagnitude,
                    gap.Aggregation,
                    "gap",
                    violations);
                break;
            case MaximumAbsoluteLeadLagRule leadLag:
                EvaluateLeadLag(result, leadLag, violations);
                break;
            case AllowedDiagnosticsRule diagnostics:
                EvaluateDiagnostics(result, diagnostics, violations);
                break;
            case RequireFinalRowsRule finalRows:
                EvaluateFinalRows(result, finalRows, violations);
                break;
            default:
                throw new ArgumentOutOfRangeException(nameof(rule), rule.GetType(), "Unknown assessment rule type.");
        }
    }

    private static void EvaluateMinimumCoverage(
        ComparisonResult result,
        MinimumCoverageRule rule,
        List<AssessmentViolation> violations)
    {
        for (var index = 0; index < result.CoverageSummaries.Count; index++)
        {
            var summary = result.CoverageSummaries[index];
            if (summary.CoverageRatio >= rule.MinimumRatio)
            {
                continue;
            }

            var evidence = result.CoverageRowsWithFinality()
                .Where(row => string.Equals(row.Row.WindowName, summary.WindowName, StringComparison.Ordinal)
                    && Equals(row.Row.Key, summary.Key)
                    && Equals(row.Row.Partition, summary.Partition))
                .Select(static row => row.Metadata.Reference)
                .ToArray();
            violations.Add(new AssessmentViolation(
                rule.Id,
                "coverage.below-minimum",
                "Coverage for " + summary.WindowName + " was "
                    + summary.CoverageRatio.ToString("R", CultureInfo.InvariantCulture)
                    + ", below the required minimum "
                    + rule.MinimumRatio.ToString("R", CultureInfo.InvariantCulture) + ".",
                evidence,
                summary.CoverageRatio,
                rule.MinimumRatio));
        }
    }

    private static void EvaluateMagnitude(
        AssessmentRule rule,
        IEnumerable<RowMagnitude> rows,
        long maximum,
        AssessmentAggregation aggregation,
        string family,
        List<AssessmentViolation> violations)
    {
        var materialized = rows.ToArray();
        if (aggregation == AssessmentAggregation.Total)
        {
            var total = materialized.Aggregate(0m, static (sum, row) => sum + row.Magnitude);
            if (total > maximum)
            {
                violations.Add(new AssessmentViolation(
                    rule.Id,
                    family + ".total-above-maximum",
                    "Total " + family + " magnitude was "
                        + total.ToString(CultureInfo.InvariantCulture)
                        + ", above the configured maximum "
                        + maximum.ToString(CultureInfo.InvariantCulture) + ".",
                    materialized.Select(static row => row.Reference),
                    (double)total,
                    maximum));
            }

            return;
        }

        for (var index = 0; index < materialized.Length; index++)
        {
            var row = materialized[index];
            if (row.Magnitude <= maximum)
            {
                continue;
            }

            violations.Add(new AssessmentViolation(
                rule.Id,
                family + ".row-above-maximum",
                family + " row magnitude was "
                    + row.Magnitude.ToString(CultureInfo.InvariantCulture)
                    + ", above the configured maximum "
                    + maximum.ToString(CultureInfo.InvariantCulture) + ".",
                [row.Reference],
                row.Magnitude,
                maximum));
        }
    }

    private static void EvaluateLeadLag(
        ComparisonResult result,
        MaximumAbsoluteLeadLagRule rule,
        List<AssessmentViolation> violations)
    {
        foreach (var row in result.LeadLagRowsWithFinality())
        {
            if (!row.Row.DeltaMagnitude.HasValue)
            {
                violations.Add(new AssessmentViolation(
                    rule.Id,
                    "lead-lag.unmatched",
                    "Lead/lag row has no comparison transition and cannot satisfy the configured limit.",
                    [row.Metadata.Reference]));
                continue;
            }

            var magnitude = AbsoluteMagnitude(row.Row.DeltaMagnitude.Value);
            if (magnitude > rule.MaximumMagnitude)
            {
                violations.Add(new AssessmentViolation(
                    rule.Id,
                    "lead-lag.above-maximum",
                    "Absolute lead/lag magnitude was "
                        + magnitude.ToString(CultureInfo.InvariantCulture)
                        + ", above the configured maximum "
                        + rule.MaximumMagnitude.ToString(CultureInfo.InvariantCulture) + ".",
                    [row.Metadata.Reference],
                    magnitude,
                    rule.MaximumMagnitude));
            }
        }
    }

    private static void EvaluateDiagnostics(
        ComparisonResult result,
        AllowedDiagnosticsRule rule,
        List<AssessmentViolation> violations)
    {
        var allowed = rule.AllowedCodes.ToHashSet();
        for (var index = 0; index < result.Diagnostics.Count; index++)
        {
            var diagnostic = result.Diagnostics[index];
            if (!allowed.Contains(diagnostic.Code))
            {
                violations.Add(new AssessmentViolation(
                    rule.Id,
                    "diagnostic.not-allowed",
                    "Diagnostic " + diagnostic.Code + " is not allowed by the assessment specification."));
            }
        }
    }

    private static void EvaluateFinalRows(
        ComparisonResult result,
        RequireFinalRowsRule rule,
        List<AssessmentViolation> violations)
    {
        for (var index = 0; index < result.RowFinalities.Count; index++)
        {
            var metadata = result.RowFinalities[index];
            if (metadata.Finality == ComparisonFinality.Provisional)
            {
                violations.Add(new AssessmentViolation(
                    rule.Id,
                    "row.provisional",
                    "Comparison row is provisional but the assessment requires final evidence.",
                    [metadata.Reference]));
            }
        }
    }

    private static long GetMagnitude(TemporalRange range)
    {
        return range.Axis switch
        {
            TemporalAxis.ProcessingPosition => range.GetPositionLength(),
            TemporalAxis.Timestamp => range.GetTimeDuration().Ticks,
            _ => throw new InvalidOperationException("Assessment magnitude requires a known temporal axis.")
        };
    }

    private static long AbsoluteMagnitude(long value)
    {
        return value == long.MinValue ? long.MaxValue : Math.Abs(value);
    }

    private readonly record struct RowMagnitude(ComparisonRowReference Reference, long Magnitude);
}
