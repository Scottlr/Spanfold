namespace Spanfold;

/// <summary>
/// Contains the emissions produced by an ingestion operation.
/// </summary>
/// <typeparam name="TEvent">The event type consumed by the pipeline.</typeparam>
public sealed class IngestionResult<TEvent>
{
    /// <summary>
    /// Creates an immutable ingestion result.
    /// </summary>
    /// <param name="emissions">The emissions produced by ingestion.</param>
    public IngestionResult(IReadOnlyList<WindowEmission<TEvent>> emissions)
    {
        ArgumentNullException.ThrowIfNull(emissions);
        Emissions = Array.AsReadOnly(emissions.ToArray());
    }

    /// <summary>
    /// Gets the emissions produced by ingestion.
    /// </summary>
    public IReadOnlyList<WindowEmission<TEvent>> Emissions { get; }

    /// <summary>
    /// Gets whether any emissions were produced.
    /// </summary>
    public bool HasEmissions => Emissions.Count > 0;

    /// <summary>
    /// Deconstructs the result into emissions and emission presence.
    /// </summary>
    /// <param name="emissions">The emissions produced by ingestion.</param>
    /// <param name="hasEmissions">Whether any emissions were produced.</param>
    public void Deconstruct(
        out IReadOnlyList<WindowEmission<TEvent>> emissions,
        out bool hasEmissions)
    {
        emissions = Emissions;
        hasEmissions = HasEmissions;
    }
}
