namespace Spanfold.Episodes;

/// <summary>
/// Interprets target episodes as references and against episodes as detections.
/// </summary>
/// <param name="ReferenceEpisodeCount">The number of target reference episodes.</param>
/// <param name="DetectedReferenceEpisodeCount">References in components containing detections.</param>
/// <param name="MissedReferenceEpisodeCount">References with no detection relationship.</param>
/// <param name="DetectionEpisodeCount">The number of against detection episodes.</param>
/// <param name="MatchedDetectionEpisodeCount">Detections in components containing references.</param>
/// <param name="UnexpectedDetectionEpisodeCount">Detections with no reference relationship.</param>
/// <param name="Recall">Detected references divided by all references.</param>
/// <param name="Precision">Matched detections divided by all detections.</param>
/// <param name="F1Score">The harmonic mean of defined precision and recall.</param>
public sealed record EpisodeReferenceScorecard(
    int ReferenceEpisodeCount,
    int DetectedReferenceEpisodeCount,
    int MissedReferenceEpisodeCount,
    int DetectionEpisodeCount,
    int MatchedDetectionEpisodeCount,
    int UnexpectedDetectionEpisodeCount,
    double? Recall,
    double? Precision,
    double? F1Score);
