namespace Spanfold.Comparison;

/// <summary>
/// Identifies one of the closed comparison-result row families.
/// </summary>
public enum ComparisonRowKind
{
    /// <summary>Rows where target and comparison evidence overlap.</summary>
    Overlap,

    /// <summary>Rows active only on the target side.</summary>
    Residual,

    /// <summary>Rows active only on the comparison side.</summary>
    Missing,

    /// <summary>Rows describing target coverage segments.</summary>
    Coverage,

    /// <summary>Rows describing empty gaps in the observed scope.</summary>
    Gap,

    /// <summary>Rows describing symmetric disagreement.</summary>
    SymmetricDifference,

    /// <summary>Rows describing target/container relationships.</summary>
    Containment,

    /// <summary>Rows describing transition lead or lag.</summary>
    LeadLag,

    /// <summary>Rows describing as-of matches.</summary>
    AsOf
}

/// <summary>
/// Provides canonical artifact labels for comparison row families.
/// </summary>
public static class ComparisonRowKindExtensions
{
    /// <summary>
    /// Gets the canonical row-family label used by comparison artifacts.
    /// </summary>
    /// <param name="kind">The row family.</param>
    /// <returns>The canonical artifact label.</returns>
    /// <exception cref="ArgumentOutOfRangeException">
    /// Thrown when <paramref name="kind" /> is not a defined enum value.
    /// </exception>
    public static string ToArtifactLabel(this ComparisonRowKind kind)
    {
        return kind switch
        {
            ComparisonRowKind.Overlap => "overlap",
            ComparisonRowKind.Residual => "residual",
            ComparisonRowKind.Missing => "missing",
            ComparisonRowKind.Coverage => "coverage",
            ComparisonRowKind.Gap => "gap",
            ComparisonRowKind.SymmetricDifference => "symmetricDifference",
            ComparisonRowKind.Containment => "containment",
            ComparisonRowKind.LeadLag => "leadLag",
            ComparisonRowKind.AsOf => "asOf",
            _ => throw new ArgumentOutOfRangeException(nameof(kind), kind, "Unknown comparison row kind.")
        };
    }

    /// <summary>
    /// Parses a canonical artifact label or a Rust 0.1.0 JSON Lines alias.
    /// </summary>
    /// <param name="value">The row-family label to parse.</param>
    /// <param name="kind">The parsed row family when successful.</param>
    /// <returns>True when the label is recognized.</returns>
    public static bool TryParseArtifactLabel(string? value, out ComparisonRowKind kind)
    {
        kind = value switch
        {
            "overlap" => ComparisonRowKind.Overlap,
            "residual" => ComparisonRowKind.Residual,
            "missing" => ComparisonRowKind.Missing,
            "coverage" => ComparisonRowKind.Coverage,
            "gap" => ComparisonRowKind.Gap,
            "symmetricDifference" or "symmetric-difference" => ComparisonRowKind.SymmetricDifference,
            "containment" => ComparisonRowKind.Containment,
            "leadLag" or "lead-lag" => ComparisonRowKind.LeadLag,
            "asOf" or "asof" => ComparisonRowKind.AsOf,
            _ => default
        };

        return value switch
        {
            "overlap" or "residual" or "missing" or "coverage" or "gap"
                or "symmetricDifference" or "symmetric-difference" or "containment"
                or "leadLag" or "lead-lag" or "asOf" or "asof" => true,
            _ => false
        };
    }

    /// <summary>
    /// Gets the typed family represented by row finality metadata.
    /// </summary>
    /// <param name="metadata">The metadata to inspect.</param>
    /// <param name="kind">The parsed family when successful.</param>
    /// <returns>True when the metadata has a recognized row-family label.</returns>
    public static bool TryGetRowKind(this ComparisonRowFinality metadata, out ComparisonRowKind kind)
    {
        ArgumentNullException.ThrowIfNull(metadata);
        kind = metadata.Reference.Kind;
        return Enum.IsDefined(kind);
    }
}
