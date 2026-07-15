using Spanfold;

namespace Spanfold.Tests.Comparison;

public sealed class HierarchyComparisonTests
{
    [Fact]
    public void ParentCanBeFullyExplainedByChildWindows()
    {
        var history = BuildHistory(
            new WindowInput("Parent", "parent-1", 1, 5),
            new WindowInput("Child", "child-1", 1, 5));

        var result = history.CompareHierarchy("Hierarchy QA", "Parent", "Child");

        var row = Assert.Single(result.Rows);
        Assert.Equal(HierarchyComparisonRowKind.ParentExplained, row.Kind);
        Assert.Equal(1, row.Range.Start.Position);
        Assert.Equal(5, row.Range.End!.Value.Position);
        Assert.Empty(result.Diagnostics);
    }

    [Fact]
    public void ParentCanBePartiallyUnexplained()
    {
        var history = BuildHistory(
            new WindowInput("Parent", "parent-1", 1, 7),
            new WindowInput("Child", "child-1", 3, 5));

        var result = history.CompareHierarchy("Hierarchy QA", "Parent", "Child");

        Assert.Collection(
            result.Rows,
            first => Assert.Equal(HierarchyComparisonRowKind.UnexplainedParent, first.Kind),
            second => Assert.Equal(HierarchyComparisonRowKind.ParentExplained, second.Kind),
            third => Assert.Equal(HierarchyComparisonRowKind.UnexplainedParent, third.Kind));
    }

    [Fact]
    public void ChildOutsideParentIsReportedAsOrphan()
    {
        var history = BuildHistory(
            new WindowInput("Parent", "parent-1", 3, 5),
            new WindowInput("Child", "child-1", 1, 7));

        var result = history.CompareHierarchy("Hierarchy QA", "Parent", "Child");

        Assert.Collection(
            result.Rows,
            first => Assert.Equal(HierarchyComparisonRowKind.OrphanChild, first.Kind),
            second => Assert.Equal(HierarchyComparisonRowKind.ParentExplained, second.Kind),
            third => Assert.Equal(HierarchyComparisonRowKind.OrphanChild, third.Kind));
    }

    [Fact]
    public void MissingLineageProducesDiagnostic()
    {
        var history = BuildHistory(new WindowInput("Parent", "parent-1", 1, 5));

        var result = history.CompareHierarchy("Hierarchy QA", "Parent", "Child");

        Assert.Contains(result.Diagnostics, diagnostic =>
            diagnostic.Code == ComparisonPlanValidationCode.MissingLineage);
        Assert.Single(result.Rows);
        Assert.Equal(HierarchyComparisonRowKind.UnexplainedParent, result.Rows[0].Kind);
    }

    [Fact]
    public void OpenWindowsProduceBoundednessDiagnostic()
    {
        var history = BuildHistoryWithOpenWindow();

        var result = history.CompareHierarchy("Hierarchy QA", "Parent", "Child");

        var diagnostic = Assert.Single(result.Diagnostics);
        Assert.Equal(ComparisonPlanValidationCode.HierarchyOpenWindowsWithoutHorizon, diagnostic.Code);
        Assert.Empty(result.Rows);
    }

    [Fact]
    public void MultiLevelRollupPathRemainsDeterministic()
    {
        var pipeline = EventPipeline
            .For<PriceTick>()
            .RecordWindows()
            .Window(
                "SelectionSuspension",
                key: tick => tick.SelectionId,
                isActive: tick => tick.Price == 0m)
            .RollUp(
                "MarketSuspension",
                key: tick => tick.MarketId,
                isActive: children => children.AnyActive())
            .RollUp(
                "FixtureSuspension",
                key: tick => tick.FixtureId,
                isActive: children => children.AnyActive())
            .Build();

        pipeline.Ingest(new PriceTick("selection-1", "market-1", "fixture-1", 0m));
        pipeline.Ingest(new PriceTick("selection-1", "market-1", "fixture-1", 1.01m));

        var first = pipeline.History.CompareHierarchy("Market explanation", "MarketSuspension", "SelectionSuspension");
        var second = pipeline.History.CompareHierarchy("Market explanation", "MarketSuspension", "SelectionSuspension");

        Assert.Equal(first.Rows.Count, second.Rows.Count);
        Assert.Equal(first.Rows[0].Kind, second.Rows[0].Kind);
        Assert.Equal(first.Rows[0].Range, second.Rows[0].Range);
        Assert.Single(first.Rows);
        Assert.Equal(HierarchyComparisonRowKind.ParentExplained, first.Rows[0].Kind);
    }

    private static WindowHistory BuildHistory(params WindowInput[] windows)
    {
        var constructor = typeof(WindowHistory).GetConstructor(
            System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic,
            binder: null,
            [typeof(bool)],
            modifiers: null)!;
        var history = (WindowHistory)constructor.Invoke([true]);
        var field = typeof(WindowHistory).GetField(
            "closedWindows",
            System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic)!;
        var closed = (List<ClosedWindow>)field.GetValue(history)!;

        for (var i = 0; i < windows.Length; i++)
        {
            var window = windows[i];
            closed.Add(new ClosedWindow(
                window.WindowName,
                window.Key,
                window.Start,
                window.End,
                Source: "source-a"));
        }

        return history;
    }

    private static WindowHistory BuildHistoryWithOpenWindow()
    {
        var parent = new OpenWindow("Parent", "parent-1", 1, Source: "source-a");
        var child = new OpenWindow("Child", "child-1", 1, Source: "source-a");
        return WindowHistory.FromRecords([], [parent, child]);
    }

    private sealed record WindowInput(string WindowName, string Key, long Start, long End);

    private sealed record PriceTick(
        string SelectionId,
        string MarketId,
        string FixtureId,
        decimal Price);
}
