using Spanfold.Comparison;

namespace Spanfold.Revisions;

/// <summary>
/// Describes one deterministic row change between comparison snapshots.
/// </summary>
public sealed record ComparisonChangelogEntry(
    ComparisonRowReference Row,
    int Version,
    ComparisonRevisionKind Kind,
    ComparisonFinality? PreviousFinality,
    ComparisonFinality? CurrentFinality,
    string? SupersedesRowId,
    string Reason);
