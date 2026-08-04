using Spanfold.Internal.Sequences;

namespace Spanfold.Sequences;

/// <summary>
/// Builds and executes an earliest-completion ordered window sequence.
/// </summary>
/// <remarks>
/// Steps are ordered by onset and may overlap. The first step anchors the
/// correlation lane, including its configured key comparer; source and
/// partition identities must match exactly. Complete matches consume their
/// source windows, so evidence is not reused by a later match.
/// </remarks>
public sealed class WindowSequenceBuilder
{
    private readonly WindowHistory history;
    private readonly string name;
    private readonly List<string> steps = [];
    private long? maximumGap;

    internal WindowSequenceBuilder(WindowHistory history, string name)
    {
        this.history = history;
        this.name = name;
    }

    /// <summary>
    /// Adds the first named window-family step.
    /// </summary>
    /// <param name="windowName">The named window family.</param>
    /// <returns>This builder.</returns>
    public WindowSequenceBuilder Step(string windowName)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(windowName);
        if (this.steps.Count != 0)
        {
            throw new InvalidOperationException("Step can only define the first sequence step. Use Then for later steps.");
        }

        this.steps.Add(windowName);
        return this;
    }

    /// <summary>
    /// Adds a later named window-family step.
    /// </summary>
    /// <param name="windowName">The named window family.</param>
    /// <returns>This builder.</returns>
    public WindowSequenceBuilder Then(string windowName)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(windowName);
        if (this.steps.Count == 0)
        {
            throw new InvalidOperationException("A sequence requires Step before Then.");
        }

        this.steps.Add(windowName);
        return this;
    }

    /// <summary>
    /// Limits the inactive processing-position gap between every pair of consecutive steps.
    /// </summary>
    /// <param name="processingPositions">The inclusive maximum inactive gap.</param>
    /// <returns>This builder.</returns>
    public WindowSequenceBuilder WithMaximumGap(long processingPositions)
    {
        ArgumentOutOfRangeException.ThrowIfNegative(processingPositions);
        this.maximumGap = processingPositions;
        return this;
    }

    /// <summary>
    /// Builds and validates the immutable sequence plan.
    /// </summary>
    /// <returns>The sequence plan.</returns>
    public WindowSequencePlan Build()
    {
        if (this.steps.Count < 2)
        {
            throw new InvalidOperationException("A sequence requires at least two named window-family steps.");
        }

        return new WindowSequencePlan(this.name, this.steps, this.maximumGap);
    }

    /// <summary>
    /// Matches a historical sequence using closed evidence.
    /// </summary>
    /// <returns>The deterministic matched sequences.</returns>
    public WindowSequenceResult Run()
    {
        return WindowSequenceRuntime.Run(this.history, Build(), evaluationHorizon: null);
    }

    /// <summary>
    /// Matches a sequence at an explicit live processing-position horizon.
    /// </summary>
    /// <param name="evaluationHorizon">The live processing-position horizon.</param>
    /// <returns>The deterministic matched sequences.</returns>
    public WindowSequenceResult RunLive(TemporalPoint evaluationHorizon)
    {
        if (evaluationHorizon.Axis != TemporalAxis.ProcessingPosition)
        {
            throw new ArgumentException(
                "Sequence live horizons must use processing positions.",
                nameof(evaluationHorizon));
        }

        return WindowSequenceRuntime.Run(this.history, Build(), evaluationHorizon);
    }
}
