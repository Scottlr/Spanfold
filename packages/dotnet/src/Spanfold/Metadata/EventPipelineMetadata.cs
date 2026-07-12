namespace Spanfold;

/// <summary>
/// Describes a built event pipeline.
/// </summary>
/// <param name="EventType">The event type consumed by the pipeline.</param>
/// <param name="Windows">The configured source windows.</param>
public sealed record EventPipelineMetadata(
    Type EventType,
    IReadOnlyList<WindowMetadata> Windows)
{
    /// <summary>Gets configured source window metadata.</summary>
    public IReadOnlyList<WindowMetadata> Windows { get; } = Array.AsReadOnly(Windows.ToArray());
}
