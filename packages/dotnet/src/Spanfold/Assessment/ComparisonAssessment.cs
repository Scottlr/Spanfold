namespace Spanfold.Assessment;

/// <summary>
/// Represents deterministic acceptance results for one comparison snapshot.
/// </summary>
public sealed class ComparisonAssessment
{
    internal ComparisonAssessment(
        AssessmentSpecification specification,
        IEnumerable<AssessmentViolation> violations)
    {
        Specification = specification;
        Violations = Array.AsReadOnly(violations.ToArray());
    }

    /// <summary>Gets the specification that was evaluated.</summary>
    public AssessmentSpecification Specification { get; }

    /// <summary>Gets violations in deterministic rule and row order.</summary>
    public IReadOnlyList<AssessmentViolation> Violations { get; }

    /// <summary>Gets whether every configured expectation passed.</summary>
    public bool Passed => Violations.Count == 0;
}
