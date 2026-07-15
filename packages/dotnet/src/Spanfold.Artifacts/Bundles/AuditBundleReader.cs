using System.Text.Json;

namespace Spanfold.Artifacts;

/// <summary>Opens audit bundles using a fail-closed versioned manifest contract.</summary>
public static class AuditBundleReader
{
    /// <summary>Opens and validates the shape of an audit-bundle manifest.</summary>
    public static AuditBundle Open(string path)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(path);
        var fullPath = Path.GetFullPath(path);
        if (!Directory.Exists(fullPath))
        {
            throw new DirectoryNotFoundException($"Audit bundle directory '{fullPath}' was not found.");
        }

        var manifestPath = Path.Combine(fullPath, AuditBundleSerialization.ManifestFileName);
        AuditBundleManifest manifest;
        try
        {
            manifest = JsonSerializer.Deserialize<AuditBundleManifest>(
                File.ReadAllText(manifestPath),
                AuditBundleSerialization.JsonOptions)
                ?? throw new InvalidDataException("The audit bundle manifest is empty.");
        }
        catch (Exception exception) when (exception is IOException or JsonException)
        {
            throw new InvalidDataException("The audit bundle manifest could not be read.", exception);
        }

        if (!StringComparer.Ordinal.Equals(manifest.Schema, AuditBundleSerialization.Schema)
            || manifest.SchemaVersion != AuditBundleSerialization.SchemaVersion
            || !StringComparer.Ordinal.Equals(manifest.IdentityDomain, AuditBundleSerialization.IdentityDomain)
            || !Enum.IsDefined(manifest.Profile)
            || manifest.Files is null
            || manifest.Files.Any(static file => string.IsNullOrWhiteSpace(file.Path)
                || Path.IsPathRooted(file.Path)
                || file.Path.Contains("..", StringComparison.Ordinal)
                || file.Size < 0
                || file.Sha256.Length != 64)
            || manifest.Files.Select(static file => file.Path).Distinct(StringComparer.Ordinal).Count() != manifest.Files.Count
            || !manifest.Files.Any(file => StringComparer.Ordinal.Equals(
                file.Path,
                manifest.Profile == ArtifactExportProfile.Full ? "result.json" : "result.redacted.json")))
        {
            throw new InvalidDataException("The audit bundle manifest uses an unsupported or unsafe contract.");
        }

        return new AuditBundle(fullPath, manifest);
    }
}
