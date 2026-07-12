using Spanfold.Internal.Comparison;

namespace Spanfold;

/// <summary>
/// Builds a staged comparison over recorded window history.
/// </summary>
/// <remarks>
/// The builder captures the comparison question without mutating or enumerating
/// the source history. Later stages add target, comparison sources,
/// normalization, and comparator choices before execution. This staged flow is
/// intended to keep "what are we comparing?" separate from "how should windows
/// be normalized and measured?".
/// </remarks>
public sealed class WindowComparisonBuilder
{
    private readonly List<ComparisonSelector> against;
    private readonly List<string> comparators;
    private ComparisonSelector? target;
    private ComparisonScope? scope;
    private ComparisonNormalizationPolicy normalization;
    private ComparisonOutputOptions output;
    private bool isStrict;

    internal WindowComparisonBuilder(WindowHistory history, string name)
    {
        History = history;
        Name = name;
        this.against = [];
        this.comparators = [];
        this.normalization = ComparisonNormalizationPolicy.Default;
        this.output = ComparisonOutputOptions.Default;
    }

    /// <summary>
    /// Gets the user-supplied comparison name.
    /// </summary>
    public string Name { get; }

    internal WindowHistory History { get; }

    /// <summary>
    /// Sets the target selector for the comparison.
    /// </summary>
    /// <param name="name">The target name used in output and diagnostics.</param>
    /// <param name="configure">The selector configuration.</param>
    /// <returns>This builder.</returns>
    public WindowComparisonBuilder Target(
        string name,
        Func<ComparisonSelectorBuilder, ComparisonSelector> configure)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(name);
        ArgumentNullException.ThrowIfNull(configure);

        this.target = configure(new ComparisonSelectorBuilder()).WithName(name);
        return this;
    }

    /// <summary>
    /// Adds a comparison selector.
    /// </summary>
    /// <param name="name">The comparison source name used in output and diagnostics.</param>
    /// <param name="configure">The selector configuration.</param>
    /// <returns>This builder.</returns>
    public WindowComparisonBuilder Against(
        string name,
        Func<ComparisonSelectorBuilder, ComparisonSelector> configure)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(name);
        ArgumentNullException.ThrowIfNull(configure);

        this.against.Add(configure(new ComparisonSelectorBuilder()).WithName(name));
        return this;
    }

    /// <summary>
    /// Adds an any-member comparison cohort.
    /// </summary>
    /// <remarks>
    /// The cohort is materialized as a single comparison side: when any member
    /// source is active over an aligned segment, the cohort covers that segment.
    /// This avoids pairwise residual overcounting for target-vs-cohort
    /// questions.
    /// </remarks>
    /// <param name="name">The cohort name used in output and diagnostics.</param>
    /// <param name="configure">The cohort configuration.</param>
    /// <returns>This builder.</returns>
    public WindowComparisonBuilder AgainstCohort(
        string name,
        Func<ComparisonCohortBuilder, ComparisonCohortBuilder> configure)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(name);
        ArgumentNullException.ThrowIfNull(configure);

        this.against.Add(configure(new ComparisonCohortBuilder()).Build(name));
        return this;
    }

    /// <summary>
    /// Sets the comparison scope.
    /// </summary>
    /// <param name="configure">The scope configuration.</param>
    /// <returns>This builder.</returns>
    public WindowComparisonBuilder Within(Func<ComparisonScopeBuilder, ComparisonScope> configure)
    {
        ArgumentNullException.ThrowIfNull(configure);

        this.scope = configure(new ComparisonScopeBuilder());
        return this;
    }

    /// <summary>
    /// Sets the normalization policy.
    /// </summary>
    /// <param name="configure">The normalization configuration.</param>
    /// <returns>This builder.</returns>
    public WindowComparisonBuilder Normalize(
        Func<ComparisonNormalizationBuilder, ComparisonNormalizationBuilder> configure)
    {
        ArgumentNullException.ThrowIfNull(configure);

        this.normalization = configure(new ComparisonNormalizationBuilder()).Build();
        return this;
    }

    /// <summary>
    /// Adds comparator declarations.
    /// </summary>
    /// <param name="configure">The comparator configuration.</param>
    /// <returns>This builder.</returns>
    public WindowComparisonBuilder Using(
        Func<ComparisonComparatorBuilder, ComparisonComparatorBuilder> configure)
    {
        ArgumentNullException.ThrowIfNull(configure);

        this.comparators.Clear();
        this.comparators.AddRange(configure(new ComparisonComparatorBuilder()).Build());
        return this;
    }

    /// <summary>
    /// Enables strict validation for later execution stages.
    /// </summary>
    /// <remarks>
    /// Strict mode promotes runtime plan-critic warnings to errors. Execution
    /// returns diagnostics without alignment or comparator rows when those
    /// errors would make the analytical output unsafe to trust.
    /// </remarks>
    /// <returns>This builder.</returns>
    public WindowComparisonBuilder Strict()
    {
        this.isStrict = true;
        return this;
    }

    /// <summary>
    /// Builds the comparison plan without executing it.
    /// </summary>
    /// <returns>The built comparison plan.</returns>
    public ComparisonPlan Build()
    {
        return new ComparisonPlan(
            Name,
            this.target,
            this.against,
            this.scope,
            this.normalization,
            this.comparators,
            this.output,
            this.isStrict);
    }

    /// <summary>
    /// Validates the current builder state.
    /// </summary>
    /// <returns>The validation diagnostics for the current plan.</returns>
    public IReadOnlyList<ComparisonPlanDiagnostic> Validate()
    {
        return Build().Validate();
    }

    /// <summary>
    /// Prepares the current plan enough to inspect validation diagnostics.
    /// </summary>
    /// <remarks>
    /// Preparation enumerates recorded history, applies selectors and scope,
    /// normalizes windows, and records exclusions without running comparators.
    /// </remarks>
    /// <returns>A prepared comparison artifact for the current plan.</returns>
    public PreparedComparison Prepare()
    {
        return ComparisonPreparer.Prepare(History, Build());
    }

    /// <summary>
    /// Prepares the current plan as a live snapshot at an evaluation horizon.
    /// </summary>
    /// <remarks>
    /// Live preparation clips currently open windows to <paramref name="evaluationHorizon" />.
    /// Rows that depend on those clipped windows are provisional until the
    /// underlying windows close and the comparison is run again.
    /// </remarks>
    /// <param name="evaluationHorizon">The exclusive horizon used to evaluate open windows.</param>
    /// <returns>A prepared comparison artifact for the live snapshot.</returns>
    public PreparedComparison PrepareLive(TemporalPoint evaluationHorizon)
    {
        return ComparisonPreparer.Prepare(History, BuildLivePlan(evaluationHorizon));
    }

    /// <summary>
    /// Runs the current plan through preparation, alignment, and comparator execution.
    /// </summary>
    /// <remarks>
    /// The returned result materializes row collections so repeated inspection,
    /// explain output, and export do not re-enumerate the source history.
    /// </remarks>
    /// <returns>A comparison result for the current plan.</returns>
    public ComparisonResult Run()
    {
        return ComparisonRuntime.Run(Prepare());
    }

    /// <summary>
    /// Runs the current plan and optionally writes a debug HTML artifact.
    /// </summary>
    /// <remarks>
    /// Use this overload when application configuration controls whether live
    /// or historical comparison runs should emit a visual debug file.
    /// </remarks>
    /// <param name="debugHtml">The optional debug HTML export configuration.</param>
    /// <returns>A comparison result for the current plan.</returns>
    public ComparisonResult Run(ComparisonDebugHtmlOptions debugHtml)
    {
        ArgumentNullException.ThrowIfNull(debugHtml);

        var result = Run();
        debugHtml.ExportIfEnabled(result);

        return result;
    }

    /// <summary>
    /// Runs the current plan and optionally writes an LLM context artifact.
    /// </summary>
    /// <remarks>
    /// Use this overload when application configuration controls whether live
    /// or historical comparison runs should emit agent-readable context JSON.
    /// </remarks>
    /// <param name="llmContext">The optional LLM context export configuration.</param>
    /// <returns>A comparison result for the current plan.</returns>
    public ComparisonResult Run(ComparisonLlmContextOptions llmContext)
    {
        ArgumentNullException.ThrowIfNull(llmContext);

        var result = Run();
        llmContext.ExportIfEnabled(result);

        return result;
    }

    /// <summary>
    /// Runs the current plan as a live snapshot at an evaluation horizon.
    /// </summary>
    /// <remarks>
    /// Live execution reuses the normal comparison pipeline and only changes
    /// normalization: open windows are clipped to <paramref name="evaluationHorizon" />.
    /// The core package does not introduce background timers or subscriptions.
    /// </remarks>
    /// <param name="evaluationHorizon">The exclusive horizon used to evaluate open windows.</param>
    /// <returns>A comparison result with evaluation-horizon and row-finality metadata.</returns>
    public ComparisonResult RunLive(TemporalPoint evaluationHorizon)
    {
        return ComparisonRuntime.Run(PrepareLive(evaluationHorizon));
    }

    /// <summary>
    /// Runs the current plan as a live snapshot and optionally writes a debug HTML artifact.
    /// </summary>
    /// <remarks>
    /// Debug export is useful for incident inspection because live rows may be
    /// provisional while the final source windows are still open.
    /// </remarks>
    /// <param name="evaluationHorizon">The exclusive horizon used to evaluate open windows.</param>
    /// <param name="debugHtml">The optional debug HTML export configuration.</param>
    /// <returns>A comparison result with evaluation-horizon and row-finality metadata.</returns>
    public ComparisonResult RunLive(
        TemporalPoint evaluationHorizon,
        ComparisonDebugHtmlOptions debugHtml)
    {
        ArgumentNullException.ThrowIfNull(debugHtml);

        var result = RunLive(evaluationHorizon);
        debugHtml.ExportIfEnabled(result);

        return result;
    }

    /// <summary>
    /// Runs the current plan as a live snapshot and optionally writes an LLM context artifact.
    /// </summary>
    /// <remarks>
    /// LLM context export is useful for incident triage because it carries the
    /// full result data, row-level documents, and finality metadata in one
    /// deterministic JSON artifact.
    /// </remarks>
    /// <param name="evaluationHorizon">The exclusive horizon used to evaluate open windows.</param>
    /// <param name="llmContext">The optional LLM context export configuration.</param>
    /// <returns>A comparison result with evaluation-horizon and row-finality metadata.</returns>
    public ComparisonResult RunLive(
        TemporalPoint evaluationHorizon,
        ComparisonLlmContextOptions llmContext)
    {
        ArgumentNullException.ThrowIfNull(llmContext);

        var result = RunLive(evaluationHorizon);
        llmContext.ExportIfEnabled(result);

        return result;
    }

    private ComparisonPlan BuildLivePlan(TemporalPoint evaluationHorizon)
    {
        var plan = Build();
        var normalization = plan.Normalization with
        {
            OpenWindowPolicy = ComparisonOpenWindowPolicy.ClipToHorizon,
            OpenWindowHorizon = evaluationHorizon
        };

        return new ComparisonPlan(
            plan.Name,
            plan.Target,
            plan.Against,
            plan.Scope,
            normalization,
            plan.Comparators,
            plan.Output,
            plan.IsStrict);
    }
}
