using Spanfold;

namespace Spanfold.Tests.Comparison;

public sealed class ComparisonPlanTests
{
    [Fact]
    public void MinimalCompletePlanIsValid()
    {
        var plan = CreatePlan();

        Assert.Empty(plan.Validate());
        Assert.True(plan.IsSerializable);
        Assert.Equal("Provider QA", plan.Name);
        Assert.Equal("overlap", Assert.Single(plan.Comparators));
    }

    [Fact]
    public void MissingTargetIsInvalid()
    {
        var plan = new ComparisonPlan(
            "Provider QA",
            target: null,
            [ComparisonSelector.ForSource("provider-b")],
            ComparisonScope.Window("DeviceOffline"),
            ComparisonNormalizationPolicy.Default,
            ["overlap"]
            );

        var diagnostic = Assert.Single(plan.Validate(), d => d.Code == ComparisonPlanValidationCode.MissingTarget);
        Assert.Equal("target", diagnostic.Path);
    }

    [Fact]
    public void MissingAgainstIsInvalid()
    {
        var plan = CreatePlan(against: []);

        var diagnostic = Assert.Single(plan.Validate(), d => d.Code == ComparisonPlanValidationCode.MissingAgainst);
        Assert.Equal("against", diagnostic.Path);
    }

    [Fact]
    public void MissingComparatorIsInvalid()
    {
        var plan = CreatePlan(comparators: []);

        var diagnostic = Assert.Single(plan.Validate(), d => d.Code == ComparisonPlanValidationCode.MissingComparator);
        Assert.Equal("comparators", diagnostic.Path);
    }

    [Fact]
    public void UnknownComparatorIsRejectedDuringPlanValidation()
    {
        var plan = CreatePlan(comparators: ["unknown"]);

        var diagnostic = Assert.Single(plan.Validate(), d => d.Code == ComparisonPlanValidationCode.UnknownComparator);

        Assert.Equal("comparators[0]", diagnostic.Path);
        Assert.Equal(ComparisonPlanDiagnosticSeverity.Error, diagnostic.Severity);
    }

    [Fact]
    public void RuntimeOnlySelectorsAreDiagnosed()
    {
        var plan = CreatePlan(
            target: ComparisonSelector.RuntimeOnly("provider-a", "runtime provider selector"),
            against:
            [
                ComparisonSelector.ForSource("provider-b"),
                ComparisonSelector.RuntimeOnly("provider-c", "runtime provider selector")
            ]);

        var diagnostics = plan.Validate()
            .Where(d => d.Code == ComparisonPlanValidationCode.NonSerializableSelector)
            .ToArray();

        Assert.False(plan.IsSerializable);
        Assert.Collection(
            diagnostics,
            target => Assert.Equal("target", target.Path),
            against => Assert.Equal("against[1]", against.Path));
    }

    [Fact]
    public void CollectionsAreMaterializedWhenPlanIsCreated()
    {
        var against = new List<ComparisonSelector>
        {
            ComparisonSelector.ForSource("provider-b")
        };
        var comparators = new List<string> { "overlap" };

        var plan = CreatePlan(against: against, comparators: comparators);

        against.Add(ComparisonSelector.ForSource("provider-c"));
        comparators.Add("coverage");

        Assert.Single(plan.Against);
        Assert.Equal("overlap", Assert.Single(plan.Comparators));
    }

    [Fact]
    public void CallerOwnedSelectorArraysCannotMutatePlanExecution()
    {
        var against = new[] { ComparisonSelector.ForSource("provider-b") };
        var plan = CreatePlan(against: against);

        against[0] = ComparisonSelector.ForSource("provider-c");

        var window = new ClosedWindow(
            "DeviceOffline",
            "device-1",
            StartPosition: 1,
            EndPosition: 2,
            Source: "provider-b");
        Assert.True(plan.Against[0].Matches(window));
        Assert.False(plan.Against is ComparisonSelector[]);
    }

    [Fact]
    public void DuplicateComparatorDeclarationsAreCollapsed()
    {
        var plan = CreatePlan(comparators: ["overlap", "coverage", "overlap"]);

        Assert.Equal(["overlap", "coverage"], plan.Comparators);
    }

    private static ComparisonPlan CreatePlan(
        ComparisonSelector? target = null,
        IEnumerable<ComparisonSelector>? against = null,
        IEnumerable<string>? comparators = null)
    {
        return new ComparisonPlan(
            "Provider QA",
            target ?? ComparisonSelector.ForSource("provider-a"),
            against ?? [ComparisonSelector.ForSource("provider-b")],
            ComparisonScope.Window("DeviceOffline"),
            ComparisonNormalizationPolicy.Default,
            comparators ?? ["overlap"]
            );
    }
}
