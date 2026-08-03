use super::{
    Episode, EpisodeComparisonResult, EpisodeReferenceScorecard, EpisodeRelation,
    EpisodeRelationKind,
};

impl EpisodeComparisonResult {
    /// Returns components of one kind in deterministic result order.
    #[must_use]
    pub fn relations_of_kind(&self, kind: EpisodeRelationKind) -> Vec<&EpisodeRelation> {
        self.relations()
            .iter()
            .filter(|relation| relation.kind() == kind)
            .collect()
    }

    /// Returns target episodes with no against relationship in result order.
    #[must_use]
    pub fn unmatched_target_episodes(&self) -> Vec<&Episode> {
        self.unmatched_episodes(EpisodeRelationKind::UnmatchedTarget, true)
    }

    /// Returns against episodes with no target relationship in result order.
    #[must_use]
    pub fn unmatched_against_episodes(&self) -> Vec<&Episode> {
        self.unmatched_episodes(EpisodeRelationKind::UnmatchedAgainst, false)
    }

    /// Explicitly interprets targets as references and against episodes as detections.
    #[must_use]
    pub fn as_reference(&self) -> EpisodeReferenceScorecard {
        let summary = self.summary();
        let recall = rate(
            summary.matched_target_episode_count(),
            self.target_episodes().episodes().len(),
        );
        let precision = rate(
            summary.matched_against_episode_count(),
            self.against_episodes().episodes().len(),
        );
        let f1_score = recall.zip(precision).map(|(recall, precision)| {
            let total = recall + precision;
            if total == 0.0 {
                0.0
            } else {
                2.0 * recall * precision / total
            }
        });
        EpisodeReferenceScorecard {
            reference_episode_count: self.target_episodes().episodes().len(),
            detected_reference_episode_count: summary.matched_target_episode_count(),
            missed_reference_episode_count: summary.unmatched_target_episode_count(),
            detection_episode_count: self.against_episodes().episodes().len(),
            matched_detection_episode_count: summary.matched_against_episode_count(),
            unexpected_detection_episode_count: summary.unmatched_against_episode_count(),
            recall,
            precision,
            f1_score,
        }
    }

    fn unmatched_episodes(&self, kind: EpisodeRelationKind, target: bool) -> Vec<&Episode> {
        self.relations()
            .iter()
            .filter(|relation| relation.kind() == kind)
            .flat_map(|relation| {
                if target {
                    relation.target_episodes().iter()
                } else {
                    relation.against_episodes().iter()
                }
            })
            .collect()
    }
}

fn rate(numerator: usize, denominator: usize) -> Option<f64> {
    (denominator != 0).then(|| numerator as f64 / denominator as f64)
}

#[cfg(test)]
mod tests {
    use crate::{
        ClosedWindow, ComparisonScope, ComparisonSelector, EpisodeRelationKind, TemporalRange,
        WindowHistory, WindowRecordId,
    };

    fn closed(id: &str, start: i64, end: i64, source: &str, key: &str) -> ClosedWindow {
        ClosedWindow {
            id: WindowRecordId::new(id).unwrap(),
            window_name: "State".to_owned(),
            key: key.to_owned(),
            range: TemporalRange::positions(start, end).unwrap(),
            known_at: None,
            source: Some(source.to_owned()),
            partition: None,
            segments: Vec::new(),
            tags: Vec::new(),
            boundary_reason: None,
            boundary_changes: Vec::new(),
        }
    }

    fn compare(records: Vec<ClosedWindow>) -> crate::EpisodeComparisonResult {
        let history = WindowHistory::from_records(records, []).unwrap();
        history
            .compare_episodes("comparison")
            .target("reference", ComparisonSelector::for_source("reference"))
            .against("detection", ComparisonSelector::for_source("detection"))
            .scope(ComparisonScope::window("State"))
            .run()
            .unwrap()
    }

    #[test]
    fn neutral_queries_preserve_materialized_result_order() {
        let result = compare(vec![
            closed("t-late", 20, 22, "reference", "late"),
            closed("t-early", 0, 2, "reference", "early"),
            closed("a", 10, 12, "detection", "against"),
        ]);
        let unmatched = result.unmatched_target_episodes();
        assert_eq!(
            unmatched
                .iter()
                .map(|episode| episode.key())
                .collect::<Vec<_>>(),
            ["early", "late"]
        );
        assert_eq!(
            result
                .relations_of_kind(EpisodeRelationKind::UnmatchedTarget)
                .len(),
            2
        );
        assert_eq!(result.unmatched_against_episodes()[0].key(), "against");
    }

    #[test]
    fn reference_scorecard_uses_side_specific_denominators_and_undefined_empty_rates() {
        let partial = compare(vec![
            closed("t1", 0, 2, "reference", "matched"),
            closed("a1", 0, 2, "detection", "matched"),
            closed("t2", 10, 12, "reference", "missed"),
            closed("a2", 20, 22, "detection", "unexpected"),
        ])
        .as_reference();
        assert_eq!(
            (
                partial.reference_episode_count(),
                partial.detected_reference_episode_count(),
                partial.missed_reference_episode_count()
            ),
            (2, 1, 1)
        );
        assert_eq!(
            (
                partial.detection_episode_count(),
                partial.matched_detection_episode_count(),
                partial.unexpected_detection_episode_count()
            ),
            (2, 1, 1)
        );
        assert_eq!(
            (partial.recall(), partial.precision(), partial.f1_score()),
            (Some(0.5), Some(0.5), Some(0.5))
        );

        let empty = compare(Vec::new()).as_reference();
        assert_eq!(
            (empty.recall(), empty.precision(), empty.f1_score()),
            (None, None, None)
        );
        let detection_only = compare(vec![closed("a", 0, 2, "detection", "only")]).as_reference();
        assert_eq!(
            (
                detection_only.recall(),
                detection_only.precision(),
                detection_only.f1_score()
            ),
            (None, Some(0.0), None)
        );
    }

    #[test]
    fn target_only_reference_scorecard_has_defined_zero_recall_and_undefined_precision() {
        let scorecard = compare(vec![closed("t", 0, 2, "reference", "only")]).as_reference();

        assert_eq!(scorecard.reference_episode_count(), 1);
        assert_eq!(scorecard.detected_reference_episode_count(), 0);
        assert_eq!(scorecard.missed_reference_episode_count(), 1);
        assert_eq!(scorecard.detection_episode_count(), 0);
        assert_eq!(scorecard.matched_detection_episode_count(), 0);
        assert_eq!(scorecard.unexpected_detection_episode_count(), 0);
        assert_eq!(scorecard.recall(), Some(0.0));
        assert_eq!(scorecard.precision(), None);
        assert_eq!(scorecard.f1_score(), None);
    }

    #[test]
    fn split_merge_and_complex_reference_scorecards_count_matched_members_by_side() {
        let split = compare(vec![
            closed("t", 0, 10, "reference", "split"),
            closed("a1", 0, 4, "detection", "split"),
            closed("a2", 6, 10, "detection", "split"),
            closed("t-missed", 20, 22, "reference", "missed"),
        ])
        .as_reference();
        assert_eq!(
            (
                split.detected_reference_episode_count(),
                split.matched_detection_episode_count(),
                split.missed_reference_episode_count(),
                split.unexpected_detection_episode_count()
            ),
            (1, 2, 1, 0)
        );
        assert_eq!((split.recall(), split.precision()), (Some(0.5), Some(1.0)));
        assert_eq!(split.f1_score(), Some(2.0 / 3.0));

        let merge = compare(vec![
            closed("t1", 0, 4, "reference", "merge"),
            closed("t2", 6, 10, "reference", "merge"),
            closed("a", 0, 10, "detection", "merge"),
            closed("a-unexpected", 20, 22, "detection", "unexpected"),
        ])
        .as_reference();
        assert_eq!(
            (
                merge.detected_reference_episode_count(),
                merge.matched_detection_episode_count(),
                merge.missed_reference_episode_count(),
                merge.unexpected_detection_episode_count()
            ),
            (2, 1, 0, 1)
        );
        assert_eq!((merge.recall(), merge.precision()), (Some(1.0), Some(0.5)));
        assert_eq!(merge.f1_score(), Some(2.0 / 3.0));

        let complex = compare(vec![
            closed("t1", 0, 4, "reference", "complex"),
            closed("t2", 6, 10, "reference", "complex"),
            closed("a1", 0, 6, "detection", "complex"),
            closed("a2", 7, 10, "detection", "complex"),
        ])
        .as_reference();
        assert_eq!(
            (
                complex.detected_reference_episode_count(),
                complex.matched_detection_episode_count(),
                complex.missed_reference_episode_count(),
                complex.unexpected_detection_episode_count()
            ),
            (2, 2, 0, 0)
        );
    }
}
