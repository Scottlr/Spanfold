using Spanfold.Comparison;

namespace Spanfold.Assessment;

/// <summary>
/// Describes one deterministic assessment failure and its supporting rows.
/// </summary>
public sealed record AssessmentViolation
{
    /// <summary>Creates an assessment violation.</summary>
    public AssessmentViolation(
        string ruleId,
        string code,
        string message,
        IEnumerable<ComparisonRowReference>? evidence = null,
        double? actual = null,
        double? expected = null)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(ruleId);
        ArgumentException.ThrowIfNullOrWhiteSpace(code);
        ArgumentException.ThrowIfNullOrWhiteSpace(message);

        RuleId = ruleId;
        Code = code;
        Message = message;
        Evidence = Array.AsReadOnly((evidence ?? []).Distinct().ToArray());
        Actual = actual;
        Expected = expected;
    }

    /// <summary>Gets the stable rule identifier.</summary>
    public string RuleId { get; }

    /// <summary>Gets the stable violation code.</summary>
    public string Code { get; }

    /// <summary>Gets the deterministic readable message.</summary>
    public string Message { get; }

    /// <summary>Gets authoritative comparison rows supporting the violation.</summary>
    public IReadOnlyList<ComparisonRowReference> Evidence { get; }

    /// <summary>Gets the measured value when the rule is numeric.</summary>
    public double? Actual { get; }

    /// <summary>Gets the configured limit when the rule is numeric.</summary>
    public double? Expected { get; }
}
