using Spanfold.Comparison;

namespace Spanfold.Assessment;

/// <summary>
/// Represents one closed, portable assessment rule.
/// </summary>
public abstract record AssessmentRule
{
    /// <summary>Creates an assessment rule.</summary>
    protected AssessmentRule(string id)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(id);
        Id = id;
    }

    /// <summary>Gets the stable rule identifier.</summary>
    public string Id { get; }
}

/// <summary>Requires every aggregate coverage summary to meet a minimum ratio.</summary>
public sealed record MinimumCoverageRule : AssessmentRule
{
    /// <summary>Creates a minimum-coverage rule.</summary>
    public MinimumCoverageRule(string id, double minimumRatio)
        : base(id)
    {
        if (!double.IsFinite(minimumRatio) || minimumRatio is < 0d or > 1d)
        {
            throw new ArgumentOutOfRangeException(nameof(minimumRatio), "Coverage ratio must be between zero and one.");
        }

        MinimumRatio = minimumRatio;
    }

    /// <summary>Gets the inclusive minimum coverage ratio.</summary>
    public double MinimumRatio { get; }
}

/// <summary>Limits residual-row magnitude.</summary>
public sealed record MaximumResidualMagnitudeRule : AssessmentRule
{
    /// <summary>Creates a residual-magnitude rule.</summary>
    public MaximumResidualMagnitudeRule(string id, long maximumMagnitude, AssessmentAggregation aggregation)
        : base(id)
    {
        ArgumentOutOfRangeException.ThrowIfNegative(maximumMagnitude);
        if (!Enum.IsDefined(aggregation))
        {
            throw new ArgumentOutOfRangeException(nameof(aggregation));
        }

        MaximumMagnitude = maximumMagnitude;
        Aggregation = aggregation;
    }

    /// <summary>Gets the inclusive maximum magnitude.</summary>
    public long MaximumMagnitude { get; }

    /// <summary>Gets how row magnitudes are aggregated.</summary>
    public AssessmentAggregation Aggregation { get; }
}

/// <summary>Limits gap-row magnitude.</summary>
public sealed record MaximumGapMagnitudeRule : AssessmentRule
{
    /// <summary>Creates a gap-magnitude rule.</summary>
    public MaximumGapMagnitudeRule(string id, long maximumMagnitude, AssessmentAggregation aggregation)
        : base(id)
    {
        ArgumentOutOfRangeException.ThrowIfNegative(maximumMagnitude);
        if (!Enum.IsDefined(aggregation))
        {
            throw new ArgumentOutOfRangeException(nameof(aggregation));
        }

        MaximumMagnitude = maximumMagnitude;
        Aggregation = aggregation;
    }

    /// <summary>Gets the inclusive maximum magnitude.</summary>
    public long MaximumMagnitude { get; }

    /// <summary>Gets how row magnitudes are aggregated.</summary>
    public AssessmentAggregation Aggregation { get; }
}

/// <summary>Limits the absolute delta emitted by lead/lag rows.</summary>
public sealed record MaximumAbsoluteLeadLagRule : AssessmentRule
{
    /// <summary>Creates a lead/lag rule.</summary>
    public MaximumAbsoluteLeadLagRule(string id, long maximumMagnitude)
        : base(id)
    {
        ArgumentOutOfRangeException.ThrowIfNegative(maximumMagnitude);
        MaximumMagnitude = maximumMagnitude;
    }

    /// <summary>Gets the inclusive maximum absolute delta.</summary>
    public long MaximumMagnitude { get; }
}

/// <summary>Rejects diagnostics not present in an explicit allow-list.</summary>
public sealed record AllowedDiagnosticsRule : AssessmentRule
{
    /// <summary>Creates an allowed-diagnostics rule.</summary>
    public AllowedDiagnosticsRule(string id, IEnumerable<ComparisonPlanValidationCode> allowedCodes)
        : base(id)
    {
        ArgumentNullException.ThrowIfNull(allowedCodes);
        AllowedCodes = Array.AsReadOnly(allowedCodes.Distinct().Order().ToArray());
    }

    /// <summary>Gets the allowed diagnostic codes.</summary>
    public IReadOnlyList<ComparisonPlanValidationCode> AllowedCodes { get; }
}

/// <summary>Requires every materialized comparison row to be final.</summary>
public sealed record RequireFinalRowsRule : AssessmentRule
{
    /// <summary>Creates a final-row rule.</summary>
    public RequireFinalRowsRule(string id)
        : base(id)
    {
    }
}
