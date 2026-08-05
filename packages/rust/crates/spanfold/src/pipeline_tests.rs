//! Pipeline behavior and atomicity tests.

#![allow(unused_must_use)]

use super::*;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

#[derive(Clone)]
struct PriceTick {
    selection_id: &'static str,
    market_id: &'static str,
    fixture_id: &'static str,
    price: f64,
    observed_at: i64,
}

#[derive(Clone)]
struct StabilizedSignal {
    key: &'static str,
    parent: &'static str,
    enter: bool,
    exit: bool,
    segment: &'static str,
    tag: &'static str,
    invalid_projection: bool,
}

#[derive(Clone)]
struct AtomicObservationSignal {
    first_active: bool,
    second_active: bool,
    invalid_second_key: bool,
    invalid_rollup_key: bool,
}

impl StabilizedSignal {
    fn new(enter: bool, exit: bool) -> Self {
        Self {
            key: "item-1",
            parent: "parent-1",
            enter,
            exit,
            segment: "steady",
            tag: "current",
            invalid_projection: false,
        }
    }
}

#[test]
fn stabilized_window_rejects_flapping_with_asymmetric_confirmation() {
    let mut pipeline = for_events::<StabilizedSignal>()
        .record_windows()
        .window("Stable", |signal| signal.key, |signal| signal.enter)
        .stabilize(|signal| signal.exit, 2, 3)
        .build_or_panic();

    for signal in [
        StabilizedSignal::new(true, false),
        StabilizedSignal::new(false, false),
        StabilizedSignal::new(true, false),
    ] {
        assert!(
            pipeline
                .ingest(signal, None, None)
                .unwrap()
                .emissions
                .is_empty()
        );
    }
    let opened = pipeline
        .ingest(StabilizedSignal::new(true, true), None, None)
        .unwrap();
    for signal in [
        StabilizedSignal::new(true, true),
        StabilizedSignal::new(false, true),
        StabilizedSignal::new(false, false),
        StabilizedSignal::new(false, true),
        StabilizedSignal::new(false, true),
    ] {
        assert!(
            pipeline
                .ingest(signal, None, None)
                .unwrap()
                .emissions
                .is_empty()
        );
    }
    let closed = pipeline
        .ingest(StabilizedSignal::new(false, true), None, None)
        .unwrap();

    assert_eq!(opened.emissions[0].kind, WindowTransitionKind::Opened);
    assert_eq!(closed.emissions[0].kind, WindowTransitionKind::Closed);
    let window = &pipeline.history().closed_windows()[0];
    assert_eq!(window.range.start(), TemporalPoint::position(4));
    assert_eq!(window.range.end(), TemporalPoint::position(10));
}

#[test]
fn stabilized_window_uses_confirmation_metadata_and_preserves_pending_exit() {
    let mut pipeline = for_events::<StabilizedSignal>()
        .record_windows()
        .window_with_metadata(
            "Stable",
            |signal| signal.key,
            |signal| signal.enter,
            |signal| vec![WindowSegment::new("state", signal.segment).unwrap()],
            |signal| vec![WindowTag::new("label", signal.tag).unwrap()],
        )
        .stabilize(|signal| signal.exit, 2, 2)
        .build_or_panic();

    let mut candidate = StabilizedSignal::new(true, false);
    candidate.segment = "candidate";
    candidate.tag = "candidate";
    pipeline.ingest(candidate, None, None).unwrap();
    let mut confirmed = StabilizedSignal::new(true, false);
    confirmed.segment = "confirmed";
    confirmed.tag = "confirmed";
    pipeline.ingest(confirmed, None, None).unwrap();
    let mut pending_exit = StabilizedSignal::new(false, true);
    pending_exit.segment = "ignored";
    pending_exit.tag = "ignored";
    pipeline.ingest(pending_exit, None, None).unwrap();

    let open = &pipeline.history().open_windows()[0];
    assert_eq!(open.segments[0].value, "confirmed".into());
    assert_eq!(open.tags[0].value, "confirmed".into());

    let mut resumed = StabilizedSignal::new(false, false);
    resumed.segment = "resumed";
    resumed.tag = "resumed";
    let resumed = pipeline.ingest(resumed, None, None).unwrap();
    assert_eq!(
        resumed
            .emissions
            .iter()
            .map(|emission| emission.kind)
            .collect::<Vec<_>>(),
        vec![WindowTransitionKind::Closed, WindowTransitionKind::Opened]
    );
    assert_eq!(
        pipeline.history().open_windows()[0].start,
        TemporalPoint::position(4)
    );
}

#[test]
fn stabilization_counts_are_scoped_by_source_and_partition() {
    let mut pipeline = for_events::<StabilizedSignal>()
        .window("Stable", |signal| signal.key, |signal| signal.enter)
        .stabilize(|signal| signal.exit, 2, 1)
        .build_or_panic();

    assert!(
        pipeline
            .ingest(StabilizedSignal::new(true, false), Some("a"), Some("one"))
            .unwrap()
            .emissions
            .is_empty()
    );
    assert!(
        pipeline
            .ingest(StabilizedSignal::new(true, false), Some("a"), Some("two"))
            .unwrap()
            .emissions
            .is_empty()
    );
    let first = pipeline
        .ingest(StabilizedSignal::new(true, false), Some("a"), Some("one"))
        .unwrap();
    let second = pipeline
        .ingest(StabilizedSignal::new(true, false), Some("a"), Some("two"))
        .unwrap();

    assert_eq!(first.emissions[0].partition.as_deref(), Some("one"));
    assert_eq!(second.emissions[0].partition.as_deref(), Some("two"));
}

#[test]
fn failed_confirmation_preserves_staged_pending_state() {
    let mut pipeline = for_events::<StabilizedSignal>()
        .record_windows()
        .window_with_metadata(
            "Stable",
            |signal| signal.key,
            |signal| signal.enter,
            |signal| {
                vec![
                    WindowSegment::new("phase", "one").unwrap(),
                    WindowSegment::new(
                        if signal.invalid_projection {
                            "state"
                        } else {
                            "period"
                        },
                        "two",
                    )
                    .unwrap(),
                ]
            },
            |_| Vec::new(),
        )
        .stabilize(|signal| signal.exit, 2, 1)
        .roll_up_with_segment_projection(
            "ProjectionRollup",
            |signal| signal.parent,
            |children| children.any_active(),
            |projection| projection.rename("state", "phase"),
        )
        .build_or_panic();

    pipeline
        .ingest(StabilizedSignal::new(true, false), None, None)
        .unwrap();
    let mut invalid = StabilizedSignal::new(true, false);
    invalid.invalid_projection = true;
    assert!(matches!(
        pipeline.ingest(invalid, None, None),
        Err(IngestionError::InvalidSegmentProjection(_))
    ));
    assert_eq!(pipeline.processing_position(), 1);
    assert!(pipeline.history().open_windows().is_empty());
    let confirmed = pipeline
        .ingest(StabilizedSignal::new(true, false), None, None)
        .unwrap();

    assert!(
        confirmed
            .emissions
            .iter()
            .any(|emission| emission.window_name == "Stable")
    );
}

#[test]
fn invalid_later_definition_is_atomic_and_does_not_advance_position_or_ids() {
    let callbacks = Arc::new(Mutex::new(Vec::new()));
    let mut pipeline = for_events::<AtomicObservationSignal>()
        .record_windows()
        .on_emission({
            let callbacks = Arc::clone(&callbacks);
            move |emission| callbacks.lock().unwrap().push(emission.window_name.clone())
        })
        .track_window("First", |_| "first", |signal| signal.first_active)
        .track_window(
            "Second",
            |signal| {
                if signal.invalid_second_key {
                    ""
                } else {
                    "second"
                }
            },
            |signal| signal.second_active,
        )
        .build_or_panic();

    let invalid = AtomicObservationSignal {
        first_active: true,
        second_active: true,
        invalid_second_key: true,
        invalid_rollup_key: false,
    };
    assert!(matches!(
        pipeline.ingest(invalid, None, None),
        Err(IngestionError::InvalidObservation(
            WindowRecorderError::EmptyWindowKey
        ))
    ));
    assert_eq!(pipeline.processing_position(), 0);
    assert!(pipeline.history().open_windows().is_empty());
    assert!(pipeline.history().closed_windows().is_empty());
    assert!(callbacks.lock().unwrap().is_empty());

    let valid = AtomicObservationSignal {
        first_active: true,
        second_active: true,
        invalid_second_key: false,
        invalid_rollup_key: false,
    };
    let result = pipeline.ingest(valid, None, None).expect("valid event");
    assert_eq!(result.processing_position, 1);
    assert_eq!(pipeline.processing_position(), 1);
    assert_eq!(
        result
            .emissions
            .iter()
            .map(|emission| emission.record_id.as_str())
            .collect::<Vec<_>>(),
        vec!["pipeline-0000", "pipeline-0001"]
    );
    assert_eq!(pipeline.history().open_windows().len(), 2);
    assert_eq!(callbacks.lock().unwrap().len(), 2);
}

#[test]
fn invalid_dynamic_rollup_observation_is_atomic() {
    let mut pipeline = for_events::<AtomicObservationSignal>()
        .record_windows()
        .window("Child", |_| "child", |signal| signal.first_active)
        .roll_up(
            "Parent",
            |signal| {
                if signal.invalid_rollup_key {
                    ""
                } else {
                    "parent"
                }
            },
            |children| children.any_active(),
        )
        .build_or_panic();

    let invalid = AtomicObservationSignal {
        first_active: true,
        second_active: false,
        invalid_second_key: false,
        invalid_rollup_key: true,
    };
    assert!(matches!(
        pipeline.ingest(invalid, None, None),
        Err(IngestionError::InvalidObservation(
            WindowRecorderError::EmptyWindowKey
        ))
    ));
    assert_eq!(pipeline.processing_position(), 0);
    assert!(pipeline.history().open_windows().is_empty());

    let valid = AtomicObservationSignal {
        first_active: true,
        second_active: false,
        invalid_second_key: false,
        invalid_rollup_key: false,
    };
    let result = pipeline.ingest(valid, None, None).expect("valid event");
    assert_eq!(result.processing_position, 1);
    assert_eq!(result.emissions.len(), 2);
    assert_eq!(pipeline.history().open_windows().len(), 2);
}

#[test]
fn rollups_and_callbacks_observe_only_confirmed_transitions() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let lifecycle_calls = Arc::new(Mutex::new(Vec::new()));
    let mut pipeline = for_events::<StabilizedSignal>()
        .record_windows()
        .on_emission({
            let calls = Arc::clone(&calls);
            move |emission| calls.lock().unwrap().push(emission.window_name.clone())
        })
        .window_with_options("Stable", |signal| signal.key, |signal| signal.enter, {
            let lifecycle_calls = Arc::clone(&lifecycle_calls);
            move |options| {
                let opened_calls = Arc::clone(&lifecycle_calls);
                let closed_calls = Arc::clone(&lifecycle_calls);
                options
                    .on_opened(move |emission| {
                        opened_calls.lock().unwrap().push(emission.kind);
                    })
                    .on_closed(move |emission| {
                        closed_calls.lock().unwrap().push(emission.kind);
                    })
            }
        })
        .stabilize(|signal| signal.exit, 2, 2)
        .roll_up(
            "AnyStable",
            |signal| signal.parent,
            |children| children.any_active(),
        )
        .build_or_panic();

    assert!(
        pipeline
            .ingest(StabilizedSignal::new(true, false), None, None)
            .unwrap()
            .emissions
            .is_empty()
    );
    assert!(calls.lock().unwrap().is_empty());
    assert!(lifecycle_calls.lock().unwrap().is_empty());
    pipeline
        .ingest(StabilizedSignal::new(true, false), None, None)
        .unwrap();
    assert_eq!(&*calls.lock().unwrap(), &["Stable", "AnyStable"]);
    assert!(
        pipeline
            .ingest(StabilizedSignal::new(false, true), None, None)
            .unwrap()
            .emissions
            .is_empty()
    );
    pipeline
        .ingest(StabilizedSignal::new(false, true), None, None)
        .unwrap();
    assert_eq!(
        &*calls.lock().unwrap(),
        &["Stable", "AnyStable", "Stable", "AnyStable"]
    );
    assert_eq!(
        &*lifecycle_calls.lock().unwrap(),
        &[WindowTransitionKind::Opened, WindowTransitionKind::Closed]
    );
}

#[test]
fn stabilization_counts_must_be_positive() {
    let result = std::panic::catch_unwind(|| {
        for_events::<StabilizedSignal>()
            .window("Stable", |signal| signal.key, |signal| signal.enter)
            .stabilize(|signal| signal.exit, 0, 1);
    });

    assert!(result.is_err());
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
fn all_active_rollup_tracks_known_inactive_children() {
    let views = Arc::new(Mutex::new(Vec::new()));
    let mut pipeline = {
        let views = Arc::clone(&views);
        for_events::<PriceTick>()
            .record_windows()
            .window(
                "SelectionPriced",
                |tick| tick.selection_id,
                |tick| tick.price > 0.0,
            )
            .roll_up(
                "MarketPriced",
                |tick| tick.market_id,
                move |children| {
                    views.lock().unwrap().push(children);
                    children.all_active()
                },
            )
            .build_or_panic()
    };

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
            fixture_id: "fixture-1",
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
        PriceTick {
            selection_id: "selection-1",
            market_id: "market-1",
            fixture_id: "fixture-1",
            price: 1.03,
            observed_at: 103,
        },
    ] {
        pipeline.ingest(tick, None, None).expect("ingest");
    }

    assert_eq!(
        &*views.lock().unwrap(),
        &[
            ChildActivityView {
                active_count: 1,
                total_count: 1,
            },
            ChildActivityView {
                active_count: 2,
                total_count: 2,
            },
            ChildActivityView {
                active_count: 1,
                total_count: 2,
            },
            ChildActivityView {
                active_count: 2,
                total_count: 2,
            },
        ]
    );
    assert_eq!(
        pipeline
            .history()
            .closed_windows()
            .iter()
            .filter(|window| window.window_name == "MarketPriced")
            .count(),
        1
    );
    assert_eq!(
        pipeline
            .history()
            .open_windows()
            .iter()
            .filter(|window| window.window_name == "MarketPriced")
            .count(),
        1
    );
}

#[test]
fn all_active_rollup_counts_a_first_observed_inactive_child() {
    let views = Arc::new(Mutex::new(Vec::new()));
    let mut pipeline = {
        let views = Arc::clone(&views);
        for_events::<PriceTick>()
            .record_windows()
            .window(
                "SelectionPriced",
                |tick| tick.selection_id,
                |tick| tick.price > 0.0,
            )
            .roll_up(
                "MarketPriced",
                |tick| tick.market_id,
                move |children| {
                    views.lock().unwrap().push(children);
                    children.all_active()
                },
            )
            .build_or_panic()
    };

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
            fixture_id: "fixture-1",
            price: 0.0,
            observed_at: 101,
        },
    ] {
        pipeline.ingest(tick, None, None).expect("ingest");
    }

    assert_eq!(
        &*views.lock().unwrap(),
        &[
            ChildActivityView {
                active_count: 1,
                total_count: 1,
            },
            ChildActivityView {
                active_count: 1,
                total_count: 2,
            },
        ]
    );
    assert_eq!(
        pipeline
            .history()
            .closed_windows()
            .iter()
            .filter(|window| window.window_name == "MarketPriced")
            .count(),
        1
    );
}

#[test]
fn all_active_rollup_removes_migrated_child_from_old_parent_membership() {
    let views = Arc::new(Mutex::new(Vec::new()));
    let mut pipeline = {
        let views = Arc::clone(&views);
        for_events::<PriceTick>()
            .record_windows()
            .window(
                "SelectionPriced",
                |tick| tick.selection_id,
                |tick| tick.price > 0.0,
            )
            .roll_up(
                "MarketPriced",
                |tick| tick.market_id,
                move |children| {
                    views.lock().unwrap().push(children);
                    children.all_active()
                },
            )
            .build_or_panic()
    };

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
            fixture_id: "fixture-1",
            price: 1.02,
            observed_at: 101,
        },
        PriceTick {
            selection_id: "selection-1",
            market_id: "market-2",
            fixture_id: "fixture-1",
            price: 1.03,
            observed_at: 102,
        },
    ] {
        pipeline.ingest(tick, None, None).expect("ingest");
    }

    assert_eq!(
        &*views.lock().unwrap(),
        &[
            ChildActivityView {
                active_count: 1,
                total_count: 1,
            },
            ChildActivityView {
                active_count: 2,
                total_count: 2,
            },
            ChildActivityView {
                active_count: 1,
                total_count: 1,
            },
            ChildActivityView {
                active_count: 1,
                total_count: 1,
            },
        ]
    );
    let open_parents = pipeline
        .history()
        .open_windows()
        .iter()
        .filter(|window| window.window_name == "MarketPriced")
        .map(|window| window.key.as_str())
        .collect::<Vec<_>>();
    assert_eq!(open_parents, vec!["market-1", "market-2"]);
    assert!(
        pipeline
            .history()
            .closed_windows()
            .iter()
            .all(|window| window.window_name != "MarketPriced")
    );
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
fn metadata_and_rollup_observations_are_evaluated_once_per_event() {
    let key_calls = Arc::new(AtomicUsize::new(0));
    let active_calls = Arc::new(AtomicUsize::new(0));
    let segment_calls = Arc::new(AtomicUsize::new(0));
    let tag_calls = Arc::new(AtomicUsize::new(0));
    let rollup_key_calls = Arc::new(AtomicUsize::new(0));
    let transform_calls = Arc::new(AtomicUsize::new(0));

    let mut pipeline = for_events::<PriceTick>()
        .record_windows()
        .window_with_metadata(
            "SelectionPriced",
            {
                let calls = Arc::clone(&key_calls);
                move |tick| {
                    calls.fetch_add(1, Ordering::Relaxed);
                    tick.selection_id
                }
            },
            {
                let calls = Arc::clone(&active_calls);
                move |tick| {
                    calls.fetch_add(1, Ordering::Relaxed);
                    tick.price > 0.0
                }
            },
            {
                let calls = Arc::clone(&segment_calls);
                move |tick| {
                    calls.fetch_add(1, Ordering::Relaxed);
                    vec![WindowSegment::new("phase", tick.market_id).expect("segment name")]
                }
            },
            {
                let calls = Arc::clone(&tag_calls);
                move |tick| {
                    calls.fetch_add(1, Ordering::Relaxed);
                    vec![WindowTag::new("fixture", tick.fixture_id).expect("tag name")]
                }
            },
        )
        .roll_up_with_segment_projection(
            "MarketPriced",
            {
                let calls = Arc::clone(&rollup_key_calls);
                move |tick| {
                    calls.fetch_add(1, Ordering::Relaxed);
                    tick.market_id
                }
            },
            |children| children.any_active(),
            {
                let calls = Arc::clone(&transform_calls);
                move |projection| {
                    projection.transform("phase", move |value| {
                        calls.fetch_add(1, Ordering::Relaxed);
                        value.clone()
                    })
                }
            },
        )
        .build_or_panic();

    pipeline
        .ingest(
            PriceTick {
                selection_id: "selection-1",
                market_id: "in-play",
                fixture_id: "fixture-1",
                price: 1.01,
                observed_at: 100,
            },
            None,
            None,
        )
        .expect("ingest");

    for calls in [
        key_calls,
        active_calls,
        segment_calls,
        tag_calls,
        rollup_key_calls,
        transform_calls,
    ] {
        assert_eq!(calls.load(Ordering::Relaxed), 1);
    }
}

#[test]
fn rollup_key_selectors_are_not_repeated_for_old_parent_removal() {
    let rollup_key_calls = Arc::new(AtomicUsize::new(0));
    let mut pipeline = for_events::<StabilizedSignal>()
        .window("Stable", |signal| signal.key, |signal| signal.enter)
        .roll_up(
            "AnyStable",
            {
                let calls = Arc::clone(&rollup_key_calls);
                move |signal| {
                    calls.fetch_add(1, Ordering::Relaxed);
                    signal.parent
                }
            },
            |children| children.any_active(),
        )
        .build_or_panic();

    pipeline
        .ingest(StabilizedSignal::new(true, false), None, None)
        .expect("first event");
    let mut migrated = StabilizedSignal::new(true, false);
    migrated.parent = "parent-2";
    pipeline
        .ingest(migrated, None, None)
        .expect("migrated event");

    assert_eq!(rollup_key_calls.load(Ordering::Relaxed), 2);
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
