using Spanfold;

namespace Spanfold.Tests.Comparison;

public sealed class SourceMatrixTests
{
    [Fact]
    public void ThreeSourceMatrixProducesExpectedPairs()
    {
        var history = BuildHistory();

        var matrix = history.CompareSources(
            "Provider matrix",
            "DeviceOffline",
            ["provider-a", "provider-b", "provider-c"]);

        Assert.Equal(3, matrix.Sources.Count);
        Assert.Equal(9, matrix.Cells.Count);
        Assert.Contains(matrix.Cells, cell =>
            Equals(cell.TargetSource, "provider-a")
            && Equals(cell.AgainstSource, "provider-b")
            && !cell.IsDiagonal);
        Assert.Contains(matrix.Cells, cell =>
            Equals(cell.TargetSource, "provider-c")
            && Equals(cell.AgainstSource, "provider-c")
            && cell.IsDiagonal
            && cell.CoverageRatio is null);
    }

    [Fact]
    public void MissingSourceCellsAreExplicit()
    {
        var history = BuildHistory();

        var matrix = history.CompareSources(
            "Provider matrix",
            "DeviceOffline",
            ["provider-a", "provider-c"]);

        var cell = Assert.Single(matrix.Cells, cell =>
            Equals(cell.TargetSource, "provider-a") && Equals(cell.AgainstSource, "provider-c"));

        Assert.True(cell.TargetHasWindows);
        Assert.False(cell.AgainstHasWindows);
        Assert.Equal(0, cell.OverlapRowCount);
        Assert.Equal(1, cell.ResidualRowCount);
        Assert.Equal(0, cell.MissingRowCount);
    }

    [Fact]
    public void MatrixValuesMatchUnderlyingComparatorRows()
    {
        var history = BuildHistory();
        var matrix = history.CompareSources(
            "Provider matrix",
            "DeviceOffline",
            ["provider-a", "provider-b"]);
        var pair = Assert.Single(matrix.Cells, cell =>
            Equals(cell.TargetSource, "provider-a") && Equals(cell.AgainstSource, "provider-b"));

        var result = history.Compare("Provider matrix provider-a vs provider-b")
            .Target("provider-a", selector => selector.Source("provider-a"))
            .Against("provider-b", selector => selector.Source("provider-b"))
            .Within(scope => scope.Window("DeviceOffline"))
            .Using(comparators => comparators.Overlap().Residual().Missing().Coverage())
            .Run();

        Assert.Equal(result.OverlapRows.Count, pair.OverlapRowCount);
        Assert.Equal(result.ResidualRows.Count, pair.ResidualRowCount);
        Assert.Equal(result.MissingRows.Count, pair.MissingRowCount);
        Assert.Equal(result.CoverageRows.Count, pair.CoverageRowCount);
    }

    [Fact]
    public void MatrixCellsCanBeLookedUpDirectionally()
    {
        var matrix = BuildHistory().CompareSources(
            "Provider matrix",
            "DeviceOffline",
            ["provider-a", "provider-b"]);

        var cell = matrix.GetCell("provider-a", "provider-b");

        Assert.False(cell.IsDiagonal);
        Assert.True(matrix.TryGetCell("provider-b", "provider-a", out var reverse));
        Assert.NotNull(reverse);
        Assert.False(matrix.TryGetCell("provider-a", "provider-c", out _));
    }

    [Fact]
    public void DuplicateSourceIdentitiesAreRejected()
    {
        var exception = Assert.Throws<ArgumentException>(() =>
            BuildHistory().CompareSources(
                "Provider matrix",
                "DeviceOffline",
                ["provider-a", "provider-a"]));

        Assert.Contains("must be unique", exception.Message);
    }

    private static WindowHistory BuildHistory()
    {
        var pipeline = Spanfold
            .For<DeviceSignal>()
            .RecordWindows()
            .TrackWindow("DeviceOffline", signal => signal.DeviceId, signal => !signal.IsOnline);

        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: false), source: "provider-a");
        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: true), source: "provider-a");
        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: false), source: "provider-b");
        pipeline.Ingest(new DeviceSignal("device-1", IsOnline: true), source: "provider-b");

        return pipeline.History;
    }

    private sealed record DeviceSignal(string DeviceId, bool IsOnline);
}
