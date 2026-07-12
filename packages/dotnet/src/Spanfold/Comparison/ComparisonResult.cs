namespace Spanfold;

/// <summary>
/// Represents the current comparison result.
/// </summary>
/// <remarks>
/// Comparison results are immutable snapshots. Row and summary collections are
/// materialized so consumers can inspect the result deterministically without
/// re-running comparison logic. Rows keep stable row positions and source
/// record IDs so a result can be exported, explained, and reviewed without a
/// debugger.
/// </remarks>
public sealed class ComparisonResult
{
    /// <summary>
    /// Creates a comparison result.
    /// </summary>
    /// <param name="plan">The comparison plan.</param>
    /// <param name="diagnostics">The validation and execution diagnostics.</param>
    /// <param name="prepared">The prepared comparison artifact, when execution reached preparation.</param>
    /// <param name="aligned">The aligned comparison artifact, when execution reached alignment.</param>
    /// <param name="comparatorSummaries">The comparator summaries.</param>
    /// <param name="overlapRows">Rows emitted by the overlap comparator.</param>
    /// <param name="residualRows">Rows emitted by the residual comparator.</param>
    /// <param name="missingRows">Rows emitted by the missing comparator.</param>
    /// <param name="coverageRows">Rows emitted by the coverage comparator.</param>
    /// <param name="coverageSummaries">Summaries emitted by the coverage comparator.</param>
    /// <param name="gapRows">Rows emitted by the gap comparator.</param>
    /// <param name="symmetricDifferenceRows">Rows emitted by the symmetric-difference comparator.</param>
    /// <param name="containmentRows">Rows emitted by the containment comparator.</param>
    /// <param name="leadLagRows">Rows emitted by the lead/lag comparator.</param>
    /// <param name="leadLagSummaries">Summaries emitted by the lead/lag comparator.</param>
    /// <param name="asOfRows">Rows emitted by the as-of comparator.</param>
    /// <param name="rowFinalities">Finality metadata for materialized rows.</param>
    /// <param name="extensionMetadata">Serializable metadata emitted by comparison extensions.</param>
    internal ComparisonResult(
        ComparisonPlan plan,
        IEnumerable<ComparisonPlanDiagnostic> diagnostics,
        PreparedComparison? prepared = null,
        AlignedComparison? aligned = null,
        IEnumerable<ComparatorSummary>? comparatorSummaries = null,
        IEnumerable<OverlapRow>? overlapRows = null,
        IEnumerable<ResidualRow>? residualRows = null,
        IEnumerable<MissingRow>? missingRows = null,
        IEnumerable<CoverageRow>? coverageRows = null,
        IEnumerable<CoverageSummary>? coverageSummaries = null,
        IEnumerable<GapRow>? gapRows = null,
        IEnumerable<SymmetricDifferenceRow>? symmetricDifferenceRows = null,
        IEnumerable<ContainmentRow>? containmentRows = null,
        IEnumerable<LeadLagRow>? leadLagRows = null,
        IEnumerable<LeadLagSummary>? leadLagSummaries = null,
        IEnumerable<AsOfRow>? asOfRows = null,
        IEnumerable<ComparisonRowFinality>? rowFinalities = null,
        IEnumerable<ComparisonExtensionMetadata>? extensionMetadata = null)
    {
        ArgumentNullException.ThrowIfNull(plan);
        ArgumentNullException.ThrowIfNull(diagnostics);

        Plan = plan;
        Diagnostics = Materialize(diagnostics);
        Prepared = prepared;
        Aligned = aligned;
        ComparatorSummaries = Materialize(comparatorSummaries);
        OverlapRows = Materialize(overlapRows);
        ResidualRows = Materialize(residualRows);
        MissingRows = Materialize(missingRows);
        CoverageRows = Materialize(coverageRows);
        CoverageSummaries = Materialize(coverageSummaries);
        GapRows = Materialize(gapRows);
        SymmetricDifferenceRows = Materialize(symmetricDifferenceRows);
        ContainmentRows = Materialize(containmentRows);
        LeadLagRows = Materialize(leadLagRows);
        LeadLagSummaries = Materialize(leadLagSummaries);
        AsOfRows = Materialize(asOfRows);
        RowFinalities = Materialize(rowFinalities);
        ExtensionMetadata = Materialize(extensionMetadata);
        RecordEvidence = prepared is null
            ? []
            : prepared.SelectedWindows
                .Select(static window => new WindowRecordEvidence(window))
                .GroupBy(static evidence => evidence.Id)
                .Select(static group => group.First())
                .OrderBy(static evidence => evidence.Id.Value, StringComparer.Ordinal)
                .ToArray();
    }

    /// <summary>
    /// Gets the comparison plan.
    /// </summary>
    public ComparisonPlan Plan { get; }

    /// <summary>
    /// Gets the known-at availability point used by the result, when one was configured.
    /// </summary>
    /// <remarks>
    /// Known-at is availability time, not event time. It describes what the
    /// comparison was allowed to know when the result was produced.
    /// </remarks>
    public TemporalPoint? KnownAt => Plan.Normalization.KnownAt;

    /// <summary>
    /// Gets the live evaluation horizon used to clip open windows, when any.
    /// </summary>
    /// <remarks>
    /// Rows that depend on open windows clipped to this horizon are provisional
    /// until those source windows close and the result is recomputed.
    /// </remarks>
    public TemporalPoint? EvaluationHorizon =>
        Plan.Normalization.OpenWindowPolicy == ComparisonOpenWindowPolicy.ClipToHorizon
            ? Plan.Normalization.OpenWindowHorizon
            : null;

    /// <summary>
    /// Gets the validation and execution diagnostics.
    /// </summary>
    public IReadOnlyList<ComparisonPlanDiagnostic> Diagnostics { get; }

    /// <summary>
    /// Gets the prepared comparison artifact, when execution reached preparation.
    /// </summary>
    public PreparedComparison? Prepared { get; }

    /// <summary>
    /// Gets the aligned comparison artifact, when execution reached alignment.
    /// </summary>
    public AlignedComparison? Aligned { get; }

    /// <summary>
    /// Gets comparator summaries in declaration order.
    /// </summary>
    public IReadOnlyList<ComparatorSummary> ComparatorSummaries { get; }

    /// <summary>
    /// Gets rows emitted by the overlap comparator.
    /// </summary>
    public IReadOnlyList<OverlapRow> OverlapRows { get; }

    /// <summary>
    /// Gets rows emitted by the residual comparator.
    /// </summary>
    public IReadOnlyList<ResidualRow> ResidualRows { get; }

    /// <summary>
    /// Gets rows emitted by the missing comparator.
    /// </summary>
    public IReadOnlyList<MissingRow> MissingRows { get; }

    /// <summary>
    /// Gets rows emitted by the coverage comparator.
    /// </summary>
    public IReadOnlyList<CoverageRow> CoverageRows { get; }

    /// <summary>
    /// Gets summaries emitted by the coverage comparator.
    /// </summary>
    public IReadOnlyList<CoverageSummary> CoverageSummaries { get; }

    /// <summary>
    /// Gets rows emitted by the gap comparator.
    /// </summary>
    public IReadOnlyList<GapRow> GapRows { get; }

    /// <summary>
    /// Gets rows emitted by the symmetric-difference comparator.
    /// </summary>
    public IReadOnlyList<SymmetricDifferenceRow> SymmetricDifferenceRows { get; }

    /// <summary>
    /// Gets rows emitted by the containment comparator.
    /// </summary>
    public IReadOnlyList<ContainmentRow> ContainmentRows { get; }

    /// <summary>
    /// Gets rows emitted by the lead/lag comparator.
    /// </summary>
    public IReadOnlyList<LeadLagRow> LeadLagRows { get; }

    /// <summary>
    /// Gets summaries emitted by the lead/lag comparator.
    /// </summary>
    public IReadOnlyList<LeadLagSummary> LeadLagSummaries { get; }

    /// <summary>
    /// Gets rows emitted by the as-of comparator.
    /// </summary>
    public IReadOnlyList<AsOfRow> AsOfRows { get; }

    /// <summary>
    /// Gets finality metadata for emitted result rows.
    /// </summary>
    public IReadOnlyList<ComparisonRowFinality> RowFinalities { get; }

    /// <summary>
    /// Gets serializable metadata emitted by comparison extensions.
    /// </summary>
    public IReadOnlyList<ComparisonExtensionMetadata> ExtensionMetadata { get; }

    /// <summary>
    /// Gets source segment, tag, and boundary evidence for selected records.
    /// </summary>
    public IReadOnlyList<WindowRecordEvidence> RecordEvidence { get; }

    /// <summary>
    /// Gets whether the result has no error diagnostics.
    /// </summary>
    /// <remarks>
    /// Warnings remain visible in <see cref="Diagnostics" /> and explain/export
    /// output, but they do not make the result invalid.
    /// </remarks>
    public bool IsValid => Diagnostics.All(static diagnostic =>
        diagnostic.Severity != ComparisonPlanDiagnosticSeverity.Error);

    private static IReadOnlyList<T> Materialize<T>(IEnumerable<T>? values)
    {
        return values switch
        {
            null => [],
            T[] array => array.ToArray(),
            _ => values.ToArray()
        };
    }
}
