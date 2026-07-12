//! Pipeline behavior and atomicity tests.

#![allow(unused_must_use)]

use super::*;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct PriceTick {
    selection_id: &'static str,
    market_id: &'static str,
    fixture_id: &'static str,
    price: f64,
    observed_at: i64,
}

#[test]
fn track_window_records_closed_history() {
    let mut pipeline = for_events::<PriceTick>()
        .record_windows()
        .track_window(
            "SelectionSuspension",
            |tick| tick.selection_id,
            |tick| tick.price == 0.0,
        )
        .build_or_panic();

    let first = pipeline
        .ingest(
            PriceTick {
                selection_id: "selection-1",
                market_id: "market-1",
                fixture_id: "fixture-1",
                price: 0.0,
                observed_at: 100,
            },
            Some("provider-a"),
            None,
        )
        .expect("first ingest");
    let second = pipeline
        .ingest(
            PriceTick {
                selection_id: "selection-1",
                market_id: "market-1",
                fixture_id: "fixture-1",
                price: 1.2,
                observed_at: 130,
            },
            Some("provider-a"),
            None,
        )
        .expect("second ingest");

    assert_eq!(first.emissions[0].kind, WindowTransitionKind::Opened);
    assert_eq!(second.emissions[0].kind, WindowTransitionKind::Closed);
    assert!(first.has_emissions());
    assert_eq!(pipeline.history().closed_windows().len(), 1);
    assert_eq!(
        pipeline.history().closed_windows()[0].window_name,
        "SelectionSuspension"
    );
    assert_eq!(pipeline.metadata().windows[0].name, "SelectionSuspension");
}

#[test]
fn nested_rollups_record_parent_windows() {
    let mut pipeline = for_events::<PriceTick>()
        .record_windows()
        .window(
            "SelectionSuspension",
            |tick| tick.selection_id,
            |tick| tick.price == 0.0,
        )
        .roll_up(
            "MarketSuspension",
            |tick| tick.market_id,
            |children| children.any_active(),
        )
        .roll_up(
            "FixtureSuspension",
            |tick| tick.fixture_id,
            |children| children.any_active(),
        )
        .build_or_panic();

    pipeline.ingest(
        PriceTick {
            selection_id: "selection-1",
            market_id: "market-1",
            fixture_id: "fixture-1",
            price: 0.0,
            observed_at: 100,
        },
        None,
        None,
    );
    pipeline.ingest(
        PriceTick {
            selection_id: "selection-1",
            market_id: "market-1",
            fixture_id: "fixture-1",
            price: 1.1,
            observed_at: 130,
        },
        None,
        None,
    );

    let history = pipeline.history();
    assert_eq!(history.closed_windows().len(), 3);
    let hierarchy = history.compare_hierarchy(
        "Market explanation",
        "MarketSuspension",
        "SelectionSuspension",
    );
    assert_eq!(hierarchy.rows.len(), 1);
    assert_eq!(
        hierarchy.rows[0].kind,
        crate::HierarchyComparisonRowKind::ParentExplained
    );
    let metadata = pipeline.metadata();
    assert_eq!(metadata.windows[0].rollups[0].name, "MarketSuspension");
    assert_eq!(
        metadata.windows[0].rollups[0].rollups[0].name,
        "FixtureSuspension"
    );
}

#[test]
fn active_rollup_child_migrates_between_parent_keys() {
    let mut pipeline = for_events::<PriceTick>()
        .record_windows()
        .window(
            "SelectionPriced",
            |tick| tick.selection_id,
            |tick| tick.price > 0.0,
        )
        .roll_up(
            "MarketPriced",
            |tick| tick.market_id,
            |children| children.any_active(),
        )
        .build_or_panic();

    pipeline
        .ingest(
            PriceTick {
                selection_id: "selection-1",
                market_id: "market-1",
                fixture_id: "fixture-1",
                price: 1.01,
                observed_at: 100,
            },
            None,
            None,
        )
        .expect("initial parent");
    pipeline
        .ingest(
            PriceTick {
                selection_id: "selection-1",
                market_id: "market-2",
                fixture_id: "fixture-1",
                price: 1.02,
                observed_at: 101,
            },
            None,
            None,
        )
        .expect("migrated parent");

    let closed_parent = pipeline
        .history()
        .closed_windows()
        .iter()
        .find(|window| window.window_name == "MarketPriced")
        .expect("old parent closed");
    let open_parents = pipeline
        .history()
        .open_windows()
        .iter()
        .filter(|window| window.window_name == "MarketPriced")
        .collect::<Vec<_>>();

    assert_eq!(closed_parent.key, "market-1");
    assert_eq!(open_parents.len(), 1);
    assert_eq!(open_parents[0].key, "market-2");
}

#[test]
fn active_rollup_child_migrates_parent_and_segment_lane_together() {
    let mut pipeline = for_events::<PriceTick>()
        .record_windows()
        .window_with_metadata(
            "SelectionPriced",
            |tick| tick.selection_id,
            |tick| tick.price > 0.0,
            |tick| vec![WindowSegment::new("fixture", tick.fixture_id).expect("segment name")],
            |_| Vec::new(),
        )
        .roll_up(
            "MarketPriced",
            |tick| tick.market_id,
            |children| children.any_active(),
        )
        .build_or_panic();

    for tick in [
        PriceTick {
            selection_id: "selection-1",
            market_id: "market-1",
            fixture_id: "fixture-1",
            price: 1.01,
            observed_at: 100,
        },
        PriceTick {
            selection_id: "selection-1",
            market_id: "market-2",
            fixture_id: "fixture-2",
            price: 1.02,
            observed_at: 101,
        },
    ] {
        pipeline.ingest(tick, None, None).expect("ingest");
    }

    let closed_parent = pipeline
        .history()
        .closed_windows()
        .iter()
        .find(|window| window.window_name == "MarketPriced")
        .expect("old parent closed");
    let open_parent = pipeline
        .history()
        .open_windows()
        .iter()
        .find(|window| window.window_name == "MarketPriced")
        .expect("new parent open");

    assert_eq!(closed_parent.key, "market-1");
    assert_eq!(
        closed_parent.segments[0].value(),
        &crate::PrimitiveValue::from("fixture-1")
    );
    assert_eq!(open_parent.key, "market-2");
    assert_eq!(
        open_parent.segments[0].value(),
        &crate::PrimitiveValue::from("fixture-2")
    );
}

#[test]
fn nested_rollup_counts_same_key_children_from_distinct_segment_lanes() {
    let mut pipeline = for_events::<PriceTick>()
        .record_windows()
        .window_with_metadata(
            "SelectionPriced",
            |tick| tick.selection_id,
            |tick| tick.price > 0.0,
            |tick| vec![WindowSegment::new("fixture", tick.fixture_id).expect("segment name")],
            |_| Vec::new(),
        )
        .roll_up(
            "MarketPriced",
            |tick| tick.market_id,
            |children| children.any_active(),
        )
        .roll_up_with_segment_projection(
            "PortfolioPriced",
            |_| "portfolio-1",
            |children| children.any_active(),
            |projection| projection.drop("fixture"),
        )
        .build_or_panic();

    for tick in [
        PriceTick {
            selection_id: "selection-1",
            market_id: "market-1",
            fixture_id: "fixture-1",
            price: 1.01,
            observed_at: 100,
        },
        PriceTick {
            selection_id: "selection-2",
            market_id: "market-1",
            fixture_id: "fixture-2",
            price: 1.02,
            observed_at: 101,
        },
        PriceTick {
            selection_id: "selection-1",
            market_id: "market-1",
            fixture_id: "fixture-1",
            price: 0.0,
            observed_at: 102,
        },
    ] {
        pipeline.ingest(tick, None, None).expect("ingest");
    }

    assert!(
        pipeline
            .history()
            .closed_windows()
            .iter()
            .all(|window| window.window_name != "PortfolioPriced")
    );
    assert_eq!(
        pipeline
            .history()
            .open_windows()
            .iter()
            .filter(|window| window.window_name == "PortfolioPriced")
            .count(),
        1
    );
}

#[test]
fn event_time_selector_records_timestamp_axis_windows() {
    let mut pipeline = for_events::<PriceTick>()
        .record_windows()
        .with_event_time(|tick| tick.observed_at)
        .track_window(
            "SelectionSuspension",
            |tick| tick.selection_id,
            |tick| tick.price == 0.0,
        )
        .build_or_panic();

    pipeline.ingest(
        PriceTick {
            selection_id: "selection-1",
            market_id: "market-1",
            fixture_id: "fixture-1",
            price: 0.0,
            observed_at: 1_000,
        },
        None,
        None,
    );
    pipeline.ingest(
        PriceTick {
            selection_id: "selection-1",
            market_id: "market-1",
            fixture_id: "fixture-1",
            price: 1.1,
            observed_at: 1_250,
        },
        None,
        None,
    );

    let window = &pipeline.history().closed_windows()[0];
    assert_eq!(window.range.start(), TemporalPoint::timestamp_ticks(1_000));
    assert_eq!(window.range.end(), TemporalPoint::timestamp_ticks(1_250));
}

#[test]
fn ingest_many_aggregates_emissions() {
    let mut pipeline = for_events::<PriceTick>()
        .record_windows()
        .track_window(
            "SelectionSuspension",
            |tick| tick.selection_id,
            |tick| tick.price == 0.0,
        )
        .build_or_panic();

    let result = pipeline
        .ingest_many(
            [
                PriceTick {
                    selection_id: "selection-1",
                    market_id: "market-1",
                    fixture_id: "fixture-1",
                    price: 0.0,
                    observed_at: 100,
                },
                PriceTick {
                    selection_id: "selection-1",
                    market_id: "market-1",
                    fixture_id: "fixture-1",
                    price: 1.1,
                    observed_at: 101,
                },
            ],
            Some("provider-a"),
            None,
        )
        .expect("ingest many");

    assert_eq!(result.processing_position, 2);
    assert_eq!(
        result
            .emissions
            .iter()
            .map(|emission| emission.kind)
            .collect::<Vec<_>>(),
        vec![WindowTransitionKind::Opened, WindowTransitionKind::Closed]
    );
}

#[test]
fn try_build_rejects_empty_and_duplicate_window_names() {
    let empty = for_events::<PriceTick>()
        .record_windows()
        .track_window("", |tick| tick.selection_id, |tick| tick.price == 0.0)
        .try_build();

    assert!(matches!(
        empty,
        Err(EventPipelineBuildError::EmptyWindowName)
    ));

    let duplicate = for_events::<PriceTick>()
        .record_windows()
        .window(
            "SelectionSuspension",
            |tick| tick.selection_id,
            |tick| tick.price == 0.0,
        )
        .roll_up(
            "SelectionSuspension",
            |tick| tick.market_id,
            |children| children.any_active(),
        )
        .try_build();

    assert!(matches!(
        duplicate,
        Err(EventPipelineBuildError::DuplicateWindowName(name)) if name == "SelectionSuspension"
    ));
}

#[test]
fn segment_change_closes_and_reopens_active_window() {
    let mut pipeline = for_events::<PriceTick>()
        .record_windows()
        .track_window_with_metadata(
            "SelectionSuspension",
            |tick| tick.selection_id,
            |tick| tick.price == 0.0,
            |tick| vec![WindowSegment::new("market", tick.market_id).expect("segment name")],
            |tick| vec![WindowTag::new("fixture", tick.fixture_id).expect("tag name")],
        )
        .build_or_panic();

    let first = pipeline
        .ingest(
            PriceTick {
                selection_id: "selection-1",
                market_id: "market-1",
                fixture_id: "fixture-1",
                price: 0.0,
                observed_at: 100,
            },
            Some("provider-a"),
            None,
        )
        .expect("first ingest");
    let second = pipeline
        .ingest(
            PriceTick {
                selection_id: "selection-1",
                market_id: "market-2",
                fixture_id: "fixture-1",
                price: 0.0,
                observed_at: 101,
            },
            Some("provider-a"),
            None,
        )
        .expect("second ingest");

    assert_eq!(first.emissions.len(), 1);
    assert_eq!(
        second
            .emissions
            .iter()
            .map(|emission| emission.kind)
            .collect::<Vec<_>>(),
        vec![WindowTransitionKind::Closed, WindowTransitionKind::Opened]
    );
    assert_eq!(pipeline.history().closed_windows().len(), 1);
    assert_eq!(pipeline.history().open_windows().len(), 1);
    assert_eq!(
        pipeline.history().closed_windows()[0].segments[0].value,
        crate::PrimitiveValue::from("market-1")
    );
    assert_eq!(
        pipeline.history().open_windows()[0].segments[0].value,
        crate::PrimitiveValue::from("market-2")
    );
    assert_eq!(pipeline.history().open_windows()[0].tags.len(), 1);
    assert_eq!(
        pipeline.history().closed_windows()[0].boundary_reason,
        Some(WindowBoundaryReason::SegmentChanged)
    );
    assert_eq!(
        pipeline.history().closed_windows()[0].boundary_changes[0].segment_name,
        "market"
    );
    assert_eq!(
        pipeline.history().closed_windows()[0].boundary_changes[0].previous_value,
        Some(crate::PrimitiveValue::from("market-1"))
    );
    assert_eq!(
        pipeline.history().closed_windows()[0].boundary_changes[0].current_value,
        Some(crate::PrimitiveValue::from("market-2"))
    );
    assert_eq!(
        second.emissions[0].boundary_reason,
        Some(WindowBoundaryReason::SegmentChanged)
    );
    assert_eq!(second.emissions[0].segments.len(), 1);
}

#[test]
fn rollups_preserve_child_segment_context_and_reopen_on_segment_change() {
    let mut pipeline = for_events::<PriceTick>()
        .record_windows()
        .window_with_metadata(
            "SelectionPriced",
            |tick| tick.selection_id,
            |tick| tick.price > 0.0,
            |tick| vec![WindowSegment::new("phase", tick.market_id).expect("segment name")],
            |_| Vec::new(),
        )
        .roll_up(
            "FixturePriced",
            |tick| tick.fixture_id,
            |children| children.any_active(),
        )
        .build_or_panic();

    pipeline.ingest(
        PriceTick {
            selection_id: "selection-1",
            market_id: "Pregame",
            fixture_id: "fixture-1",
            price: 1.01,
            observed_at: 100,
        },
        None,
        None,
    );
    pipeline.ingest(
        PriceTick {
            selection_id: "selection-1",
            market_id: "InPlay",
            fixture_id: "fixture-1",
            price: 1.01,
            observed_at: 101,
        },
        None,
        None,
    );

    let closed_rollup = pipeline
        .history()
        .closed_windows()
        .iter()
        .find(|window| window.window_name == "FixturePriced")
        .expect("closed roll-up");
    let open_rollup = pipeline
        .history()
        .open_windows()
        .iter()
        .find(|window| window.window_name == "FixturePriced")
        .expect("open roll-up");

    assert_eq!(
        closed_rollup.segments[0].value,
        crate::PrimitiveValue::from("Pregame")
    );
    assert_eq!(
        open_rollup.segments[0].value,
        crate::PrimitiveValue::from("InPlay")
    );
}

#[test]
fn rollup_segment_projection_can_drop_rename_and_transform() {
    let mut pipeline = for_events::<PriceTick>()
        .record_windows()
        .window_with_metadata(
            "SelectionPriced",
            |tick| tick.selection_id,
            |tick| tick.price > 0.0,
            |tick| {
                vec![
                    WindowSegment::new("phase", tick.market_id).expect("segment name"),
                    WindowSegment::new("state", tick.fixture_id)
                        .and_then(|segment| segment.with_parent("phase"))
                        .expect("segment names"),
                ]
            },
            |_| Vec::new(),
        )
        .roll_up_with_segment_projection(
            "MarketPriced",
            |tick| tick.fixture_id,
            |children| children.any_active(),
            |projection| {
                projection
                    .preserve("phase")
                    .rename("phase", "lifecycle")
                    .transform("phase", |value| match value {
                        crate::PrimitiveValue::String(value) => {
                            crate::PrimitiveValue::from(value.to_uppercase())
                        }
                        other => other.clone(),
                    })
            },
        )
        .build_or_panic();

    pipeline.ingest(
        PriceTick {
            selection_id: "selection-1",
            market_id: "in-play",
            fixture_id: "fixture-1",
            price: 1.01,
            observed_at: 100,
        },
        None,
        None,
    );

    let open_rollup = pipeline
        .history()
        .open_windows()
        .iter()
        .find(|window| window.window_name == "MarketPriced")
        .expect("open roll-up");

    assert_eq!(open_rollup.segments.len(), 1);
    assert_eq!(open_rollup.segments[0].name, "lifecycle");
    assert_eq!(
        open_rollup.segments[0].value,
        crate::PrimitiveValue::from("IN-PLAY")
    );
    assert_eq!(open_rollup.segments[0].parent_name, None);
}

#[test]
fn rollup_rejects_duplicate_projected_segment_names() {
    let mut pipeline = for_events::<PriceTick>()
        .record_windows()
        .window_with_metadata(
            "SelectionPriced",
            |tick| tick.selection_id,
            |tick| tick.price > 0.0,
            |tick| {
                vec![
                    WindowSegment::new("phase", tick.market_id).expect("segment name"),
                    WindowSegment::new("state", tick.fixture_id).expect("segment name"),
                ]
            },
            |_| Vec::new(),
        )
        .roll_up_with_segment_projection(
            "MarketPriced",
            |tick| tick.fixture_id,
            |children| children.any_active(),
            |projection| projection.rename("state", "phase"),
        )
        .build_or_panic();

    let error = pipeline
        .ingest(
            PriceTick {
                selection_id: "selection-1",
                market_id: "InPlay",
                fixture_id: "Suspended",
                price: 1.01,
                observed_at: 100,
            },
            None,
            None,
        )
        .expect_err("duplicate projected names must be rejected");
    assert!(matches!(error, IngestionError::InvalidSegmentProjection(_)));
}

#[test]
fn projection_preflight_keeps_multi_definition_ingestion_atomic() {
    let mut pipeline = for_events::<PriceTick>()
        .record_windows()
        .track_window("First", |tick| tick.selection_id, |tick| tick.price > 0.0)
        .window_with_metadata(
            "Second",
            |tick| tick.selection_id,
            |tick| tick.price > 0.0,
            |tick| {
                vec![
                    WindowSegment::new("phase", tick.market_id).expect("segment name"),
                    WindowSegment::new("state", tick.fixture_id).expect("segment name"),
                ]
            },
            |_| Vec::new(),
        )
        .roll_up_with_segment_projection(
            "SecondRollup",
            |tick| tick.fixture_id,
            |children| children.any_active(),
            |projection| projection.rename("state", "phase"),
        )
        .build()
        .expect("pipeline configuration is structurally valid");

    let error = pipeline
        .ingest(
            PriceTick {
                selection_id: "selection-1",
                market_id: "InPlay",
                fixture_id: "Suspended",
                price: 1.01,
                observed_at: 100,
            },
            None,
            None,
        )
        .expect_err("projection failure must abort the whole event");
    assert!(matches!(error, IngestionError::InvalidSegmentProjection(_)));
    assert_eq!(pipeline.processing_position(), 0);
    assert!(pipeline.history().closed_windows().is_empty());
    assert!(pipeline.history().open_windows().is_empty());
}

#[test]
fn callbacks_run_window_specific_before_global_callbacks() {
    let calls = Arc::new(Mutex::new(Vec::<String>::new()));
    let opened = Arc::clone(&calls);
    let closed = Arc::clone(&calls);
    let global = Arc::clone(&calls);
    let maintenance = Arc::clone(&calls);

    let mut pipeline = for_events::<PriceTick>()
        .record_windows()
        .on_emission(move |emission| {
            global
                .lock()
                .expect("callback lock")
                .push(format!("global:{:?}", emission.kind));
        })
        .track_window_with_options(
            "SelectionSuspension",
            |tick| tick.selection_id,
            |tick| tick.price == 0.0,
            move |options| {
                let opened = Arc::clone(&opened);
                let closed = Arc::clone(&closed);
                options
                    .on_opened(move |emission| {
                        opened
                            .lock()
                            .expect("callback lock")
                            .push(format!("opened:{}", emission.window_name));
                    })
                    .on_closed(move |emission| {
                        closed
                            .lock()
                            .expect("callback lock")
                            .push(format!("closed:{}", emission.window_name));
                    })
            },
        )
        .track_window_with_options(
            "SelectionMaintenance",
            |tick| tick.selection_id,
            |tick| tick.price < 0.0,
            move |options| {
                options.on_opened(move |emission| {
                    maintenance
                        .lock()
                        .expect("callback lock")
                        .push(format!("maintenance:{}", emission.window_name));
                })
            },
        )
        .build_or_panic();

    pipeline.ingest(
        PriceTick {
            selection_id: "selection-1",
            market_id: "market-1",
            fixture_id: "fixture-1",
            price: 0.0,
            observed_at: 100,
        },
        None,
        None,
    );
    pipeline.ingest(
        PriceTick {
            selection_id: "selection-1",
            market_id: "market-1",
            fixture_id: "fixture-1",
            price: 1.1,
            observed_at: 101,
        },
        None,
        None,
    );

    assert_eq!(
        &*calls.lock().expect("callback lock"),
        &[
            "opened:SelectionSuspension",
            "global:Opened",
            "closed:SelectionSuspension",
            "global:Closed",
        ]
    );
}

#[test]
fn active_predicate_close_records_boundary_reason() {
    let mut pipeline = for_events::<PriceTick>()
        .record_windows()
        .track_window(
            "SelectionSuspension",
            |tick| tick.selection_id,
            |tick| tick.price == 0.0,
        )
        .build_or_panic();

    pipeline.ingest(
        PriceTick {
            selection_id: "selection-1",
            market_id: "market-1",
            fixture_id: "fixture-1",
            price: 0.0,
            observed_at: 100,
        },
        None,
        None,
    );
    let result = pipeline
        .ingest(
            PriceTick {
                selection_id: "selection-1",
                market_id: "market-1",
                fixture_id: "fixture-1",
                price: 1.1,
                observed_at: 101,
            },
            None,
            None,
        )
        .expect("ingest");

    assert_eq!(
        pipeline.history().closed_windows()[0].boundary_reason,
        Some(WindowBoundaryReason::ActivePredicateEnded)
    );
    assert!(
        pipeline.history().closed_windows()[0]
            .boundary_changes
            .is_empty()
    );
    assert_eq!(
        result.emissions[0].boundary_reason,
        Some(WindowBoundaryReason::ActivePredicateEnded)
    );
}

#[test]
fn window_recording_is_opt_in_but_emissions_still_fire() {
    let mut pipeline = for_events::<PriceTick>()
        .track_window(
            "SelectionSuspension",
            |tick| tick.selection_id,
            |tick| tick.price == 0.0,
        )
        .build_or_panic();

    let opened = pipeline
        .ingest(
            PriceTick {
                selection_id: "selection-1",
                market_id: "market-1",
                fixture_id: "fixture-1",
                price: 0.0,
                observed_at: 100,
            },
            None,
            None,
        )
        .expect("open ingest");
    let closed = pipeline
        .ingest(
            PriceTick {
                selection_id: "selection-1",
                market_id: "market-1",
                fixture_id: "fixture-1",
                price: 1.1,
                observed_at: 101,
            },
            None,
            None,
        )
        .expect("close ingest");

    assert_eq!(opened.emissions.len(), 1);
    assert_eq!(closed.emissions.len(), 1);
    assert!(pipeline.history().open_windows().is_empty());
    assert!(pipeline.history().closed_windows().is_empty());
}

#[test]
fn ingest_rejects_backwards_event_time_without_mutating_state() {
    let mut pipeline = for_events::<PriceTick>()
        .record_windows()
        .with_event_time(|tick| tick.observed_at)
        .track_window(
            "SelectionSuspension",
            |tick| tick.selection_id,
            |tick| tick.price == 0.0,
        )
        .build_or_panic();

    pipeline
        .ingest(
            PriceTick {
                selection_id: "selection-1",
                market_id: "market-1",
                fixture_id: "fixture-1",
                price: 0.0,
                observed_at: 10,
            },
            None,
            None,
        )
        .expect("initial event");
    let error = pipeline
        .ingest(
            PriceTick {
                selection_id: "selection-1",
                market_id: "market-1",
                fixture_id: "fixture-1",
                price: 1.1,
                observed_at: 9,
            },
            None,
            None,
        )
        .expect_err("backwards event must be rejected");

    assert!(matches!(
        error,
        IngestionError::Temporal(crate::TemporalRangeError::EndBeforeStart { .. })
    ));
    assert_eq!(pipeline.processing_position(), 1);
    assert_eq!(pipeline.history().open_windows().len(), 1);
    assert!(pipeline.history().closed_windows().is_empty());
}
