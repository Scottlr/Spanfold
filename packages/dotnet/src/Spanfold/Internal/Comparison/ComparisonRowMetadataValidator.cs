using Spanfold;

namespace Spanfold.Internal.Comparison;

internal static class ComparisonRowMetadataValidator
{
    private static readonly ComparisonRowKind[] FamilyOrder =
    [
        ComparisonRowKind.Overlap,
        ComparisonRowKind.Residual,
        ComparisonRowKind.Missing,
        ComparisonRowKind.Coverage,
        ComparisonRowKind.Gap,
        ComparisonRowKind.SymmetricDifference,
        ComparisonRowKind.Containment,
        ComparisonRowKind.LeadLag,
        ComparisonRowKind.AsOf
    ];

    internal static int ValidateAndGetOffset(ComparisonResult result, ComparisonRowKind requestedKind)
    {
        ArgumentNullException.ThrowIfNull(result);

        var expectedTotal = 0;
        for (var familyIndex = 0; familyIndex < FamilyOrder.Length; familyIndex++)
        {
            expectedTotal += Count(result, FamilyOrder[familyIndex]);
        }

        var actualTotal = result.RowFinalities.Count;
        var start = 0;
        var requestedOffset = -1;

        for (var familyIndex = 0; familyIndex < FamilyOrder.Length; familyIndex++)
        {
            var family = FamilyOrder[familyIndex];
            var expectedCount = Count(result, family);
            if (family == requestedKind)
            {
                requestedOffset = start;
            }

            var available = Math.Min(expectedCount, Math.Max(0, actualTotal - start));
            for (var relativeIndex = 0; relativeIndex < available; relativeIndex++)
            {
                var metadata = result.RowFinalities[start + relativeIndex];
                if (metadata is null
                    || !metadata.TryGetRowKind(out var actual)
                    || actual != family)
                {
                    throw new ComparisonRowMetadataException(
                        family,
                        start + relativeIndex,
                        expectedCount,
                        CountMatching(result, start, available, family),
                        family,
                        metadata?.RowType);
                }
            }

            if (available < expectedCount)
            {
                throw new ComparisonRowMetadataException(
                    family,
                    available + start,
                    expectedCount,
                    available,
                    family,
                    null);
            }

            start += expectedCount;
        }

        if (actualTotal > expectedTotal)
        {
            var family = ComparisonRowKind.AsOf;
            var expectedCount = Count(result, family);
            var familyStart = expectedTotal - expectedCount;
            throw new ComparisonRowMetadataException(
                family,
                expectedTotal,
                expectedCount,
                actualTotal - familyStart,
                family,
                result.RowFinalities[expectedTotal]?.RowType);
        }

        return requestedOffset >= 0
            ? requestedOffset
            : throw new InvalidOperationException($"No metadata layout exists for {requestedKind}.");
    }

    private static int Count(ComparisonResult result, ComparisonRowKind family)
    {
        return family switch
        {
            ComparisonRowKind.Overlap => result.OverlapRows.Count,
            ComparisonRowKind.Residual => result.ResidualRows.Count,
            ComparisonRowKind.Missing => result.MissingRows.Count,
            ComparisonRowKind.Coverage => result.CoverageRows.Count,
            ComparisonRowKind.Gap => result.GapRows.Count,
            ComparisonRowKind.SymmetricDifference => result.SymmetricDifferenceRows.Count,
            ComparisonRowKind.Containment => result.ContainmentRows.Count,
            ComparisonRowKind.LeadLag => result.LeadLagRows.Count,
            ComparisonRowKind.AsOf => result.AsOfRows.Count,
            _ => throw new ArgumentOutOfRangeException(nameof(family), family, "Unknown comparison row kind.")
        };
    }

    private static int CountMatching(
        ComparisonResult result,
        int start,
        int available,
        ComparisonRowKind expectedKind)
    {
        var count = 0;
        for (var index = 0; index < available; index++)
        {
            var metadata = result.RowFinalities[start + index];
            if (metadata is not null
                && metadata.TryGetRowKind(out var actual)
                && actual == expectedKind)
            {
                count++;
            }
        }

        return count;
    }
}
