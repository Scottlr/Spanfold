using Spanfold.Artifacts.Internal;
using Spanfold.Episodes;

namespace Spanfold.Artifacts.Episodes;

/// <summary>
/// Couples a portable Episode analysis document with its materialized result.
/// </summary>
public sealed record EpisodeAnalysisResultDocument
{
    internal EpisodeAnalysisResultDocument(
        EpisodeAnalysisDocument document,
        EpisodeComparisonResult result)
    {
        Document = document;
        Result = result;
    }

    /// <summary>Gets the portable analysis document.</summary>
    public EpisodeAnalysisDocument Document { get; }

    /// <summary>Gets the materialized analytical result.</summary>
    public EpisodeComparisonResult Result { get; }

    /// <summary>Exports deterministic portable JSON without runtime-specific Episode IDs.</summary>
    /// <returns>The result JSON.</returns>
    public string ExportJson()
    {
        return EpisodeAnalysisExporter.ExportJson(this);
    }

    /// <summary>Exports deterministic portable Markdown without runtime-specific Episode IDs.</summary>
    /// <returns>The result Markdown.</returns>
    public string ExportMarkdown()
    {
        return EpisodeAnalysisExporter.ExportMarkdown(this);
    }
}
