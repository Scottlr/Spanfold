namespace Spanfold.Assessment;

/// <summary>
/// Selects how row magnitudes are evaluated by an assessment rule.
/// </summary>
public enum AssessmentAggregation
{
    /// <summary>Each row must satisfy the configured limit.</summary>
    Single = 0,

    /// <summary>The sum of every matching row must satisfy the configured limit.</summary>
    Total = 1
}
