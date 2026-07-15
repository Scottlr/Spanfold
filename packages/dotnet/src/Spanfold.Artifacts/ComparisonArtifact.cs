using System.Text.Json;

namespace Spanfold.Artifacts;

/// <summary>
/// Represents a parsed comparison-result artifact without reconstructing an executable runtime result.
/// </summary>
public sealed class ComparisonArtifact
{
    private ComparisonArtifact(
        string schema,
        int schemaVersion,
        string name,
        bool isValid,
        IEnumerable<ComparisonArtifactRow> rows,
        string rawJson)
    {
        Schema = schema;
        SchemaVersion = schemaVersion;
        Name = name;
        IsValid = isValid;
        RowMetadata = Array.AsReadOnly(rows.ToArray());
        Rows = Array.AsReadOnly(RowMetadata.Select(static row => row.Reference).ToArray());
        RawJson = rawJson;
    }

    /// <summary>Gets the artifact schema.</summary>
    public string Schema { get; }

    /// <summary>Gets the artifact schema version.</summary>
    public int SchemaVersion { get; }

    /// <summary>Gets the comparison plan name.</summary>
    public string Name { get; }

    /// <summary>Gets whether the producing comparison result was valid.</summary>
    public bool IsValid { get; }

    /// <summary>Gets canonical row references declared by the artifact.</summary>
    public IReadOnlyList<ComparisonRowReference> Rows { get; }

    /// <summary>Gets canonical row identity, finality, and version metadata.</summary>
    public IReadOnlyList<ComparisonArtifactRow> RowMetadata { get; }

    /// <summary>Gets the original JSON document.</summary>
    public string RawJson { get; }

    /// <summary>Parses a versioned comparison-result JSON artifact.</summary>
    public static ComparisonArtifact Parse(string json)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(json);
        using var document = JsonDocument.Parse(json);
        var root = document.RootElement;
        var schema = root.GetProperty("schema").GetString();
        var schemaVersion = root.GetProperty("schemaVersion").GetInt32();
        if (!StringComparer.Ordinal.Equals(schema, "spanfold.comparison.result") || schemaVersion != 0)
        {
            throw new InvalidDataException("The comparison artifact uses an unsupported schema contract.");
        }

        var rows = new List<ComparisonArtifactRow>();
        foreach (var item in root.GetProperty("rowFinalities").EnumerateArray())
        {
            var label = item.GetProperty("rowType").GetString();
            var rowId = item.GetProperty("rowId").GetString();
            if (!ComparisonRowKindExtensions.TryParseArtifactLabel(label, out var kind)
                || string.IsNullOrWhiteSpace(rowId))
            {
                throw new InvalidDataException("The comparison artifact contains malformed row identity metadata.");
            }

            if (!Enum.TryParse<ComparisonFinality>(item.GetProperty("finality").GetString(), out var finality))
            {
                throw new InvalidDataException("The comparison artifact contains malformed row finality metadata.");
            }

            rows.Add(new ComparisonArtifactRow(
                new ComparisonRowReference(kind, rowId),
                finality,
                item.GetProperty("version").GetInt32()));
        }

        return new ComparisonArtifact(
            schema!,
            schemaVersion,
            root.GetProperty("plan").GetProperty("name").GetString() ?? string.Empty,
            root.GetProperty("isValid").GetBoolean(),
            rows,
            json);
    }

    /// <summary>Reads and parses a comparison-result JSON artifact.</summary>
    public static ComparisonArtifact Read(string path)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(path);
        return Parse(File.ReadAllText(Path.GetFullPath(path)));
    }
}
