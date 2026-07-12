namespace Spanfold;

/// <summary>
/// Builds a comparison extension descriptor.
/// </summary>
internal sealed class ComparisonExtensionBuilder
{
    private readonly string id;
    private readonly string displayName;
    private readonly List<ComparisonExtensionSelector> selectors = [];
    private readonly List<ComparisonExtensionComparator> comparators = [];
    private readonly List<string> metadataKeys = [];

    /// <summary>
    /// Creates an extension descriptor builder.
    /// </summary>
    /// <param name="id">The stable extension identifier.</param>
    /// <param name="displayName">The human-readable extension name.</param>
    internal ComparisonExtensionBuilder(string id, string displayName)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(id);
        ArgumentException.ThrowIfNullOrWhiteSpace(displayName);

        this.id = id;
        this.displayName = displayName;
    }

    /// <summary>
    /// Registers a selector descriptor exposed by the extension.
    /// </summary>
    /// <param name="name">The selector name.</param>
    /// <param name="description">The selector description.</param>
    /// <returns>This builder.</returns>
    internal ComparisonExtensionBuilder AddSelector(string name, string description)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(name);
        ArgumentException.ThrowIfNullOrWhiteSpace(description);

        this.selectors.Add(new ComparisonExtensionSelector(name, description));
        return this;
    }

    /// <summary>
    /// Registers a comparator declaration exposed by the extension.
    /// </summary>
    /// <param name="declaration">The comparator declaration string.</param>
    /// <param name="description">The comparator description.</param>
    /// <returns>This builder.</returns>
    internal ComparisonExtensionBuilder AddComparator(string declaration, string description)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(declaration);
        ArgumentException.ThrowIfNullOrWhiteSpace(description);

        this.comparators.Add(new ComparisonExtensionComparator(declaration, description));
        return this;
    }

    /// <summary>
    /// Registers a metadata key emitted by the extension.
    /// </summary>
    /// <param name="key">The metadata key.</param>
    /// <returns>This builder.</returns>
    internal ComparisonExtensionBuilder AddMetadataKey(string key)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(key);

        this.metadataKeys.Add(key);
        return this;
    }

    /// <summary>
    /// Builds the immutable extension descriptor.
    /// </summary>
    /// <returns>The extension descriptor.</returns>
    internal ComparisonExtensionDescriptor Build()
    {
        return new ComparisonExtensionDescriptor(
            this.id,
            this.displayName,
            this.selectors.ToArray(),
            this.comparators.ToArray(),
            this.metadataKeys.ToArray());
    }
}
