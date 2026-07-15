using System.Security.Cryptography;
using System.Text.Json;
using System.Text.Json.Serialization;

namespace Spanfold.Artifacts;

internal static class AuditBundleSerialization
{
    internal const string ManifestFileName = "manifest.json";
    internal const string Schema = "spanfold.audit-bundle.manifest";
    internal const int SchemaVersion = 1;
    internal const string IdentityDomain = "spanfold.dotnet.comparison-row.v1";

    internal static JsonSerializerOptions JsonOptions { get; } = CreateJsonOptions();

    internal static string HashFile(string path)
    {
        using var stream = File.OpenRead(path);
        return Convert.ToHexString(SHA256.HashData(stream)).ToLowerInvariant();
    }

    internal static string HashText(string content) =>
        Convert.ToHexString(SHA256.HashData(System.Text.Encoding.UTF8.GetBytes(content))).ToLowerInvariant();

    private static JsonSerializerOptions CreateJsonOptions()
    {
        var options = new JsonSerializerOptions
        {
            PropertyNamingPolicy = JsonNamingPolicy.CamelCase,
            WriteIndented = true
        };
        options.Converters.Add(new JsonStringEnumConverter());
        return options;
    }
}
