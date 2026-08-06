using System.Globalization;
using System.Text.Json;

namespace Spanfold.Comparison;

internal static class CohortEvidenceMetadataCompatibilityProjection
{
    private const string ExtensionId = "spanfold.cohort";

    internal static IReadOnlyList<ComparisonExtensionMetadata> Project(
        IEnumerable<ComparisonExtensionMetadata>? extensionMetadata,
        IReadOnlyList<CohortEvidenceMetadata> evidence)
    {
        var metadata = new List<ComparisonExtensionMetadata>();
        if (extensionMetadata is not null)
        {
            foreach (var item in extensionMetadata)
            {
                var isTypedProjection = evidence.Count > 0
                    && string.Equals(item.ExtensionId, ExtensionId, StringComparison.Ordinal);
                if (!isTypedProjection)
                {
                    metadata.Add(item);
                }
            }
        }

        for (var i = 0; i < evidence.Count; i++)
        {
            var item = evidence[i];
            metadata.Add(new ComparisonExtensionMetadata(
                ExtensionId,
                "segment[" + item.SegmentIndex.ToString(CultureInfo.InvariantCulture) + "]",
                item.RawValue));
        }

        return Array.AsReadOnly(metadata.ToArray());
    }

    internal static string SerializeValue(CohortEvidenceMetadata evidence)
    {
        return JsonSerializer.Serialize(new
        {
            rule = evidence.Rule,
            required = evidence.RequiredCount,
            activeCount = evidence.ActiveCount,
            isActive = evidence.IsActive,
            activeSources = evidence.ActiveSources
        });
    }
}
