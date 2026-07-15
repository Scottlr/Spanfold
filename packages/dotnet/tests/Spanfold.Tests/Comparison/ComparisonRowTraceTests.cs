using Spanfold.Testing;

namespace Spanfold.Tests.Comparison;

public sealed class ComparisonRowTraceTests
{
    [Fact]
    public void ResidualTracePreservesTypedRowAndAuthoritativeLineage()
    {
        var history = new WindowHistoryFixtureBuilder()
            .AddClosedWindow("Offline", "device-1", 0, 10, source: "target")
            .AddClosedWindow("Offline", "device-1", 0, 7, source: "against")
            .Build();
        var result = history.Compare("trace")
            .Target("target", selector => selector.Source("target"))
            .Against("against", selector => selector.Source("against"))
            .Within(scope => scope.Window("Offline"))
            .Using(comparators => comparators.Residual())
            .Run();

        var row = Assert.Single(result.ResidualRowsWithFinality());
        var trace = result.TraceRow(row);

        Assert.Equal(row.Row, trace.Row);
        Assert.Equal(row.Metadata.Reference, trace.Reference);
        Assert.Single(trace.ContributingRecords);
        Assert.Single(trace.NormalizedWindows);
        Assert.Contains(trace.AlignedSegments, segment => segment.Range == row.Row.Range);
    }

    [Fact]
    public void UnknownReferenceFailsClosed()
    {
        var result = new WindowHistoryFixtureBuilder()
            .AddClosedWindow("Offline", "device-1", 0, 1, source: "target")
            .Build()
            .Compare("trace")
            .Target("target", selector => selector.Source("target"))
            .Against("against", selector => selector.Source("against"))
            .Within(scope => scope.Window("Offline"))
            .Using(comparators => comparators.Residual())
            .Run();

        Assert.Throws<KeyNotFoundException>(() => result.TraceRow(
            new ComparisonRowReference(ComparisonRowKind.Residual, "missing-row")));
    }
}
