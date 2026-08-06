namespace Spanfold.Comparison;

/// <summary>
/// Provides typed access to cohort evidence emitted in comparison metadata.
/// </summary>
public static class CohortEvidenceMetadataExtensions
{
    /// <summary>
    /// Gets typed cohort evidence metadata from a comparison result.
    /// </summary>
    /// <param name="result">The comparison result.</param>
    /// <returns>Cohort evidence in result metadata order.</returns>
    public static IReadOnlyList<CohortEvidenceMetadata> CohortEvidence(this ComparisonResult result)
    {
        ArgumentNullException.ThrowIfNull(result);

        return result.CohortEvidenceMetadata;
    }
}
