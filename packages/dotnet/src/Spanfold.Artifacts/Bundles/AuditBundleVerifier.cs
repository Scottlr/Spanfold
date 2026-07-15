namespace Spanfold.Artifacts;

internal static class AuditBundleVerifier
{
    internal static ArtifactVerificationResult Verify(string directory, AuditBundleManifest manifest)
    {
        var errors = new List<string>();
        foreach (var file in manifest.Files.OrderBy(static file => file.Path, StringComparer.Ordinal))
        {
            var path = Path.Combine(directory, file.Path);
            if (!File.Exists(path))
            {
                errors.Add($"Missing file: {file.Path}");
                continue;
            }

            var info = new FileInfo(path);
            if (info.Length != file.Size)
            {
                errors.Add($"Size mismatch: {file.Path}");
            }

            if (!StringComparer.OrdinalIgnoreCase.Equals(AuditBundleSerialization.HashFile(path), file.Sha256))
            {
                errors.Add($"SHA-256 mismatch: {file.Path}");
            }
        }

        return new ArtifactVerificationResult(errors);
    }
}
