using Spanfold;

namespace Spanfold.Tests.Comparison;

public sealed class ComparisonExplainTests
{
    [Fact]
    public void PlanExplainIncludesNameAndWarnings()
    {
        var plan = new ComparisonPlan(
            "Runtime selector QA",
            ComparisonSelector.RuntimeOnly("dynamic-target", "uses an in-memory predicate"),
            [ComparisonSelector.ForSource("provider-b")],
            ComparisonScope.Window("DeviceOffline"),
            ComparisonNormalizationPolicy.Default,
            ["overlap"]
            );

        var first = plan.Explain();
        var second = plan.Explain();

        Assert.Equal(first, second);
        Assert.Contains("# Comparison Explain: Runtime selector QA", first);
        Assert.Contains("diagnostic[0]: Warning NonSerializableSelector", first);
        Assert.Contains("target: dynamic-target", first);
    }

    [Fact]
    public void ResultExplainIncludesRowIdsAndRecordIds()
    {
        var target = new ClosedWindow("DeviceOffline", "device-1", StartPosition: 1, EndPosition: 5, Source: "provider-a");
        var against = new ClosedWindow("DeviceOffline", "device-1", StartPosition: 3, EndPosition: 7, Source: "provider-b");
        var result = CreateResult(target, against);

        var explanation = result.Explain();

        Assert.Contains("selected[0]: record=" + target.Id, explanation);
        Assert.Contains("selected[1]: record=" + against.Id, explanation);
        Assert.Contains("normalized[0]: record=" + target.Id, explanation);
        Assert.Contains("segment[1]:", explanation);
        Assert.Contains("overlap[0]:", explanation);
        Assert.Contains(target.Id.ToString(), explanation);
        Assert.Contains(against.Id.ToString(), explanation);
    }

    [Fact]
    public void PlainTextExplainOmitsMarkdownHeadings()
    {
        var plan = new ComparisonPlan(
            "Plain text QA",
            ComparisonSelector.ForSource("provider-a"),
            [ComparisonSelector.ForSource("provider-b")],
            ComparisonScope.Window("DeviceOffline"),
            ComparisonNormalizationPolicy.Default,
            ["overlap"]
            );

        var explanation = plan.Explain(ComparisonExplanationFormat.PlainText);

        Assert.StartsWith("Comparison Explain: Plain text QA", explanation);
        Assert.DoesNotContain("# Comparison Explain", explanation);
    }

    [Fact]
    public void MarkdownExplainEscapesUntrustedValues()
    {
        var plan = new ComparisonPlan(
            "# plan\n- forged",
            ComparisonSelector.ForSource("provider-a"),
            [ComparisonSelector.ForSource("provider-b")],
            ComparisonScope.Window("Device_[Offline]"),
            ComparisonNormalizationPolicy.Default,
            ["overlap"]
            );

        var explanation = plan.Explain(ComparisonExplanationFormat.Markdown);

        Assert.Contains("\\# plan\\n- forged", explanation);
        Assert.DoesNotContain("\n- forged", explanation, StringComparison.Ordinal);
    }

    private static ComparisonResult CreateResult(ClosedWindow target, ClosedWindow against)
    {
        var plan = new ComparisonPlan(
            "Provider QA",
            ComparisonSelector.ForSource("provider-a"),
            [ComparisonSelector.ForSource("provider-b")],
            ComparisonScope.Window("DeviceOffline"),
            ComparisonNormalizationPolicy.Default,
            ["overlap"]
            );
        var prepared = new PreparedComparison(
            plan,
            [],
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

        return new ComparisonResult(
            plan,
            [],
            prepared,
            aligned,
            [new ComparatorSummary("overlap", 1)],
            [
                new OverlapRow(
                    overlap.WindowName,
                    overlap.Key,
                    overlap.Partition,
                    overlap.Range,
                    overlap.TargetRecordIds,
                    overlap.AgainstRecordIds)
            ]);
    }
}
