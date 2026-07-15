using System.Globalization;
using Spanfold.Assessment;

namespace Spanfold.Revisions;

/// <summary>
/// Represents the semantic difference between two immutable comparison snapshots.
/// </summary>
public sealed class ComparisonRevision
{
    private ComparisonRevision(
        IEnumerable<ComparisonChangelogEntry> rows,
        IEnumerable<CoverageSummaryRevision> coverageSummaries,
        IEnumerable<LeadLagSummaryRevision> leadLagSummaries,
        IEnumerable<AssessmentViolationRevision> assessmentViolations)
    {
        Rows = Array.AsReadOnly(rows.ToArray());
        CoverageSummaries = Array.AsReadOnly(coverageSummaries.ToArray());
        LeadLagSummaries = Array.AsReadOnly(leadLagSummaries.ToArray());
        AssessmentViolations = Array.AsReadOnly(assessmentViolations.ToArray());
    }

    /// <summary>Gets added, revised, and retracted comparison rows.</summary>
    public IReadOnlyList<ComparisonChangelogEntry> Rows { get; }

    /// <summary>Gets changed aggregate coverage scopes.</summary>
    public IReadOnlyList<CoverageSummaryRevision> CoverageSummaries { get; }

    /// <summary>Gets changed lead/lag summaries.</summary>
    public IReadOnlyList<LeadLagSummaryRevision> LeadLagSummaries { get; }

    /// <summary>Gets introduced, revised, and resolved assessment violations.</summary>
    public IReadOnlyList<AssessmentViolationRevision> AssessmentViolations { get; }

    /// <summary>Gets whether the snapshots are semantically equivalent in the compared domains.</summary>
    public bool IsEmpty => Rows.Count == 0
        && CoverageSummaries.Count == 0
        && LeadLagSummaries.Count == 0
        && AssessmentViolations.Count == 0;

    /// <summary>Creates a semantic revision between two comparison snapshots.</summary>
    public static ComparisonRevision Between(
        ComparisonResult previous,
        ComparisonResult current,
        ComparisonAssessment? previousAssessment = null,
        ComparisonAssessment? currentAssessment = null)
    {
        ArgumentNullException.ThrowIfNull(previous);
        ArgumentNullException.ThrowIfNull(current);

        if ((previousAssessment is null) != (currentAssessment is null))
        {
            throw new ArgumentException("Assessment revisions require both previous and current assessments.");
        }

        if (previousAssessment is not null
            && !StringComparer.Ordinal.Equals(
                previousAssessment.Specification.Name,
                currentAssessment!.Specification.Name))
        {
            throw new ArgumentException("Assessment revisions require the same specification name.");
        }

        return new ComparisonRevision(
            ComparisonChangelog.Create(previous.RowFinalities, current.RowFinalities),
            CompareCoverage(previous.CoverageSummaries, current.CoverageSummaries),
            CompareLeadLag(previous.LeadLagSummaries, current.LeadLagSummaries),
            CompareAssessments(previousAssessment, currentAssessment));
    }

    private static IEnumerable<CoverageSummaryRevision> CompareCoverage(
        IEnumerable<CoverageSummary> previous,
        IEnumerable<CoverageSummary> current)
    {
        var previousByKey = previous.ToDictionary(CoverageKey.Create);
        var currentByKey = current.ToDictionary(CoverageKey.Create);
        foreach (var key in previousByKey.Keys.Union(currentByKey.Keys).OrderBy(static key => key.SortKey, StringComparer.Ordinal))
        {
            previousByKey.TryGetValue(key, out var before);
            currentByKey.TryGetValue(key, out var after);
            if (Equals(before, after))
            {
                continue;
            }

            yield return new CoverageSummaryRevision(
                key.WindowName,
                after?.Key ?? before!.Key,
                after?.Partition ?? before!.Partition,
                before?.TargetMagnitudeExact,
                after?.TargetMagnitudeExact,
                before?.CoveredMagnitudeExact,
                after?.CoveredMagnitudeExact,
                before?.CoverageRatio,
                after?.CoverageRatio);
        }
    }

    private static IEnumerable<LeadLagSummaryRevision> CompareLeadLag(
        IEnumerable<LeadLagSummary> previous,
        IEnumerable<LeadLagSummary> current)
    {
        var previousByKey = IndexLeadLag(previous);
        var currentByKey = IndexLeadLag(current);
        foreach (var key in previousByKey.Keys.Union(currentByKey.Keys).OrderBy(static key => key))
        {
            previousByKey.TryGetValue(key, out var before);
            currentByKey.TryGetValue(key, out var after);
            if (!Equals(before, after))
            {
                yield return new LeadLagSummaryRevision(key.Transition, key.Axis, key.Tolerance, before, after);
            }
        }
    }

    private static Dictionary<LeadLagKey, LeadLagSummary> IndexLeadLag(IEnumerable<LeadLagSummary> summaries)
    {
        var occurrences = new Dictionary<(LeadLagTransition, TemporalAxis, long), int>();
        var indexed = new Dictionary<LeadLagKey, LeadLagSummary>();
        foreach (var summary in summaries)
        {
            var identity = (summary.Transition, summary.Axis, summary.ToleranceMagnitude);
            occurrences.TryGetValue(identity, out var occurrence);
            occurrences[identity] = occurrence + 1;
            indexed.Add(new LeadLagKey(identity.Item1, identity.Item2, identity.Item3, occurrence), summary);
        }

        return indexed;
    }

    private static IEnumerable<AssessmentViolationRevision> CompareAssessments(
        ComparisonAssessment? previous,
        ComparisonAssessment? current)
    {
        if (previous is null || current is null)
        {
            yield break;
        }

        var previousByKey = IndexViolations(previous.Violations);
        var currentByKey = IndexViolations(current.Violations);
        foreach (var key in previousByKey.Keys.Union(currentByKey.Keys).OrderBy(static key => key, StringComparer.Ordinal))
        {
            previousByKey.TryGetValue(key, out var before);
            currentByKey.TryGetValue(key, out var after);
            if (before is null)
            {
                yield return new AssessmentViolationRevision(ComparisonRevisionKind.Added, null, after);
            }
            else if (after is null)
            {
                yield return new AssessmentViolationRevision(ComparisonRevisionKind.Retracted, before, null);
            }
            else if (!Equals(before, after))
            {
                yield return new AssessmentViolationRevision(ComparisonRevisionKind.Revised, before, after);
            }
        }
    }

    private static Dictionary<string, AssessmentViolation> IndexViolations(
        IEnumerable<AssessmentViolation> violations)
    {
        var occurrences = new Dictionary<string, int>(StringComparer.Ordinal);
        var indexed = new Dictionary<string, AssessmentViolation>(StringComparer.Ordinal);
        foreach (var violation in violations)
        {
            var identity = ViolationKey.Create(violation);
            occurrences.TryGetValue(identity, out var occurrence);
            occurrences[identity] = occurrence + 1;
            indexed.Add($"{identity}\u001d{occurrence}", violation);
        }

        return indexed;
    }

    private readonly record struct CoverageKey(string WindowName, object Key, object? Partition, string SortKey)
    {
        public static CoverageKey Create(CoverageSummary value)
        {
            var key = Format(value.Key);
            var partition = Format(value.Partition);
            return new(value.WindowName, value.Key, value.Partition, $"{value.WindowName}\u001f{key}\u001f{partition}");
        }

        private static string Format(object? value) => value is null
            ? "<null>"
            : $"{value.GetType().FullName}:{Convert.ToString(value, CultureInfo.InvariantCulture)}";
    }

    private readonly record struct LeadLagKey(
        LeadLagTransition Transition,
        TemporalAxis Axis,
        long Tolerance,
        int Occurrence)
        : IComparable<LeadLagKey>
    {
        public int CompareTo(LeadLagKey other)
        {
            var transition = Transition.CompareTo(other.Transition);
            var axis = Axis.CompareTo(other.Axis);
            var tolerance = Tolerance.CompareTo(other.Tolerance);
            return transition != 0
                ? transition
                : axis != 0
                    ? axis
                    : tolerance != 0
                        ? tolerance
                        : Occurrence.CompareTo(other.Occurrence);
        }
    }

    private static class ViolationKey
    {
        public static string Create(AssessmentViolation value) => string.Join(
            "\u001f",
            value.RuleId,
            value.Code,
            string.Join("\u001e", value.Evidence
                .OrderBy(static row => row.Kind)
                .ThenBy(static row => row.RowId, StringComparer.Ordinal)
                .Select(static row => $"{row.Kind}:{row.RowId}")));
    }
}
