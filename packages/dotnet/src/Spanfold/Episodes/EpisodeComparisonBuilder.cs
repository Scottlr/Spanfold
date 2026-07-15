using Spanfold.Internal.Episodes;

namespace Spanfold.Episodes;

/// <summary>
/// Builds and executes a comparison between two episode definitions.
/// </summary>
public sealed class EpisodeComparisonBuilder
{
    private readonly WindowHistory history;
    private readonly string name;
    private string? targetName;
    private ComparisonSelector? target;
    private string? againstName;
    private ComparisonSelector? against;
    private ComparisonScope? scope;
    private ComparisonNormalizationPolicy normalization = ComparisonNormalizationPolicy.Default;
    private TemporalAxis? stitchToleranceAxis;
    private long stitchToleranceMagnitude;
    private TemporalAxis? relationToleranceAxis;
    private long relationToleranceMagnitude;

    internal EpisodeComparisonBuilder(WindowHistory history, string name)
    {
        this.history = history;
        this.name = name;
    }

    /// <summary>
    /// Configures the target episode selector.
    /// </summary>
    /// <param name="name">The target side name.</param>
    /// <param name="configure">The selector configuration.</param>
    /// <returns>This builder.</returns>
    public EpisodeComparisonBuilder Target(
        string name,
        Func<ComparisonSelectorBuilder, ComparisonSelector> configure)
    {
        if (this.target.HasValue)
        {
            throw new InvalidOperationException("Episode comparison accepts exactly one target selector.");
        }

        ArgumentException.ThrowIfNullOrWhiteSpace(name);
        ArgumentNullException.ThrowIfNull(configure);
        this.targetName = name;
        this.target = configure(new ComparisonSelectorBuilder());
        return this;
    }

    /// <summary>
    /// Configures the against episode selector.
    /// </summary>
    /// <param name="name">The against side name.</param>
    /// <param name="configure">The selector configuration.</param>
    /// <returns>This builder.</returns>
    public EpisodeComparisonBuilder Against(
        string name,
        Func<ComparisonSelectorBuilder, ComparisonSelector> configure)
    {
        if (this.against.HasValue)
        {
            throw new InvalidOperationException("Episode comparison accepts exactly one against selector.");
        }

        ArgumentException.ThrowIfNullOrWhiteSpace(name);
        ArgumentNullException.ThrowIfNull(configure);
        this.againstName = name;
        this.against = configure(new ComparisonSelectorBuilder());
        return this;
    }

    /// <summary>
    /// Restricts both sides to one named window family and temporal axis.
    /// </summary>
    /// <param name="configure">The scope configuration.</param>
    /// <returns>This builder.</returns>
    public EpisodeComparisonBuilder Within(
        Func<ComparisonScopeBuilder, ComparisonScope> configure)
    {
        ArgumentNullException.ThrowIfNull(configure);
        this.scope = configure(new ComparisonScopeBuilder());
        return this;
    }

    /// <summary>
    /// Configures temporal normalization shared by both episode sets.
    /// </summary>
    /// <param name="configure">The normalization configuration.</param>
    /// <returns>This builder.</returns>
    public EpisodeComparisonBuilder Normalize(
        Func<ComparisonNormalizationBuilder, ComparisonNormalizationBuilder> configure)
    {
        ArgumentNullException.ThrowIfNull(configure);
        var configured = configure(new ComparisonNormalizationBuilder());
        ArgumentNullException.ThrowIfNull(configured);
        this.normalization = configured.Build();
        return this;
    }

    /// <summary>
    /// Stitches same-side processing-position fragments within the supplied gap.
    /// </summary>
    /// <param name="positionMagnitude">The maximum same-side gap.</param>
    /// <returns>This builder.</returns>
    public EpisodeComparisonBuilder StitchGapsUpTo(long positionMagnitude)
    {
        ArgumentOutOfRangeException.ThrowIfNegative(positionMagnitude);
        this.stitchToleranceAxis = TemporalAxis.ProcessingPosition;
        this.stitchToleranceMagnitude = positionMagnitude;
        return this;
    }

    /// <summary>
    /// Stitches same-side event-time fragments within the supplied duration.
    /// </summary>
    /// <param name="duration">The maximum same-side gap.</param>
    /// <returns>This builder.</returns>
    public EpisodeComparisonBuilder StitchGapsUpTo(TimeSpan duration)
    {
        ArgumentOutOfRangeException.ThrowIfNegative(duration.Ticks);
        this.stitchToleranceAxis = TemporalAxis.Timestamp;
        this.stitchToleranceMagnitude = duration.Ticks;
        return this;
    }

    /// <summary>
    /// Relates cross-side processing-position fragments within the supplied gap.
    /// </summary>
    /// <param name="positionMagnitude">The maximum cross-side gap.</param>
    /// <returns>This builder.</returns>
    public EpisodeComparisonBuilder RelateWithin(long positionMagnitude)
    {
        ArgumentOutOfRangeException.ThrowIfNegative(positionMagnitude);
        this.relationToleranceAxis = TemporalAxis.ProcessingPosition;
        this.relationToleranceMagnitude = positionMagnitude;
        return this;
    }

    /// <summary>
    /// Relates cross-side event-time fragments within the supplied duration.
    /// </summary>
    /// <param name="duration">The maximum cross-side gap.</param>
    /// <returns>This builder.</returns>
    public EpisodeComparisonBuilder RelateWithin(TimeSpan duration)
    {
        ArgumentOutOfRangeException.ThrowIfNegative(duration.Ticks);
        this.relationToleranceAxis = TemporalAxis.Timestamp;
        this.relationToleranceMagnitude = duration.Ticks;
        return this;
    }

    /// <summary>
    /// Builds an immutable episode-comparison plan without executing it.
    /// </summary>
    /// <returns>The validated comparison plan.</returns>
    public EpisodeComparisonPlan Build()
    {
        if (!this.target.HasValue || !this.target.Value.IsDefined)
        {
            throw new InvalidOperationException("Episode comparison requires one target selector.");
        }

        if (!this.against.HasValue || !this.against.Value.IsDefined)
        {
            throw new InvalidOperationException("Episode comparison requires one against selector.");
        }

        if (this.scope is null || string.IsNullOrWhiteSpace(this.scope.WindowName))
        {
            throw new InvalidOperationException("Episode comparison requires one named window family.");
        }

        if (this.scope.TimeAxis != this.normalization.TimeAxis)
        {
            throw new InvalidOperationException("Episode scope and normalization must use the same temporal axis.");
        }

        if (this.stitchToleranceAxis.HasValue
            && this.stitchToleranceAxis.Value != this.normalization.TimeAxis)
        {
            throw new InvalidOperationException("Episode stitch tolerance must use the normalized temporal axis.");
        }

        if (this.relationToleranceAxis.HasValue
            && this.relationToleranceAxis.Value != this.normalization.TimeAxis)
        {
            throw new InvalidOperationException("Episode relation tolerance must use the normalized temporal axis.");
        }

        ValidateHorizonPolicy(this.normalization);

        return new EpisodeComparisonPlan(
            this.name,
            this.targetName!,
            this.target.Value,
            this.againstName!,
            this.against.Value,
            this.scope,
            this.normalization,
            new EpisodeFormationPolicy(this.normalization.TimeAxis, this.stitchToleranceMagnitude),
            new EpisodeRelationPolicy(this.normalization.TimeAxis, this.relationToleranceMagnitude));
    }

    /// <summary>
    /// Forms and relates both episode sets using the configured horizon, if any.
    /// </summary>
    /// <returns>The materialized episode comparison.</returns>
    public EpisodeComparisonResult Run()
    {
        return EpisodeRelationRuntime.Run(this.history, Build());
    }

    /// <summary>
    /// Forms and relates both episode sets at an explicit live horizon.
    /// </summary>
    /// <param name="evaluationHorizon">The effective end for open evidence and settling.</param>
    /// <returns>The materialized episode comparison.</returns>
    public EpisodeComparisonResult RunLive(TemporalPoint evaluationHorizon)
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
        var effectivePlan = new EpisodeComparisonPlan(
            plan.Name,
            plan.TargetName,
            plan.Target,
            plan.AgainstName,
            plan.Against,
            plan.Scope,
            effectiveNormalization,
            plan.Formation,
            plan.Relation);

        return EpisodeRelationRuntime.Run(this.history, effectivePlan);
    }

    private static void ValidateHorizonPolicy(ComparisonNormalizationPolicy normalization)
    {
        if (normalization.KnownAt.HasValue && normalization.OpenWindowHorizon.HasValue)
        {
            throw new InvalidOperationException("Episode comparison accepts only one horizon source.");
        }

        if (normalization.KnownAt is { Axis: not TemporalAxis.ProcessingPosition })
        {
            throw new InvalidOperationException("Episode known-at horizons must use processing positions.");
        }

        if (normalization.KnownAt.HasValue && normalization.TimeAxis == TemporalAxis.Timestamp)
        {
            throw new InvalidOperationException("Known-at episode comparison is not supported on the event-time axis.");
        }

        if (normalization.OpenWindowHorizon.HasValue
            && normalization.OpenWindowHorizon.Value.Axis != normalization.TimeAxis)
        {
            throw new InvalidOperationException("Episode open-window horizon must use the normalized temporal axis.");
        }
    }
}
