using System.Text.Json;
using Spanfold.Assessment;

namespace Spanfold.Artifacts;

/// <summary>Reads portable assessment specifications and suites from JSON.</summary>
public static class AssessmentDocument
{
    /// <summary>Reads one assessment specification from a JSON file.</summary>
    public static AssessmentSpecification ReadSpecification(string path)
    {
        using var document = JsonDocument.Parse(File.ReadAllText(Path.GetFullPath(path)));
        return ParseSpecification(document.RootElement);
    }

    /// <summary>Reads one assessment suite from a JSON file.</summary>
    public static AssessmentSuite ReadSuite(string path)
    {
        using var document = JsonDocument.Parse(File.ReadAllText(Path.GetFullPath(path)));
        var root = document.RootElement;
        RequireSchema(root, "spanfold.assessment-suite");
        return new AssessmentSuite(
            root.GetProperty("name").GetString()!,
            root.GetProperty("specifications").EnumerateArray().Select(ParseSpecification));
    }

    private static AssessmentSpecification ParseSpecification(JsonElement root)
    {
        RequireSchema(root, "spanfold.assessment-specification");
        return new AssessmentSpecification(
            root.GetProperty("name").GetString()!,
            root.GetProperty("rules").EnumerateArray().Select(ParseRule));
    }

    private static AssessmentRule ParseRule(JsonElement rule)
    {
        var id = rule.GetProperty("id").GetString()!;
        return rule.GetProperty("type").GetString() switch
        {
            "minimumCoverage" => new MinimumCoverageRule(id, rule.GetProperty("minimumRatio").GetDouble()),
            "maximumResidualMagnitude" => new MaximumResidualMagnitudeRule(
                id,
                rule.GetProperty("maximumMagnitude").GetInt64(),
                ReadAggregation(rule)),
            "maximumGapMagnitude" => new MaximumGapMagnitudeRule(
                id,
                rule.GetProperty("maximumMagnitude").GetInt64(),
                ReadAggregation(rule)),
            "maximumAbsoluteLeadLag" => new MaximumAbsoluteLeadLagRule(
                id,
                rule.GetProperty("maximumMagnitude").GetInt64()),
            "allowedDiagnostics" => new AllowedDiagnosticsRule(
                id,
                rule.GetProperty("allowedCodes").EnumerateArray().Select(static value =>
                    Enum.Parse<ComparisonPlanValidationCode>(value.GetString()!, ignoreCase: false))),
            "requireFinalRows" => new RequireFinalRowsRule(id),
            var type => throw new InvalidDataException($"Unsupported assessment rule type '{type}'.")
        };
    }

    private static AssessmentAggregation ReadAggregation(JsonElement rule) =>
        Enum.Parse<AssessmentAggregation>(rule.GetProperty("aggregation").GetString()!, ignoreCase: true);

    private static void RequireSchema(JsonElement root, string schema)
    {
        if (!StringComparer.Ordinal.Equals(root.GetProperty("schema").GetString(), schema)
            || root.GetProperty("schemaVersion").GetInt32() != 1)
        {
            throw new InvalidDataException($"Unsupported {schema} contract.");
        }
    }
}
