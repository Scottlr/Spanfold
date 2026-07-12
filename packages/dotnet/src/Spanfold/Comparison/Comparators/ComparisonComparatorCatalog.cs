namespace Spanfold;

/// <summary>Identifies a built-in comparator implementation.</summary>
public enum ComparisonComparatorKind
{
    /// <summary>No known comparator kind.</summary>
    Unknown = 0,
    /// <summary>Overlap comparator.</summary>
    Overlap,
    /// <summary>Residual comparator.</summary>
    Residual,
    /// <summary>Missing comparator.</summary>
    Missing,
    /// <summary>Coverage comparator.</summary>
    Coverage,
    /// <summary>Gap comparator.</summary>
    Gap,
    /// <summary>Symmetric-difference comparator.</summary>
    SymmetricDifference,
    /// <summary>Containment comparator.</summary>
    Containment
}

/// <summary>
/// Describes comparator declarations understood by core Spanfold.
/// </summary>
/// <remarks>
/// The catalog is intended for tooling and fixture validation.
/// Runtime execution is still driven by declarations in the comparison plan.
/// Extension packages can expose additional declarations with
/// <see cref="ComparisonExtensionBuilder" />.
/// </remarks>
public static class ComparisonComparatorCatalog
{
    private static readonly string[] BuiltIns =
    [
        "overlap",
        "residual",
        "missing",
        "coverage",
        "gap",
        "symmetric-difference",
        "containment"
    ];

    /// <summary>
    /// Gets exact built-in comparator declarations.
    /// </summary>
    public static IReadOnlyList<string> BuiltInDeclarations => BuiltIns;

    /// <summary>
    /// Returns true when the declaration is an exact built-in comparator name.
    /// </summary>
    /// <param name="declaration">The comparator declaration.</param>
    /// <returns>True when the declaration is an exact built-in comparator name.</returns>
    public static bool IsBuiltInDeclaration(string declaration)
    {
        ArgumentNullException.ThrowIfNull(declaration);

        for (var i = 0; i < BuiltIns.Length; i++)
        {
            if (string.Equals(BuiltIns[i], declaration, StringComparison.Ordinal))
            {
                return true;
            }
        }

        return false;
    }

    /// <summary>
    /// Resolves an exact built-in declaration to its typed implementation kind.
    /// </summary>
    /// <param name="declaration">The comparator declaration.</param>
    /// <returns>The built-in kind, or <see cref="ComparisonComparatorKind.Unknown" />.</returns>
    public static ComparisonComparatorKind GetBuiltInKind(string declaration)
    {
        ArgumentNullException.ThrowIfNull(declaration);
        return declaration switch
        {
            "overlap" => ComparisonComparatorKind.Overlap,
            "residual" => ComparisonComparatorKind.Residual,
            "missing" => ComparisonComparatorKind.Missing,
            "coverage" => ComparisonComparatorKind.Coverage,
            "gap" => ComparisonComparatorKind.Gap,
            "symmetric-difference" => ComparisonComparatorKind.SymmetricDifference,
            "containment" => ComparisonComparatorKind.Containment,
            _ => ComparisonComparatorKind.Unknown
        };
    }

    /// <summary>
    /// Returns true when core Spanfold can execute the comparator declaration.
    /// </summary>
    /// <param name="declaration">The comparator declaration.</param>
    /// <returns>True when core Spanfold can execute the declaration.</returns>
    public static bool IsKnownDeclaration(string declaration)
    {
        ArgumentNullException.ThrowIfNull(declaration);

        return IsBuiltInDeclaration(declaration)
            || IsLeadLagDeclaration(declaration)
            || IsAsOfDeclaration(declaration);
    }

    private static bool IsLeadLagDeclaration(string declaration)
    {
        var parts = declaration.Split(':');
        return parts.Length == 4
            && string.Equals(parts[0], "lead-lag", StringComparison.Ordinal)
            && Enum.TryParse<LeadLagTransition>(parts[1], ignoreCase: false, out _)
            && Enum.TryParse<TemporalAxis>(parts[2], ignoreCase: false, out var axis)
            && axis != TemporalAxis.Unknown
            && long.TryParse(parts[3], out var tolerance)
            && tolerance >= 0;
    }

    private static bool IsAsOfDeclaration(string declaration)
    {
        var parts = declaration.Split(':');
        return parts.Length == 4
            && string.Equals(parts[0], "asof", StringComparison.Ordinal)
            && Enum.TryParse<AsOfDirection>(parts[1], ignoreCase: false, out _)
            && Enum.TryParse<TemporalAxis>(parts[2], ignoreCase: false, out var axis)
            && axis != TemporalAxis.Unknown
            && long.TryParse(parts[3], out var tolerance)
            && tolerance >= 0;
    }
}
