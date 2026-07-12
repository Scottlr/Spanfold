namespace Spanfold;

/// <summary>
/// Reports callback failures after an event has already been committed.
/// </summary>
/// <typeparam name="TEvent">The event type consumed by the pipeline.</typeparam>
public sealed class IngestionCallbackException<TEvent> : Exception
{
    /// <summary>
    /// Creates a callback failure containing the committed ingestion result.
    /// </summary>
    /// <param name="result">The result committed before callbacks ran.</param>
    /// <param name="errors">The callback failures collected from all callbacks.</param>
    public IngestionCallbackException(
        IngestionResult<TEvent> result,
        IEnumerable<Exception> errors)
        : base("One or more ingestion callbacks failed after the event was committed.", new AggregateException(errors))
    {
        ArgumentNullException.ThrowIfNull(result);
        ArgumentNullException.ThrowIfNull(errors);

        Result = result;
        CallbackErrors = errors.ToArray();
    }

    /// <summary>
    /// Gets the committed ingestion result.
    /// </summary>
    public IngestionResult<TEvent> Result { get; }

    /// <summary>
    /// Gets all callback failures collected for the committed event.
    /// </summary>
    public IReadOnlyList<Exception> CallbackErrors { get; }
}
