namespace Spanfold;

/// <summary>
/// Describes one deterministic row-finality change between comparison snapshots.
/// </summary>
/// <param name="RowType">The exported row family, such as overlap or residual.</param>
/// <param name="RowId">The deterministic row identifier inside that row family.</param>
/// <param name="Version">The deterministic version for this row metadata change.</param>
/// <param name="Finality">The finality state represented by the change.</param>
/// <param name="SupersedesRowId">The prior row identifier superseded by this change, when any.</param>
/// <param name="Reason">A short human-readable revision reason.</param>
/// <param name="ResultFinality">The resulting row finality when this is a revision.</param>
public sealed record ComparisonChangelogEntry(
    string RowType,
    string RowId,
    int Version,
    ComparisonFinality Finality,
    string? SupersedesRowId,
    string Reason,
    ComparisonFinality? ResultFinality = null);
