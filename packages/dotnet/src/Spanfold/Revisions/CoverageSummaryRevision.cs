namespace Spanfold.Revisions;

/// <summary>Describes an exact aggregate coverage change in one logical scope.</summary>
public sealed record CoverageSummaryRevision(
    string WindowName,
    object Key,
    object? Partition,
    long? PreviousTargetMagnitude,
    long? CurrentTargetMagnitude,
    long? PreviousCoveredMagnitude,
    long? CurrentCoveredMagnitude,
    double? PreviousCoverageRatio,
    double? CurrentCoverageRatio);
