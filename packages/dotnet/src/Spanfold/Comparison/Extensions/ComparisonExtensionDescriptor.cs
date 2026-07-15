namespace Spanfold.Comparison;

/// <summary>
/// Describes the selectors, comparator declarations, and metadata keys exposed by an extension package.
/// </summary>
/// <remarks>
/// Extension descriptors are documentation and plan-construction contracts.
/// Registering a descriptor does not put extension code on the built-in
/// comparator hot path. Domain packages remain responsible for executing their
/// own comparators or translating declarations into built-in Spanfold plans.
/// </remarks>
/// <param name="Id">The stable extension identifier.</param>
/// <param name="DisplayName">The human-readable extension name.</param>
/// <param name="Selectors">Selector descriptors provided by the extension.</param>
/// <param name="Comparators">Comparator declarations provided by the extension.</param>
/// <param name="MetadataKeys">Metadata keys emitted by the extension.</param>
internal sealed record ComparisonExtensionDescriptor(
    string Id,
    string DisplayName,
    IReadOnlyList<ComparisonExtensionSelector> Selectors,
    IReadOnlyList<ComparisonExtensionComparator> Comparators,
    IReadOnlyList<string> MetadataKeys);
