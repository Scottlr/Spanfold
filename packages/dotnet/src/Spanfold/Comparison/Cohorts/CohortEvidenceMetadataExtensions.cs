using System.Globalization;
using System.Text.Json;

namespace Spanfold;

/// <summary>
/// Provides typed access to cohort evidence emitted in comparison metadata.
/// </summary>
public static class CohortEvidenceMetadataExtensions
{
    private const string CohortExtensionId = "spanfold.cohort";

    /// <summary>
    /// Gets parsed cohort evidence metadata from a comparison result.
    /// </summary>
    /// <param name="result">The comparison result.</param>
    /// <returns>Parsed cohort evidence in result metadata order.</returns>
    public static IReadOnlyList<CohortEvidenceMetadata> CohortEvidence(this ComparisonResult result)
    {
        ArgumentNullException.ThrowIfNull(result);

        var evidence = new List<CohortEvidenceMetadata>();
        for (var i = 0; i < result.ExtensionMetadata.Count; i++)
        {
            var item = result.ExtensionMetadata[i];
            if (!string.Equals(item.ExtensionId, CohortExtensionId, StringComparison.Ordinal))
            {
                continue;
            }

            if (TryParse(item, out var parsed))
            {
                evidence.Add(parsed);
            }
        }

        return evidence.ToArray();
    }

    private static bool TryParse(
        ComparisonExtensionMetadata metadata,
        out CohortEvidenceMetadata evidence)
    {
        evidence = default!;

        if (!TryParseSegmentIndex(metadata.Key, out var segmentIndex))
        {
            return false;
        }

        if (TryParseJson(metadata.Value, segmentIndex, metadata.Value, out evidence))
        {
            return true;
        }

        var values = ParseFields(metadata.Value);
        if (!values.TryGetValue("rule", out var rule)
            || !values.TryGetValue("required", out var required)
            || !values.TryGetValue("activeCount", out var activeCount)
            || !values.TryGetValue("isActive", out var isActive)
            || !int.TryParse(required, NumberStyles.Integer, CultureInfo.InvariantCulture, out var requiredValue)
            || !int.TryParse(activeCount, NumberStyles.Integer, CultureInfo.InvariantCulture, out var activeCountValue)
            || !bool.TryParse(isActive, out var isActiveValue))
        {
            return false;
        }

        evidence = new CohortEvidenceMetadata(
            segmentIndex,
            rule,
            requiredValue,
            activeCountValue,
            isActiveValue,
            ParseActiveSources(values.TryGetValue("activeSources", out var sources) ? sources : string.Empty),
            metadata.Value);
        return true;
    }

    private static bool TryParseJson(
        string value,
        int segmentIndex,
        string rawValue,
        out CohortEvidenceMetadata evidence)
    {
        evidence = default!;
        try
        {
            using var document = JsonDocument.Parse(value);
            var root = document.RootElement;
            if (root.ValueKind != JsonValueKind.Object
                || !root.TryGetProperty("rule", out var rule)
                || !root.TryGetProperty("required", out var required)
                || !root.TryGetProperty("activeCount", out var activeCount)
                || !root.TryGetProperty("isActive", out var isActive)
                || !root.TryGetProperty("activeSources", out var sources))
            {
                return false;
            }

            if (rule.ValueKind != JsonValueKind.String
                || !required.TryGetInt32(out var requiredValue)
                || !activeCount.TryGetInt32(out var activeCountValue)
                || (isActive.ValueKind != JsonValueKind.True && isActive.ValueKind != JsonValueKind.False)
                || sources.ValueKind != JsonValueKind.Array)
            {
                return false;
            }

            var activeValue = isActive.GetBoolean();

            var activeSources = sources.EnumerateArray()
                .Where(static item => item.ValueKind == JsonValueKind.String)
                .Select(static item => item.GetString()!)
                .ToArray();
            evidence = new CohortEvidenceMetadata(
                segmentIndex,
                rule.GetString()!,
                requiredValue,
                activeCountValue,
                activeValue,
                activeSources,
                rawValue);
            return true;
        }
        catch (JsonException)
        {
            return false;
        }
    }

    private static bool TryParseSegmentIndex(string key, out int index)
    {
        const string prefix = "segment[";
        index = 0;

        if (!key.StartsWith(prefix, StringComparison.Ordinal) || !key.EndsWith(']'))
        {
            return false;
        }

        var value = key[prefix.Length..^1];
        return int.TryParse(value, NumberStyles.Integer, CultureInfo.InvariantCulture, out index);
    }

    private static Dictionary<string, string> ParseFields(string value)
    {
        var fields = new Dictionary<string, string>(StringComparer.Ordinal);
        var parts = value.Split(';');

        for (var i = 0; i < parts.Length; i++)
        {
            var part = parts[i].Trim();
            var separator = part.IndexOf('=');
            if (separator <= 0)
            {
                continue;
            }

            fields[part[..separator]] = part[(separator + 1)..];
        }

        return fields;
    }

    private static IReadOnlyList<string> ParseActiveSources(string value)
    {
        if (string.IsNullOrWhiteSpace(value))
        {
            return [];
        }

        return value.Split(',', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries);
    }
}
