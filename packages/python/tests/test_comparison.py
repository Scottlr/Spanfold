import json
from dataclasses import dataclass
from datetime import UTC, datetime, timedelta

from spanfold import (
    AsOfDirection,
    AsOfMatchStatus,
    ClosedWindow,
    ComparisonChangelog,
    ComparisonComparatorBuilder,
    ComparisonComparatorCatalog,
    ComparisonDebugHtmlOptions,
    ComparisonDiagnosticSeverity,
    ComparisonDuplicateWindowPolicy,
    ComparisonExplanationFormat,
    ComparisonExportException,
    ComparisonExtensionBuilder,
    ComparisonExtensionMetadata,
    ComparisonFinality,
    ComparisonLlmContextOptions,
    ComparisonNormalizationBuilder,
    ComparisonNormalizationPolicy,
    ComparisonNullTimestampPolicy,
    ComparisonOpenWindowPolicy,
    ComparisonOutputOptions,
    ComparisonPlan,
    ComparisonPlanValidationCode,
    ComparisonResult,
    ComparisonRowFinality,
    ComparisonScope,
    ComparisonScopeBuilder,
    ComparisonSelector,
    ComparisonSide,
    ContainmentStatus,
    CoverageRow,
    LeadLagDirection,
    LeadLagTransition,
    MissingRow,
    ResidualRow,
    Spanfold,
    TemporalAxis,
    TemporalPoint,
    TemporalRange,
    total_covered_magnitude,
    total_position_length,
    total_target_magnitude,
    total_time_duration,
)
from spanfold.testing import WindowHistoryFixtureBuilder


@dataclass(frozen=True)
class DeviceStatus:
    device_id: str
    is_online: bool


@dataclass(frozen=True)
class TimedDeviceStatus:
    device_id: str
    is_online: bool
    occurred_at: datetime


def _pipeline():
    return (
        Spanfold.for_events()
        .record_windows()
        .track_window(
            "DeviceOffline",
            key=lambda event: event.device_id,
            is_active=lambda event: not event.is_online,
        )
    )


def test_comparison_plan_selectors_and_validation_match_csharp_surface() -> None:
    selector = ComparisonSelector.for_window_name("DeviceOffline").and_(
        ComparisonSelector.for_source("provider-a")
    )
    window = ClosedWindow(
        "DeviceOffline",
        "device-1",
        1,
        2,
        source="provider-a",
    )
    assert selector.matches(window)

    plan = ComparisonPlan(
        "Provider QA",
        ComparisonSelector.runtime_only("provider-a", "runtime provider selector"),
        [ComparisonSelector.serializable("provider-b", "source = provider-b")],
        ComparisonScope.window("DeviceOffline").segment("region", "north"),
        ComparisonNormalizationPolicy.default(),
        ["overlap"],
        ComparisonOutputOptions.default(),
    )

    diagnostics = plan.validate()
    assert not plan.is_serializable
    assert diagnostics[0].code is ComparisonPlanValidationCode.NON_SERIALIZABLE_SELECTOR
    assert diagnostics[0].path == "target"
    assert diagnostics[0].severity is ComparisonDiagnosticSeverity.WARNING


def test_overlap_residual_missing_and_coverage() -> None:
    pipeline = _pipeline()
    pipeline.ingest(DeviceStatus("device-1", False), source="provider-a")  # [1, 5)
    pipeline.ingest(DeviceStatus("noise", True), source="provider-a")
    pipeline.ingest(DeviceStatus("device-1", False), source="provider-b")  # [3, 7)
    pipeline.ingest(DeviceStatus("noise", True), source="provider-a")
    pipeline.ingest(DeviceStatus("device-1", True), source="provider-a")
    pipeline.ingest(DeviceStatus("noise", True), source="provider-a")
    pipeline.ingest(DeviceStatus("device-1", True), source="provider-b")

    result = (
        pipeline.history.compare("Provider QA")
        .target("provider-a")
        .against("provider-b")
        .within(window_name="DeviceOffline")
        .using("overlap", "residual", "missing", "coverage")
        .run()
    )

    assert result.plan is not None
    assert result.plan.name == "Provider QA"
    assert result.prepared is not None
    assert len(result.prepared.normalized_windows) == 2
    assert [
        (row.range.start.position, row.range.require_end().position) for row in result.overlap_rows
    ] == [(3, 5)]
    assert [
        (row.range.start.position, row.range.require_end().position) for row in result.residual_rows
    ] == [(1, 3)]
    assert [
        (row.range.start.position, row.range.require_end().position) for row in result.missing_rows
    ] == [(5, 7)]
    assert result.coverage_rows[0].target_magnitude == 4
    assert result.coverage_rows[0].covered_magnitude == 2
    assert result.coverage_rows[0].coverage_ratio == 0.5
    assert result.coverage_summaries[0].target_magnitude == 4
    assert result.coverage_summaries[0].covered_magnitude == 2
    assert result.coverage_summaries[0].coverage_ratio == 0.5
    assert len(result.final_row_finalities()) == 4
    assert not result.provisional_row_finalities()


def test_row_aggregation_helpers_sum_lengths_and_magnitudes() -> None:
    start = datetime(2026, 4, 20, 10, tzinfo=UTC)
    residual_rows = (
        ResidualRow(
            "DeviceOffline",
            "device-1",
            None,
            TemporalRange.closed(TemporalPoint.for_position(1), TemporalPoint.for_position(4)),
            (),
        ),
        ResidualRow(
            "DeviceOffline",
            "device-1",
            None,
            TemporalRange.closed(TemporalPoint.for_position(10), TemporalPoint.for_position(12)),
            (),
        ),
    )
    missing_rows = (
        MissingRow(
            "DeviceOffline",
            "device-1",
            None,
            TemporalRange.closed(
                TemporalPoint.for_timestamp(start),
                TemporalPoint.for_timestamp(start + timedelta(minutes=7)),
            ),
            (),
        ),
    )
    coverage_rows = (
        CoverageRow(
            "DeviceOffline",
            "device-1",
            None,
            TemporalRange.closed(TemporalPoint.for_position(1), TemporalPoint.for_position(4)),
            3,
            2,
            (),
            (),
        ),
        CoverageRow(
            "DeviceOffline",
            "device-1",
            None,
            TemporalRange.closed(TemporalPoint.for_position(4), TemporalPoint.for_position(6)),
            2,
            1,
            (),
            (),
        ),
    )

    assert total_position_length(residual_rows) == 5
    assert total_time_duration(missing_rows) == timedelta(minutes=7)
    assert total_position_length(coverage_rows) == 5
    assert total_target_magnitude(coverage_rows) == 5
    assert total_covered_magnitude(coverage_rows) == 3


def test_open_windows_are_excluded_until_horizon_is_explicit() -> None:
    pipeline = _pipeline()
    pipeline.ingest(DeviceStatus("device-1", False), source="provider-a")
    pipeline.ingest(DeviceStatus("device-1", False), source="provider-b")

    excluded = (
        pipeline.history.compare("Live")
        .target("provider-a")
        .against("provider-b")
        .within(window_name="DeviceOffline")
        .using("overlap")
        .run()
    )
    included = (
        pipeline.history.compare("Live")
        .target("provider-a")
        .against("provider-b")
        .within(window_name="DeviceOffline")
        .normalize(horizon=TemporalPoint.for_position(5))
        .using("overlap")
        .run()
    )

    assert not excluded.overlap_rows
    assert included.overlap_rows[0].finality is ComparisonFinality.PROVISIONAL
    assert included.overlap_rows[0].range.start.position == 2
    assert included.overlap_rows[0].range.require_end().position == 5


def test_run_live_clips_open_windows_to_horizon() -> None:
    pipeline = _pipeline()
    pipeline.ingest(DeviceStatus("device-1", False), source="provider-a")
    pipeline.ingest(DeviceStatus("device-1", False), source="provider-b")

    result = (
        pipeline.history.compare("Live")
        .target("provider-a")
        .against("provider-b")
        .within(window_name="DeviceOffline")
        .using("overlap")
        .run_live(TemporalPoint.for_position(5))
    )

    assert result.evaluation_horizon == TemporalPoint.for_position(5)
    assert result.overlap_rows[0].finality is ComparisonFinality.PROVISIONAL
    assert result.has_provisional_rows()


def test_result_exposes_row_finality_snapshot() -> None:
    pipeline = _pipeline()
    pipeline.ingest(DeviceStatus("device-1", False), source="provider-a")
    pipeline.ingest(DeviceStatus("device-1", False), source="provider-b")

    result = (
        pipeline.history.compare("Live")
        .target("provider-a")
        .against("provider-b")
        .within(window_name="DeviceOffline")
        .normalize(horizon=TemporalPoint.for_position(5))
        .using("overlap")
        .run()
    )

    finalities = result.row_finalities
    assert result.has_provisional_rows()
    assert len(result.provisional_row_finalities()) == 1
    assert not result.final_row_finalities()
    assert [(row.row_type, row.row_id, row.finality) for row in finalities] == [
        ("overlap", "overlap[0]", ComparisonFinality.PROVISIONAL)
    ]
    assert "open window" in finalities[0].reason


def test_comparison_changelog_revises_and_retracts_rows() -> None:
    previous = (
        ComparisonRowFinality(
            "missing",
            "missing[0]",
            ComparisonFinality.FINAL,
            "All contributing windows were closed when the row was produced.",
        ),
        ComparisonRowFinality(
            "overlap",
            "overlap[0]",
            ComparisonFinality.PROVISIONAL,
            "Depends on at least one open window clipped to the evaluation horizon.",
        ),
    )
    current = (
        ComparisonRowFinality(
            "overlap",
            "overlap[0]",
            ComparisonFinality.FINAL,
            "All contributing windows were closed when the row was produced.",
        ),
    )

    entries = ComparisonChangelog.create(previous, current)

    assert [(entry.row_type, entry.finality, entry.version) for entry in entries] == [
        ("overlap", ComparisonFinality.REVISED, 2),
        ("missing", ComparisonFinality.RETRACTED, 2),
    ]
    assert entries[0].supersedes_row_id == "overlap[0]"
    assert ComparisonChangelog.replay(previous, entries) == (
        ComparisonRowFinality(
            "overlap",
            "overlap[0]",
            ComparisonFinality.FINAL,
            entries[0].reason,
            2,
            "overlap[0]",
        ),
    )


def test_known_at_filters_windows_before_comparison_and_exports_metadata() -> None:
    pipeline = _pipeline()
    pipeline.ingest(DeviceStatus("device-1", False), source="provider-a")
    pipeline.ingest(DeviceStatus("device-1", False), source="provider-b")
    pipeline.ingest(DeviceStatus("device-1", True), source="provider-a")
    pipeline.ingest(DeviceStatus("device-1", True), source="provider-b")

    early = (
        pipeline.history.compare("Known-at QA")
        .target("provider-a")
        .against("provider-b")
        .within(window_name="DeviceOffline")
        .normalize(known_at=TemporalPoint.for_position(2))
        .using("overlap")
        .run()
    )
    known = (
        pipeline.history.compare("Known-at QA")
        .target("provider-a")
        .against("provider-b")
        .within(window_name="DeviceOffline")
        .normalize(known_at=TemporalPoint.for_position(4))
        .using("overlap")
        .run()
    )

    assert not early.overlap_rows
    assert "future_window_excluded" in early.diagnostics
    assert len(known.overlap_rows) == 1
    assert "future_window_excluded" not in known.diagnostics
    assert '"known_at": {' in known.to_json()
    assert "knownAt=4" in known.to_markdown()


def test_event_time_mode_diagnoses_missing_timestamps() -> None:
    pipeline = _pipeline()
    pipeline.ingest(DeviceStatus("device-1", False), source="provider-a")
    pipeline.ingest(DeviceStatus("device-1", True), source="provider-a")
    pipeline.ingest(DeviceStatus("device-1", False), source="provider-b")
    pipeline.ingest(DeviceStatus("device-1", True), source="provider-b")

    result = (
        pipeline.history.compare("Event-time QA")
        .target("provider-a")
        .against("provider-b")
        .within(window_name="DeviceOffline")
        .normalize(axis=TemporalAxis.TIMESTAMP)
        .using("overlap")
        .run()
    )

    assert not result.overlap_rows
    assert "missing_event_time" in result.diagnostics


def test_event_time_mode_compares_recorded_timestamps() -> None:
    start = datetime(2026, 4, 21, 10, tzinfo=UTC)
    pipeline = (
        Spanfold.for_events()
        .record_windows()
        .with_event_time(lambda event: event.occurred_at)
        .track_window(
            "DeviceOffline",
            key=lambda event: event.device_id,
            is_active=lambda event: not event.is_online,
        )
    )
    pipeline.ingest(TimedDeviceStatus("device-1", False, start), source="provider-a")
    pipeline.ingest(
        TimedDeviceStatus("device-1", False, start + timedelta(minutes=2)),
        source="provider-b",
    )
    pipeline.ingest(
        TimedDeviceStatus("device-1", True, start + timedelta(minutes=5)),
        source="provider-a",
    )
    pipeline.ingest(
        TimedDeviceStatus("device-1", True, start + timedelta(minutes=7)),
        source="provider-b",
    )

    result = (
        pipeline.history.compare("Event-time QA")
        .target("provider-a")
        .against("provider-b")
        .within(window_name="DeviceOffline")
        .normalize(axis=TemporalAxis.TIMESTAMP)
        .using("overlap")
        .run()
    )

    assert not result.diagnostics
    assert result.overlap_rows[0].range.start.timestamp == start + timedelta(minutes=2)
    assert result.overlap_rows[0].range.require_end().timestamp == start + timedelta(minutes=5)


def test_default_normalization_policy_matches_historical_comparison_defaults() -> None:
    policy = ComparisonNormalizationPolicy.default()

    assert policy.require_closed_windows
    assert policy.use_half_open_ranges
    assert policy.time_axis is TemporalAxis.PROCESSING_POSITION
    assert policy.open_window_policy is ComparisonOpenWindowPolicy.REQUIRE_CLOSED
    assert policy.open_window_horizon is None
    assert policy.null_timestamp_policy is ComparisonNullTimestampPolicy.REJECT
    assert not policy.coalesce_adjacent_windows
    assert policy.duplicate_window_policy is ComparisonDuplicateWindowPolicy.PRESERVE


def test_normalization_policy_modifiers_match_builder_surface() -> None:
    known_at = TemporalPoint.for_position(10)

    policy = (
        ComparisonNormalizationPolicy.event_time()
        .with_known_at(known_at)
        .coalescing_adjacent_windows()
        .rejecting_duplicate_windows()
    )

    assert policy.time_axis is TemporalAxis.TIMESTAMP
    assert policy.known_at == known_at
    assert policy.coalesce_adjacent_windows
    assert policy.duplicate_window_policy is ComparisonDuplicateWindowPolicy.REJECT


def test_normalize_accepts_policy_for_live_horizon() -> None:
    pipeline = _pipeline()
    pipeline.ingest(DeviceStatus("device-1", False), source="provider-a")
    pipeline.ingest(DeviceStatus("device-1", False), source="provider-b")

    result = (
        pipeline.history.compare("Live")
        .target("provider-a")
        .against("provider-b")
        .within(window_name="DeviceOffline")
        .normalize(ComparisonNormalizationPolicy.clip_open_windows_to(TemporalPoint.for_position(5)))
        .using("overlap")
        .run()
    )

    assert result.evaluation_horizon == TemporalPoint.for_position(5)
    assert result.overlap_rows[0].finality is ComparisonFinality.PROVISIONAL


def test_normalization_policy_can_select_event_time_and_missing_timestamp_policy() -> None:
    pipeline = _pipeline()
    pipeline.ingest(DeviceStatus("device-1", False), source="provider-a")
    pipeline.ingest(DeviceStatus("device-1", True), source="provider-a")
    pipeline.ingest(DeviceStatus("device-1", False), source="provider-b")
    pipeline.ingest(DeviceStatus("device-1", True), source="provider-b")

    result = (
        pipeline.history.compare("Event-time QA")
        .target("provider-a")
        .against("provider-b")
        .within(window_name="DeviceOffline")
        .normalize(
            ComparisonNormalizationPolicy.event_time(
                null_timestamp_policy=ComparisonNullTimestampPolicy.EXCLUDE
            )
        )
        .using("overlap")
        .run()
    )

    assert not result.overlap_rows
    assert not result.diagnostics


def test_normalization_can_reject_duplicate_windows() -> None:
    history = (
        WindowHistoryFixtureBuilder()
        .add_closed_window("DeviceOffline", "device-1", 1, 5, source="provider-a")
        .add_closed_window("DeviceOffline", "device-1", 1, 5, source="provider-a")
        .add_closed_window("DeviceOffline", "device-1", 1, 5, source="provider-b")
        .build()
    )

    result = (
        history.compare("Duplicate QA")
        .target("provider-a")
        .against("provider-b")
        .within(window_name="DeviceOffline")
        .normalize(duplicate_windows="reject")
        .using("overlap")
        .run()
    )

    assert len(result.overlap_rows) == 1
    assert "duplicate_window" in result.diagnostics


def test_normalization_can_coalesce_adjacent_windows() -> None:
    history = (
        WindowHistoryFixtureBuilder()
        .add_closed_window("DeviceOffline", "device-1", 1, 3, source="provider-a")
        .add_closed_window("DeviceOffline", "device-1", 3, 5, source="provider-a")
        .add_closed_window("DeviceOffline", "device-1", 1, 5, source="provider-b")
        .build()
    )

    result = (
        history.compare("Coalesce QA")
        .target("provider-a")
        .against("provider-b")
        .within(window_name="DeviceOffline")
        .normalize(coalesce_adjacent=True)
        .using("overlap")
        .run()
    )

    assert len(result.overlap_rows) == 1
    assert result.overlap_rows[0].range.start.position == 1
    assert result.overlap_rows[0].range.require_end().position == 5
    assert len(result.overlap_rows[0].target_record_ids) == 2


def test_exports_are_deterministic() -> None:
    pipeline = _pipeline()
    pipeline.ingest(DeviceStatus("device-1", False), source="provider-a")
    pipeline.ingest(DeviceStatus("device-1", False), source="provider-b")
    result = (
        pipeline.history.compare("Provider QA")
        .target("provider-a")
        .against("provider-b")
        .within(window_name="DeviceOffline")
        .normalize(horizon=TemporalPoint.for_position(3))
        .using("overlap")
        .run()
    )

    assert '"name": "Provider QA"' in result.to_json()
    assert '"row_finalities": [' in result.to_json()
    assert '"row_type": "overlap"' in result.to_json_lines()
    json_lines = [json.loads(line) for line in result.to_json_lines().splitlines()]
    assert json_lines[0]["artifact"] == "result-summary"
    assert json_lines[1]["artifact"] == "result-row"
    assert json_lines[1]["row_id"] == "overlap[0]"
    assert "| overlap | 1 |" in result.to_markdown()
    debug_html = result.to_debug_html()
    assert "<html" in debug_html
    assert "Diagnostics" in debug_html
    assert "Prepared" in debug_html
    assert "Aligned Segments" in debug_html
    assert "# Comparison Explain: Provider QA" in result.explain()
    assert str(result.overlap_rows[0].target_record_ids[0]) in result.explain()


def test_portable_exports_and_llm_context_are_deterministic(tmp_path) -> None:
    pipeline = _pipeline()
    pipeline.ingest(DeviceStatus("device-1", False), source="provider-a")
    pipeline.ingest(DeviceStatus("device-1", False), source="provider-b")
    pipeline.ingest(DeviceStatus("device-1", True), source="provider-b")
    pipeline.ingest(DeviceStatus("device-1", True), source="provider-a")

    result = (
        pipeline.history.compare("Provider QA")
        .target("provider-a")
        .against("provider-b")
        .within(window_name="DeviceOffline")
        .using("overlap")
        .run()
    )

    plan_json = json.loads(result.plan.export_json())
    result_json = json.loads(result.export_json())
    context = json.loads(result.export_llm_context())
    lines = [json.loads(line) for line in result.export_json_lines().splitlines()]

    assert plan_json["schema"] == "spanfold.comparison.plan"
    assert result_json["schema"] == "spanfold.comparison.result"
    assert result_json["rows"]["overlap"][0]["rowId"] == "overlap[0]"
    assert context["schema"] == "spanfold.comparison.llm-context"
    assert context["summary"]["planName"] == "Provider QA"
    assert context["summary"]["rowCounts"]["overlap"] == 1
    assert context["fullResult"]["schema"] == "spanfold.comparison.result"
    assert context["rowDocuments"][0]["artifact"] == "result-summary"
    assert context["rowDocuments"][1]["rowId"] == "overlap[0]"
    assert lines[0]["artifact"] == "result-summary"
    assert lines[1]["artifact"] == "result-row"

    destination = tmp_path / "nested" / "comparison.llm.json"
    result.to_llm_context(destination)
    assert "spanfold.comparison.llm-context" in destination.read_text(encoding="utf-8")


def test_builder_can_export_custom_output_options() -> None:
    pipeline = _pipeline()
    result = (
        pipeline.history.compare("Output QA")
        .target("provider-a")
        .against("provider-b")
        .within(window_name="DeviceOffline")
        .output(
            ComparisonOutputOptions(
                include_aligned_segments=False,
                include_explain_data=False,
            )
        )
        .using("overlap")
        .run()
    )

    plan_json = json.loads(result.plan.export_json())
    assert plan_json["output"] == {
        "includeAlignedSegments": False,
        "includeExplainData": False,
    }


def test_builder_can_write_configured_debug_and_llm_exports(tmp_path) -> None:
    pipeline = _pipeline()
    pipeline.ingest(DeviceStatus("device-1", False), source="provider-a")
    pipeline.ingest(DeviceStatus("device-1", False), source="provider-b")
    pipeline.ingest(DeviceStatus("device-1", True), source="provider-b")
    pipeline.ingest(DeviceStatus("device-1", True), source="provider-a")

    debug_path = tmp_path / "debug" / "provider-qa.html"
    llm_path = tmp_path / "llm" / "provider-qa.llm.json"
    result = (
        pipeline.history.compare("Provider QA")
        .target("provider-a")
        .against("provider-b")
        .within(window_name="DeviceOffline")
        .using("overlap")
        .run(
            ComparisonDebugHtmlOptions.to_file(debug_path),
            llm_context=ComparisonLlmContextOptions.to_file(llm_path),
        )
    )

    assert result.is_valid
    assert "Provider QA" in debug_path.read_text(encoding="utf-8")
    assert "spanfold.comparison.llm-context" in llm_path.read_text(encoding="utf-8")


def test_runtime_only_plan_export_raises_with_diagnostics() -> None:
    plan = ComparisonPlan(
        "Runtime selector QA",
        ComparisonSelector.runtime_only("target", "dynamic target"),
        (ComparisonSelector.for_source("provider-b"),),
        ComparisonScope.window("DeviceOffline"),
        ComparisonNormalizationPolicy.default(),
        ("overlap",),
        ComparisonOutputOptions.default(),
    )

    try:
        plan.export_json()
    except ComparisonExportException as exception:
        assert "runtime-only selectors" in str(exception)
        assert (
            exception.diagnostics[0].code
            is ComparisonPlanValidationCode.NON_SERIALIZABLE_SELECTOR
        )
    else:
        raise AssertionError("Expected ComparisonExportException.")


def test_builder_build_validate_prepare_and_alignment_match_csharp_surface() -> None:
    pipeline = _pipeline()
    pipeline.ingest(DeviceStatus("device-1", False), source="provider-a")
    pipeline.ingest(DeviceStatus("device-1", False), source="provider-b")
    pipeline.ingest(DeviceStatus("device-1", True), source="provider-a")
    pipeline.ingest(DeviceStatus("device-1", True), source="provider-b")

    builder = (
        pipeline.history.compare("Provider QA")
        .target("provider-a")
        .against("provider-b")
        .within(window_name="DeviceOffline")
        .using("overlap")
    )

    plan = builder.build()
    prepared = builder.prepare()
    aligned = prepared.align()

    assert plan.name == "Provider QA"
    assert not builder.validate()
    assert len(prepared.normalized_windows) == 2
    assert len(aligned.segments) == 3
    assert aligned.segments[1].target_record_ids
    assert aligned.segments[1].against_record_ids
    assert builder.run().aligned.segments == aligned.segments


def test_comparator_catalog_and_explanation_format_are_public() -> None:
    assert "overlap" in ComparisonComparatorCatalog.built_in_declarations
    assert ComparisonComparatorCatalog.is_built_in_declaration("containment")
    assert ComparisonComparatorCatalog.is_known_declaration("lead-lag:Start:ProcessingPosition:5")
    assert ComparisonComparatorCatalog.is_known_declaration("asof:Previous:ProcessingPosition:10")
    assert not ComparisonComparatorCatalog.is_known_declaration("quality:drift")

    plan = ComparisonPlan(
        "Plain text QA",
        ComparisonSelector.for_source("provider-a"),
        (ComparisonSelector.for_source("provider-b"),),
        ComparisonScope.window("DeviceOffline"),
        ComparisonNormalizationPolicy.default(),
        ("overlap",),
        ComparisonOutputOptions.default(),
    )

    assert plan.explain().startswith("# Comparison Plan: Plain text QA")
    assert plan.explain(ComparisonExplanationFormat.PLAIN_TEXT).startswith(
        "Comparison Plan: Plain text QA"
    )


def test_fluent_builder_objects_match_csharp_workflows() -> None:
    pipeline = _pipeline()
    pipeline.ingest(DeviceStatus("device-1", False), source="provider-a")
    pipeline.ingest(DeviceStatus("device-1", False), source="provider-b")

    prepared = (
        pipeline.history.compare("Live Provider QA")
        .target("provider-a")
        .against("provider-b")
        .within(lambda scope: scope.window("DeviceOffline"))
        .normalize(lambda normal: normal.clip_open_windows_to(TemporalPoint.for_position(5)))
        .using(lambda comparators: comparators.overlap().residual())
        .strict_if(False)
        .prepare_live(TemporalPoint.for_position(5))
    )

    assert prepared.plan.scope == ComparisonScopeBuilder().window("DeviceOffline")
    assert prepared.plan.normalization.open_window_horizon == TemporalPoint.for_position(5)
    assert prepared.plan.comparators == ("overlap", "residual")
    assert prepared.normalized_windows[0].range.require_end() == TemporalPoint.for_position(5)

    declarations = (
        ComparisonComparatorBuilder()
        .gap()
        .symmetric_difference()
        .containment()
        .lead_lag(LeadLagTransition.START, TemporalAxis.PROCESSING_POSITION, 5)
        .as_of(AsOfDirection.PREVIOUS, TemporalAxis.PROCESSING_POSITION, 10)
        .declaration("quality:drift")
        .build()
    )
    assert declarations[-1] == "quality:drift"
    assert ComparisonNormalizationBuilder().known_at_position(3).build().known_at == (
        TemporalPoint.for_position(3)
    )


def test_comparison_extension_descriptor_builder() -> None:
    descriptor = (
        ComparisonExtensionBuilder("experimental-quality", "Experimental Quality")
        .add_selector("region", "Selects a region-scoped window.")
        .add_comparator("quality:drift", "Compares quality drift windows.")
        .add_metadata_key("driftThreshold")
        .build()
    )

    assert descriptor.id == "experimental-quality"
    assert descriptor.selectors[0].name == "region"
    assert descriptor.comparators[0].declaration == "quality:drift"
    assert descriptor.metadata_keys == ("driftThreshold",)


def test_result_export_and_explain_include_extension_metadata() -> None:
    result = ComparisonResult(
        "Extension QA",
        extension_metadata=(
            ComparisonExtensionMetadata("experimental-quality", "driftThreshold", "0.025"),
        ),
    )

    assert '"extension_metadata": [' in result.to_json()
    assert "experimental-quality" in result.to_json()
    assert "extensionMetadata[0]: experimental-quality.driftThreshold=0.025" in result.explain()
    assert result.explain(markdown=False).startswith("Comparison Explain: Extension QA")


def test_gap_detects_internal_uncovered_space() -> None:
    pipeline = _pipeline()
    pipeline.ingest(DeviceStatus("device-1", False), source="provider-a")  # [1, 3)
    pipeline.ingest(DeviceStatus("noise", True), source="provider-a")
    pipeline.ingest(DeviceStatus("device-1", True), source="provider-a")
    pipeline.ingest(DeviceStatus("noise", True), source="provider-a")
    pipeline.ingest(DeviceStatus("device-1", False), source="provider-b")  # [5, 7)
    pipeline.ingest(DeviceStatus("noise", True), source="provider-a")
    pipeline.ingest(DeviceStatus("device-1", True), source="provider-b")

    result = (
        pipeline.history.compare("Provider QA")
        .target("provider-a")
        .against("provider-b")
        .within(window_name="DeviceOffline")
        .using("gap")
        .run()
    )

    assert _ranges(result.gap_rows) == [(3, 5)]
    assert result.comparator_summaries[0].row_count == 1


def test_gap_does_not_invent_boundary_gaps() -> None:
    pipeline = _pipeline()
    pipeline.ingest(DeviceStatus("device-1", False), source="provider-a")
    pipeline.ingest(DeviceStatus("device-1", True), source="provider-a")

    result = (
        pipeline.history.compare("Provider QA")
        .target("provider-a")
        .against("provider-b")
        .within(window_name="DeviceOffline")
        .using("gap")
        .run()
    )

    assert not result.gap_rows


def test_symmetric_difference_includes_both_disagreement_sides() -> None:
    pipeline = _pipeline()
    pipeline.ingest(DeviceStatus("device-1", False), source="provider-a")  # [1, 5)
    pipeline.ingest(DeviceStatus("noise", True), source="provider-a")
    pipeline.ingest(DeviceStatus("device-1", False), source="provider-b")  # [3, 7)
    pipeline.ingest(DeviceStatus("noise", True), source="provider-a")
    pipeline.ingest(DeviceStatus("device-1", True), source="provider-a")
    pipeline.ingest(DeviceStatus("noise", True), source="provider-a")
    pipeline.ingest(DeviceStatus("device-1", True), source="provider-b")

    result = (
        pipeline.history.compare("Provider QA")
        .target("provider-a")
        .against("provider-b")
        .within(window_name="DeviceOffline")
        .using("symmetric-difference")
        .run()
    )

    assert [row.side for row in result.symmetric_difference_rows] == [
        ComparisonSide.TARGET,
        ComparisonSide.AGAINST,
    ]
    assert _ranges(result.symmetric_difference_rows) == [(1, 3), (5, 7)]


def test_containment_splits_overhangs_and_contained_ranges() -> None:
    pipeline = _pipeline()
    pipeline.ingest(DeviceStatus("device-1", False), source="target")  # [1, 7)
    pipeline.ingest(DeviceStatus("noise", True), source="target")
    pipeline.ingest(DeviceStatus("device-1", False), source="container")  # [3, 5)
    pipeline.ingest(DeviceStatus("noise", True), source="target")
    pipeline.ingest(DeviceStatus("device-1", True), source="container")
    pipeline.ingest(DeviceStatus("noise", True), source="target")
    pipeline.ingest(DeviceStatus("device-1", True), source="target")

    result = (
        pipeline.history.compare("Containment QA")
        .target("target")
        .against("container")
        .within(window_name="DeviceOffline")
        .using("containment")
        .run()
    )

    assert [row.status for row in result.containment_rows] == [
        ContainmentStatus.LEFT_OVERHANG,
        ContainmentStatus.CONTAINED,
        ContainmentStatus.RIGHT_OVERHANG,
    ]
    assert _ranges(result.containment_rows) == [(1, 3), (3, 5), (5, 7)]


def test_multiple_containers_contribute_contained_rows() -> None:
    pipeline = _pipeline()
    pipeline.ingest(DeviceStatus("device-1", False), source="target")  # [1, 7)
    pipeline.ingest(DeviceStatus("device-1", False), source="container")  # [2, 4)
    pipeline.ingest(DeviceStatus("device-1", True), source="container")
    pipeline.ingest(DeviceStatus("device-1", False), source="container")  # [4, 6)
    pipeline.ingest(DeviceStatus("device-1", True), source="container")
    pipeline.ingest(DeviceStatus("device-1", True), source="target")

    result = (
        pipeline.history.compare("Containment QA")
        .target("target")
        .against("container")
        .within(window_name="DeviceOffline")
        .using("containment")
        .run()
    )

    assert sum(
        1 for row in result.containment_rows if row.status is ContainmentStatus.CONTAINED
    ) == 2


def test_lead_lag_reports_target_leads_against() -> None:
    pipeline = _pipeline()
    pipeline.ingest(DeviceStatus("device-1", False), source="target")  # start 1
    pipeline.ingest(DeviceStatus("noise", True), source="target")
    pipeline.ingest(DeviceStatus("device-1", False), source="comparison")  # start 3

    result = (
        pipeline.history.compare("Latency QA")
        .target("target")
        .against("comparison")
        .within(window_name="DeviceOffline")
        .normalize(horizon=TemporalPoint.for_position(6))
        .using("lead-lag:start:processing-position:5")
        .run()
    )

    row = result.lead_lag_rows[0]
    assert row.transition is LeadLagTransition.START
    assert row.direction is LeadLagDirection.TARGET_LEADS
    assert row.delta_magnitude == -2
    assert row.is_within_tolerance
    assert result.lead_lag_summaries[0].target_lead_count == 1


def test_lead_lag_reports_target_lags_and_outside_tolerance() -> None:
    pipeline = _pipeline()
    pipeline.ingest(DeviceStatus("device-1", False), source="comparison")  # start 1
    pipeline.ingest(DeviceStatus("noise", True), source="comparison")
    pipeline.ingest(DeviceStatus("noise", True), source="comparison")
    pipeline.ingest(DeviceStatus("noise", True), source="comparison")
    pipeline.ingest(DeviceStatus("device-1", False), source="target")  # start 5

    result = (
        pipeline.history.compare("Latency QA")
        .target("target")
        .against("comparison")
        .within(window_name="DeviceOffline")
        .normalize(horizon=TemporalPoint.for_position(7))
        .using("lead_lag:start:processing_position:1")
        .run()
    )

    row = result.lead_lag_rows[0]
    assert row.direction is LeadLagDirection.TARGET_LAGS
    assert row.delta_magnitude == 4
    assert not row.is_within_tolerance
    assert result.lead_lag_summaries[0].outside_tolerance_count == 1


def test_lead_lag_can_compare_end_transitions() -> None:
    pipeline = _pipeline()
    pipeline.ingest(DeviceStatus("device-1", False), source="comparison")
    pipeline.ingest(DeviceStatus("device-1", False), source="target")
    pipeline.ingest(DeviceStatus("noise", True), source="target")
    pipeline.ingest(DeviceStatus("device-1", True), source="comparison")  # end 4
    pipeline.ingest(DeviceStatus("noise", True), source="target")
    pipeline.ingest(DeviceStatus("device-1", True), source="target")  # end 6

    result = (
        pipeline.history.compare("Latency QA")
        .target("target")
        .against("comparison")
        .within(window_name="DeviceOffline")
        .using("lead-lag:end:processing-position:5")
        .run()
    )

    row = result.lead_lag_rows[0]
    assert row.transition is LeadLagTransition.END
    assert row.direction is LeadLagDirection.TARGET_LAGS
    assert row.delta_magnitude == 2


def test_lead_lag_reports_missing_comparison_transition() -> None:
    pipeline = _pipeline()
    pipeline.ingest(DeviceStatus("device-1", False), source="target")

    result = (
        pipeline.history.compare("Latency QA")
        .target("target")
        .against("comparison")
        .within(window_name="DeviceOffline")
        .normalize(horizon=TemporalPoint.for_position(5))
        .using("lead-lag:start:processing-position:5")
        .run()
    )

    row = result.lead_lag_rows[0]
    assert row.direction is LeadLagDirection.MISSING_COMPARISON
    assert row.delta_magnitude is None
    assert row.comparison_point is None
    assert not row.is_within_tolerance


def test_as_of_emits_previous_match_within_tolerance() -> None:
    pipeline = _pipeline()
    pipeline.ingest(DeviceStatus("device-1", False), source="quote")  # start 1
    pipeline.ingest(DeviceStatus("noise", True), source="quote")
    pipeline.ingest(DeviceStatus("device-1", False), source="trade")  # start 3

    result = (
        pipeline.history.compare("Quote at trade")
        .target("trade")
        .against("quote")
        .within(window_name="DeviceOffline")
        .normalize(horizon=TemporalPoint.for_position(6))
        .using("asof:previous:processing-position:5")
        .run()
    )

    row = result.as_of_rows[0]
    assert row.direction is AsOfDirection.PREVIOUS
    assert row.status is AsOfMatchStatus.MATCHED
    assert row.distance_magnitude == 2
    assert row.matched_point is not None
    assert row.matched_point.position == 1


def test_as_of_rejects_future_match_for_previous_direction() -> None:
    pipeline = _pipeline()
    pipeline.ingest(DeviceStatus("device-1", False), source="trade")  # start 1
    pipeline.ingest(DeviceStatus("device-1", False), source="quote")  # start 2

    result = (
        pipeline.history.compare("Quote at trade")
        .target("trade")
        .against("quote")
        .within(window_name="DeviceOffline")
        .normalize(horizon=TemporalPoint.for_position(4))
        .using("asof:previous:processing-position:5")
        .run()
    )

    row = result.as_of_rows[0]
    assert row.status is AsOfMatchStatus.FUTURE_REJECTED
    assert row.distance_magnitude == 1
    assert row.matched_record_id is None


def test_as_of_can_match_next_transition() -> None:
    pipeline = _pipeline()
    pipeline.ingest(DeviceStatus("device-1", False), source="trade")  # start 1
    pipeline.ingest(DeviceStatus("noise", True), source="trade")
    pipeline.ingest(DeviceStatus("device-1", False), source="quote")  # start 3

    result = (
        pipeline.history.compare("Quote at trade")
        .target("trade")
        .against("quote")
        .within(window_name="DeviceOffline")
        .normalize(horizon=TemporalPoint.for_position(5))
        .using("asof:next:processing-position:5")
        .run()
    )

    assert result.as_of_rows[0].status is AsOfMatchStatus.MATCHED
    assert result.as_of_rows[0].distance_magnitude == 2


def test_as_of_reports_no_match_outside_tolerance() -> None:
    pipeline = _pipeline()
    pipeline.ingest(DeviceStatus("device-1", False), source="quote")  # start 1
    pipeline.ingest(DeviceStatus("noise", True), source="quote")
    pipeline.ingest(DeviceStatus("noise", True), source="quote")
    pipeline.ingest(DeviceStatus("noise", True), source="quote")
    pipeline.ingest(DeviceStatus("device-1", False), source="trade")  # start 5

    result = (
        pipeline.history.compare("Quote at trade")
        .target("trade")
        .against("quote")
        .within(window_name="DeviceOffline")
        .normalize(horizon=TemporalPoint.for_position(7))
        .using("asof:previous:processing-position:2")
        .run()
    )

    assert result.as_of_rows[0].status is AsOfMatchStatus.NO_MATCH
    assert result.as_of_rows[0].distance_magnitude == 4


def test_as_of_nearest_ambiguous_match_is_diagnosed() -> None:
    pipeline = _pipeline()
    pipeline.ingest(DeviceStatus("device-1", False), source="quote-a")  # start 1
    pipeline.ingest(DeviceStatus("noise", True), source="quote-a")
    pipeline.ingest(DeviceStatus("device-1", False), source="trade")  # start 3
    pipeline.ingest(DeviceStatus("noise", True), source="quote-a")
    pipeline.ingest(DeviceStatus("device-1", False), source="quote-b")  # start 5

    result = (
        pipeline.history.compare("Quote at trade")
        .target("trade")
        .against(predicate=lambda window: window.source in {"quote-a", "quote-b"})
        .within(window_name="DeviceOffline")
        .normalize(horizon=TemporalPoint.for_position(7))
        .using("asof:nearest:processing-position:5")
        .run()
    )

    assert result.as_of_rows[0].status is AsOfMatchStatus.AMBIGUOUS
    assert result.as_of_rows[0].distance_magnitude == 2
    assert "ambiguous_as_of_match" in result.diagnostics
    assert "future_leakage_risk" in result.diagnostics


def test_diagnostics_expose_structured_severity_and_strict_promotion() -> None:
    pipeline = _pipeline()
    pipeline.ingest(DeviceStatus("device-1", False), source="provider-a")
    pipeline.ingest(DeviceStatus("noise", True), source="provider-a")
    pipeline.ingest(DeviceStatus("device-1", False), source="provider-b")

    result = (
        pipeline.history.compare("As-of QA")
        .target("provider-a")
        .against("provider-b")
        .within(window_name="DeviceOffline")
        .normalize(horizon=TemporalPoint.for_position(5))
        .using("asof:next:processing-position:10")
        .run()
    )
    strict = (
        pipeline.history.compare("As-of QA")
        .target("provider-a")
        .against("provider-b")
        .within(window_name="DeviceOffline")
        .normalize(horizon=TemporalPoint.for_position(5))
        .using("asof:next:processing-position:10")
        .strict()
        .run()
    )

    assert result.is_valid
    assert result.diagnostic_rows[0].code == "future_leakage_risk"
    assert result.diagnostic_rows[0].severity is ComparisonDiagnosticSeverity.WARNING
    assert [row.code for row in result.warning_diagnostics()] == ["future_leakage_risk"]
    assert not result.error_diagnostics()
    assert '"diagnostic_rows": [' in result.to_json()
    assert not strict.is_valid
    assert strict.diagnostic_rows[0].severity is ComparisonDiagnosticSeverity.ERROR
    assert [row.code for row in strict.error_diagnostics()] == ["future_leakage_risk"]


def test_mixed_timestamp_clocks_are_diagnosed() -> None:
    start = datetime(2026, 4, 20, 10, tzinfo=UTC)
    pipeline = _pipeline()

    result = (
        pipeline.history.compare("Clock QA")
        .target("provider-a")
        .against("provider-b")
        .within(window_name="DeviceOffline")
        .normalize(
            axis=TemporalAxis.TIMESTAMP,
            horizon=TemporalPoint.for_timestamp(start, "provider"),
            known_at=TemporalPoint.for_timestamp(start, "received"),
        )
        .using("overlap")
        .run()
    )
    strict = (
        pipeline.history.compare("Clock QA")
        .target("provider-a")
        .against("provider-b")
        .within(window_name="DeviceOffline")
        .normalize(
            axis=TemporalAxis.TIMESTAMP,
            horizon=TemporalPoint.for_timestamp(start, "provider"),
            known_at=TemporalPoint.for_timestamp(start, "received"),
        )
        .using("overlap")
        .strict()
        .run()
    )

    mixed = [row for row in result.diagnostic_rows if row.code == "mixed_clock_risk"]
    strict_mixed = [row for row in strict.diagnostic_rows if row.code == "mixed_clock_risk"]
    assert mixed[0].severity is ComparisonDiagnosticSeverity.WARNING
    assert strict_mixed[0].severity is ComparisonDiagnosticSeverity.ERROR


def test_unbounded_open_window_duration_is_diagnosed() -> None:
    pipeline = _pipeline()
    pipeline.ingest(DeviceStatus("device-1", False), source="provider-a")

    result = (
        pipeline.history.compare("Open QA")
        .target("provider-a")
        .against("provider-b")
        .within(window_name="DeviceOffline")
        .using("overlap")
        .run()
    )

    assert not result.is_valid
    assert "open_windows_without_policy" in result.diagnostics
    assert "unbounded_open_duration" in result.diagnostics


def test_run_live_does_not_mutate_builder_normalization() -> None:
    pipeline = _pipeline()
    pipeline.ingest(DeviceStatus("device-1", False), source="provider-a")
    pipeline.ingest(DeviceStatus("device-1", False), source="provider-b")

    builder = (
        pipeline.history.compare("Live QA")
        .target("provider-a")
        .against("provider-b")
        .within(window_name="DeviceOffline")
        .using(lambda comparators: comparators.residual())
    )

    builder.run_live(TemporalPoint.for_position(5))

    plan = builder.build()
    assert plan.normalization.open_window_horizon is None
    assert plan.normalization.open_window_policy is ComparisonOpenWindowPolicy.REQUIRE_CLOSED


def test_export_json_lines_can_be_iterated_without_materializing_payloads() -> None:
    pipeline = _pipeline()
    pipeline.ingest(DeviceStatus("device-1", False), source="provider-a")
    pipeline.ingest(DeviceStatus("device-1", True), source="provider-a")
    pipeline.ingest(DeviceStatus("device-1", False), source="provider-b")
    pipeline.ingest(DeviceStatus("device-1", True), source="provider-b")

    result = (
        pipeline.history.compare("JSONL QA")
        .target("provider-a")
        .against("provider-b")
        .within(window_name="DeviceOffline")
        .using(lambda comparators: comparators.overlap())
        .run()
    )

    iterated = list(result.iter_export_json_lines())
    assert iterated == result.export_json_lines().splitlines()
    assert json.loads(iterated[0])["artifact"] == "result-summary"


def _ranges(rows) -> list[tuple[int, int]]:  # type: ignore[no-untyped-def]
    return [(row.range.start.position, row.range.require_end().position) for row in rows]
