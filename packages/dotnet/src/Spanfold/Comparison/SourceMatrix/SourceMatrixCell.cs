namespace Spanfold.Comparison;

/// <summary>
/// Summarizes one directional source-pair comparison in a source matrix.
/// </summary>
/// <remarks>
/// Source matrix cells are directional. Target source is the row source and
/// comparison source is the column source. Diagonal cells are emitted as
/// identity rows and do not run pairwise comparators.
/// </remarks>
/// <param name="TargetSource">The row source being treated as target.</param>
/// <param name="AgainstSource">The column source being compared against the target.</param>
/// <param name="IsDiagonal">Whether this cell compares a source with itself.</param>
/// <param name="TargetHasWindows">Whether the target source has recorded windows in the matrix window.</param>
/// <param name="AgainstHasWindows">Whether the comparison source has recorded windows in the matrix window.</param>
/// <param name="OverlapRowCount">The overlap row count.</param>
/// <param name="ResidualRowCount">The target-only residual row count.</param>
/// <param name="MissingRowCount">The comparison-only missing row count.</param>
/// <param name="CoverageRowCount">The coverage row count.</param>
/// <param name="CoverageRatio">The aggregate coverage ratio, when target coverage exists.</param>
public sealed record SourceMatrixCell(
    object TargetSource,
    object AgainstSource,
    bool IsDiagonal,
    bool TargetHasWindows,
    bool AgainstHasWindows,
    int OverlapRowCount,
    int ResidualRowCount,
    int MissingRowCount,
    int CoverageRowCount,
    double? CoverageRatio);
