namespace Spanfold;

/// <summary>
/// Describes a comparator declaration provided by an extension package.
/// </summary>
/// <param name="Declaration">The comparator declaration string used in plans.</param>
/// <param name="Description">A human-readable description for documentation and explain output.</param>
internal sealed record ComparisonExtensionComparator(
    string Declaration,
    string Description);
