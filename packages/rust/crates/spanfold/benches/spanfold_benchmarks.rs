use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use spanfold::{
    CohortActivity, PrimitiveValue, TemporalPoint, WindowHistory, WindowSegment, WindowTag,
    export_result_json, export_result_llm_context, for_events,
};

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
        .build();
    let mut occurrences = vec![0_usize; device_count * source_count];
    for event_index in 0..event_count {
        let device_index = event_index % device_count;
        let source_index = (event_index / device_count) % source_count;
        let occurrence_index = device_index * source_count + source_index;
        let occurrence = occurrences[occurrence_index];
        occurrences[occurrence_index] += 1;
        let source = format!("provider-{source_index}");
        pipeline.ingest(
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
                    WindowSegment::new("market", signal.market_id.clone()),
                    WindowSegment::new("fixture", signal.fixture_id.clone()),
                    WindowSegment::new("phase", signal.phase.clone()),
                    WindowSegment::new("period", signal.period.clone()).with_parent("phase"),
                ]
            },
            |signal| vec![WindowTag::new("state", signal.state.clone())],
        )
        .roll_up(
            "MarketPriced",
            |signal| signal.market_id.clone(),
            |children| children.any_active(),
        )
        .build();

    for event_index in 0..event_count {
        let selection_index = event_index % selection_count;
        let source_index = (event_index / selection_count) % source_count;
        let occurrence = event_index / (selection_count * source_count);
        let source = format!("provider-{source_index}");
        pipeline.ingest(
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

fn comparison_builder(history: &WindowHistory) -> spanfold::WindowComparisonBuilder<'_> {
    history
        .compare("Benchmark Provider QA")
        .target_source("provider-0")
        .against_source("provider-1")
        .scope_window("DeviceOffline")
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
    for event_count in [128, 1_024, 8_192] {
        group.bench_function(format!("ingest_{event_count}"), |b| {
            b.iter(|| create_comparison_data(black_box(event_count), 128, 2));
        });
    }
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
        .run_live(live_horizon);

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
                .run_live(live_horizon)
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

criterion_group!(
    benches,
    ingestion_benchmarks,
    comparison_benchmarks,
    segment_cohort_benchmarks
);
criterion_main!(benches);
