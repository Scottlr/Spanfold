namespace Spanfold.Artifacts;

/// <summary>Describes one integrity-protected file in an audit bundle.</summary>
public sealed record AuditBundleFile(string Path, long Size, string Sha256);
