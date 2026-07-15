using Spanfold.Comparison;

namespace Spanfold.Assessment;

/// <summary>
/// Groups named assessment specifications evaluated over one comparison snapshot.
/// </summary>
public sealed class AssessmentSuite
{
    /// <summary>Creates an assessment suite.</summary>
    public AssessmentSuite(string name, IEnumerable<AssessmentSpecification> specifications)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(name);
        ArgumentNullException.ThrowIfNull(specifications);

        Name = name;
        var materialized = specifications.ToArray();
        if (materialized.Length == 0)
        {
            throw new ArgumentException("An assessment suite requires at least one specification.", nameof(specifications));
        }

        var duplicate = materialized
            .GroupBy(static specification => specification.Name, StringComparer.Ordinal)
            .FirstOrDefault(static group => group.Count() > 1);
        if (duplicate is not null)
        {
            throw new ArgumentException("Assessment specification names must be unique: " + duplicate.Key, nameof(specifications));
        }

        Specifications = Array.AsReadOnly(materialized);
    }

    /// <summary>Gets the suite name.</summary>
    public string Name { get; }

    /// <summary>Gets specifications in declaration order.</summary>
    public IReadOnlyList<AssessmentSpecification> Specifications { get; }

    /// <summary>Evaluates every specification over one comparison result.</summary>
    public AssessmentSuiteResult Evaluate(ComparisonResult result)
    {
        ArgumentNullException.ThrowIfNull(result);
        return new AssessmentSuiteResult(
            this,
            Specifications.Select(result.Assess));
    }
}
