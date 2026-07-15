namespace Spanfold;

/// <summary>
/// Entry point for creating Spanfold event pipelines.
/// </summary>
[Obsolete("Use EventPipeline.For<TEvent>() instead.")]
public static class Spanfold
{
    /// <summary>
    /// Starts a pipeline definition for events of type <typeparamref name="TEvent"/>.
    /// </summary>
    /// <typeparam name="TEvent">The event type consumed by the pipeline.</typeparam>
    /// <returns>A builder for configuring state windows over the event type.</returns>
    public static EventPipelineBuilder<TEvent> For<TEvent>()
    {
        return EventPipeline.For<TEvent>();
    }
}
