using Spanfold;
using Spanfold.Internal.Analysis;
using Spanfold.Internal.Comparison;

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

    [Theory]
    [InlineData(TemporalAxis.Unknown)]
    [InlineData((TemporalAxis)999)]
    public void UnknownAndUndefinedScopeAxesAreRejected(TemporalAxis timeAxis)
    {
        var plan = CreatePlan(scope: ComparisonScope.Window("DeviceOffline", timeAxis));

        var diagnostic = Assert.Single(
            plan.Validate(),
            diagnostic => diagnostic.Code == ComparisonPlanValidationCode.InvalidTemporalAxis);

        Assert.Equal("scope.timeAxis", diagnostic.Path);
        Assert.Equal(ComparisonPlanDiagnosticSeverity.Error, diagnostic.Severity);
    }

    [Theory]
    [InlineData(TemporalAxis.Unknown)]
    [InlineData((TemporalAxis)999)]
    public void UnknownAndUndefinedNormalizationAxesAreRejected(TemporalAxis timeAxis)
    {
        var plan = CreatePlan(normalization: ComparisonNormalizationPolicy.Default with
        {
            TimeAxis = timeAxis
        });

        var diagnostic = Assert.Single(
            plan.Validate(),
            diagnostic => diagnostic.Code == ComparisonPlanValidationCode.InvalidTemporalAxis);

        Assert.Equal("normalization.timeAxis", diagnostic.Path);
        Assert.Equal(ComparisonPlanDiagnosticSeverity.Error, diagnostic.Severity);
    }

    [Fact]
    public void UndefinedOpenWindowPolicyIsRejected()
    {
        var plan = CreatePlan(normalization: ComparisonNormalizationPolicy.Default with
        {
            OpenWindowPolicy = (ComparisonOpenWindowPolicy)999
        });

        var diagnostic = Assert.Single(
            plan.Validate(),
            diagnostic => diagnostic.Code == ComparisonPlanValidationCode.InvalidOpenWindowPolicy);

        Assert.Equal("normalization.openWindowPolicy", diagnostic.Path);
    }

    [Fact]
    public void UndefinedNullTimestampPolicyIsRejected()
    {
        var plan = CreatePlan(normalization: ComparisonNormalizationPolicy.Default with
        {
            NullTimestampPolicy = (ComparisonNullTimestampPolicy)999
        });

        var diagnostic = Assert.Single(
            plan.Validate(),
            diagnostic => diagnostic.Code == ComparisonPlanValidationCode.InvalidNullTimestampPolicy);

        Assert.Equal("normalization.nullTimestampPolicy", diagnostic.Path);
    }

    [Fact]
    public void ClipPolicyRequiresMatchingHorizon()
    {
        var missingHorizonPlan = CreatePlan(normalization: ComparisonNormalizationPolicy.Default with
        {
            OpenWindowPolicy = ComparisonOpenWindowPolicy.ClipToHorizon
        });
        var wrongAxisPlan = CreatePlan(normalization: ComparisonNormalizationPolicy.Default with
        {
            OpenWindowPolicy = ComparisonOpenWindowPolicy.ClipToHorizon,
            OpenWindowHorizon = TemporalPoint.ForTimestamp(DateTimeOffset.UnixEpoch)
        });

        var missingHorizon = Assert.Single(
            missingHorizonPlan.Validate(),
            diagnostic => diagnostic.Code == ComparisonPlanValidationCode.OpenWindowsWithoutPolicy);
        var wrongAxis = Assert.Single(
            wrongAxisPlan.Validate(),
            diagnostic => diagnostic.Code == ComparisonPlanValidationCode.MixedTimeAxes);

        Assert.Equal("normalization.openWindowHorizon", missingHorizon.Path);
        Assert.Equal("normalization.openWindowHorizon", wrongAxis.Path);
    }

    [Fact]
    public void RequireClosedPolicyRejectsUnusedHorizon()
    {
        var plan = CreatePlan(normalization: ComparisonNormalizationPolicy.Default with
        {
            OpenWindowHorizon = TemporalPoint.ForPosition(10)
        });

        var diagnostic = Assert.Single(
            plan.Validate(),
            diagnostic => diagnostic.Code == ComparisonPlanValidationCode.InvalidNormalizationPolicy);

        Assert.Equal("normalization.openWindowHorizon", diagnostic.Path);
    }

    [Fact]
    public void ProcessingPositionPolicyRejectsEventTimeExclusion()
    {
        var plan = CreatePlan(normalization: ComparisonNormalizationPolicy.Default with
        {
            NullTimestampPolicy = ComparisonNullTimestampPolicy.Exclude
        });

        var diagnostic = Assert.Single(
            plan.Validate(),
            diagnostic => diagnostic.Code == ComparisonPlanValidationCode.InvalidNormalizationPolicy);

        Assert.Equal("normalization.nullTimestampPolicy", diagnostic.Path);
    }

    [Fact]
    public void InvalidTemporalPlanStopsBeforeWindowPreparation()
    {
        var window = new ClosedWindow(
            "DeviceOffline",
            "device-1",
            StartPosition: 1,
            EndPosition: 2,
            Source: "provider-a");
        var history = WindowHistory.FromRecords([window], []);
        var plan = CreatePlan(
            scope: ComparisonScope.Window("DeviceOffline", TemporalAxis.Unknown),
            normalization: ComparisonNormalizationPolicy.Default with
            {
                TimeAxis = TemporalAxis.Unknown
            });

        var prepared = ComparisonPreparer.Prepare(history, plan);

        Assert.Empty(prepared.SelectedWindows);
        Assert.Empty(prepared.ExcludedWindows);
        Assert.Empty(prepared.NormalizedWindows);
        Assert.Equal(2, prepared.Diagnostics.Count(
            diagnostic => diagnostic.Code == ComparisonPlanValidationCode.InvalidTemporalAxis));
    }

    [Fact]
    public void WindowNormalizerNeverTreatsUnknownAxisAsProcessingPosition()
    {
        var window = new ClosedWindow(
            "DeviceOffline",
            "device-1",
            StartPosition: 1,
            EndPosition: 2);
        var policy = ComparisonNormalizationPolicy.Default with
        {
            TimeAxis = TemporalAxis.Unknown
        };
        var knownAt = TemporalPoint.ForPosition(0);

        var exception = Assert.Throws<ArgumentOutOfRangeException>(() =>
            WindowRangeNormalizer.TryNormalize(window, policy, knownAt, out _, out _));

        Assert.Equal("policy", exception.ParamName);
    }

    [Theory]
    [InlineData(TemporalAxis.ProcessingPosition)]
    [InlineData(TemporalAxis.Timestamp)]
    public void DefinedMatchingAxesRemainValid(TemporalAxis timeAxis)
    {
        var plan = CreatePlan(
            scope: ComparisonScope.Window("DeviceOffline", timeAxis),
            normalization: ComparisonNormalizationPolicy.Default with
            {
                TimeAxis = timeAxis
            });

        Assert.Empty(plan.Validate());
    }

    private static ComparisonPlan CreatePlan(
        ComparisonSelector? target = null,
        IEnumerable<ComparisonSelector>? against = null,
        IEnumerable<string>? comparators = null,
        ComparisonScope? scope = null,
        ComparisonNormalizationPolicy? normalization = null)
    {
        return new ComparisonPlan(
            "Provider QA",
            target ?? ComparisonSelector.ForSource("provider-a"),
            against ?? [ComparisonSelector.ForSource("provider-b")],
            scope ?? ComparisonScope.Window("DeviceOffline"),
            normalization ?? ComparisonNormalizationPolicy.Default,
            comparators ?? ["overlap"]
            );
    }
}
