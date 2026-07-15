using Spanfold;

namespace Spanfold.Tests.Comparison;

public sealed class ComparisonPlanDiagnosticTests
{
    [Fact]
    public void StructuralDiagnosticsAreErrors()
    {
        var plan = new ComparisonPlan(
            "",
            target: null,
            against: [],
            scope: null,
            normalization: null,
            comparators: []);

        Assert.All(plan.Validate(), diagnostic =>
            Assert.Equal(ComparisonPlanDiagnosticSeverity.Error, diagnostic.Severity));
    }

    [Fact]
    public void RuntimeOnlySelectorsAreWarningsByDefault()
    {
        var plan = PlanWithRuntimeOnlySelector(isStrict: false);

        var diagnostic = Assert.Single(plan.Validate());

        Assert.Equal(ComparisonPlanValidationCode.NonSerializableSelector, diagnostic.Code);
        Assert.Equal(ComparisonPlanDiagnosticSeverity.Warning, diagnostic.Severity);
    }

    [Fact]
    public void StrictModePromotesRuntimeOnlySelectorsToErrors()
    {
        var plan = PlanWithRuntimeOnlySelector(isStrict: true);

        var diagnostic = Assert.Single(plan.Validate());

        Assert.Equal(ComparisonPlanValidationCode.NonSerializableSelector, diagnostic.Code);
        Assert.Equal(ComparisonPlanDiagnosticSeverity.Error, diagnostic.Severity);
    }

    [Fact]
    public void ResultValidityDependsOnErrorSeverity()
    {
        var warningOnly = new ComparisonResult(
            PlanWithRuntimeOnlySelector(isStrict: false),
            [new ComparisonPlanDiagnostic(
                ComparisonPlanValidationCode.NonSerializableSelector,
                "Runtime selector.",
                "target",
                ComparisonPlanDiagnosticSeverity.Warning)]);
        var error = new ComparisonResult(
            PlanWithRuntimeOnlySelector(isStrict: true),
            [new ComparisonPlanDiagnostic(
                ComparisonPlanValidationCode.MissingTarget,
                "Missing target.",
                "target",
                ComparisonPlanDiagnosticSeverity.Error)]);

        Assert.True(warningOnly.IsValid);
        Assert.False(error.IsValid);
    }

    private static ComparisonPlan PlanWithRuntimeOnlySelector(bool isStrict)
    {
        return new ComparisonPlan(
            "Provider QA",
            ComparisonSelector.RuntimeOnly("provider-a", "runtime provider selector"),
            [ComparisonSelector.ForSource("provider-b")],
            ComparisonScope.Window("DeviceOffline"),
            ComparisonNormalizationPolicy.Default,
            ["overlap"],
            isStrict);
    }
}
