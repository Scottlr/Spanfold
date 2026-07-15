namespace Spanfold.Revisions;

/// <summary>Describes a lead/lag aggregate change for one measurement configuration.</summary>
public sealed record LeadLagSummaryRevision(
    LeadLagTransition Transition,
    TemporalAxis Axis,
    long ToleranceMagnitude,
    LeadLagSummary? Previous,
    LeadLagSummary? Current);
