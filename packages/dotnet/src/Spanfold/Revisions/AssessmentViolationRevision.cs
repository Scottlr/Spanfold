using Spanfold.Assessment;

namespace Spanfold.Revisions;

/// <summary>Describes an assessment violation introduced, revised, or resolved by a new snapshot.</summary>
public sealed record AssessmentViolationRevision(
    ComparisonRevisionKind Kind,
    AssessmentViolation? Previous,
    AssessmentViolation? Current);
