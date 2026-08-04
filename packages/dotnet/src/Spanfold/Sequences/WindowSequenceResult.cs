namespace Spanfold.Sequences;

/// <summary>
/// Contains deterministic matches for one ordered sequence plan.
/// </summary>
public sealed class WindowSequenceResult
{
    internal WindowSequenceResult(
        WindowSequencePlan plan,
        IReadOnlyList<WindowSequenceMatch> matches,
        TemporalPoint? evaluationHorizon)
    {
        Plan = plan;
        Matches = Array.AsReadOnly(matches.ToArray());
        EvaluationHorizon = evaluationHorizon;
    }

    /// <summary>Gets the executed sequence plan.</summary>
    public WindowSequencePlan Plan { get; }

    /// <summary>Gets the deterministic earliest-completion matches.</summary>
    public IReadOnlyList<WindowSequenceMatch> Matches { get; }

    /// <summary>Gets the live horizon, or <see langword="null" /> for historical execution.</summary>
    public TemporalPoint? EvaluationHorizon { get; }
}
