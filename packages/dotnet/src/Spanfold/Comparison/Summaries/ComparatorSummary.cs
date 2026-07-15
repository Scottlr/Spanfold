namespace Spanfold.Comparison;

/// <summary>
/// Summarizes the output produced by one comparator.
/// </summary>
/// <param name="ComparatorName">The comparator name.</param>
/// <param name="RowCount">The number of rows emitted by the comparator.</param>
public sealed record ComparatorSummary(
    string ComparatorName,
    int RowCount);
