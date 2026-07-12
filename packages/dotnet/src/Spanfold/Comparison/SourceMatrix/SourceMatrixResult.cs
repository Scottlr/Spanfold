namespace Spanfold;

/// <summary>
/// Represents a directional pairwise matrix over recorded window sources.
/// </summary>
/// <param name="Name">The matrix name.</param>
/// <param name="WindowName">The recorded window name used for every cell.</param>
/// <param name="Sources">The sources in requested row and column order.</param>
/// <param name="Cells">The directional matrix cells in row-major source order.</param>
public sealed record SourceMatrixResult(
    string Name,
    string WindowName,
    IReadOnlyList<object> Sources,
    IReadOnlyList<SourceMatrixCell> Cells)
{
    /// <summary>Gets sources in requested row and column order.</summary>
    public IReadOnlyList<object> Sources { get; } = Array.AsReadOnly(Sources.ToArray());

    /// <summary>Gets directional cells in row-major order.</summary>
    public IReadOnlyList<SourceMatrixCell> Cells { get; } = Array.AsReadOnly(Cells.ToArray());

    /// <summary>
    /// Gets one directional matrix cell.
    /// </summary>
    /// <param name="targetSource">The row source treated as target.</param>
    /// <param name="againstSource">The column source compared against the target.</param>
    /// <returns>The matching matrix cell.</returns>
    /// <exception cref="KeyNotFoundException">Thrown when the cell is not present.</exception>
    public SourceMatrixCell GetCell(object targetSource, object againstSource)
    {
        if (TryGetCell(targetSource, againstSource, out var cell))
        {
            return cell!;
        }

        throw new KeyNotFoundException("Source matrix cell was not found.");
    }

    /// <summary>
    /// Tries to get one directional matrix cell.
    /// </summary>
    /// <param name="targetSource">The row source treated as target.</param>
    /// <param name="againstSource">The column source compared against the target.</param>
    /// <param name="cell">The matching cell, when found.</param>
    /// <returns>True when the cell is present.</returns>
    public bool TryGetCell(object targetSource, object againstSource, out SourceMatrixCell? cell)
    {
        ArgumentNullException.ThrowIfNull(targetSource);
        ArgumentNullException.ThrowIfNull(againstSource);

        for (var i = 0; i < Cells.Count; i++)
        {
            var candidate = Cells[i];
            if (Equals(candidate.TargetSource, targetSource)
                && Equals(candidate.AgainstSource, againstSource))
            {
                cell = candidate;
                return true;
            }
        }

        cell = null;
        return false;
    }
}
