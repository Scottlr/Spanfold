using Spanfold.Comparison;

namespace Spanfold.Assessment;

/// <summary>
/// Builds a closed set of portable comparison-assessment rules.
/// </summary>
public sealed class AssessmentRuleBuilder
{
    private readonly List<AssessmentRule> rules = [];

    /// <summary>Adds a minimum aggregate coverage rule.</summary>
    public AssessmentRuleBuilder MinimumCoverage(double minimumRatio, string id = "minimum-coverage")
    {
        this.rules.Add(new MinimumCoverageRule(id, minimumRatio));
        return this;
    }

    /// <summary>Adds a maximum residual-magnitude rule.</summary>
    public AssessmentRuleBuilder MaximumResidualMagnitude(
        long maximumMagnitude,
        AssessmentAggregation aggregation = AssessmentAggregation.Single,
        string id = "maximum-residual-magnitude")
    {
        this.rules.Add(new MaximumResidualMagnitudeRule(id, maximumMagnitude, aggregation));
        return this;
    }

    /// <summary>Adds a maximum gap-magnitude rule.</summary>
    public AssessmentRuleBuilder MaximumGapMagnitude(
        long maximumMagnitude,
        AssessmentAggregation aggregation = AssessmentAggregation.Single,
        string id = "maximum-gap-magnitude")
    {
        this.rules.Add(new MaximumGapMagnitudeRule(id, maximumMagnitude, aggregation));
        return this;
    }

    /// <summary>Adds a maximum absolute lead/lag rule.</summary>
    public AssessmentRuleBuilder MaximumAbsoluteLeadLag(
        long maximumMagnitude,
        string id = "maximum-absolute-lead-lag")
    {
        this.rules.Add(new MaximumAbsoluteLeadLagRule(id, maximumMagnitude));
        return this;
    }

    /// <summary>Adds a diagnostic allow-list rule.</summary>
    public AssessmentRuleBuilder AllowDiagnostics(
        IEnumerable<ComparisonPlanValidationCode> allowedCodes,
        string id = "allowed-diagnostics")
    {
        this.rules.Add(new AllowedDiagnosticsRule(id, allowedCodes));
        return this;
    }

    /// <summary>Requires every materialized row to be final.</summary>
    public AssessmentRuleBuilder RequireFinalRows(string id = "require-final-rows")
    {
        this.rules.Add(new RequireFinalRowsRule(id));
        return this;
    }

    internal IReadOnlyList<AssessmentRule> Build()
    {
        return this.rules.ToArray();
    }
}
