namespace Spanfold.Assessment;

/// <summary>
/// Represents a named, portable set of comparison acceptance rules.
/// </summary>
public sealed class AssessmentSpecification
{
    /// <summary>Creates an assessment specification from materialized rules.</summary>
    public AssessmentSpecification(string name, IEnumerable<AssessmentRule> rules)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(name);
        ArgumentNullException.ThrowIfNull(rules);

        Name = name;
        var materialized = rules.ToArray();
        if (materialized.Length == 0)
        {
            throw new ArgumentException("An assessment specification requires at least one rule.", nameof(rules));
        }

        var duplicate = materialized
            .GroupBy(static rule => rule.Id, StringComparer.Ordinal)
            .FirstOrDefault(static group => group.Count() > 1);
        if (duplicate is not null)
        {
            throw new ArgumentException("Assessment rule IDs must be unique: " + duplicate.Key, nameof(rules));
        }

        Rules = Array.AsReadOnly(materialized);
    }

    /// <summary>Gets the specification name.</summary>
    public string Name { get; }

    /// <summary>Gets the rules in declaration order.</summary>
    public IReadOnlyList<AssessmentRule> Rules { get; }

    /// <summary>Creates a specification with the fluent rule builder.</summary>
    public static AssessmentSpecification Create(
        string name,
        Func<AssessmentRuleBuilder, AssessmentRuleBuilder> configure)
    {
        ArgumentNullException.ThrowIfNull(configure);
        var rules = configure(new AssessmentRuleBuilder()).Build();
        return new AssessmentSpecification(name, rules);
    }
}
