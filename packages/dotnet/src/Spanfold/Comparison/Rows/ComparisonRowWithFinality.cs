namespace Spanfold.Comparison;

/// <summary>
/// Associates one typed result row with the authoritative metadata from the
/// result snapshot that produced it.
/// </summary>
/// <typeparam name="TRow">The concrete comparison row type.</typeparam>
public readonly record struct ComparisonRowWithFinality<TRow>(
    TRow Row,
    ComparisonRowFinality Metadata);
