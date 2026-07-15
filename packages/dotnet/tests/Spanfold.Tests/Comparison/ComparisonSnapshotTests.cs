using Spanfold;
using Spanfold.Internal.Comparison;
using Spanfold.Tests.Support;

namespace Spanfold.Tests.Comparison;

public sealed class ComparisonSnapshotTests
{
    [Fact]
    public void SnapshotHelperPassesForStableResult()
    {
        var result = CreateFirstVerticalSliceResult();

        SnapshotAssert.Match("comparison-first-vertical-slice", BuildSnapshotArtifact(result));
    }

    [Fact]
    public void SnapshotHelperFailsWithUsefulDiff()
    {
        var exception = Assert.Throws<InvalidOperationException>(() =>
            SnapshotAssert.Equal("row[0]: unchanged\nrow[1]: expected\n", "row[0]: unchanged\nrow[1]: actual\n"));

        Assert.Contains("First difference at line 2.", exception.Message);
        Assert.Contains("- 2: row[1]: expected", exception.Message);
        Assert.Contains("+ 2: row[1]: actual", exception.Message);
    }

    private static string BuildSnapshotArtifact(ComparisonResult result)
    {
        return result.ExportJson()
            + "\n--- markdown ---\n"
            + result.ExportMarkdown();
    }

    private static ComparisonResult CreateFirstVerticalSliceResult()
    {
        var target = new ClosedWindow("DeviceOffline", "device-1", StartPosition: 1, EndPosition: 5, Source: "provider-a");
        var against = new ClosedWindow("DeviceOffline", "device-1", StartPosition: 3, EndPosition: 7, Source: "provider-b");
        var plan = new ComparisonPlan(
            "Provider QA",
            ComparisonSelector.ForSource("provider-a"),
            [ComparisonSelector.ForSource("provider-b")],
            ComparisonScope.Window("DeviceOffline"),
            ComparisonNormalizationPolicy.Default,
            ["overlap", "residual", "coverage"]
            );
        var diagnostics = new[]
        {
            new ComparisonPlanDiagnostic(
                ComparisonPlanValidationCode.UnknownComparator,
                "Comparator 'shape' is not registered.",
                "comparators[3]",
                ComparisonPlanDiagnosticSeverity.Warning)
        };
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
        var residual = Assert.Single(
            aligned.Segments,
            static segment => segment.TargetRecordIds.Count == 1 && segment.AgainstRecordIds.Count == 0);
        var coverageRows = aligned.Segments
            .Where(static segment => segment.TargetRecordIds.Count > 0)
            .Select(static segment =>
            {
                var length = segment.Range.GetPositionLength();
                return new CoverageRow(
                    segment.WindowName,
                    segment.Key,
                    segment.Partition,
                    segment.Range,
                    TargetMagnitude: length,
                    CoveredMagnitude: segment.AgainstRecordIds.Count > 0 ? length : 0d,
                    segment.TargetRecordIds,
                    segment.AgainstRecordIds);
            })
            .ToArray();
        var overlapRow = new OverlapRow(
            overlap.WindowName,
            overlap.Key,
            overlap.Partition,
            overlap.Range,
            overlap.TargetRecordIds,
            overlap.AgainstRecordIds);
        var residualRow = new ResidualRow(
            residual.WindowName,
            residual.Key,
            residual.Partition,
            residual.Range,
            residual.TargetRecordIds);
        var rowFinalities = new List<ComparisonRowFinality>
        {
            new(
                new ComparisonRowReference(
                    ComparisonRowKind.Overlap,
                    ComparisonRowIdentity.Create(ComparisonRowKind.Overlap, overlapRow)),
                ComparisonFinality.Final,
                "closed"),
            new(
                new ComparisonRowReference(
                    ComparisonRowKind.Residual,
                    ComparisonRowIdentity.Create(ComparisonRowKind.Residual, residualRow)),
                ComparisonFinality.Final,
                "closed")
        };
        rowFinalities.AddRange(coverageRows.Select(static row => new ComparisonRowFinality(
            new ComparisonRowReference(
                ComparisonRowKind.Coverage,
                ComparisonRowIdentity.Create(ComparisonRowKind.Coverage, row)),
            ComparisonFinality.Final,
            "closed")));

        return new ComparisonResult(
            plan,
            diagnostics,
            prepared,
            aligned,
            [
                new ComparatorSummary("overlap", 1),
                new ComparatorSummary("residual", 1),
                new ComparatorSummary("coverage", 2)
            ],
            [
                overlapRow
            ],
            [
                residualRow
            ],
            coverageRows: coverageRows,
            coverageSummaries:
            [
                new CoverageSummary(
                    "DeviceOffline",
                    "device-1",
                    null,
                    TargetMagnitude: 4,
                    CoveredMagnitude: 2,
                    CoverageRatio: 0.5)
            ],
            rowFinalities: rowFinalities);
    }
}
