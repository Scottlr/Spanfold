namespace Spanfold.Cli;

internal static class ArtifactCommands
{
    internal static int VerifyBundle(string[] args, TextWriter stdout)
    {
        var verification = AuditBundleReader.Open(args[1]).Verify();
        CliOutput.WriteJson(stdout, verification);
        return verification.IsValid ? 0 : 1;
    }

    internal static int Diff(string[] args, TextWriter stdout)
    {
        if (args.Length != 3)
        {
            throw new ArgumentException("The diff command requires <baseline> <current>.");
        }

        var revision = ComparisonArtifactRevision.Between(
            ReadComparisonArtifact(args[1]),
            ReadComparisonArtifact(args[2]));
        CliOutput.WriteJson(stdout, revision);
        return revision.IsEmpty ? 0 : 1;
    }

    private static ComparisonArtifact ReadComparisonArtifact(string path)
    {
        var fullPath = Path.GetFullPath(path);
        if (Directory.Exists(fullPath))
        {
            var bundle = AuditBundleReader.Open(fullPath);
            var verification = bundle.Verify();
            if (!verification.IsValid)
            {
                throw new InvalidDataException("The comparison bundle failed integrity verification.");
            }

            fullPath = Path.Combine(fullPath, "result.json");
        }

        return ComparisonArtifact.Read(fullPath);
    }
}
