using System.Security.Cryptography;
using System.Text;
using System.Text.Json;

namespace Spanfold.Artifacts.Internal;

internal static class ComparisonArtifactPlanIdentity
{
    internal static string CreateLegacy(JsonElement plan)
    {
        var canonical = new StringBuilder();
        AppendOptionalProperty(canonical, plan, "isStrict");
        AppendSelector(canonical, GetOptionalProperty(plan, "target"));

        var against = GetOptionalProperty(plan, "against");
        var againstCount = against.ValueKind == JsonValueKind.Array
            ? against.GetArrayLength()
            : 0;
        Append(canonical, againstCount.ToString());
        if (against.ValueKind == JsonValueKind.Array)
        {
            foreach (var selector in against.EnumerateArray())
            {
                AppendSelector(canonical, selector);
            }
        }

        AppendOptionalProperty(canonical, plan, "scope");
        AppendNormalization(canonical, GetOptionalProperty(plan, "normalization"));
        AppendOptionalProperty(canonical, plan, "comparators");

        return Convert.ToHexString(
            SHA256.HashData(Encoding.UTF8.GetBytes(canonical.ToString())));
    }

    private static void AppendSelector(StringBuilder canonical, JsonElement selector)
    {
        if (selector.ValueKind is JsonValueKind.Null or JsonValueKind.Undefined)
        {
            Append(canonical, "null");
            return;
        }

        AppendOptionalProperty(canonical, selector, "description");
        AppendOptionalProperty(canonical, selector, "isSerializable");
        if (selector.TryGetProperty("cohort", out var cohort))
        {
            AppendJson(canonical, cohort);
        }
    }

    private static void AppendNormalization(StringBuilder canonical, JsonElement normalization)
    {
        if (normalization.ValueKind != JsonValueKind.Object)
        {
            Append(canonical, "missing");
            return;
        }

        AppendOptionalProperty(canonical, normalization, "timeAxis");
        AppendOptionalProperty(canonical, normalization, "openWindowPolicy");
        AppendOptionalProperty(canonical, normalization, "nullTimestampPolicy");
    }

    private static void AppendOptionalProperty(
        StringBuilder canonical,
        JsonElement value,
        string propertyName)
    {
        if (value.ValueKind == JsonValueKind.Object
            && value.TryGetProperty(propertyName, out var property))
        {
            AppendJson(canonical, property);
            return;
        }

        Append(canonical, "missing");
    }

    private static JsonElement GetOptionalProperty(JsonElement value, string propertyName)
    {
        return value.ValueKind == JsonValueKind.Object
            && value.TryGetProperty(propertyName, out var property)
                ? property
                : default;
    }

    private static void AppendJson(StringBuilder canonical, JsonElement value)
    {
        switch (value.ValueKind)
        {
            case JsonValueKind.Object:
                foreach (var property in value.EnumerateObject().OrderBy(static property => property.Name, StringComparer.Ordinal))
                {
                    Append(canonical, property.Name);
                    AppendJson(canonical, property.Value);
                }

                break;
            case JsonValueKind.Array:
                Append(canonical, value.GetArrayLength().ToString());
                foreach (var item in value.EnumerateArray())
                {
                    AppendJson(canonical, item);
                }

                break;
            default:
                Append(canonical, value.GetRawText());
                break;
        }
    }

    private static void Append(StringBuilder canonical, string value)
    {
        canonical.Append(value.Length).Append(':').Append(value).Append(';');
    }
}
