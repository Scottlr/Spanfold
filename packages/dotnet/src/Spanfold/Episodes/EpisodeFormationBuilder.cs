using Spanfold.Internal.Episodes;

namespace Spanfold.Episodes;

/// <summary>
/// Builds and executes an episode-formation plan over recorded windows.
/// </summary>
public sealed class EpisodeFormationBuilder
{
    private readonly WindowHistory history;
    private readonly string name;
    private ComparisonSelector? selector;
    private ComparisonScope? scope;
    private ComparisonNormalizationPolicy normalization = ComparisonNormalizationPolicy.Default;
    private TemporalAxis? toleranceAxis;
    private long stitchToleranceMagnitude;

    internal EpisodeFormationBuilder(WindowHistory history, string name)
    {
        this.history = history;
        this.name = name;
    }

    /// <summary>
    /// Selects the recorded windows that can contribute episode fragments.
    /// </summary>
    /// <param name="configure">The selector configuration.</param>
    /// <returns>This builder.</returns>
    public EpisodeFormationBuilder From(
        Func<ComparisonSelectorBuilder, ComparisonSelector> configure)
    {
        ArgumentNullException.ThrowIfNull(configure);
        this.selector = configure(new ComparisonSelectorBuilder());
        return this;
    }

    /// <summary>
    /// Restricts episode formation to one named window family and temporal axis.
    /// </summary>
    /// <param name="configure">The scope configuration.</param>
    /// <returns>This builder.</returns>
    public EpisodeFormationBuilder Within(
        Func<ComparisonScopeBuilder, ComparisonScope> configure)
    {
        ArgumentNullException.ThrowIfNull(configure);
        this.scope = configure(new ComparisonScopeBuilder());
        return this;
    }

    /// <summary>
    /// Configures temporal normalization before fragments are stitched.
    /// </summary>
    /// <param name="configure">The normalization configuration.</param>
    /// <returns>This builder.</returns>
    public EpisodeFormationBuilder Normalize(
        Func<ComparisonNormalizationBuilder, ComparisonNormalizationBuilder> configure)
    {
        ArgumentNullException.ThrowIfNull(configure);
        var configured = configure(new ComparisonNormalizationBuilder());
        ArgumentNullException.ThrowIfNull(configured);
        this.normalization = configured.Build();
        return this;
    }

    /// <summary>
    /// Stitches processing-position fragments separated by at most the supplied magnitude.
    /// </summary>
    /// <param name="positionMagnitude">The maximum processing-position gap.</param>
    /// <returns>This builder.</returns>
    public EpisodeFormationBuilder StitchGapsUpTo(long positionMagnitude)
    {
        ArgumentOutOfRangeException.ThrowIfNegative(positionMagnitude);
        this.toleranceAxis = TemporalAxis.ProcessingPosition;
        this.stitchToleranceMagnitude = positionMagnitude;
        return this;
    }

    /// <summary>
    /// Stitches timestamp fragments separated by at most the supplied duration.
    /// </summary>
    /// <param name="duration">The maximum event-time gap.</param>
    /// <returns>This builder.</returns>
    public EpisodeFormationBuilder StitchGapsUpTo(TimeSpan duration)
    {
        ArgumentOutOfRangeException.ThrowIfNegative(duration.Ticks);
        this.toleranceAxis = TemporalAxis.Timestamp;
        this.stitchToleranceMagnitude = duration.Ticks;
        return this;
    }

    /// <summary>
    /// Builds an immutable episode-formation plan without executing it.
    /// </summary>
    /// <returns>The validated plan.</returns>
    public EpisodeFormationPlan Build()
    {
        if (!this.selector.HasValue || !this.selector.Value.IsDefined)
        {
            throw new InvalidOperationException("Episode formation requires a source selector.");
        }

        if (this.scope is null)
        {
            throw new InvalidOperationException("Episode formation requires a scope.");
        }

        if (string.IsNullOrWhiteSpace(this.scope.WindowName))
        {
            throw new InvalidOperationException("Episode formation requires one named window family.");
        }

        if (this.scope.TimeAxis != this.normalization.TimeAxis)
        {
            throw new InvalidOperationException("Episode scope and normalization must use the same temporal axis.");
        }

        if (this.toleranceAxis.HasValue && this.toleranceAxis.Value != this.normalization.TimeAxis)
        {
            throw new InvalidOperationException("Episode stitch tolerance must use the normalized temporal axis.");
        }

        ValidateHorizonPolicy(this.normalization);

        return new EpisodeFormationPlan(
            this.name,
            this.selector.Value,
            this.scope,
            this.normalization,
            new EpisodeFormationPolicy(this.normalization.TimeAxis, this.stitchToleranceMagnitude));
    }

    /// <summary>
    /// Forms episodes using the horizon, if any, already configured on the plan.
    /// </summary>
    /// <returns>The materialized episode set.</returns>
    public EpisodeSet Run()
    {
        return EpisodeFormationRuntime.Run(this.history, Build());
    }

    /// <summary>
    /// Forms episodes at an explicit live evaluation horizon.
    /// </summary>
    /// <param name="evaluationHorizon">The effective end for open evidence and settling.</param>
    /// <returns>The materialized episode set.</returns>
    public EpisodeSet RunLive(TemporalPoint evaluationHorizon)
    {
        var plan = Build();
        if (evaluationHorizon.Axis == TemporalAxis.Unknown)
        {
            throw new ArgumentException("Episode evaluation horizon must use a known temporal axis.", nameof(evaluationHorizon));
        }

        if (evaluationHorizon.Axis != plan.Formation.TimeAxis)
        {
            throw new ArgumentException("Episode evaluation horizon must use the plan temporal axis.", nameof(evaluationHorizon));
        }

        if (plan.Normalization.KnownAt.HasValue || plan.Normalization.OpenWindowHorizon.HasValue)
        {
            throw new InvalidOperationException("RunLive cannot be combined with a separately configured episode horizon.");
        }

        var effectiveNormalization = plan.Normalization with
        {
            OpenWindowPolicy = ComparisonOpenWindowPolicy.ClipToHorizon,
            OpenWindowHorizon = evaluationHorizon
        };
        var effectivePlan = new EpisodeFormationPlan(
            plan.Name,
            plan.Selector,
            plan.Scope,
            effectiveNormalization,
            plan.Formation);

        return EpisodeFormationRuntime.Run(this.history, effectivePlan);
    }

    private static void ValidateHorizonPolicy(ComparisonNormalizationPolicy normalization)
    {
        if (normalization.KnownAt.HasValue && normalization.OpenWindowHorizon.HasValue)
        {
            throw new InvalidOperationException("Episode formation accepts only one horizon source.");
        }

        if (normalization.KnownAt is { Axis: not TemporalAxis.ProcessingPosition })
        {
            throw new InvalidOperationException("Episode known-at horizons must use processing positions.");
        }

        if (normalization.KnownAt.HasValue && normalization.TimeAxis == TemporalAxis.Timestamp)
        {
            throw new InvalidOperationException("Known-at episode formation is not supported on the event-time axis.");
        }

        if (normalization.OpenWindowHorizon.HasValue
            && normalization.OpenWindowHorizon.Value.Axis != normalization.TimeAxis)
        {
            throw new InvalidOperationException("Episode open-window horizon must use the normalized temporal axis.");
        }
    }
}
