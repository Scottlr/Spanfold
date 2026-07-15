namespace Spanfold.Comparison;

/// <summary>
/// Describes serializable metadata emitted by a comparison extension.
/// </summary>
/// <remarks>
/// Metadata values should be compact, deterministic strings. Extension packages
/// should avoid storing large row payloads here; use normal result rows or
/// domain-specific exports for large data.
/// </remarks>
/// <param name="ExtensionId">The stable extension identifier.</param>
/// <param name="Key">The metadata key.</param>
/// <param name="Value">The metadata value.</param>
public sealed record ComparisonExtensionMetadata(
    string ExtensionId,
    string Key,
    string Value);
