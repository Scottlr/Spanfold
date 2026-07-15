namespace Spanfold.Comparison;

/// <summary>
/// Represents a window comparison question as inspectable data.
/// </summary>
/// <remarks>
/// A comparison plan is the question Spanfold should answer. It does not execute
/// the comparison or enumerate recorded window history. Plans are deterministic
/// data contracts when all selectors are serializable, which makes them suitable
/// for review, CI fixtures, and later execution against recorded window history.
/// </remarks>
public sealed class ComparisonPlan
{
    /// <summary>
    /// Creates a comparison plan.
    /// </summary>
    /// <param name="name">The human-readable plan name.</param>
    /// <param name="target">The target selector.</param>
    /// <param name="against">The comparison selectors.</param>
    /// <param name="scope">The comparison scope.</param>
    /// <param name="normalization">The normalization policy.</param>
    /// <param name="comparators">The comparator declarations.</param>
    /// <param name="isStrict">Whether validation warnings should be treated strictly by later execution stages.</param>
    public ComparisonPlan(
        string name,
        ComparisonSelector? target,
        IEnumerable<ComparisonSelector>? against,
        ComparisonScope? scope,
        ComparisonNormalizationPolicy? normalization,
        IEnumerable<string>? comparators,
        bool isStrict = false)
    {
        Name = name;
        Target = target;
        Against = Materialize(against);
        Scope = scope;
        Normalization = normalization ?? ComparisonNormalizationPolicy.Default;
        Comparators = MaterializeComparators(comparators);
        IsStrict = isStrict;
    }

    /// <summary>
    /// Gets the human-readable plan name.
    /// </summary>
    public string Name { get; }

    /// <summary>
    /// Gets the target selector.
    /// </summary>
    public ComparisonSelector? Target { get; }

    /// <summary>
    /// Gets the comparison selectors in deterministic declaration order.
    /// </summary>
    public IReadOnlyList<ComparisonSelector> Against { get; }

    /// <summary>
    /// Gets the temporal comparison scope.
    /// </summary>
    public ComparisonScope? Scope { get; }

    /// <summary>
    /// Gets the normalization policy.
    /// </summary>
    public ComparisonNormalizationPolicy Normalization { get; }

    /// <summary>
    /// Gets comparator declarations in deterministic declaration order.
    /// </summary>
    public IReadOnlyList<string> Comparators { get; }

    /// <summary>
    /// Gets whether later execution should treat validation warnings strictly.
    /// </summary>
    public bool IsStrict { get; }

    /// <summary>
    /// Gets whether every selector in the plan can be exported as portable data.
    /// </summary>
    /// <remarks>
    /// Runtime-only selectors may still execute locally, but deterministic JSON
    /// export rejects them because the predicate delegate cannot be represented
    /// as a portable comparison contract.
    /// </remarks>
    public bool IsSerializable => !Target.HasValue
        ? Against.All(static selector => selector.IsSerializable)
        : Target.Value.IsSerializable && Against.All(static selector => selector.IsSerializable);

    /// <summary>
    /// Validates the structural completeness of the comparison plan.
    /// </summary>
    /// <remarks>
    /// Diagnostics are returned in stable path order so tooling can snapshot and
    /// compare validation output. Strict plans promote selector exportability
    /// issues to errors, while non-strict plans keep them visible as warnings.
    /// </remarks>
    /// <returns>The validation diagnostics in stable order.</returns>
    public IReadOnlyList<ComparisonPlanDiagnostic> Validate()
    {
        var diagnostics = new List<ComparisonPlanDiagnostic>();
        var exportabilitySeverity = IsStrict
            ? ComparisonPlanDiagnosticSeverity.Error
            : ComparisonPlanDiagnosticSeverity.Warning;

        if (string.IsNullOrWhiteSpace(Name))
        {
            diagnostics.Add(new ComparisonPlanDiagnostic(
                ComparisonPlanValidationCode.MissingName,
                "Comparison plan name is required.",
                "name",
                ComparisonPlanDiagnosticSeverity.Error));
        }

        if (!Target.HasValue || !Target.Value.IsDefined)
        {
            diagnostics.Add(new ComparisonPlanDiagnostic(
                ComparisonPlanValidationCode.MissingTarget,
                "Comparison plan target selector is required.",
                "target",
                ComparisonPlanDiagnosticSeverity.Error));
        }
        else if (!Target.Value.IsSerializable)
        {
            diagnostics.Add(new ComparisonPlanDiagnostic(
                ComparisonPlanValidationCode.NonSerializableSelector,
                "Target selector is runtime-only and cannot be exported as plan data.",
                "target",
                exportabilitySeverity));
        }

        if (Against.Count == 0)
        {
            diagnostics.Add(new ComparisonPlanDiagnostic(
                ComparisonPlanValidationCode.MissingAgainst,
                "At least one comparison selector is required.",
                "against",
                ComparisonPlanDiagnosticSeverity.Error));
        }
        else
        {
            for (var i = 0; i < Against.Count; i++)
            {
                if (Against[i].IsDefined && Against[i].IsSerializable)
                {
                    continue;
                }

                diagnostics.Add(new ComparisonPlanDiagnostic(
                    ComparisonPlanValidationCode.NonSerializableSelector,
                    Against[i].IsDefined
                        ? "Comparison selector is runtime-only and cannot be exported as plan data."
                        : "Comparison selector is uninitialized.",
                    $"against[{i}]",
                    Against[i].IsDefined
                        ? exportabilitySeverity
                        : ComparisonPlanDiagnosticSeverity.Error));
            }
        }

        if (Scope is null)
        {
            diagnostics.Add(new ComparisonPlanDiagnostic(
                ComparisonPlanValidationCode.MissingScope,
                "Comparison scope is required.",
                "scope",
                ComparisonPlanDiagnosticSeverity.Error));
        }

        if (Comparators.Count == 0)
        {
            diagnostics.Add(new ComparisonPlanDiagnostic(
                ComparisonPlanValidationCode.MissingComparator,
                "At least one comparator is required.",
                "comparators",
                ComparisonPlanDiagnosticSeverity.Error));
        }
        else
        {
            for (var i = 0; i < Comparators.Count; i++)
            {
                if (ComparisonComparatorCatalog.IsKnownDeclaration(Comparators[i]))
                {
                    continue;
                }

                diagnostics.Add(new ComparisonPlanDiagnostic(
                    ComparisonPlanValidationCode.UnknownComparator,
                    $"Comparator '{Comparators[i]}' is not registered.",
                    $"comparators[{i}]",
                    ComparisonPlanDiagnosticSeverity.Error));
            }
        }

        return diagnostics.ToArray();
    }

    private static IReadOnlyList<ComparisonSelector> Materialize(IEnumerable<ComparisonSelector>? selectors)
    {
        if (selectors is null)
        {
            return [];
        }

        return Array.AsReadOnly(selectors.ToArray());
    }

    private static IReadOnlyList<string> MaterializeComparators(IEnumerable<string>? comparators)
    {
        if (comparators is null)
        {
            return [];
        }

        return Array.AsReadOnly(comparators
            .Where(static comparator => !string.IsNullOrWhiteSpace(comparator))
            .Distinct(StringComparer.Ordinal)
            .ToArray());
    }
}
