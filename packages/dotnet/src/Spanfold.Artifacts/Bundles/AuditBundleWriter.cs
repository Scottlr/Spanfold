using System.Reflection;
using System.Text;
using System.Text.Json;
using Spanfold.Assessment;

namespace Spanfold.Artifacts;

/// <summary>Writes versioned, integrity-verifiable comparison audit bundles.</summary>
public static class AuditBundleWriter
{
    /// <summary>Writes a bundle atomically to a directory.</summary>
    public static AuditBundle Write(
        string path,
        ComparisonResult result,
        ComparisonAssessment? assessment = null,
        IEnumerable<ComparisonRowTrace>? traces = null,
        AuditBundleOptions? options = null)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(path);
        ArgumentNullException.ThrowIfNull(result);
        options ??= AuditBundleOptions.Default;
        options.Validate();

        var fullPath = Path.GetFullPath(path);
        var parent = Path.GetDirectoryName(fullPath)
            ?? throw new ArgumentException("The bundle path must have a parent directory.", nameof(path));
        Directory.CreateDirectory(parent);
        var temporary = Path.Combine(parent, "." + Path.GetFileName(fullPath) + ".tmp-" + Guid.NewGuid().ToString("N"));
        var backup = Path.Combine(parent, "." + Path.GetFileName(fullPath) + ".bak-" + Guid.NewGuid().ToString("N"));

        try
        {
            Directory.CreateDirectory(temporary);
            var files = new List<AuditBundleFile>();
            var (evidenceName, evidence, includeSupportingArtifacts) = options.Profile switch
            {
                ArtifactExportProfile.Full => ("result.json", result.ExportJson(), true),
                ArtifactExportProfile.Redacted => ("result.redacted.json", result.ExportRedactedAgentContext(), false),
                _ => throw new ArgumentOutOfRangeException(
                    nameof(options),
                    options.Profile,
                    "Unknown artifact export profile.")
            };
            WriteArtifact(temporary, evidenceName, evidence, files);

            if (includeSupportingArtifacts && assessment is not null)
            {
                WriteArtifact(
                    temporary,
                    "assessment.json",
                    JsonSerializer.Serialize(assessment, AuditBundleSerialization.JsonOptions),
                    files);
            }

            if (includeSupportingArtifacts && traces is not null)
            {
                var traceElements = traces.Select(static trace =>
                    JsonSerializer.SerializeToElement(trace, trace.GetType(), AuditBundleSerialization.JsonOptions));
                WriteArtifact(
                    temporary,
                    "traces.json",
                    JsonSerializer.Serialize(traceElements, AuditBundleSerialization.JsonOptions),
                    files);
            }

            var manifest = new AuditBundleManifest(
                AuditBundleSerialization.Schema,
                AuditBundleSerialization.SchemaVersion,
                "Spanfold.Artifacts",
                typeof(AuditBundleWriter).Assembly.GetCustomAttribute<AssemblyInformationalVersionAttribute>()?.InformationalVersion
                    ?? typeof(AuditBundleWriter).Assembly.GetName().Version?.ToString()
                    ?? "unknown",
                AuditBundleSerialization.IdentityDomain,
                options.Profile,
                AuditBundleSerialization.HashText(result.Plan.ExportJson()),
                AuditBundleSerialization.HashText(evidence),
                Array.AsReadOnly(files.OrderBy(static file => file.Path, StringComparer.Ordinal).ToArray()));
            File.WriteAllText(
                Path.Combine(temporary, AuditBundleSerialization.ManifestFileName),
                JsonSerializer.Serialize(manifest, AuditBundleSerialization.JsonOptions),
                new UTF8Encoding(false));

            if (Directory.Exists(fullPath))
            {
                Directory.Move(fullPath, backup);
            }

            Directory.Move(temporary, fullPath);
            if (Directory.Exists(backup) && Directory.Exists(fullPath))
            {
                Directory.Delete(backup, recursive: true);
            }

            return new AuditBundle(fullPath, manifest);
        }
        catch
        {
            if (!Directory.Exists(fullPath) && Directory.Exists(backup))
            {
                Directory.Move(backup, fullPath);
            }

            throw;
        }
        finally
        {
            if (Directory.Exists(temporary))
            {
                Directory.Delete(temporary, recursive: true);
            }

            if (Directory.Exists(backup))
            {
                Directory.Delete(backup, recursive: true);
            }
        }
    }

    private static void WriteArtifact(
        string directory,
        string name,
        string content,
        ICollection<AuditBundleFile> files)
    {
        var path = Path.Combine(directory, name);
        File.WriteAllText(path, content, new UTF8Encoding(false));
        var info = new FileInfo(path);
        files.Add(new AuditBundleFile(name, info.Length, AuditBundleSerialization.HashFile(path)));
    }
}
