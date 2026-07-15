namespace Spanfold.Artifacts;

/// <summary>
/// Describes the text format used when rendering deterministic comparison explain output.
/// </summary>
public enum ComparisonExplanationFormat
{
    /// <summary>
    /// Render readable plain text.
    /// </summary>
    PlainText = 0,

    /// <summary>
    /// Render Markdown suitable for reports, notebooks, and agent-readable artifacts.
    /// </summary>
    Markdown = 1
}
