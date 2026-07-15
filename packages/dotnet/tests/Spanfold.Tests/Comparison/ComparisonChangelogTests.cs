using Spanfold;

namespace Spanfold.Tests.Comparison;

public sealed class ComparisonChangelogTests
{
    [Fact]
    public void LateCloseRevisesOpenAtHorizonRow()
    {
        var previous = new[]
        {
            new ComparisonRowFinality(new ComparisonRowReference(ComparisonRowKind.Residual, "residual[0]"), ComparisonFinality.Provisional,
                "Depends on at least one open window clipped to the evaluation horizon.")
        };
        var current = new[]
        {
            new ComparisonRowFinality(new ComparisonRowReference(ComparisonRowKind.Residual, "residual[0]"), ComparisonFinality.Final,
                "All contributing windows were closed when the row was produced.")
        };

        var entry = Assert.Single(ComparisonChangelog.Create(previous, current));

        Assert.Equal("residual[0]", entry.Row.RowId);
        Assert.Equal(2, entry.Version);
        Assert.Equal(ComparisonRevisionKind.Revised, entry.Kind);
        Assert.Equal(ComparisonFinality.Provisional, entry.PreviousFinality);
        Assert.Equal(ComparisonFinality.Final, entry.CurrentFinality);
        Assert.Equal("residual[0]", entry.SupersedesRowId);
    }

    [Fact]
    public void RetractionRemovesPreviouslyEmittedRow()
    {
        var previous = new[]
        {
            new ComparisonRowFinality(new ComparisonRowReference(ComparisonRowKind.Residual, "residual[0]"), ComparisonFinality.Provisional, "open")
        };

        var entry = Assert.Single(ComparisonChangelog.Create(previous, []));

        Assert.Equal(ComparisonRevisionKind.Retracted, entry.Kind);
        Assert.Equal(2, entry.Version);
        Assert.Equal("residual[0]", entry.SupersedesRowId);
    }

    [Fact]
    public void ChangelogReplayProducesCurrentSnapshotMetadata()
    {
        var previous = new[]
        {
            new ComparisonRowFinality(new ComparisonRowReference(ComparisonRowKind.Residual, "residual[0]"), ComparisonFinality.Provisional, "open"),
            new ComparisonRowFinality(new ComparisonRowReference(ComparisonRowKind.Missing, "missing[0]"), ComparisonFinality.Final, "closed")
        };
        var current = new[]
        {
            new ComparisonRowFinality(new ComparisonRowReference(ComparisonRowKind.Residual, "residual[0]"), ComparisonFinality.Final, "closed")
        };

        var entries = ComparisonChangelog.Create(previous, current);
        var replayed = ComparisonChangelog.Replay(previous, entries);

        var row = Assert.Single(replayed);
        Assert.Equal("residual[0]", row.RowId);
        Assert.Equal(ComparisonFinality.Final, row.Finality);
        Assert.Equal(2, row.Version);
        Assert.Equal("residual[0]", row.SupersedesRowId);
    }

    [Fact]
    public void ChangelogReplayPreservesFinalToProvisionalRevision()
    {
        var previous = new[]
        {
            new ComparisonRowFinality(new ComparisonRowReference(ComparisonRowKind.Residual, "residual[0]"), ComparisonFinality.Final, "closed")
        };
        var current = new[]
        {
            new ComparisonRowFinality(new ComparisonRowReference(ComparisonRowKind.Residual, "residual[0]"), ComparisonFinality.Provisional, "reopened")
        };

        var replayed = ComparisonChangelog.Replay(previous, ComparisonChangelog.Create(previous, current));

        var row = Assert.Single(replayed);
        Assert.Equal(ComparisonFinality.Provisional, row.Finality);
        Assert.Equal("reopened", row.Reason);
    }

    [Fact]
    public void RowVersionsAreDeterministic()
    {
        var previous = new[]
        {
            new ComparisonRowFinality(new ComparisonRowReference(ComparisonRowKind.Coverage, "coverage[1]"), ComparisonFinality.Provisional, "open"),
            new ComparisonRowFinality(new ComparisonRowReference(ComparisonRowKind.Coverage, "coverage[0]"), ComparisonFinality.Provisional, "open")
        };
        var current = new[]
        {
            new ComparisonRowFinality(new ComparisonRowReference(ComparisonRowKind.Coverage, "coverage[1]"), ComparisonFinality.Final, "closed"),
            new ComparisonRowFinality(new ComparisonRowReference(ComparisonRowKind.Coverage, "coverage[0]"), ComparisonFinality.Final, "closed")
        };

        var first = ComparisonChangelog.Create(previous, current);
        var second = ComparisonChangelog.Create(previous.Reverse(), current.Reverse());

        Assert.Equal(first, second);
        Assert.All(first, entry => Assert.Equal(2, entry.Version));
        Assert.Collection(
            first,
            entry => Assert.Equal("coverage[0]", entry.Row.RowId),
            entry => Assert.Equal("coverage[1]", entry.Row.RowId));
    }
}
