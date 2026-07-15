namespace Spanfold.Assessment;

/// <summary>
/// Represents all assessment results produced by a named suite.
/// </summary>
public sealed class AssessmentSuiteResult
{
    internal AssessmentSuiteResult(
        AssessmentSuite suite,
        IEnumerable<ComparisonAssessment> assessments)
    {
        Suite = suite;
        Assessments = Array.AsReadOnly(assessments.ToArray());
    }

    /// <summary>Gets the evaluated suite.</summary>
    public AssessmentSuite Suite { get; }

    /// <summary>Gets assessment results in specification order.</summary>
    public IReadOnlyList<ComparisonAssessment> Assessments { get; }

    /// <summary>Gets whether every assessment passed.</summary>
    public bool Passed => Assessments.All(static assessment => assessment.Passed);
}
