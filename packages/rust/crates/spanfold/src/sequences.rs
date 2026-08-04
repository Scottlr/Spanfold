//! Ordered cross-window sequence matching.

use std::{cmp::Ordering, collections::BTreeSet};

use serde::Serialize;
use thiserror::Error;

use crate::{
    ComparisonFinality, TemporalAxis, TemporalPoint, TemporalRangeError, WindowHistory,
    WindowRecord, WindowRecordId, WindowSnapshotRecord,
};

/// Error returned when defining or evaluating an ordered sequence.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum WindowSequenceError {
    /// The sequence name was blank.
    #[error("sequence name cannot be empty")]
    EmptyName,
    /// A step named a blank window family.
    #[error("sequence step window family cannot be empty")]
    EmptyStep,
    /// A sequence requires at least two steps.
    #[error("sequence requires at least two steps")]
    TooFewSteps,
    /// The maximum transition gap cannot be negative.
    #[error("sequence maximum gap cannot be negative")]
    NegativeMaximumGap,
    /// Ordered sequences currently support only processing-position evidence.
    #[error("ordered sequences support only processing-position evidence")]
    UnsupportedTemporalAxis,
    /// Historical matching selected an open record.
    #[error("historical sequence matching cannot use open record '{record_id}'")]
    OpenEvidence {
        /// Selected open record identifier.
        record_id: WindowRecordId,
    },
    /// A live snapshot could not be materialized.
    #[error(transparent)]
    Temporal(#[from] TemporalRangeError),
}

/// One completed ordered sequence with its contributing window evidence.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct WindowSequenceMatch {
    name: String,
    key: String,
    source: Option<String>,
    partition: Option<String>,
    evidence: Vec<WindowSnapshotRecord>,
    start: TemporalPoint,
    end: TemporalPoint,
    end_to_end_magnitude: i64,
    total_gap: i64,
    finality: ComparisonFinality,
}

impl WindowSequenceMatch {
    /// Returns the configured sequence name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the exact logical key shared by all evidence.
    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }

    /// Returns the exact source shared by all evidence.
    #[must_use]
    pub fn source(&self) -> Option<&str> {
        self.source.as_deref()
    }

    /// Returns the exact partition shared by all evidence.
    #[must_use]
    pub fn partition(&self) -> Option<&str> {
        self.partition.as_deref()
    }

    /// Returns the ordered window evidence for every sequence step.
    #[must_use]
    pub fn evidence(&self) -> &[WindowSnapshotRecord] {
        &self.evidence
    }

    /// Returns the first evidence start.
    #[must_use]
    pub fn start(&self) -> &TemporalPoint {
        &self.start
    }

    /// Returns the latest effective end across all evidence.
    #[must_use]
    pub fn end(&self) -> &TemporalPoint {
        &self.end
    }

    /// Returns the distance from the first start to the final effective end.
    #[must_use]
    pub const fn end_to_end_magnitude(&self) -> i64 {
        self.end_to_end_magnitude
    }

    /// Returns the sum of inactive gaps between consecutive steps.
    #[must_use]
    pub const fn total_gap(&self) -> i64 {
        self.total_gap
    }

    /// Returns finality derived from all contributing evidence.
    #[must_use]
    pub const fn finality(&self) -> &ComparisonFinality {
        &self.finality
    }
}

/// Fluent builder for one ordered sequence over a window history.
#[derive(Clone, Debug)]
pub struct WindowSequenceBuilder<'a> {
    history: &'a WindowHistory,
    name: String,
    steps: Vec<String>,
    maximum_gap: Option<i64>,
}

impl<'a> WindowSequenceBuilder<'a> {
    pub(crate) fn new(history: &'a WindowHistory, name: String) -> Self {
        Self {
            history,
            name,
            steps: Vec::new(),
            maximum_gap: None,
        }
    }

    /// Adds the first literal named window-family step.
    #[must_use]
    pub fn step(mut self, window_name: impl Into<String>) -> Self {
        self.steps.push(window_name.into());
        self
    }

    /// Adds the next literal named window-family step.
    #[must_use]
    pub fn then(mut self, window_name: impl Into<String>) -> Self {
        self.steps.push(window_name.into());
        self
    }

    /// Sets an inclusive maximum processing-position gap for every transition.
    #[must_use]
    pub const fn with_maximum_gap(mut self, magnitude: i64) -> Self {
        self.maximum_gap = Some(magnitude);
        self
    }

    /// Matches final historical evidence.
    pub fn run(&self) -> Result<Vec<WindowSequenceMatch>, WindowSequenceError> {
        self.validate()?;
        let selected_families = self.selected_families();
        let mut evidence = Vec::new();

        for window in self
            .history
            .windows()
            .into_iter()
            .filter(|window| selected_families.contains(window.window_name()))
        {
            let WindowRecord::Closed(closed) = &window else {
                return Err(WindowSequenceError::OpenEvidence {
                    record_id: window.id().clone(),
                });
            };
            if window.start().axis() != TemporalAxis::ProcessingPosition {
                return Err(WindowSequenceError::UnsupportedTemporalAxis);
            }
            evidence.push(WindowSnapshotRecord {
                range: closed.range.clone(),
                window,
                finality: ComparisonFinality::Final,
            });
        }

        Ok(match_evidence(
            &self.name,
            &self.steps,
            self.maximum_gap,
            evidence,
        ))
    }

    /// Matches evidence visible at an explicit processing-position horizon.
    pub fn run_live(
        &self,
        evaluation_horizon: TemporalPoint,
    ) -> Result<Vec<WindowSequenceMatch>, WindowSequenceError> {
        self.validate()?;
        if evaluation_horizon.axis() != TemporalAxis::ProcessingPosition {
            return Err(WindowSequenceError::UnsupportedTemporalAxis);
        }
        let selected_families = self.selected_families();
        if self
            .history
            .windows()
            .iter()
            .filter(|window| selected_families.contains(window.window_name()))
            .any(|window| window.start().axis() != TemporalAxis::ProcessingPosition)
        {
            return Err(WindowSequenceError::UnsupportedTemporalAxis);
        }
        let evidence = self
            .history
            .snapshot_at(evaluation_horizon)?
            .records
            .into_iter()
            .filter(|record| selected_families.contains(record.window.window_name()))
            .collect();
        Ok(match_evidence(
            &self.name,
            &self.steps,
            self.maximum_gap,
            evidence,
        ))
    }

    fn validate(&self) -> Result<(), WindowSequenceError> {
        if self.name.trim().is_empty() {
            return Err(WindowSequenceError::EmptyName);
        }
        if self.steps.iter().any(|step| step.trim().is_empty()) {
            return Err(WindowSequenceError::EmptyStep);
        }
        if self.steps.len() < 2 {
            return Err(WindowSequenceError::TooFewSteps);
        }
        if self.maximum_gap.is_some_and(|gap| gap < 0) {
            return Err(WindowSequenceError::NegativeMaximumGap);
        }
        Ok(())
    }

    fn selected_families(&self) -> BTreeSet<&str> {
        self.steps.iter().map(String::as_str).collect()
    }
}

impl WindowHistory {
    /// Starts an ordered cross-window sequence matcher over this history.
    #[must_use]
    pub fn match_sequence(&self, name: impl Into<String>) -> WindowSequenceBuilder<'_> {
        WindowSequenceBuilder::new(self, name.into())
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct SequenceLane {
    key: String,
    source: Option<String>,
    partition: Option<String>,
}

impl SequenceLane {
    fn from_record(record: &WindowSnapshotRecord) -> Self {
        Self {
            key: record.window.key().to_owned(),
            source: record.window.source().map(str::to_owned),
            partition: record.window.partition().map(str::to_owned),
        }
    }
}

fn match_evidence(
    name: &str,
    steps: &[String],
    maximum_gap: Option<i64>,
    evidence: Vec<WindowSnapshotRecord>,
) -> Vec<WindowSequenceMatch> {
    let mut lanes = evidence
        .iter()
        .map(SequenceLane::from_record)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    lanes.sort();

    let mut matches = Vec::new();
    for lane in lanes {
        let mut candidates = steps
            .iter()
            .map(|step| {
                let mut records = evidence
                    .iter()
                    .filter(|record| record.window.window_name() == step)
                    .filter(|record| SequenceLane::from_record(record) == lane)
                    .collect::<Vec<_>>();
                records.sort_by(|left, right| compare_candidates(left, right));
                records
            })
            .collect::<Vec<_>>();
        let mut used = BTreeSet::new();

        for first in candidates.remove(0) {
            if used.contains(first.window.id()) {
                continue;
            }
            let mut selected = vec![first];
            let mut selected_ids = BTreeSet::from([first.window.id().clone()]);

            for step_candidates in &candidates {
                let previous = selected.last().expect("first sequence step");
                let next = step_candidates.iter().copied().find(|candidate| {
                    !used.contains(candidate.window.id())
                        && !selected_ids.contains(candidate.window.id())
                        && transition_is_compatible(previous, candidate, maximum_gap)
                });
                let Some(next) = next else {
                    break;
                };
                selected_ids.insert(next.window.id().clone());
                selected.push(next);
            }

            if selected.len() == steps.len() {
                used.extend(selected_ids);
                matches.push(materialize_match(name, &lane, &selected));
            }
        }
    }

    matches.sort_by(compare_matches);
    matches
}

fn compare_candidates(left: &WindowSnapshotRecord, right: &WindowSnapshotRecord) -> Ordering {
    left.range
        .end()
        .magnitude()
        .cmp(&right.range.end().magnitude())
        .then_with(|| {
            left.range
                .start()
                .magnitude()
                .cmp(&right.range.start().magnitude())
        })
        .then_with(|| left.window.id().cmp(right.window.id()))
}

fn transition_is_compatible(
    previous: &WindowSnapshotRecord,
    candidate: &WindowSnapshotRecord,
    maximum_gap: Option<i64>,
) -> bool {
    let start = candidate.range.start();
    if start.magnitude() < previous.range.start().magnitude() {
        return false;
    }
    let inactive_gap = (start.magnitude() - previous.range.end().magnitude()).max(0);
    maximum_gap.is_none_or(|maximum| inactive_gap <= maximum)
}

fn materialize_match(
    name: &str,
    lane: &SequenceLane,
    evidence: &[&WindowSnapshotRecord],
) -> WindowSequenceMatch {
    let start = evidence.first().expect("complete sequence").range.start();
    let end = evidence
        .iter()
        .map(|record| record.range.end())
        .max_by_key(TemporalPoint::magnitude)
        .expect("complete sequence");
    let total_gap = evidence
        .windows(2)
        .map(|pair| (pair[1].range.start().magnitude() - pair[0].range.end().magnitude()).max(0))
        .sum();
    let finality = if evidence
        .iter()
        .any(|record| record.finality == ComparisonFinality::Provisional)
    {
        ComparisonFinality::Provisional
    } else {
        ComparisonFinality::Final
    };

    WindowSequenceMatch {
        name: name.to_owned(),
        key: lane.key.clone(),
        source: lane.source.clone(),
        partition: lane.partition.clone(),
        evidence: evidence.iter().map(|record| (*record).clone()).collect(),
        end_to_end_magnitude: end.magnitude() - start.magnitude(),
        start,
        end,
        total_gap,
        finality,
    }
}

fn compare_matches(left: &WindowSequenceMatch, right: &WindowSequenceMatch) -> Ordering {
    left.end
        .magnitude()
        .cmp(&right.end.magnitude())
        .then_with(|| left.start.magnitude().cmp(&right.start.magnitude()))
        .then_with(|| {
            left.evidence
                .iter()
                .map(|record| record.window.id())
                .cmp(right.evidence.iter().map(|record| record.window.id()))
        })
        .then_with(|| left.key.cmp(&right.key))
        .then_with(|| left.source.cmp(&right.source))
        .then_with(|| left.partition.cmp(&right.partition))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ClosedWindow, TemporalRange, WindowHistoryFixture, WindowRecordId};

    fn no_metadata(
        metadata: crate::WindowHistoryFixtureWindow,
    ) -> crate::WindowHistoryFixtureWindow {
        metadata
    }

    #[test]
    fn matches_three_steps_in_one_exact_lane() {
        let history = WindowHistoryFixture::new()
            .closed_window("Detected", "item", 0, 4, |window| {
                window.source("source-a").partition("one")
            })
            .unwrap()
            .closed_window("Reviewed", "item", 3, 5, |window| {
                window.source("source-a").partition("one")
            })
            .unwrap()
            .closed_window("Resolved", "item", 7, 9, |window| {
                window.source("source-a").partition("one")
            })
            .unwrap()
            .closed_window("Reviewed", "item", 3, 4, |window| {
                window.source("source-b").partition("one")
            })
            .unwrap()
            .build();

        let matches = history
            .match_sequence("resolution")
            .step("Detected")
            .then("Reviewed")
            .then("Resolved")
            .run()
            .unwrap();

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].source(), Some("source-a"));
        assert_eq!(matches[0].partition(), Some("one"));
        assert_eq!(matches[0].end_to_end_magnitude(), 9);
        assert_eq!(matches[0].total_gap(), 2);
        assert_eq!(
            matches[0]
                .evidence()
                .iter()
                .map(|record| record.window.window_name())
                .collect::<Vec<_>>(),
            ["Detected", "Reviewed", "Resolved"]
        );
    }

    #[test]
    fn chooses_earliest_completion_and_never_reuses_committed_evidence() {
        let history = WindowHistoryFixture::new()
            .closed_window("A", "item", 0, 1, no_metadata)
            .unwrap()
            .closed_window("A", "item", 1, 2, no_metadata)
            .unwrap()
            .closed_window("B", "item", 2, 5, no_metadata)
            .unwrap()
            .closed_window("B", "item", 3, 4, no_metadata)
            .unwrap()
            .closed_window("C", "item", 5, 6, no_metadata)
            .unwrap()
            .build();

        let matches = history
            .match_sequence("abc")
            .step("A")
            .then("B")
            .then("C")
            .run()
            .unwrap();

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].evidence()[0].window.id().as_str(), "window-0000");
        assert_eq!(matches[0].evidence()[1].window.id().as_str(), "window-0003");
        assert_eq!(matches[0].evidence()[2].window.id().as_str(), "window-0004");
    }

    #[test]
    fn maximum_gap_is_inclusive_for_each_transition() {
        let history = WindowHistoryFixture::new()
            .closed_window("A", "item", 0, 2, no_metadata)
            .unwrap()
            .closed_window("B", "item", 5, 6, no_metadata)
            .unwrap()
            .build();

        assert_eq!(
            history
                .match_sequence("within")
                .step("A")
                .then("B")
                .with_maximum_gap(3)
                .run()
                .unwrap()
                .len(),
            1
        );
        assert!(
            history
                .match_sequence("outside")
                .step("A")
                .then("B")
                .with_maximum_gap(2)
                .run()
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn live_matching_clips_open_evidence_and_preserves_provisional_finality() {
        let history = WindowHistoryFixture::new()
            .closed_window("A", "item", 0, 2, no_metadata)
            .unwrap()
            .open_window("B", "item", 3, no_metadata)
            .unwrap()
            .build();

        let matches = history
            .match_sequence("live")
            .step("A")
            .then("B")
            .run_live(TemporalPoint::position(5))
            .unwrap();

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].end().magnitude(), 5);
        assert_eq!(matches[0].finality(), &ComparisonFinality::Provisional);
        assert_eq!(matches[0].evidence()[1].range.magnitude(), 2);
        assert!(matches!(
            history
                .match_sequence("historical")
                .step("A")
                .then("B")
                .run(),
            Err(WindowSequenceError::OpenEvidence { .. })
        ));
    }

    #[test]
    fn live_matching_rejects_selected_timestamp_evidence_before_snapshot_filtering() {
        let timestamp = ClosedWindow {
            id: WindowRecordId::new("timestamp-a").unwrap(),
            window_name: "A".to_owned(),
            key: "item".to_owned(),
            range: TemporalRange::new(
                TemporalPoint::timestamp_ticks(0),
                TemporalPoint::timestamp_ticks(2),
            )
            .unwrap(),
            known_at: None,
            source: None,
            partition: None,
            segments: Vec::new(),
            tags: Vec::new(),
            boundary_reason: None,
            boundary_changes: Vec::new(),
        };
        let history = WindowHistory::from_records([timestamp], []).unwrap();

        let result = history
            .match_sequence("mixed axis")
            .step("A")
            .then("B")
            .run_live(TemporalPoint::position(5));

        assert_eq!(result, Err(WindowSequenceError::UnsupportedTemporalAxis));
    }
}
