using System.Text.Json;

namespace Spanfold;

/// <summary>
/// Provides a conservative, value-redacted context export for agents.
/// </summary>
public static class ComparisonAgentExportExtensions
{
    /// <summary>
    /// Exports counts, row identities, and diagnostic codes without keys,
    /// source values, partitions, tags, segments, or diagnostic messages.
    /// </summary>
    /// <param name="result">The comparison result to summarize.</param>
    /// <returns>A safe JSON context suitable for external agent workflows.</returns>
    public static string ExportRedactedAgentContext(this ComparisonResult result)
    {
        ArgumentNullException.ThrowIfNull(result);

        using var stream = new MemoryStream();
        using (var writer = new Utf8JsonWriter(stream, new JsonWriterOptions { Indented = true }))
        {
            writer.WriteStartObject();
            writer.WriteString("schema", "spanfold.comparison.agent-context.redacted");
            writer.WriteNumber("schemaVersion", 1);
            writer.WriteString("artifact", "redacted-agent-context");
            writer.WriteBoolean("isValid", result.IsValid);
            writer.WriteString("planName", result.Plan.Name);
            writer.WriteStartArray("diagnosticCodes");
            foreach (var diagnostic in result.Diagnostics)
            {
                writer.WriteStringValue(diagnostic.Code.ToString());
            }

            writer.WriteEndArray();
            writer.WriteStartObject("rowCounts");
            writer.WriteNumber("overlap", result.OverlapRows.Count);
            writer.WriteNumber("residual", result.ResidualRows.Count);
            writer.WriteNumber("missing", result.MissingRows.Count);
            writer.WriteNumber("coverage", result.CoverageRows.Count);
            writer.WriteNumber("gap", result.GapRows.Count);
            writer.WriteNumber("symmetricDifference", result.SymmetricDifferenceRows.Count);
            writer.WriteNumber("containment", result.ContainmentRows.Count);
            writer.WriteNumber("leadLag", result.LeadLagRows.Count);
            writer.WriteNumber("asOf", result.AsOfRows.Count);
            writer.WriteEndObject();
            writer.WriteStartArray("rowIds");
            foreach (var finality in result.RowFinalities)
            {
                writer.WriteStartObject();
                writer.WriteString("rowType", finality.RowType);
                writer.WriteString("rowId", finality.RowId);
                writer.WriteString("finality", finality.Finality.ToString());
                writer.WriteEndObject();
            }

            writer.WriteEndArray();
            writer.WriteEndObject();
        }

        return System.Text.Encoding.UTF8.GetString(stream.ToArray());
    }
}
