//! Window-history query and serialization tests.

use super::*;
use proptest::prelude::*;

#[test]
fn public_metadata_constructors_reject_blank_names() {
    assert_eq!(
        WindowRecordId::new(" "),
        Err(WindowMetadataError::EmptyRecordId)
    );
    assert_eq!(
        WindowSegment::new("", "value"),
        Err(WindowMetadataError::EmptySegmentName)
    );
    assert_eq!(
        WindowTag::new("\t", "value"),
        Err(WindowMetadataError::EmptyTagName)
    );
    assert_eq!(
        WindowSegment::new("child", "value")
            .expect("segment")
            .with_parent(" "),
        Err(WindowMetadataError::EmptyParentSegmentName)
    );

    let fixture_error = WindowHistoryFixture::new()
        .open_window("DeviceOffline", "device-1", 1, |window| {
            window.segment("", "invalid")
        })
        .expect_err("blank fixture metadata");
    assert_eq!(
        fixture_error,
        WindowHistoryFixtureError::Metadata(WindowMetadataError::EmptySegmentName)
    );
}

#[test]
fn fixture_builder_creates_closed_windows_with_metadata() {
    let history = WindowHistoryFixture::new()
        .closed_window("DeviceOffline", "device-1", 1, 5, |w| {
            w.source("provider-a")
                .partition("fleet-a")
                .segment("lifecycle", "Incident")
                .child_segment("stage", "Escalated", "lifecycle")
                .tag("fleet", "critical")
        })
        .expect("valid fixture window")
        .build();

    let window = &history.closed_windows()[0];
    assert_eq!(window.id.as_str(), "window-0000");
    assert_eq!(window.source.as_deref(), Some("provider-a"));
    assert_eq!(window.partition.as_deref(), Some("fleet-a"));
    assert_eq!(window.segments.len(), 2);
    assert_eq!(window.tags.len(), 1);
    assert_eq!(window.range.magnitude(), 4);
}

#[test]
fn fixture_builder_creates_open_windows() {
    let history = WindowHistoryFixture::new()
        .open_window("DeviceOffline", "device-1", 10, |w| w.source("provider-a"))
        .expect("open provider-a")
        .build();

    assert_eq!(history.open_windows().len(), 1);
    assert_eq!(history.open_windows()[0].start, TemporalPoint::position(10));
}

#[test]
fn direct_history_query_filters_and_aliases() {
    let history = segmented_history();

    let rows = history
        .query()
        .where_window("DeviceOffline")
        .where_key("device-1")
        .where_source("provider-a")
        .where_partition("p1")
        .where_segment("lifecycle", "Incident")
        .where_tag("fleet", "warehouse")
        .closed()
        .windows();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].source(), Some("provider-a"));
    assert_eq!(
        rows[0].segments()[0].value,
        PrimitiveValue::from("Incident")
    );

    let latest = history
        .query()
        .where_window("DeviceOffline")
        .where_source("provider-a")
        .latest()
        .expect("latest window");
    assert_eq!(latest.key(), "device-2");
}

#[test]
fn snapshot_records_include_final_and_provisional_ranges() {
    let history = segmented_history();
    let snapshot = history
        .snapshot_at(TemporalPoint::position(6))
        .expect("snapshot");
    let rows = snapshot
        .query()
        .where_window("DeviceOffline")
        .where_source("provider-a")
        .windows();

    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].finality, ComparisonFinality::Final);
    assert_eq!(rows[1].finality, ComparisonFinality::Provisional);
    assert_eq!(rows[1].range.magnitude(), 3);
}

#[test]
fn summaries_group_recorded_and_snapshot_windows() {
    let history = segmented_history();

    let summaries = history
        .query()
        .where_window("DeviceOffline")
        .summarize_by_segment("lifecycle")
        .expect("segment summaries");
    let incident = summaries
        .iter()
        .find(|summary| summary.value == PrimitiveValue::from("Incident"))
        .expect("incident summary");
    assert_eq!(incident.group_kind, WindowGroupKind::Segment);
    assert_eq!(incident.record_count, 2);
    assert_eq!(incident.final_count, 1);
    assert_eq!(incident.provisional_count, 1);
    assert_eq!(incident.measured_position_count, 1);
    assert_eq!(incident.total_position_length, 1);

    let snapshot_summaries = history
        .snapshot_at(TemporalPoint::position(6))
        .expect("snapshot")
        .query()
        .where_window("DeviceOffline")
        .summarize_by_segment("lifecycle")
        .expect("snapshot summaries");
    let snapshot_incident = snapshot_summaries
        .iter()
        .find(|summary| summary.value == PrimitiveValue::from("Incident"))
        .expect("snapshot incident summary");
    assert_eq!(snapshot_incident.measured_position_count, 2);
    assert_eq!(snapshot_incident.total_position_length, 4);

    assert!(history.query().summarize_by_segment("").is_err());
}

#[test]
fn direct_overlap_and_residual_helpers_match_query_surface() {
    let history = WindowHistoryFixture::new()
        .closed_window("SelectionSuspension", "selection-1", 1, 5, |w| {
            w.source("provider-a")
        })
        .expect("target")
        .closed_window("SelectionSuspension", "selection-1", 3, 6, |w| {
            w.source("provider-b")
        })
        .expect("against")
        .build();

    let overlap = history.find_overlaps().remove(0);
    assert_eq!(overlap.first.source.as_deref(), Some("provider-a"));
    assert_eq!(overlap.second.source.as_deref(), Some("provider-b"));

    let residual = history.find_residuals("provider-a").remove(0);
    assert_eq!(residual.start_position, 1);
    assert_eq!(residual.end_position, 3);
}

#[test]
fn annotations_append_revisions_and_filter_by_known_at() {
    let mut history = WindowHistoryFixture::new()
        .open_window("DeviceOffline", "device-1", 1, |w| w.source("lane-a"))
        .expect("open lane-a")
        .build();
    let open = history.query().open_windows()[0].clone();
    let target = WindowAnnotationTarget::from_open(&open);

    let first = history.annotate(target.clone(), "classification", "initial", None);
    let second = history.annotate(
        target.clone(),
        "classification",
        "revised",
        Some(TemporalPoint::position(5)),
    );
    history.annotate(
        target.clone(),
        "classification",
        "future",
        Some(TemporalPoint::position(8)),
    );
    history.annotate(
        target.clone(),
        "timestamp-note",
        "different-axis",
        Some(TemporalPoint::timestamp_ticks(10)),
    );

    assert_eq!(first.revision, 1);
    assert_eq!(second.revision, 2);
    assert_eq!(history.annotations_for(&target).len(), 4);

    let known = history.annotations_known_at(&target, TemporalPoint::position(6));
    assert_eq!(known, vec![second]);
}

#[test]
fn serde_rejects_empty_record_identity_and_metadata_names() {
    assert!(serde_json::from_str::<WindowRecordId>(r#"\" \""#).is_err());
    assert!(
        serde_json::from_str::<WindowSegment>(r#"{"name":"","value":"x","parent_name":null}"#)
            .is_err()
    );
    assert!(serde_json::from_str::<WindowTag>(r#"{"name":" ","value":"x"}"#).is_err());
    assert!(serde_json::from_str::<ClosedWindow>(
        r#"{"id":"id","window_name":"","key":"key","range":{"start":{"axis":"ProcessingPosition","magnitude":1},"end":{"axis":"ProcessingPosition","magnitude":2}},"known_at":null,"source":null,"partition":null,"segments":[],"tags":[],"boundary_reason":null,"boundary_changes":[]}"#
    )
    .is_err());
}

proptest! {
    #[test]
    fn borrowed_query_matches_sorted_history_order(
        first_start in 0_i64..100,
        first_length in 1_i64..20,
        second_start in 0_i64..100,
        second_length in 1_i64..20,
    ) {
        let mut builder = WindowHistoryFixture::new();
        builder = builder
            .closed_window("Window", "b", first_start, first_start + first_length, |w| w)
            .expect("generated first range");
        builder = builder
            .closed_window("Window", "a", second_start, second_start + second_length, |w| w)
            .expect("generated second range");
        let history = builder.build();
        let borrowed = history.query().windows();
        prop_assert_eq!(borrowed, history.windows());
    }
}

fn segmented_history() -> WindowHistory {
    WindowHistoryFixture::new()
        .closed_window("DeviceOffline", "device-1", 1, 2, |w| {
            w.source("provider-a")
                .partition("p1")
                .segment("lifecycle", "Incident")
                .tag("fleet", "warehouse")
        })
        .expect("closed provider-a")
        .open_window("DeviceOffline", "device-2", 3, |w| {
            w.source("provider-a")
                .partition("p1")
                .segment("lifecycle", "Incident")
                .tag("fleet", "warehouse")
        })
        .expect("open provider-a")
        .closed_window("DeviceOffline", "device-3", 4, 5, |w| {
            w.source("provider-b")
                .partition("p1")
                .segment("lifecycle", "Normal")
                .tag("fleet", "warehouse")
        })
        .expect("closed provider-b")
        .build()
}
