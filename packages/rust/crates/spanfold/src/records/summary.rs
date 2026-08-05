use std::collections::BTreeMap;

use super::model::{
    WindowGroupKind, WindowGroupSummary, WindowRecord, WindowSnapshotFinality, WindowSnapshotRecord,
};
use crate::{PrimitiveValue, TemporalAxis};

/// Summarizes recorded windows by segment.
pub fn summarize_by_segment(
    windows: impl IntoIterator<Item = WindowRecord>,
    name: &str,
) -> Result<Vec<WindowGroupSummary>, SummaryError> {
    summarize_windows(windows, WindowGroupKind::Segment, name)
}

/// Summarizes recorded windows by tag.
pub fn summarize_by_tag(
    windows: impl IntoIterator<Item = WindowRecord>,
    name: &str,
) -> Result<Vec<WindowGroupSummary>, SummaryError> {
    summarize_windows(windows, WindowGroupKind::Tag, name)
}

#[derive(Clone, Debug)]
struct SummaryAccumulator {
    group_kind: WindowGroupKind,
    name: String,
    value: PrimitiveValue,
    record_count: usize,
    final_count: usize,
    provisional_count: usize,
    measured_position_count: usize,
    total_position_length: i64,
}

impl SummaryAccumulator {
    fn new(group_kind: WindowGroupKind, name: &str, value: PrimitiveValue) -> Self {
        Self {
            group_kind,
            name: name.to_owned(),
            value,
            record_count: 0,
            final_count: 0,
            provisional_count: 0,
            measured_position_count: 0,
            total_position_length: 0,
        }
    }

    fn add_window(&mut self, window: &WindowRecord) {
        self.record_count += 1;
        match window {
            WindowRecord::Closed(closed) => {
                self.final_count += 1;
                if closed.range.start().axis() == TemporalAxis::ProcessingPosition {
                    self.measured_position_count += 1;
                    self.total_position_length += closed.range.magnitude();
                }
            }
            WindowRecord::Open(_) => {
                self.provisional_count += 1;
            }
        }
    }

    fn add_snapshot(&mut self, record: &WindowSnapshotRecord) {
        self.record_count += 1;
        match record.finality {
            WindowSnapshotFinality::Final => self.final_count += 1,
            WindowSnapshotFinality::Provisional => self.provisional_count += 1,
        }
        if record.range.start().axis() == TemporalAxis::ProcessingPosition {
            self.measured_position_count += 1;
            self.total_position_length += record.range.magnitude();
        }
    }

    fn into_summary(self) -> WindowGroupSummary {
        WindowGroupSummary {
            group_kind: self.group_kind,
            name: self.name,
            value: self.value,
            record_count: self.record_count,
            final_count: self.final_count,
            provisional_count: self.provisional_count,
            measured_position_count: self.measured_position_count,
            total_position_length: self.total_position_length,
        }
    }
}

fn summarize_windows(
    windows: impl IntoIterator<Item = WindowRecord>,
    group_kind: WindowGroupKind,
    name: &str,
) -> Result<Vec<WindowGroupSummary>, SummaryError> {
    validate_summary_name(name)?;
    let mut groups = BTreeMap::<String, SummaryAccumulator>::new();
    for window in windows {
        for value in metadata_values(&window, group_kind, name) {
            groups
                .entry(primitive_sort_key(&value))
                .or_insert_with(|| SummaryAccumulator::new(group_kind, name, value.clone()))
                .add_window(&window);
        }
    }
    Ok(groups
        .into_values()
        .map(SummaryAccumulator::into_summary)
        .collect())
}

pub(crate) fn summarize_snapshot_records(
    records: &[WindowSnapshotRecord],
    group_kind: WindowGroupKind,
    name: &str,
) -> Result<Vec<WindowGroupSummary>, SummaryError> {
    validate_summary_name(name)?;
    let mut groups = BTreeMap::<String, SummaryAccumulator>::new();
    for record in records {
        for value in metadata_values(&record.window, group_kind, name) {
            groups
                .entry(primitive_sort_key(&value))
                .or_insert_with(|| SummaryAccumulator::new(group_kind, name, value.clone()))
                .add_snapshot(record);
        }
    }
    Ok(groups
        .into_values()
        .map(SummaryAccumulator::into_summary)
        .collect())
}

/// Error returned when a summary dimension is invalid.
#[derive(Clone, Copy, Debug, Eq, thiserror::Error, PartialEq)]
pub enum SummaryError {
    /// A segment or tag name must contain non-whitespace text.
    #[error("summary dimension name cannot be empty")]
    EmptyName,
}

fn validate_summary_name(name: &str) -> Result<(), SummaryError> {
    if name.trim().is_empty() {
        return Err(SummaryError::EmptyName);
    }
    Ok(())
}

fn metadata_values(
    window: &WindowRecord,
    group_kind: WindowGroupKind,
    name: &str,
) -> Vec<PrimitiveValue> {
    let mut values = BTreeMap::<String, PrimitiveValue>::new();
    match group_kind {
        WindowGroupKind::Segment => {
            for segment in window.segments() {
                if segment.name == name {
                    let value = segment.value.clone();
                    values.entry(primitive_sort_key(&value)).or_insert(value);
                }
            }
        }
        WindowGroupKind::Tag => {
            for tag in window.tags() {
                if tag.name == name {
                    let value = tag.value.clone();
                    values.entry(primitive_sort_key(&value)).or_insert(value);
                }
            }
        }
    }
    values.into_values().collect()
}

pub(crate) fn primitive_sort_key(value: &PrimitiveValue) -> String {
    match value {
        PrimitiveValue::String(value) => format!("string:{value}"),
        PrimitiveValue::Integer(value) => format!("integer:{value:020}"),
        PrimitiveValue::Float(value) => format!("float:{:?}", value.as_f64()),
        PrimitiveValue::Bool(value) => format!("bool:{value}"),
        PrimitiveValue::Null => "null:".to_owned(),
    }
}
