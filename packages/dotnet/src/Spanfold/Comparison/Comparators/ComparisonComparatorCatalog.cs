namespace Spanfold.Comparison;

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
    /// <summary>
    /// Gets exact built-in comparator declarations.
    /// </summary>
    public static IReadOnlyList<string> BuiltInDeclarations =>
        ComparisonComparatorDeclarationParser.BuiltInDeclarations;

    /// <summary>
    /// Returns true when the declaration is an exact built-in comparator name.
    /// </summary>
    /// <param name="declaration">The comparator declaration.</param>
    /// <returns>True when the declaration is an exact built-in comparator name.</returns>
    public static bool IsBuiltInDeclaration(string declaration)
    {
        ArgumentNullException.ThrowIfNull(declaration);

        return ComparisonComparatorDeclarationParser.TryParse(declaration, out var parsed)
            && parsed is ComparisonComparatorDeclaration.BuiltIn;
    }

    /// <summary>
    /// Returns true when core Spanfold can execute the comparator declaration.
    /// </summary>
    /// <param name="declaration">The comparator declaration.</param>
    /// <returns>True when core Spanfold can execute the declaration.</returns>
    public static bool IsKnownDeclaration(string declaration)
    {
        ArgumentNullException.ThrowIfNull(declaration);

        return ComparisonComparatorDeclarationParser.TryParse(declaration, out _);
    }
}
