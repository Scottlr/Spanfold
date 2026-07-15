namespace Spanfold;

/// <summary>
/// Entry point for creating Spanfold event pipelines.
/// </summary>
public static class EventPipeline
{
    /// <summary>
    /// Starts a pipeline definition for events of type <typeparamref name="TEvent" />.
    /// </summary>
    public static EventPipelineBuilder<TEvent> For<TEvent>()
    {
        return new EventPipelineBuilder<TEvent>();
    }
}
