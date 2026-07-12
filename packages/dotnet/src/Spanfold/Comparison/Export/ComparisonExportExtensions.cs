using Spanfold.Internal.Comparison;

namespace Spanfold;

/// <summary>
/// Provides deterministic export helpers for comparison artifacts.
/// </summary>
/// <remarks>
/// JSON export uses a stable schema envelope and deterministic property order.
/// It is intended for CI artifacts, issue reports, notebooks, and agent
/// consumption rather than hot-path analytics execution.
/// </remarks>
public static class ComparisonExportExtensions
{
    /// <summary>
    /// Exports a comparison plan as deterministic JSON.
    /// </summary>
    /// <param name="plan">The plan to export.</param>
    /// <returns>Deterministic JSON for the plan.</returns>
    /// <exception cref="ComparisonExportException">
    /// Thrown when the plan contains runtime-only selectors that cannot be
    /// exported as portable data.
    /// </exception>
    public static string ExportJson(this ComparisonPlan plan)
    {
        ArgumentNullException.ThrowIfNull(plan);

        return ComparisonExporter.ExportJson(plan);
    }

    /// <summary>
    /// Exports a plan with structured selector descriptors suitable for
    /// reconstructing executable selectors in another process.
    /// </summary>
    /// <param name="plan">The serializable plan to export.</param>
    /// <returns>Portable plan JSON with selector descriptors.</returns>
    public static string ExportPortableJson(this ComparisonPlan plan)
    {
        ArgumentNullException.ThrowIfNull(plan);
        return ComparisonExporter.ExportPortableJson(plan);
    }

    /// <summary>
    /// Exports a comparison result as deterministic JSON.
    /// </summary>
    /// <param name="result">The result to export.</param>
    /// <returns>Deterministic JSON for the result.</returns>
    /// <exception cref="ComparisonExportException">
    /// Thrown when the result's plan contains runtime-only selectors that
    /// cannot be exported as portable data.
    /// </exception>
    public static string ExportJson(this ComparisonResult result)
    {
        ArgumentNullException.ThrowIfNull(result);

        return ComparisonExporter.ExportJson(result);
    }

    /// <summary>
    /// Exports a comparison plan as deterministic Markdown.
    /// </summary>
    /// <param name="plan">The plan to export.</param>
    /// <returns>Deterministic Markdown for the plan.</returns>
    public static string ExportMarkdown(this ComparisonPlan plan)
    {
        ArgumentNullException.ThrowIfNull(plan);

        return plan.Explain(ComparisonExplanationFormat.Markdown);
    }

    /// <summary>
    /// Exports a comparison result as deterministic Markdown.
    /// </summary>
    /// <param name="result">The result to export.</param>
    /// <returns>Deterministic Markdown for the result.</returns>
    public static string ExportMarkdown(this ComparisonResult result)
    {
        ArgumentNullException.ThrowIfNull(result);

        return result.Explain(ComparisonExplanationFormat.Markdown);
    }

    /// <summary>
    /// Exports result rows as deterministic JSON Lines.
    /// </summary>
    /// <param name="result">The result to export.</param>
    /// <returns>A lazy sequence of JSON Lines documents for result rows.</returns>
    /// <exception cref="ComparisonExportException">
    /// Thrown when the result's plan contains runtime-only selectors that
    /// cannot be exported as portable data.
    /// </exception>
    public static IEnumerable<string> ExportJsonLines(this ComparisonResult result)
    {
        ArgumentNullException.ThrowIfNull(result);

        return ComparisonExporter.ExportJsonLines(result);
    }

    /// <summary>
    /// Exports a comparison result as a self-contained debug HTML document.
    /// </summary>
    /// <remarks>
    /// The document is intended for workflow-boundary debugging, issue reports,
    /// live incident inspection, and support handoff. It includes inline styles
    /// and does not require a web server, CDN, JavaScript package, or external
    /// asset.
    /// </remarks>
    /// <param name="result">The result to export.</param>
    /// <returns>A self-contained HTML document for visual comparison debugging.</returns>
    public static string ExportDebugHtml(this ComparisonResult result)
    {
        ArgumentNullException.ThrowIfNull(result);

        return ComparisonExporter.ExportDebugHtml(result);
    }

    /// <summary>
    /// Exports a comparison result as deterministic LLM context JSON.
    /// </summary>
    /// <remarks>
    /// The artifact combines a concise analysis contract, summary fields,
    /// Markdown orientation, the full deterministic result JSON, and row-level
    /// documents that can be chunked or streamed into agents.
    /// </remarks>
    /// <param name="result">The result to export.</param>
    /// <returns>Deterministic JSON shaped for LLM and agent analysis.</returns>
    /// <exception cref="ComparisonExportException">
    /// Thrown when the result's plan contains runtime-only selectors that
    /// cannot be exported as portable data.
    /// </exception>
    public static string ExportLlmContext(this ComparisonResult result)
    {
        ArgumentNullException.ThrowIfNull(result);

        return ComparisonExporter.ExportLlmContext(result);
    }

    /// <summary>
    /// Writes a comparison result as a self-contained debug HTML document.
    /// </summary>
    /// <remarks>
    /// Parent directories are created automatically. The resulting file can be
    /// opened directly in a browser and is intended for debug artifacts rather
    /// than ingestion hot paths.
    /// </remarks>
    /// <param name="result">The result to export.</param>
    /// <param name="path">The destination HTML file path.</param>
    /// <exception cref="ArgumentException">Thrown when <paramref name="path" /> is empty.</exception>
    public static void ExportDebugHtml(this ComparisonResult result, string path)
    {
        ArgumentNullException.ThrowIfNull(result);
        ArgumentException.ThrowIfNullOrWhiteSpace(path);

        var fullPath = Path.GetFullPath(path);
        var directory = Path.GetDirectoryName(fullPath);
        if (!string.IsNullOrEmpty(directory))
        {
            Directory.CreateDirectory(directory);
        }

        WriteAtomically(fullPath, result.ExportDebugHtml());
    }

    /// <summary>
    /// Writes a comparison result as deterministic LLM context JSON.
    /// </summary>
    /// <remarks>
    /// Parent directories are created automatically. The resulting file is
    /// intended for LLMs, coding agents, CI triage, issue reports, and support
    /// handoff rather than ingestion hot paths.
    /// </remarks>
    /// <param name="result">The result to export.</param>
    /// <param name="path">The destination JSON file path.</param>
    /// <exception cref="ArgumentException">Thrown when <paramref name="path" /> is empty.</exception>
    public static void ExportLlmContext(this ComparisonResult result, string path)
    {
        ArgumentNullException.ThrowIfNull(result);
        ArgumentException.ThrowIfNullOrWhiteSpace(path);

        var fullPath = Path.GetFullPath(path);
        var directory = Path.GetDirectoryName(fullPath);
        if (!string.IsNullOrEmpty(directory))
        {
            Directory.CreateDirectory(directory);
        }

        WriteAtomically(fullPath, result.ExportLlmContext());
    }

    private static void WriteAtomically(string path, string content)
    {
        var directory = Path.GetDirectoryName(path)!;
        var temporary = Path.Combine(directory, "." + Path.GetFileName(path) + ".tmp-" + Guid.NewGuid().ToString("N"));
        try
        {
            File.WriteAllText(temporary, content);
            File.Move(temporary, path, overwrite: true);
        }
        finally
        {
            if (File.Exists(temporary))
            {
                File.Delete(temporary);
            }
        }
    }
}
