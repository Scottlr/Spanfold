namespace Spanfold;

/// <summary>
/// Describes a selector descriptor provided by an extension package.
/// </summary>
/// <param name="Name">The selector name used in plans and output.</param>
/// <param name="Description">A human-readable selector description.</param>
internal sealed record ComparisonExtensionSelector(
    string Name,
    string Description);
