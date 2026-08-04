using System.Text.Json;

using Spanfold.Episodes;

namespace Spanfold.Artifacts.Episodes;

/// <summary>
/// Describes one portable comparison between a target source and an against source.
/// </summary>
public sealed record EpisodeAnalysisDocument
{
    private const string Schema = "spanfold.episode.analysis";
    private const int SchemaVersion = 1;

    private EpisodeAnalysisDocument(
        string name,
        EpisodeAnalysisSource target,
        EpisodeAnalysisSource against,
        string windowName,
        TemporalAxis normalizationAxis,
        long stitchTolerance,
        long relationTolerance,
        long? liveHorizon)
    {
        Name = name;
        Target = target;
        Against = against;
        WindowName = windowName;
        NormalizationAxis = normalizationAxis;
        StitchTolerance = stitchTolerance;
        RelationTolerance = relationTolerance;
        LiveHorizon = liveHorizon;
    }

    /// <summary>Gets the analytical comparison name.</summary>
    public string Name { get; }

    /// <summary>Gets the target source definition.</summary>
    public EpisodeAnalysisSource Target { get; }

    /// <summary>Gets the against source definition.</summary>
    public EpisodeAnalysisSource Against { get; }

    /// <summary>Gets the one named window family.</summary>
    public string WindowName { get; }

    /// <summary>Gets the normalization axis.</summary>
    public TemporalAxis NormalizationAxis { get; }

    /// <summary>Gets the maximum same-side gap magnitude.</summary>
    public long StitchTolerance { get; }

    /// <summary>Gets the maximum cross-side gap magnitude.</summary>
    public long RelationTolerance { get; }

    /// <summary>Gets the optional live evaluation-horizon magnitude.</summary>
    public long? LiveHorizon { get; }

    /// <summary>Parses and validates a versioned portable Episode analysis document.</summary>
    /// <param name="json">The document JSON.</param>
    /// <returns>The validated document.</returns>
    public static EpisodeAnalysisDocument ParseJson(string json)
    {
        ArgumentNullException.ThrowIfNull(json);

        using var parsed = JsonDocument.Parse(json);
        var root = parsed.RootElement;
        RequireKind(root, "$", JsonValueKind.Object);
        RequireExactString(root, "schema", "$", Schema);
        RequireExactNumber(root, "schemaVersion", "$", SchemaVersion);

        var name = RequireNonEmptyString(root, "name", "$");
        var target = ReadSource(root, "target");
        var against = ReadSource(root, "against");
        if (string.Equals(target.Source, against.Source, StringComparison.Ordinal))
        {
            throw new ArgumentException("$.target.source and $.against.source must be different.");
        }

        var windowName = RequireNonEmptyString(root, "windowName", "$");
        var normalizationAxis = ReadAxis(RequireNonEmptyString(root, "normalizationAxis", "$"));
        var stitchTolerance = RequireNonNegativeInt64(root, "stitchTolerance", "$");
        var relationTolerance = RequireNonNegativeInt64(root, "relationTolerance", "$");
        var liveHorizon = ReadOptionalInt64(root, "liveHorizon", "$");
        if (liveHorizon is < 0)
        {
            throw new ArgumentException("$.liveHorizon must be a non-negative integer or null.");
        }

        return new EpisodeAnalysisDocument(
            name,
            target,
            against,
            windowName,
            normalizationAxis,
            stitchTolerance,
            relationTolerance,
            liveHorizon);
    }

    /// <summary>Reads and validates a portable Episode analysis document.</summary>
    /// <param name="path">The document path.</param>
    /// <returns>The validated document.</returns>
    public static EpisodeAnalysisDocument Read(string path)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(path);
        return ParseJson(File.ReadAllText(path));
    }

    /// <summary>Executes this document through the existing Episode comparison module.</summary>
    /// <param name="history">The recorded windows to analyze.</param>
    /// <returns>A portable result document.</returns>
    public EpisodeAnalysisResultDocument Execute(WindowHistory history)
    {
        ArgumentNullException.ThrowIfNull(history);

        var builder = history.CompareEpisodes(Name)
            .Target(Target.Name, selector => selector.Source(Target.Source))
            .Against(Against.Name, selector => selector.Source(Against.Source))
            .Within(scope => scope.Window(WindowName, NormalizationAxis))
            .Normalize(BuildNormalization)
            .StitchGapsUpTo(StitchTolerance)
            .RelateWithin(RelationTolerance);

        var result = LiveHorizon.HasValue
            ? builder.RunLive(CreatePoint(LiveHorizon.Value))
            : builder.Run();

        ValidatePortableIdentities(result.TargetEpisodes);
        ValidatePortableIdentities(result.AgainstEpisodes);

        return new EpisodeAnalysisResultDocument(this, result);
    }

    private static void ValidatePortableIdentities(EpisodeSet set)
    {
        for (var index = 0; index < set.Episodes.Count; index++)
        {
            var episode = set.Episodes[index];
            if (episode.Key is not string || episode.Partition is not null and not string)
            {
                throw new InvalidOperationException(
                    "Portable Episode analysis requires string keys and string-or-null partitions.");
            }
        }
    }

    private ComparisonNormalizationBuilder BuildNormalization(ComparisonNormalizationBuilder builder)
    {
        return builder.OnPosition();
    }

    private TemporalPoint CreatePoint(long magnitude)
    {
        return TemporalPoint.ForPosition(magnitude);
    }

    private static EpisodeAnalysisSource ReadSource(JsonElement root, string propertyName)
    {
        var path = "$." + propertyName;
        var source = RequireProperty(root, propertyName, "$", JsonValueKind.Object);
        return new EpisodeAnalysisSource(
            RequireNonEmptyString(source, "name", path),
            RequireNonEmptyString(source, "source", path));
    }

    private static TemporalAxis ReadAxis(string value)
    {
        if (!string.Equals(value, "processingPosition", StringComparison.Ordinal))
        {
            throw new ArgumentException("Episode analysis schemaVersion 1 supports only the 'processingPosition' normalizationAxis.");
        }

        return TemporalAxis.ProcessingPosition;
    }

    private static long RequireNonNegativeInt64(JsonElement root, string propertyName, string path)
    {
        var property = RequireProperty(root, propertyName, path, JsonValueKind.Number);
        if (!property.TryGetInt64(out var value) || value < 0)
        {
            throw new ArgumentException(path + "." + propertyName + " must be a non-negative integer.");
        }

        return value;
    }

    private static long? ReadOptionalInt64(JsonElement root, string propertyName, string path)
    {
        if (!root.TryGetProperty(propertyName, out var property) || property.ValueKind == JsonValueKind.Null)
        {
            return null;
        }

        RequireKind(property, path + "." + propertyName, JsonValueKind.Number);
        if (!property.TryGetInt64(out var value))
        {
            throw new ArgumentException(path + "." + propertyName + " must be an integer.");
        }

        return value;
    }

    private static string RequireNonEmptyString(JsonElement root, string propertyName, string path)
    {
        var value = RequireProperty(root, propertyName, path, JsonValueKind.String).GetString();
        if (string.IsNullOrWhiteSpace(value))
        {
            throw new ArgumentException(path + "." + propertyName + " cannot be empty.");
        }

        return value;
    }

    private static void RequireExactString(
        JsonElement root,
        string propertyName,
        string path,
        string expected)
    {
        var actual = RequireProperty(root, propertyName, path, JsonValueKind.String).GetString();
        if (!string.Equals(actual, expected, StringComparison.Ordinal))
        {
            throw new ArgumentException(path + "." + propertyName + " must be '" + expected + "'.");
        }
    }

    private static void RequireExactNumber(
        JsonElement root,
        string propertyName,
        string path,
        int expected)
    {
        var property = RequireProperty(root, propertyName, path, JsonValueKind.Number);
        if (!property.TryGetInt32(out var actual) || actual != expected)
        {
            throw new ArgumentException(path + "." + propertyName + " must be " + expected + ".");
        }
    }

    private static JsonElement RequireProperty(
        JsonElement element,
        string propertyName,
        string path,
        params JsonValueKind[] expectedKinds)
    {
        if (!element.TryGetProperty(propertyName, out var property))
        {
            throw new ArgumentException(path + " is missing required property '" + propertyName + "'.");
        }

        RequireKind(property, path + "." + propertyName, expectedKinds);
        return property;
    }

    private static void RequireKind(JsonElement element, string path, params JsonValueKind[] expectedKinds)
    {
        for (var i = 0; i < expectedKinds.Length; i++)
        {
            if (element.ValueKind == expectedKinds[i])
            {
                return;
            }
        }

        throw new ArgumentException(path + " has unsupported JSON kind " + element.ValueKind + ".");
    }
}

/// <summary>Names one source participating in a portable Episode analysis.</summary>
/// <param name="Name">The side display name.</param>
/// <param name="Source">The exact source identity.</param>
public sealed record EpisodeAnalysisSource(string Name, string Source);
