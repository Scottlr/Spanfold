namespace Spanfold.Internal.Definitions;

internal sealed class PipelineBuilderOptions<TEvent>
{
    public bool RecordWindows { get; set; }

    public Func<TEvent, DateTimeOffset>? EventTimeSelector { get; set; }

    public string? EventTimeClock { get; set; }
}
