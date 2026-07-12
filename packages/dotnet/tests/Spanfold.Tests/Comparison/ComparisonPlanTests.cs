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
            [ComparisonSelector.Serializable("provider-b", "source = provider-b")],
            ComparisonScope.Window("DeviceOffline"),
            ComparisonNormalizationPolicy.Default,
            ["overlap"],
            ComparisonOutputOptions.Default);

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
                ComparisonSelector.Serializable("provider-b", "source = provider-b"),
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
            ComparisonSelector.Serializable("provider-b", "source = provider-b")
        };
        var comparators = new List<string> { "overlap" };

        var plan = CreatePlan(against: against, comparators: comparators);

        against.Add(ComparisonSelector.Serializable("provider-c", "source = provider-c"));
        comparators.Add("coverage");

        Assert.Single(plan.Against);
        Assert.Equal("overlap", Assert.Single(plan.Comparators));
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
            target ?? ComparisonSelector.Serializable("provider-a", "source = provider-a"),
            against ?? [ComparisonSelector.Serializable("provider-b", "source = provider-b")],
            ComparisonScope.Window("DeviceOffline"),
            ComparisonNormalizationPolicy.Default,
            comparators ?? ["overlap"],
            ComparisonOutputOptions.Default);
    }
}
