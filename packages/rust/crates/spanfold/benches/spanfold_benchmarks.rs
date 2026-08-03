use std::hint::black_box;

use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};
use spanfold::{
    AsOfDirection, CohortActivity, Comparator, ComparisonScope, ComparisonSelector,
    EpisodeComparisonBuilder, EventPipeline, LeadLagTransition, PrimitiveValue, TemporalAxis,
    TemporalPoint, TemporalTolerance, WindowComparisonBuilder, WindowHistory, WindowHistoryFixture,
    WindowSegment, WindowTag, export_result_json, export_result_llm_context, for_events,
};

const EPISODE_DETECTION_SOURCE: &str = "detection";
const EPISODE_TARGET_SOURCE: &str = "reference";
const EPISODE_WINDOW_NAME: &str = "State";
const HISTORY_DIRECT_QUERY_DENSE_RECORDS_PER_SOURCE: usize = 64;
const HISTORY_DIRECT_QUERY_DISTINCT_SCOPES: usize = 4_096;
const HISTORY_DIRECT_QUERY_TARGET_SOURCE: &str = "target";
const DENSE_DUPLICATES_PER_SOURCE: usize = 1_024;
const DENSE_DUPLICATE_AGAINST_SOURCES: [&str; 3] = ["against-0", "against-1", "against-2"];
const DENSE_DUPLICATE_KEY: &str = "shared-key";
const DENSE_DUPLICATE_PARTITION: &str = "partition-0";
const DENSE_DUPLICATE_TARGET_SOURCE: &str = "target";
const DENSE_DUPLICATE_WINDOW_NAME: &str = "DenseDuplicate";
const COHORT_ACTIVITY_SOURCE_COUNT: usize = 24;
const COHORT_ACTIVITY_WINDOWS_PER_SOURCE: usize = 64;
const COHORT_ACTIVITY_STRIDE: i64 = 32;
const COHORT_ACTIVITY_WINDOW_WIDTH: i64 = 512;
const COHORT_ACTIVITY_KEY: &str = "shared-key";
const COHORT_ACTIVITY_PARTITION: &str = "partition-0";
const COHORT_ACTIVITY_TARGET_SOURCE: &str = "target";
const COHORT_ACTIVITY_WINDOW_NAME: &str = "CohortActivity";
const FRAGMENTS_PER_EPISODE: usize = 8;
const RELATION_STRIDE: i64 = 20;
const TRANSITION_AGAINST_SOURCE: &str = "against";
const TRANSITION_EXPECTED_DELTA: i64 = 1;
const TRANSITION_KEY: &str = "dense-scope";
const TRANSITION_PARTITION: &str = "partition-0";
const TRANSITION_STRIDE: i64 = 10;
const TRANSITION_TARGET_SOURCE: &str = "target";
const TRANSITION_TOLERANCE: i64 = 2;
const TRANSITION_WINDOW_NAME: &str = "State";

#[derive(Clone)]
struct DeviceSignal {
    device_id: String,
    is_online: bool,
}

#[derive(Clone)]
struct SegmentSignal {
    selection_id: String,
    market_id: String,
    fixture_id: String,
    is_online: bool,
    phase: String,
    period: String,
    state: String,
}

fn create_comparison_data(
    event_count: usize,
    device_count: usize,
    source_count: usize,
) -> WindowHistory {
    let mut pipeline = for_events::<DeviceSignal>()
        .record_windows()
        .track_window(
            "DeviceOffline",
            |signal| signal.device_id.clone(),
            |signal| !signal.is_online,
        )
        .build()
        .expect("valid benchmark pipeline");
    let mut occurrences = vec![0_usize; device_count * source_count];
    for event_index in 0..event_count {
        let device_index = event_index % device_count;
        let source_index = (event_index / device_count) % source_count;
        let occurrence_index = device_index * source_count + source_index;
        let occurrence = occurrences[occurrence_index];
        occurrences[occurrence_index] += 1;
        let source = format!("provider-{source_index}");
        let _ = pipeline.ingest(
            DeviceSignal {
                device_id: format!("device-{device_index}"),
                is_online: occurrence.is_multiple_of(2),
            },
            Some(&source),
            None,
        );
    }
    pipeline.history().clone()
}

fn create_segment_cohort_data() -> WindowHistory {
    let event_count = 2_048_usize;
    let selection_count = 128_usize;
    let source_count = 4_usize;
    let mut pipeline = for_events::<SegmentSignal>()
        .record_windows()
        .window_with_metadata(
            "SelectionPriced",
            |signal| signal.selection_id.clone(),
            |signal| !signal.is_online,
            |signal| {
                vec![
                    WindowSegment::new("market", signal.market_id.clone()).expect("segment name"),
                    WindowSegment::new("fixture", signal.fixture_id.clone()).expect("segment name"),
                    WindowSegment::new("phase", signal.phase.clone()).expect("segment name"),
                    WindowSegment::new("period", signal.period.clone())
                        .and_then(|segment| segment.with_parent("phase"))
                        .expect("segment names"),
                ]
            },
            |signal| vec![WindowTag::new("state", signal.state.clone()).expect("tag name")],
        )
        .roll_up(
            "MarketPriced",
            |signal| signal.market_id.clone(),
            |children| children.any_active(),
        )
        .build()
        .expect("valid benchmark pipeline");

    for event_index in 0..event_count {
        let selection_index = event_index % selection_count;
        let source_index = (event_index / selection_count) % source_count;
        let occurrence = event_index / (selection_count * source_count);
        let source = format!("provider-{source_index}");
        let _ = pipeline.ingest(
            SegmentSignal {
                selection_id: format!("selection-{selection_index}"),
                market_id: format!("market-{}", selection_index % 16),
                fixture_id: format!("fixture-{}", selection_index % 8),
                is_online: (occurrence + source_index).is_multiple_of(2),
                phase: if occurrence.is_multiple_of(2) {
                    "in-play".to_owned()
                } else {
                    "pre-match".to_owned()
                },
                period: format!("period-{}", occurrence % 4),
                state: if occurrence.is_multiple_of(2) {
                    "active".to_owned()
                } else {
                    "settled".to_owned()
                },
            },
            Some(&source),
            None,
        );
    }
    pipeline.history().clone()
}

fn create_metadata_rollup_pipeline() -> EventPipeline<SegmentSignal> {
    for_events::<SegmentSignal>()
        .record_windows()
        .window_with_metadata(
            "SelectionPriced",
            |signal| signal.selection_id.clone(),
            |signal| !signal.is_online,
            |signal| {
                vec![
                    WindowSegment::new("market", signal.market_id.clone()).expect("segment name"),
                    WindowSegment::new("fixture", signal.fixture_id.clone()).expect("segment name"),
                    WindowSegment::new("phase", signal.phase.clone()).expect("segment name"),
                    WindowSegment::new("period", signal.period.clone())
                        .and_then(|segment| segment.with_parent("phase"))
                        .expect("segment names"),
                ]
            },
            |signal| vec![WindowTag::new("state", signal.state.clone()).expect("tag name")],
        )
        .roll_up_with_segment_projection(
            "MarketPriced",
            |signal| signal.market_id.clone(),
            |children| children.any_active(),
            |projection| {
                projection
                    .preserve("phase")
                    .preserve("period")
                    .rename("phase", "trading_phase")
            },
        )
        .build()
        .expect("valid benchmark pipeline")
}

fn create_metadata_rollup_events(event_count: usize) -> Vec<(SegmentSignal, String)> {
    let selection_count = 128_usize;
    let source_count = 4_usize;
    let mut events = Vec::with_capacity(event_count);
    for event_index in 0..event_count {
        let selection_index = event_index % selection_count;
        let source_index = (event_index / selection_count) % source_count;
        let occurrence = event_index / (selection_count * source_count);
        let source = format!("provider-{source_index}");
        let (is_online, phase, period, state) = match occurrence % 4 {
            0 => (false, "pre-match", "period-0", "available"),
            1 => (false, "in-play", "period-1", "active"),
            2 => (false, "in-play", "period-1", "paused"),
            _ => (true, "in-play", "period-1", "settled"),
        };
        events.push((
            SegmentSignal {
                selection_id: format!("selection-{selection_index}"),
                market_id: format!("market-{}", selection_index % 16),
                fixture_id: format!("fixture-{}", selection_index % 8),
                is_online,
                phase: phase.to_owned(),
                period: period.to_owned(),
                state: state.to_owned(),
            },
            source,
        ));
    }
    events
}

fn comparison_builder(history: &WindowHistory) -> spanfold::WindowComparisonBuilder<'_> {
    history
        .compare("Benchmark Provider QA")
        .target_source("provider-0")
        .against_source("provider-1")
        .scope_window("DeviceOffline")
}

fn create_dense_duplicate_cohort_history() -> WindowHistory {
    let mut fixture = WindowHistoryFixture::new();
    for _ in 0..DENSE_DUPLICATES_PER_SOURCE {
        fixture = fixture
            .closed_window(
                DENSE_DUPLICATE_WINDOW_NAME,
                DENSE_DUPLICATE_KEY,
                0,
                10,
                |window| {
                    window
                        .source(DENSE_DUPLICATE_TARGET_SOURCE)
                        .partition(DENSE_DUPLICATE_PARTITION)
                },
            )
            .expect("valid target duplicate window");

        for source in DENSE_DUPLICATE_AGAINST_SOURCES {
            fixture = fixture
                .closed_window(
                    DENSE_DUPLICATE_WINDOW_NAME,
                    DENSE_DUPLICATE_KEY,
                    0,
                    10,
                    |window| window.source(source).partition(DENSE_DUPLICATE_PARTITION),
                )
                .expect("valid against duplicate window");
        }
    }
    fixture.build()
}

fn dense_duplicate_cohort_builder(
    history: &WindowHistory,
) -> spanfold::WindowComparisonBuilder<'_> {
    history
        .compare("Dense duplicate cohort")
        .target_source(DENSE_DUPLICATE_TARGET_SOURCE)
        .against_cohort(
            "against cohort",
            DENSE_DUPLICATE_AGAINST_SOURCES,
            CohortActivity::Any,
        )
        .scope_window(DENSE_DUPLICATE_WINDOW_NAME)
        .scope_key(DENSE_DUPLICATE_KEY)
        .scope_partition(DENSE_DUPLICATE_PARTITION)
        .overlap()
}

fn create_cohort_activity_history(source_count: usize) -> (WindowHistory, Vec<String>) {
    let sources = (0..source_count)
        .map(|source_index| format!("source-{source_index:02}"))
        .collect::<Vec<_>>();
    let last_end = (COHORT_ACTIVITY_WINDOWS_PER_SOURCE - 1) as i64 * COHORT_ACTIVITY_STRIDE
        + source_count as i64
        - 1
        + COHORT_ACTIVITY_WINDOW_WIDTH;
    let mut fixture = WindowHistoryFixture::new()
        .closed_window(
            COHORT_ACTIVITY_WINDOW_NAME,
            COHORT_ACTIVITY_KEY,
            0,
            last_end,
            |window| {
                window
                    .source(COHORT_ACTIVITY_TARGET_SOURCE)
                    .partition(COHORT_ACTIVITY_PARTITION)
            },
        )
        .expect("valid cohort activity target window");

    for (source_index, source) in sources.iter().enumerate() {
        for window_index in 0..COHORT_ACTIVITY_WINDOWS_PER_SOURCE {
            let start = window_index as i64 * COHORT_ACTIVITY_STRIDE + source_index as i64;
            fixture = fixture
                .closed_window(
                    COHORT_ACTIVITY_WINDOW_NAME,
                    COHORT_ACTIVITY_KEY,
                    start,
                    start + COHORT_ACTIVITY_WINDOW_WIDTH,
                    |window| window.source(source).partition(COHORT_ACTIVITY_PARTITION),
                )
                .expect("valid staggered cohort activity window");
        }
    }

    (fixture.build(), sources)
}

fn cohort_activity_builder<'a>(
    history: &'a WindowHistory,
    sources: &[String],
) -> spanfold::WindowComparisonBuilder<'a> {
    history
        .compare("Staggered cohort activity")
        .target_source(COHORT_ACTIVITY_TARGET_SOURCE)
        .against_cohort(
            "against cohort",
            sources.iter().cloned(),
            CohortActivity::AtLeast {
                count: sources.len().div_ceil(2),
            },
        )
        .scope_window(COHORT_ACTIVITY_WINDOW_NAME)
        .scope_key(COHORT_ACTIVITY_KEY)
        .scope_partition(COHORT_ACTIVITY_PARTITION)
        .overlap()
}

fn segment_builder(history: &WindowHistory) -> spanfold::WindowComparisonBuilder<'_> {
    history
        .compare("Segment Cohort QA")
        .target_source("provider-0")
        .against_cohort(
            "cohort",
            ["provider-1", "provider-2", "provider-3"],
            CohortActivity::Any,
        )
        .scope_window("SelectionPriced")
        .scope_segment("phase", PrimitiveValue::from("in-play"))
}

fn ingestion_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("ingestion");
    for event_count in [128, 1_024, 8_192, 100_000, 1_000_000] {
        group.bench_function(format!("ingest_{event_count}"), |b| {
            b.iter(|| create_comparison_data(black_box(event_count), 128, 2));
        });
    }
    let metadata_rollup_events = create_metadata_rollup_events(8_192);
    group.bench_function("metadata_rollup_8192", |b| {
        b.iter_batched(
            || {
                (
                    create_metadata_rollup_pipeline(),
                    metadata_rollup_events.clone(),
                )
            },
            |(mut pipeline, events)| {
                for (event, source) in events {
                    pipeline
                        .ingest(event, Some(&source), None)
                        .expect("benchmark ingestion");
                }
                black_box(pipeline)
            },
            BatchSize::SmallInput,
        );
    });
    group.finish();
}

fn comparison_benchmarks(c: &mut Criterion) {
    let history = create_comparison_data(1_024, 128, 2);
    let result_for_export = comparison_builder(&history)
        .overlap()
        .residual()
        .missing()
        .coverage()
        .gap()
        .symmetric_difference()
        .run();
    let live_horizon = TemporalPoint::position(1_025);
    let live_result_for_export = comparison_builder(&history)
        .residual()
        .run_live(live_horizon.clone());

    let mut group = c.benchmark_group("comparison");
    group.bench_function("prepare", |b| {
        b.iter(|| comparison_builder(black_box(&history)).overlap().prepare());
    });
    group.bench_function("align", |b| {
        b.iter(|| comparison_builder(black_box(&history)).overlap().align());
    });
    group.bench_function("run_overlap", |b| {
        b.iter(|| comparison_builder(black_box(&history)).overlap().run());
    });
    group.bench_function("run_residual", |b| {
        b.iter(|| comparison_builder(black_box(&history)).residual().run());
    });
    group.bench_function("run_coverage", |b| {
        b.iter(|| comparison_builder(black_box(&history)).coverage().run());
    });
    group.bench_function("run_multi_comparator", |b| {
        b.iter(|| {
            comparison_builder(black_box(&history))
                .overlap()
                .residual()
                .missing()
                .coverage()
                .gap()
                .symmetric_difference()
                .run()
        });
    });
    group.bench_function("run_live_residual", |b| {
        b.iter(|| {
            comparison_builder(black_box(&history))
                .residual()
                .run_live(live_horizon.clone())
        });
    });
    group.bench_function("export_json", |b| {
        b.iter(|| export_result_json(black_box(&result_for_export)).expect("json export"));
    });
    group.bench_function("export_live_json", |b| {
        b.iter(|| export_result_json(black_box(&live_result_for_export)).expect("json export"));
    });
    group.finish();
}

fn dense_duplicate_cohort_benchmarks(c: &mut Criterion) {
    let history = create_dense_duplicate_cohort_history();
    let comparison = dense_duplicate_cohort_builder(&history);
    let result = comparison.run();
    assert!(result.is_valid);
    assert_eq!(result.overlap_rows.len(), 1);
    assert_eq!(
        result.overlap_rows[0].target_record_ids.len(),
        DENSE_DUPLICATES_PER_SOURCE
    );
    assert_eq!(
        result.overlap_rows[0].against_record_ids.len(),
        DENSE_DUPLICATES_PER_SOURCE * DENSE_DUPLICATE_AGAINST_SOURCES.len()
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "DuplicateWindow")
    );
    let mut group = c.benchmark_group("dense_duplicate_cohort");
    group.bench_function("align", |b| {
        b.iter(|| black_box(&comparison).align());
    });
    group.bench_function("run_overlap", |b| {
        b.iter(|| black_box(&comparison).run());
    });
    group.finish();
}

fn cohort_activity_benchmarks(c: &mut Criterion) {
    let (history, sources) = create_cohort_activity_history(COHORT_ACTIVITY_SOURCE_COUNT);
    let comparison = cohort_activity_builder(&history, &sources);
    let result = comparison.run();
    let evidence = result.cohort_evidence();
    assert!(result.is_valid);
    assert!(!result.overlap_rows.is_empty());
    assert!(
        result
            .overlap_rows
            .iter()
            .any(|row| row.against_record_ids.len() > COHORT_ACTIVITY_SOURCE_COUNT)
    );
    assert_eq!(
        evidence.first().expect("first cohort segment").active_count,
        1
    );
    assert!(!evidence.first().expect("first cohort segment").is_active);
    assert!(evidence.iter().any(
        |segment| segment.active_count == COHORT_ACTIVITY_SOURCE_COUNT
            && segment.is_active
            && segment.active_sources == sources
    ));
    assert_eq!(
        evidence.last().expect("last cohort segment").active_count,
        1
    );
    assert!(!evidence.last().expect("last cohort segment").is_active);

    let (control_history, control_sources) = create_cohort_activity_history(1);
    let control = cohort_activity_builder(&control_history, &control_sources);
    assert!(control.run().is_valid);

    let mut group = c.benchmark_group("cohort_activity");
    group.bench_function("run_staggered_24_sources", |b| {
        b.iter(|| black_box(&comparison).run());
    });
    group.bench_function("run_staggered_1_source_control", |b| {
        b.iter(|| black_box(&control).run());
    });
    group.finish();
}

fn segment_cohort_benchmarks(c: &mut Criterion) {
    let history = create_segment_cohort_data();
    let result_for_export = segment_builder(&history).residual().run();
    let mut group = c.benchmark_group("segment_cohort");
    group.bench_function("segment_cohort_residual", |b| {
        b.iter(|| segment_builder(black_box(&history)).residual().run());
    });
    group.bench_function("segment_cohort_llm_context", |b| {
        b.iter(|| export_result_llm_context(black_box(&result_for_export)).expect("llm export"));
    });
    group.finish();
}

fn create_transition_comparator_history(transition_count_per_side: usize) -> WindowHistory {
    let mut fixture = WindowHistoryFixture::new();
    for index in 0..transition_count_per_side {
        let against_start = i64::try_from(index).expect("transition index") * TRANSITION_STRIDE;
        fixture = fixture
            .closed_window(
                TRANSITION_WINDOW_NAME,
                TRANSITION_KEY,
                against_start,
                against_start + 4,
                |window| {
                    window
                        .source(TRANSITION_AGAINST_SOURCE)
                        .partition(TRANSITION_PARTITION)
                },
            )
            .expect("valid against transition window");

        let target_start = against_start + TRANSITION_EXPECTED_DELTA;
        fixture = fixture
            .closed_window(
                TRANSITION_WINDOW_NAME,
                TRANSITION_KEY,
                target_start,
                target_start + 4,
                |window| {
                    window
                        .source(TRANSITION_TARGET_SOURCE)
                        .partition(TRANSITION_PARTITION)
                },
            )
            .expect("valid target transition window");
    }
    fixture.build()
}

fn transition_comparison(
    history: &WindowHistory,
    comparator: Comparator,
) -> WindowComparisonBuilder<'_> {
    history
        .compare("Dense transition matching")
        .target_source(TRANSITION_TARGET_SOURCE)
        .against_source(TRANSITION_AGAINST_SOURCE)
        .scope_window(TRANSITION_WINDOW_NAME)
        .scope_key(TRANSITION_KEY)
        .scope_partition(TRANSITION_PARTITION)
        .use_comparator(comparator)
}

fn transition_comparator_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("transition_comparators");
    for transition_count_per_side in [64_usize, 256, 1_024] {
        let history = create_transition_comparator_history(transition_count_per_side);
        let lead_lag = transition_comparison(
            &history,
            Comparator::LeadLag {
                transition: LeadLagTransition::Start,
                axis: TemporalAxis::ProcessingPosition,
                tolerance_magnitude: TRANSITION_TOLERANCE,
            },
        );
        let as_of = transition_comparison(
            &history,
            Comparator::AsOf {
                direction: AsOfDirection::Previous,
                axis: TemporalAxis::ProcessingPosition,
                tolerance_magnitude: TRANSITION_TOLERANCE,
            },
        );

        let lead_lag_result = lead_lag.run();
        assert_eq!(
            lead_lag_result.lead_lag_rows.len(),
            transition_count_per_side
        );
        assert!(lead_lag_result.lead_lag_rows.iter().all(|row| {
            row.comparison_record_id.is_some()
                && row.delta_magnitude == Some(TRANSITION_EXPECTED_DELTA)
                && row.is_within_tolerance
        }));

        let as_of_result = as_of.run();
        assert_eq!(as_of_result.as_of_rows.len(), transition_count_per_side);
        assert!(as_of_result.as_of_rows.iter().all(|row| {
            row.matched_record_id.is_some()
                && row.distance_magnitude == Some(TRANSITION_EXPECTED_DELTA)
                && row.status == spanfold::AsOfMatchStatus::Matched
        }));

        group.bench_with_input(
            BenchmarkId::new("lead_lag_start", transition_count_per_side),
            &transition_count_per_side,
            |b, _| b.iter(|| black_box(&lead_lag).run()),
        );
        group.bench_with_input(
            BenchmarkId::new("as_of_previous", transition_count_per_side),
            &transition_count_per_side,
            |b, _| b.iter(|| black_box(&as_of).run()),
        );
    }
    group.finish();
}

fn create_episode_formation_history(window_count: usize) -> WindowHistory {
    let mut fixture = WindowHistoryFixture::new();
    for index in 0..window_count {
        let episode_index = index / FRAGMENTS_PER_EPISODE;
        let fragment_index = index % FRAGMENTS_PER_EPISODE;
        let start = i64::try_from(fragment_index).expect("fragment index") * 10;
        fixture = fixture
            .closed_window(
                EPISODE_WINDOW_NAME,
                format!("device-{episode_index}"),
                start,
                start + 4,
                |window| window.source(EPISODE_TARGET_SOURCE),
            )
            .expect("valid episode formation window");
    }
    fixture.build()
}

fn create_episode_relation_history(episode_count_per_side: usize) -> WindowHistory {
    let mut fixture = WindowHistoryFixture::new();
    for episode_index in 0..episode_count_per_side {
        let key = format!("device-{episode_index}");
        let target_start = i64::try_from(episode_index).expect("episode index") * RELATION_STRIDE;
        fixture = fixture
            .closed_window(
                EPISODE_WINDOW_NAME,
                key.clone(),
                target_start,
                target_start + 4,
                |window| window.source(EPISODE_TARGET_SOURCE),
            )
            .expect("valid target episode window");

        let detection_start = if episode_index % 4 == 3 {
            target_start + 10
        } else {
            target_start + 1
        };
        fixture = fixture
            .closed_window(
                EPISODE_WINDOW_NAME,
                key,
                detection_start,
                detection_start + 4,
                |window| window.source(EPISODE_DETECTION_SOURCE),
            )
            .expect("valid detection episode window");
    }
    fixture.build()
}

fn episode_comparison(history: &WindowHistory) -> EpisodeComparisonBuilder<'_> {
    history
        .compare_episodes("Benchmark detector evaluation")
        .target(
            EPISODE_TARGET_SOURCE,
            ComparisonSelector::for_source(EPISODE_TARGET_SOURCE),
        )
        .against(
            EPISODE_DETECTION_SOURCE,
            ComparisonSelector::for_source(EPISODE_DETECTION_SOURCE),
        )
        .scope(ComparisonScope::window(EPISODE_WINDOW_NAME))
}

fn episode_benchmarks(c: &mut Criterion) {
    let mut formation_group = c.benchmark_group("episode_formation");
    for window_count in [128_usize, 1_024, 8_192] {
        let history = create_episode_formation_history(window_count);
        let formation = history
            .form_episodes("Benchmark formation")
            .from(ComparisonSelector::for_source(EPISODE_TARGET_SOURCE))
            .scope(ComparisonScope::window(EPISODE_WINDOW_NAME))
            .stitch_gaps_up_to(
                TemporalTolerance::processing_positions(6).expect("formation tolerance"),
            );
        let _plan = formation.build().expect("valid episode formation plan");
        formation_group.bench_function(format!("form_{window_count}_windows"), |b| {
            b.iter(|| formation.run().expect("episode formation"));
        });
    }
    formation_group.finish();

    let mut relation_group = c.benchmark_group("episode_relation_sparse");
    for episode_count_per_side in [64_usize, 256, 1_024] {
        let history = create_episode_relation_history(episode_count_per_side);
        let comparison = episode_comparison(&history);
        let _plan = comparison.build().expect("valid episode comparison plan");
        relation_group.bench_function(
            format!("build_{episode_count_per_side}_episodes_per_side"),
            |b| {
                b.iter(|| comparison.run().expect("episode relation graph"));
            },
        );
    }
    relation_group.finish();

    let history = create_episode_relation_history(1_024);
    let comparison = episode_comparison(&history)
        .run()
        .expect("materialized episode comparison");
    c.bench_function(
        "episode_summary/interpret_materialized_reference_scorecard",
        |b| {
            b.iter(|| black_box(&comparison).as_reference());
        },
    );
}

fn create_incompatible_scope_history(scope_count: usize) -> WindowHistory {
    let mut fixture = WindowHistoryFixture::new();
    for scope_index in 0..scope_count {
        fixture = fixture
            .closed_window(
                "HistoryQuery",
                format!("key-{scope_index}"),
                0,
                10,
                |window| {
                    window.source(if scope_index.is_multiple_of(2) {
                        HISTORY_DIRECT_QUERY_TARGET_SOURCE
                    } else {
                        "against"
                    })
                },
            )
            .expect("valid incompatible-scope history window");
    }
    fixture.build()
}

fn create_dense_scope_history(records_per_source: usize) -> WindowHistory {
    let mut fixture = WindowHistoryFixture::new();
    for source in [HISTORY_DIRECT_QUERY_TARGET_SOURCE, "against"] {
        for _ in 0..records_per_source {
            fixture = fixture
                .closed_window("HistoryQuery", "shared-key", 0, 10, |window| {
                    window.source(source).partition("shared-partition")
                })
                .expect("valid dense-scope history window");
        }
    }
    fixture.build()
}

fn history_direct_query_benchmarks(c: &mut Criterion) {
    let incompatible = create_incompatible_scope_history(HISTORY_DIRECT_QUERY_DISTINCT_SCOPES);
    assert!(incompatible.find_overlaps().is_empty());
    assert_eq!(
        incompatible
            .find_residuals(HISTORY_DIRECT_QUERY_TARGET_SOURCE)
            .len(),
        HISTORY_DIRECT_QUERY_DISTINCT_SCOPES / 2
    );

    let dense = create_dense_scope_history(HISTORY_DIRECT_QUERY_DENSE_RECORDS_PER_SOURCE);
    let dense_record_count = HISTORY_DIRECT_QUERY_DENSE_RECORDS_PER_SOURCE * 2;
    assert_eq!(
        dense.find_overlaps().len(),
        dense_record_count * (dense_record_count - 1) / 2
    );
    assert!(
        dense
            .find_residuals(HISTORY_DIRECT_QUERY_TARGET_SOURCE)
            .is_empty()
    );

    let mut group = c.benchmark_group("history_direct_queries");
    group.bench_function("find_overlaps_4096_incompatible_scopes", |b| {
        b.iter(|| black_box(&incompatible).find_overlaps());
    });
    group.bench_function("find_residuals_4096_incompatible_scopes", |b| {
        b.iter(|| black_box(&incompatible).find_residuals(HISTORY_DIRECT_QUERY_TARGET_SOURCE));
    });
    group.bench_function("find_overlaps_128_dense_same_scope", |b| {
        b.iter(|| black_box(&dense).find_overlaps());
    });
    group.bench_function("find_residuals_128_dense_same_scope", |b| {
        b.iter(|| black_box(&dense).find_residuals(HISTORY_DIRECT_QUERY_TARGET_SOURCE));
    });
    group.finish();
}

criterion_group!(
    benches,
    ingestion_benchmarks,
    comparison_benchmarks,
    dense_duplicate_cohort_benchmarks,
    cohort_activity_benchmarks,
    segment_cohort_benchmarks,
    transition_comparator_benchmarks,
    episode_benchmarks,
    history_direct_query_benchmarks
);
criterion_main!(benches);
