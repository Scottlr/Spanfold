"""Window comparison planning, row models, and deterministic exports."""

from __future__ import annotations

import html
import json
from collections import defaultdict
from collections.abc import Callable, Iterable, Iterator, Mapping
from dataclasses import asdict, dataclass, replace
from datetime import timedelta
from enum import Enum
from pathlib import Path
from typing import Any, cast

from spanfold.records import (
    ClosedWindow,
    WindowHistory,
    WindowRecord,
    WindowRecordId,
    WindowSegment,
)
from spanfold.temporal import TemporalAxis, TemporalPoint, TemporalRange


class ComparisonFinality(Enum):
    """Describes whether a comparison row is final or horizon-provisional."""

    FINAL = "final"
    PROVISIONAL = "provisional"
    REVISED = "revised"
    RETRACTED = "retracted"


class ComparisonDiagnosticSeverity(Enum):
    """Describes whether a comparison diagnostic is advisory or blocking."""

    WARNING = "warning"
    ERROR = "error"


ComparisonPlanDiagnosticSeverity = ComparisonDiagnosticSeverity


class ComparisonPlanValidationCode(Enum):
    """Identifies a validation diagnostic produced by a comparison plan."""

    UNKNOWN = "unknown"
    MISSING_NAME = "missing_name"
    MISSING_TARGET = "missing_target"
    MISSING_AGAINST = "missing_against"
    MISSING_COMPARATOR = "missing_comparator"
    NON_SERIALIZABLE_SELECTOR = "non_serializable_selector"
    MISSING_SCOPE = "missing_scope"
    MIXED_TIME_AXES = "mixed_time_axes"
    OPEN_WINDOWS_WITHOUT_POLICY = "open_windows_without_policy"
    MISSING_EVENT_TIME = "missing_event_time"
    INVALID_RANGE_DURATION = "invalid_range_duration"
    CLIPPED_WINDOW = "clipped_window"
    UNKNOWN_COMPARATOR = "unknown_comparator"
    AMBIGUOUS_AS_OF_MATCH = "ambiguous_as_of_match"
    MISSING_LINEAGE = "missing_lineage"
    FUTURE_WINDOW_EXCLUDED = "future_window_excluded"
    KNOWN_AT_REQUIRES_PROCESSING_POSITION = "known_at_requires_processing_position"
    RUNTIME_NON_SERIALIZABLE_PLAN = "runtime_non_serializable_plan"
    BROAD_SELECTOR = "broad_selector"
    FUTURE_LEAKAGE_RISK = "future_leakage_risk"
    LIVE_FINALITY_WITHOUT_HORIZON = "live_finality_without_horizon"
    UNBOUNDED_OPEN_DURATION = "unbounded_open_duration"
    MIXED_CLOCK_RISK = "mixed_clock_risk"


class ComparisonExplanationFormat(Enum):
    """Describes the text format used for deterministic explain output."""

    PLAIN_TEXT = "plain_text"
    MARKDOWN = "markdown"


class ComparisonSide(Enum):
    """Identifies which side is active for a directional comparison row."""

    TARGET = "target"
    AGAINST = "against"


class ComparisonOpenWindowPolicy(Enum):
    """Describes how open windows are handled during normalization."""

    REQUIRE_CLOSED = "require_closed"
    CLIP_TO_HORIZON = "clip_to_horizon"


class ComparisonNullTimestampPolicy(Enum):
    """Describes how event-time normalization handles missing timestamps."""

    REJECT = "reject"
    EXCLUDE = "exclude"


class ComparisonDuplicateWindowPolicy(Enum):
    """Describes how duplicate normalized windows are handled."""

    PRESERVE = "preserve"
    REJECT = "reject"


class ContainmentStatus(Enum):
    """Describes whether target windows are contained by comparison windows."""

    CONTAINED = "contained"
    NOT_CONTAINED = "not_contained"
    LEFT_OVERHANG = "left_overhang"
    RIGHT_OVERHANG = "right_overhang"


class LeadLagTransition(Enum):
    """Describes which window transition is used for lead/lag measurement."""

    START = "start"
    END = "end"


class LeadLagDirection(Enum):
    """Describes the ordering relationship between target and comparison."""

    EQUAL = "equal"
    TARGET_LEADS = "target_leads"
    TARGET_LAGS = "target_lags"
    MISSING_COMPARISON = "missing_comparison"


class AsOfDirection(Enum):
    """Describes which comparison transition is eligible for as-of lookup."""

    PREVIOUS = "previous"
    NEXT = "next"
    NEAREST = "nearest"


class AsOfMatchStatus(Enum):
    """Describes the outcome of an as-of lookup."""

    EXACT = "exact"
    MATCHED = "matched"
    NO_MATCH = "no_match"
    FUTURE_REJECTED = "future_rejected"
    AMBIGUOUS = "ambiguous"


class ComparisonComparatorCatalog:
    """Describes comparator declarations understood by core Spanfold."""

    built_in_declarations = (
        "overlap",
        "residual",
        "missing",
        "coverage",
        "gap",
        "symmetric-difference",
        "containment",
    )
    _normalized_built_ins = (
        "overlap",
        "residual",
        "missing",
        "coverage",
        "gap",
        "symmetric_difference",
        "containment",
    )

    @classmethod
    def is_built_in_declaration(cls, declaration: str) -> bool:
        """Return whether the declaration is an exact built-in comparator name."""

        _require_text(declaration, "Comparator declaration")
        return declaration in cls.built_in_declarations

    @classmethod
    def is_known_declaration(cls, declaration: str) -> bool:
        """Return whether core Spanfold can execute the comparator declaration."""

        _require_text(declaration, "Comparator declaration")
        normalized = _normalize_comparator(declaration)
        return (
            cls.is_built_in_declaration(declaration)
            or normalized in cls._normalized_built_ins
            or _try_parse_lead_lag(normalized) is not None
            or _try_parse_as_of(normalized) is not None
        )


class HierarchyComparisonRowKind(Enum):
    """Describes how a parent/child hierarchy segment is interpreted."""

    PARENT_EXPLAINED = "parent_explained"
    UNEXPLAINED_PARENT = "unexplained_parent"
    ORPHAN_CHILD = "orphan_child"


@dataclass(frozen=True, slots=True)
class ComparisonNormalizationPolicy:
    """Describes how recorded windows are normalized before comparison."""

    require_closed_windows: bool = True
    use_half_open_ranges: bool = True
    time_axis: TemporalAxis = TemporalAxis.PROCESSING_POSITION
    open_window_policy: ComparisonOpenWindowPolicy = ComparisonOpenWindowPolicy.REQUIRE_CLOSED
    open_window_horizon: TemporalPoint | None = None
    null_timestamp_policy: ComparisonNullTimestampPolicy = ComparisonNullTimestampPolicy.REJECT
    coalesce_adjacent_windows: bool = False
    duplicate_window_policy: ComparisonDuplicateWindowPolicy = (
        ComparisonDuplicateWindowPolicy.PRESERVE
    )
    known_at: TemporalPoint | None = None

    def __post_init__(self) -> None:
        if not self.use_half_open_ranges:
            msg = "Only half-open ranges are supported."
            raise ValueError(msg)
        if (
            self.open_window_policy is ComparisonOpenWindowPolicy.CLIP_TO_HORIZON
            and self.open_window_horizon is None
        ):
            msg = "open_window_horizon is required when clipping open windows."
            raise ValueError(msg)
        if (
            self.open_window_policy is ComparisonOpenWindowPolicy.REQUIRE_CLOSED
            and self.open_window_horizon is not None
        ):
            msg = "open_window_horizon cannot be set when open windows require closure."
            raise ValueError(msg)

    @classmethod
    def default(cls) -> ComparisonNormalizationPolicy:
        """Return the default historical comparison normalization policy."""

        return cls()

    @classmethod
    def require_closed(cls) -> ComparisonNormalizationPolicy:
        """Return a policy that excludes open windows from historical comparison."""

        return cls()

    @classmethod
    def clip_open_windows_to(
        cls,
        horizon: TemporalPoint,
        *,
        time_axis: TemporalAxis | None = None,
    ) -> ComparisonNormalizationPolicy:
        """Return a policy that clips open windows to an explicit horizon."""

        return cls(
            require_closed_windows=False,
            time_axis=time_axis or horizon.axis,
            open_window_policy=ComparisonOpenWindowPolicy.CLIP_TO_HORIZON,
            open_window_horizon=horizon,
        )

    @classmethod
    def event_time(
        cls,
        *,
        null_timestamp_policy: ComparisonNullTimestampPolicy = (
            ComparisonNullTimestampPolicy.REJECT
        ),
    ) -> ComparisonNormalizationPolicy:
        """Return a policy that normalizes on the event-time axis."""

        return cls(
            time_axis=TemporalAxis.TIMESTAMP,
            null_timestamp_policy=null_timestamp_policy,
        )

    def with_known_at(self, point: TemporalPoint) -> ComparisonNormalizationPolicy:
        """Return this policy with a known-at availability point."""

        return self._replace(known_at=point)

    def coalescing_adjacent_windows(self) -> ComparisonNormalizationPolicy:
        """Return this policy with adjacent-window coalescing enabled."""

        return self._replace(coalesce_adjacent_windows=True)

    def rejecting_duplicate_windows(self) -> ComparisonNormalizationPolicy:
        """Return this policy with duplicate normalized windows rejected."""

        return self._replace(
            duplicate_window_policy=ComparisonDuplicateWindowPolicy.REJECT
        )

    def _replace(self, **changes: Any) -> ComparisonNormalizationPolicy:
        return replace(self, **changes)


@dataclass(frozen=True, slots=True)
class WindowSegmentFilter:
    """Describes a segment value required by a comparison scope."""

    name: str
    value: Any


@dataclass(frozen=True, slots=True)
class WindowTagFilter:
    """Describes a tag value required by a comparison scope."""

    name: str
    value: Any


@dataclass(frozen=True, slots=True)
class ComparisonScope:
    """Describes the temporal scope for a comparison plan."""

    window_name: str | None = None
    time_axis: TemporalAxis = TemporalAxis.PROCESSING_POSITION
    segment_filters: tuple[WindowSegmentFilter, ...] = ()
    tag_filters: tuple[WindowTagFilter, ...] = ()

    @classmethod
    def all(cls) -> ComparisonScope:
        """Create an unrestricted processing-position scope."""

        return cls()

    @classmethod
    def window(
        cls,
        window_name: str,
        time_axis: TemporalAxis = TemporalAxis.PROCESSING_POSITION,
    ) -> ComparisonScope:
        """Create a scope restricted to one window name."""

        _require_text(window_name, "Window name")
        return cls(window_name, time_axis)

    def segment(self, name: str, value: Any) -> ComparisonScope:
        """Return a new scope with a segment filter appended."""

        _require_text(name, "Segment name")
        return replace(
            self,
            segment_filters=(*self.segment_filters, WindowSegmentFilter(name, value)),
        )

    def tag(self, name: str, value: Any) -> ComparisonScope:
        """Return a new scope with a tag filter appended."""

        _require_text(name, "Tag name")
        return replace(self, tag_filters=(*self.tag_filters, WindowTagFilter(name, value)))


class ComparisonScopeBuilder:
    """Builds comparison scopes."""

    def all(self) -> ComparisonScope:
        """Return an unrestricted processing-position comparison scope."""

        return ComparisonScope.all()

    def window(self, window_name: str) -> ComparisonScope:
        """Return a scope restricted to one configured window."""

        return ComparisonScope.window(window_name)


class ComparisonNormalizationBuilder:
    """Builds normalization policy for a comparison plan."""

    def __init__(self) -> None:
        self._policy = ComparisonNormalizationPolicy.default()

    def require_closed_windows(self) -> ComparisonNormalizationBuilder:
        """Require recorded windows to be closed before historical comparison."""

        self._policy = ComparisonNormalizationPolicy.require_closed()
        return self

    def clip_open_windows_to(self, horizon: TemporalPoint) -> ComparisonNormalizationBuilder:
        """Clip open windows to an explicit horizon."""

        self._policy = self._policy._replace(
            require_closed_windows=False,
            time_axis=horizon.axis,
            open_window_policy=ComparisonOpenWindowPolicy.CLIP_TO_HORIZON,
            open_window_horizon=horizon,
        )
        return self

    def half_open(self) -> ComparisonNormalizationBuilder:
        """Use half-open start-inclusive, end-exclusive ranges."""

        self._policy = self._policy._replace(use_half_open_ranges=True)
        return self

    def on_position(self) -> ComparisonNormalizationBuilder:
        """Normalize windows on the processing-position axis."""

        self._policy = self._policy._replace(time_axis=TemporalAxis.PROCESSING_POSITION)
        return self

    def on_event_time(self) -> ComparisonNormalizationBuilder:
        """Normalize windows on the event-time axis."""

        self._policy = self._policy._replace(time_axis=TemporalAxis.TIMESTAMP)
        return self

    def reject_missing_event_time(self) -> ComparisonNormalizationBuilder:
        """Reject records with missing event timestamps in event-time mode."""

        self._policy = self._policy._replace(
            null_timestamp_policy=ComparisonNullTimestampPolicy.REJECT
        )
        return self

    def exclude_missing_event_time(self) -> ComparisonNormalizationBuilder:
        """Exclude records with missing event timestamps in event-time mode."""

        self._policy = self._policy._replace(
            null_timestamp_policy=ComparisonNullTimestampPolicy.EXCLUDE
        )
        return self

    def coalesce_adjacent_windows(self) -> ComparisonNormalizationBuilder:
        """Coalesce adjacent normalized windows with identical comparison scope."""

        self._policy = self._policy._replace(coalesce_adjacent_windows=True)
        return self

    def reject_duplicate_windows(self) -> ComparisonNormalizationBuilder:
        """Reject duplicate normalized windows."""

        self._policy = self._policy._replace(
            duplicate_window_policy=ComparisonDuplicateWindowPolicy.REJECT
        )
        return self

    def known_at_position(self, position: int) -> ComparisonNormalizationBuilder:
        """Apply a processing-position known-at point."""

        return self.known_at(TemporalPoint.for_position(position))

    def known_at(self, point: TemporalPoint) -> ComparisonNormalizationBuilder:
        """Apply a known-at point to prevent future leakage."""

        self._policy = self._policy._replace(known_at=point)
        return self

    def build(self) -> ComparisonNormalizationPolicy:
        """Return the built normalization policy."""

        return self._policy


class ComparisonComparatorBuilder:
    """Builds comparator declarations for a comparison plan."""

    def __init__(self) -> None:
        self._comparators: list[str] = []

    def overlap(self) -> ComparisonComparatorBuilder:
        """Add the overlap comparator."""

        self._comparators.append("overlap")
        return self

    def residual(self) -> ComparisonComparatorBuilder:
        """Add the residual comparator."""

        self._comparators.append("residual")
        return self

    def missing(self) -> ComparisonComparatorBuilder:
        """Add the missing comparator."""

        self._comparators.append("missing")
        return self

    def coverage(self) -> ComparisonComparatorBuilder:
        """Add the coverage comparator."""

        self._comparators.append("coverage")
        return self

    def gap(self) -> ComparisonComparatorBuilder:
        """Add the gap comparator."""

        self._comparators.append("gap")
        return self

    def symmetric_difference(self) -> ComparisonComparatorBuilder:
        """Add the symmetric-difference comparator."""

        self._comparators.append("symmetric-difference")
        return self

    def containment(self) -> ComparisonComparatorBuilder:
        """Add the containment comparator."""

        self._comparators.append("containment")
        return self

    def lead_lag(
        self,
        transition: LeadLagTransition,
        axis: TemporalAxis,
        tolerance_magnitude: int,
    ) -> ComparisonComparatorBuilder:
        """Add a lead/lag comparator declaration."""

        if tolerance_magnitude < 0:
            msg = "Lead/lag tolerance cannot be negative."
            raise ValueError(msg)
        self._comparators.append(
            f"lead-lag:{transition.value}:{axis.value}:{tolerance_magnitude}"
        )
        return self

    def as_of(
        self,
        direction: AsOfDirection,
        axis: TemporalAxis,
        tolerance_magnitude: int,
    ) -> ComparisonComparatorBuilder:
        """Add an as-of lookup comparator declaration."""

        if tolerance_magnitude < 0:
            msg = "As-of tolerance cannot be negative."
            raise ValueError(msg)
        self._comparators.append(
            f"asof:{direction.value}:{axis.value}:{tolerance_magnitude}"
        )
        return self

    def declaration(self, declaration: str) -> ComparisonComparatorBuilder:
        """Add a comparator declaration by name."""

        _require_text(declaration, "Comparator declaration")
        self._comparators.append(declaration)
        return self

    def build(self) -> tuple[str, ...]:
        """Return comparator declarations in insertion order."""

        return tuple(self._comparators)


@dataclass(frozen=True, slots=True)
class ComparisonOutputOptions:
    """Describes output preferences for a comparison plan."""

    include_aligned_segments: bool = True
    include_explain_data: bool = True

    @classmethod
    def default(cls) -> ComparisonOutputOptions:
        """Return default output options."""

        return cls()


@dataclass(frozen=True, slots=True)
class ComparisonSelector:
    """Describes a selection used by a window comparison plan."""

    name: str
    description: str
    is_serializable: bool = True
    predicate: Callable[[WindowRecord], bool] | None = None
    cohort_activity: CohortActivity | None = None
    cohort_sources: tuple[Any, ...] = ()

    @classmethod
    def serializable(cls, name: str, description: str) -> ComparisonSelector:
        """Create a serializable selector descriptor."""

        _require_text(name, "Selector name")
        _require_text(description, "Selector description")
        return cls(name, description)

    @classmethod
    def for_window_name(cls, window_name: str) -> ComparisonSelector:
        """Create a selector for a configured window name."""

        _require_text(window_name, "Window name")
        return cls(
            f"window:{window_name}",
            f"window name = {window_name}",
            predicate=lambda window: window.window_name == window_name,
        )

    @classmethod
    def for_key(cls, key: Any) -> ComparisonSelector:
        """Create a selector for a recorded window key."""

        _require_not_none(key, "Key")
        return cls(f"key:{key}", f"key = {key}", predicate=lambda window: window.key == key)

    @classmethod
    def for_source(cls, source: Any) -> ComparisonSelector:
        """Create a selector for a source identity."""

        _require_not_none(source, "Source")
        return cls(
            f"source:{source}",
            f"source = {source}",
            predicate=lambda window: window.source == source,
        )

    @classmethod
    def for_sources(cls, sources: Iterable[Any]) -> ComparisonSelector:
        """Create a selector for any of several source identities."""

        return cls._for_sources_core(sources, None)

    @classmethod
    def for_cohort_sources(
        cls,
        sources: Iterable[Any],
        activity: CohortActivity,
    ) -> ComparisonSelector:
        """Create a selector for a cohort of source identities."""

        _require_not_none(activity, "Cohort activity")
        return cls._for_sources_core(sources, activity)

    @classmethod
    def _for_sources_core(
        cls,
        sources: Iterable[Any],
        activity: CohortActivity | None,
    ) -> ComparisonSelector:
        ordered_sources = tuple(sources)
        if not ordered_sources:
            msg = "At least one source is required."
            raise ValueError(msg)
        if any(source is None for source in ordered_sources):
            msg = "Sources cannot include None."
            raise ValueError(msg)
        return cls(
            "sources:" + ",".join(str(source) for source in ordered_sources),
            "source in [" + ", ".join(str(source) for source in ordered_sources) + "]",
            predicate=lambda window: window.source in ordered_sources,
            cohort_activity=activity,
            cohort_sources=ordered_sources,
        )

    @classmethod
    def for_partition(cls, partition: Any) -> ComparisonSelector:
        """Create a selector for a partition identity."""

        _require_not_none(partition, "Partition")
        return cls(
            f"partition:{partition}",
            f"partition = {partition}",
            predicate=lambda window: window.partition == partition,
        )

    @classmethod
    def for_position_range(
        cls,
        start_inclusive: int,
        end_exclusive: int | None = None,
    ) -> ComparisonSelector:
        """Create a selector for a half-open processing-position start range."""

        if end_exclusive is not None and end_exclusive < start_inclusive:
            msg = "Position range end cannot be earlier than the start."
            raise ValueError(msg)
        end_label = end_exclusive if end_exclusive is not None else "*"
        return cls(
            f"position:{start_inclusive}..{end_label}",
            f"start position in [{start_inclusive}, {end_label})",
            predicate=lambda window: window.start_position >= start_inclusive
            and (end_exclusive is None or window.start_position < end_exclusive),
        )

    @classmethod
    def for_time_range(
        cls,
        start_inclusive: Any,
        end_exclusive: Any | None = None,
    ) -> ComparisonSelector:
        """Create a selector for a half-open event-time start range."""

        if end_exclusive is not None and end_exclusive < start_inclusive:
            msg = "Time range end cannot be earlier than the start."
            raise ValueError(msg)
        end_label = end_exclusive if end_exclusive is not None else "*"
        return cls(
            f"time:{start_inclusive}..{end_label}",
            f"start time in [{start_inclusive}, {end_label})",
            predicate=lambda window: window.start_time is not None
            and window.start_time >= start_inclusive
            and (end_exclusive is None or window.start_time < end_exclusive),
        )

    @classmethod
    def runtime_only(
        cls,
        name: str,
        description: str,
        predicate: Callable[[WindowRecord], bool] | None = None,
    ) -> ComparisonSelector:
        """Create a runtime-only selector backed by an optional predicate."""

        _require_text(name, "Selector name")
        _require_text(description, "Selector description")
        return cls(
            name,
            description,
            is_serializable=False,
            predicate=predicate or (lambda _: True),
        )

    def with_name(self, name: str) -> ComparisonSelector:
        """Return a copy of this selector with a different display name."""

        _require_text(name, "Selector name")
        return replace(self, name=name)

    def and_(self, other: ComparisonSelector) -> ComparisonSelector:
        """Create a selector that requires both selectors to match."""

        return ComparisonSelector(
            f"{self.name}&{other.name}",
            f"({self.description}) and ({other.description})",
            self.is_serializable and other.is_serializable,
            lambda window: self.matches(window) and other.matches(window),
        )

    def or_(self, other: ComparisonSelector) -> ComparisonSelector:
        """Create a selector that allows either selector to match."""

        return ComparisonSelector(
            f"{self.name}|{other.name}",
            f"({self.description}) or ({other.description})",
            self.is_serializable and other.is_serializable,
            lambda window: self.matches(window) or other.matches(window),
        )

    def matches(self, window: WindowRecord) -> bool:
        """Return whether this selector matches a recorded window."""

        return True if self.predicate is None else bool(self.predicate(window))


@dataclass(frozen=True, slots=True)
class ComparisonPlanDiagnostic:
    """Structured diagnostic emitted while validating a comparison plan."""

    code: ComparisonPlanValidationCode
    message: str
    path: str
    severity: ComparisonDiagnosticSeverity


@dataclass(frozen=True, init=False, slots=True)
class ComparisonPlan:
    """Represents a window comparison question as inspectable data."""

    name: str
    target: ComparisonSelector | None
    against: tuple[ComparisonSelector, ...]
    scope: ComparisonScope | None
    normalization: ComparisonNormalizationPolicy
    comparators: tuple[str, ...]
    output: ComparisonOutputOptions
    is_strict: bool

    def __init__(
        self,
        name: str,
        target: ComparisonSelector | None,
        against: Iterable[ComparisonSelector] | None,
        scope: ComparisonScope | None,
        normalization: ComparisonNormalizationPolicy | None,
        comparators: Iterable[str] | None,
        output: ComparisonOutputOptions | None,
        is_strict: bool = False,
    ) -> None:
        object.__setattr__(self, "name", name)
        object.__setattr__(self, "target", target)
        object.__setattr__(self, "against", tuple(against or ()))
        object.__setattr__(self, "scope", scope)
        object.__setattr__(
            self,
            "normalization",
            normalization or ComparisonNormalizationPolicy.default(),
        )
        object.__setattr__(
            self,
            "comparators",
            tuple(comparator for comparator in (comparators or ()) if comparator.strip()),
        )
        object.__setattr__(self, "output", output or ComparisonOutputOptions.default())
        object.__setattr__(self, "is_strict", is_strict)

    @property
    def is_serializable(self) -> bool:
        """Return whether every selector can be exported as portable data."""

        selectors = self.against if self.target is None else (self.target, *self.against)
        return all(selector.is_serializable for selector in selectors)

    def validate(self) -> tuple[ComparisonPlanDiagnostic, ...]:
        """Validate structural completeness of the comparison plan."""

        diagnostics: list[ComparisonPlanDiagnostic] = []
        exportability_severity = (
            ComparisonDiagnosticSeverity.ERROR
            if self.is_strict
            else ComparisonDiagnosticSeverity.WARNING
        )
        if not self.name or not self.name.strip():
            diagnostics.append(
                ComparisonPlanDiagnostic(
                    ComparisonPlanValidationCode.MISSING_NAME,
                    "Comparison plan name is required.",
                    "name",
                    ComparisonDiagnosticSeverity.ERROR,
                )
            )
        if self.target is None:
            diagnostics.append(
                ComparisonPlanDiagnostic(
                    ComparisonPlanValidationCode.MISSING_TARGET,
                    "Comparison plan target selector is required.",
                    "target",
                    ComparisonDiagnosticSeverity.ERROR,
                )
            )
        elif not self.target.is_serializable:
            diagnostics.append(
                ComparisonPlanDiagnostic(
                    ComparisonPlanValidationCode.NON_SERIALIZABLE_SELECTOR,
                    "Target selector is runtime-only and cannot be exported as plan data.",
                    "target",
                    exportability_severity,
                )
            )
        if not self.against:
            diagnostics.append(
                ComparisonPlanDiagnostic(
                    ComparisonPlanValidationCode.MISSING_AGAINST,
                    "At least one comparison selector is required.",
                    "against",
                    ComparisonDiagnosticSeverity.ERROR,
                )
            )
        else:
            for index, selector in enumerate(self.against):
                if selector.is_serializable:
                    continue
                diagnostics.append(
                    ComparisonPlanDiagnostic(
                        ComparisonPlanValidationCode.NON_SERIALIZABLE_SELECTOR,
                        "Comparison selector is runtime-only and cannot be exported as plan data.",
                        f"against[{index}]",
                        exportability_severity,
                    )
                )
        if self.scope is None:
            diagnostics.append(
                ComparisonPlanDiagnostic(
                    ComparisonPlanValidationCode.MISSING_SCOPE,
                    "Comparison scope is required.",
                    "scope",
                    ComparisonDiagnosticSeverity.ERROR,
                )
            )
        if not self.comparators:
            diagnostics.append(
                ComparisonPlanDiagnostic(
                    ComparisonPlanValidationCode.MISSING_COMPARATOR,
                    "At least one comparator is required.",
                    "comparators",
                    ComparisonDiagnosticSeverity.ERROR,
                )
            )
        return tuple(diagnostics)

    def export_json(self, path: str | Path | None = None) -> str:
        """Return deterministic portable JSON for this comparison plan."""

        _ensure_exportable(self)
        text = json.dumps(_plan_export_payload(self, self.validate()), indent=2, sort_keys=True)
        _write_text_if_requested(path, text)
        return text

    def to_json(self, path: str | Path | None = None) -> str:
        """Return deterministic portable JSON for this comparison plan."""

        return self.export_json(path)

    def to_markdown(self, path: str | Path | None = None) -> str:
        """Return deterministic Markdown for this comparison plan."""

        diagnostics = self.validate()
        lines = [f"# Comparison Plan: {self.name}", ""]
        lines.append(f"- strict: {self.is_strict}")
        lines.append(f"- serializable: {self.is_serializable}")
        lines.append(f"- target: {self.target.name if self.target else '<missing>'}")
        lines.append(
            "- against: "
            + (
                ", ".join(selector.name for selector in self.against)
                if self.against
                else "<missing>"
            )
        )
        lines.append(f"- comparators: {', '.join(self.comparators) or '<missing>'}")
        if self.scope is not None:
            lines.append(f"- window: {self.scope.window_name or '<any>'}")
            lines.append(f"- time axis: {self.scope.time_axis.value}")
        if diagnostics:
            lines.extend(["", "## Diagnostics"])
            for index, diagnostic in enumerate(diagnostics):
                lines.append(
                    f"- diagnostic[{index}]: {diagnostic.severity.value} "
                    f"{diagnostic.code.value} path={diagnostic.path}"
                )
        text = "\n".join(lines).rstrip() + "\n"
        _write_text_if_requested(path, text)
        return text

    def explain(
        self,
        format: ComparisonExplanationFormat = ComparisonExplanationFormat.MARKDOWN,
        *,
        markdown: bool | None = None,
        path: str | Path | None = None,
    ) -> str:
        """Return deterministic human-readable plan output."""

        use_markdown = (
            format is ComparisonExplanationFormat.MARKDOWN
            if markdown is None
            else markdown
        )
        if use_markdown:
            return self.to_markdown(path)
        text = self.to_markdown()
        text = text.replace("# Comparison Plan:", "Comparison Plan:")
        text = text.replace("\n## Diagnostics", "\nDiagnostics")
        _write_text_if_requested(path, text)
        return text


class ComparisonExportException(ValueError):
    """Raised when a comparison artifact cannot be exported as portable data."""

    def __init__(
        self,
        message: str,
        diagnostics: Iterable[ComparisonPlanDiagnostic],
    ) -> None:
        super().__init__(message)
        self.diagnostics = tuple(diagnostics)


@dataclass(frozen=True, slots=True)
class NormalizedWindowRecord:
    """Describes a recorded window after comparison normalization."""

    window: WindowRecord
    record_id: WindowRecordId
    selector_name: str
    side: ComparisonSide
    range: TemporalRange
    segments: tuple[Any, ...] = ()


@dataclass(frozen=True, slots=True)
class ExcludedWindowRecord:
    """Describes a recorded window excluded during comparison preparation."""

    window: WindowRecord
    reason: str
    diagnostic_code: ComparisonPlanValidationCode | None = None


@dataclass(frozen=True, slots=True)
class PreparedComparison:
    """Represents the current prepared comparison state."""

    plan: ComparisonPlan
    diagnostics: tuple[ComparisonPlanDiagnostic, ...]
    selected_windows: tuple[WindowRecord, ...]
    excluded_windows: tuple[ExcludedWindowRecord, ...]
    normalized_windows: tuple[NormalizedWindowRecord, ...]

    def align(self) -> AlignedComparison:
        """Split normalized windows into deterministic aligned segments."""

        return _align_prepared(self)


@dataclass(frozen=True, slots=True)
class AlignedSegment:
    """One aligned temporal segment with active target and comparison windows."""

    window_name: str
    key: Any
    partition: Any
    range: TemporalRange
    target_record_ids: tuple[WindowRecordId, ...]
    against_record_ids: tuple[WindowRecordId, ...]
    segments: tuple[WindowSegment, ...] = ()


@dataclass(frozen=True, slots=True)
class AlignedComparison:
    """A prepared comparison after temporal segment alignment."""

    prepared: PreparedComparison
    segments: tuple[AlignedSegment, ...]


@dataclass(frozen=True, slots=True)
class CohortActivity:
    """Rule describing when a source cohort is considered active."""

    name: str
    count: int | None = None

    @classmethod
    def any(cls) -> CohortActivity:
        """Return an any-member activity rule."""

        return cls("any")

    @classmethod
    def all(cls) -> CohortActivity:
        """Return an all-members activity rule."""

        return cls("all")

    @classmethod
    def none(cls) -> CohortActivity:
        """Return a no-active-members activity rule."""

        return cls("none")

    @classmethod
    def at_least(cls, count: int) -> CohortActivity:
        """Return an at-least activity rule."""

        if count < 1:
            raise ValueError("At-least cohort count must be greater than zero.")
        return cls("at_least", count)

    @classmethod
    def at_most(cls, count: int) -> CohortActivity:
        """Return an at-most activity rule."""

        if count < 0:
            raise ValueError("At-most cohort count cannot be negative.")
        return cls("at_most", count)

    @classmethod
    def exactly(cls, count: int) -> CohortActivity:
        """Return an exact-count activity rule."""

        if count < 0:
            raise ValueError("Exact cohort count cannot be negative.")
        return cls("exactly", count)

    def required_count(self, member_count: int) -> int:
        """Return the configured active-member count."""

        if self.name == "any":
            return 1
        if self.name == "all":
            return member_count
        if self.name == "none":
            return 0
        if self.count is None:
            raise ValueError("Cohort activity count is required.")
        return self.count

    def is_active(self, active_count: int, member_count: int) -> bool:
        """Return whether a segment satisfies this activity rule."""

        if self.name == "any":
            return active_count >= 1
        if self.name == "all":
            return active_count == member_count
        if self.name == "none":
            return active_count == 0
        if self.name == "at_least":
            return self.count is not None and active_count >= self.count
        if self.name == "at_most":
            return self.count is not None and active_count <= self.count
        if self.name == "exactly":
            return self.count is not None and active_count == self.count
        raise ValueError(f"Unknown cohort activity rule: {self.name}")


@dataclass(frozen=True, slots=True)
class OverlapRow:
    """Aligned segment where target and comparison windows overlap."""

    window_name: str
    key: Any
    partition: Any
    range: TemporalRange
    target_record_ids: tuple[WindowRecordId, ...]
    against_record_ids: tuple[WindowRecordId, ...]
    finality: ComparisonFinality = ComparisonFinality.FINAL


@dataclass(frozen=True, slots=True)
class ResidualRow:
    """Segment where target windows are active without comparison coverage."""

    window_name: str
    key: Any
    partition: Any
    range: TemporalRange
    target_record_ids: tuple[WindowRecordId, ...]
    finality: ComparisonFinality = ComparisonFinality.FINAL


@dataclass(frozen=True, slots=True)
class MissingRow:
    """Segment where comparison windows are active without target coverage."""

    window_name: str
    key: Any
    partition: Any
    range: TemporalRange
    against_record_ids: tuple[WindowRecordId, ...]
    finality: ComparisonFinality = ComparisonFinality.FINAL


@dataclass(frozen=True, slots=True)
class CoverageRow:
    """Target coverage for one aligned target segment."""

    window_name: str
    key: Any
    partition: Any
    range: TemporalRange
    target_magnitude: float
    covered_magnitude: float
    target_record_ids: tuple[WindowRecordId, ...]
    against_record_ids: tuple[WindowRecordId, ...]
    finality: ComparisonFinality = ComparisonFinality.FINAL

    @property
    def coverage_ratio(self) -> float | None:
        """Return covered magnitude divided by target magnitude."""

        if self.target_magnitude == 0:
            return None
        return self.covered_magnitude / self.target_magnitude


@dataclass(frozen=True, slots=True)
class GapRow:
    """Uncovered temporal space between observed comparison segments."""

    window_name: str
    key: Any
    partition: Any
    range: TemporalRange
    finality: ComparisonFinality = ComparisonFinality.FINAL


@dataclass(frozen=True, slots=True)
class SymmetricDifferenceRow:
    """Target-only or comparison-only disagreement segment."""

    window_name: str
    key: Any
    partition: Any
    range: TemporalRange
    side: ComparisonSide
    target_record_ids: tuple[WindowRecordId, ...]
    against_record_ids: tuple[WindowRecordId, ...]
    finality: ComparisonFinality = ComparisonFinality.FINAL


@dataclass(frozen=True, slots=True)
class ContainmentRow:
    """Directional target containment by comparison windows for one segment."""

    window_name: str
    key: Any
    partition: Any
    range: TemporalRange
    status: ContainmentStatus
    target_record_ids: tuple[WindowRecordId, ...]
    container_record_ids: tuple[WindowRecordId, ...]
    finality: ComparisonFinality = ComparisonFinality.FINAL


@dataclass(frozen=True, slots=True)
class LeadLagRow:
    """Lead or lag between a target transition and nearest comparison transition."""

    window_name: str
    key: Any
    partition: Any
    transition: LeadLagTransition
    axis: TemporalAxis
    target_point: TemporalPoint
    comparison_point: TemporalPoint | None
    delta_magnitude: int | None
    tolerance_magnitude: int
    is_within_tolerance: bool
    direction: LeadLagDirection
    target_record_id: WindowRecordId
    comparison_record_id: WindowRecordId | None
    finality: ComparisonFinality = ComparisonFinality.FINAL


@dataclass(frozen=True, slots=True)
class AsOfRow:
    """Point-in-time as-of lookup result."""

    window_name: str
    key: Any
    partition: Any
    axis: TemporalAxis
    direction: AsOfDirection
    target_point: TemporalPoint
    matched_point: TemporalPoint | None
    distance_magnitude: int | None
    tolerance_magnitude: int
    status: AsOfMatchStatus
    target_record_id: WindowRecordId
    matched_record_id: WindowRecordId | None
    finality: ComparisonFinality = ComparisonFinality.FINAL


@dataclass(frozen=True, slots=True)
class ComparatorSummary:
    """Summary row count for one comparator."""

    comparator: str
    row_count: int


@dataclass(frozen=True, slots=True)
class CoverageSummary:
    """Summarizes target coverage within one comparison scope."""

    window_name: str
    key: Any
    partition: Any
    target_magnitude: float
    covered_magnitude: float
    coverage_ratio: float


@dataclass(frozen=True, slots=True)
class ComparisonDiagnostic:
    """Structured diagnostic emitted while preparing or running a comparison."""

    code: str
    message: str
    path: str
    severity: ComparisonDiagnosticSeverity


@dataclass(frozen=True, slots=True)
class ComparisonExtensionSelector:
    """Selector declaration exposed by a comparison extension."""

    name: str
    description: str


@dataclass(frozen=True, slots=True)
class ComparisonExtensionComparator:
    """Comparator declaration exposed by a comparison extension."""

    declaration: str
    description: str


@dataclass(frozen=True, slots=True)
class ComparisonExtensionDescriptor:
    """Describes selectors, comparators, and metadata keys exposed by an extension."""

    id: str
    display_name: str
    selectors: tuple[ComparisonExtensionSelector, ...] = ()
    comparators: tuple[ComparisonExtensionComparator, ...] = ()
    metadata_keys: tuple[str, ...] = ()


@dataclass(frozen=True, slots=True)
class ComparisonExtensionMetadata:
    """Serializable metadata emitted by a comparison extension."""

    extension_id: str
    key: str
    value: str


@dataclass(frozen=True, slots=True)
class CohortEvidenceMetadata:
    """Parsed evidence for one cohort-aligned segment."""

    segment_index: int
    rule: str
    required_count: int
    active_count: int
    is_active: bool
    active_sources: tuple[str, ...]
    raw_value: str


class ComparisonExtensionBuilder:
    """Builds comparison extension descriptors."""

    def __init__(self, extension_id: str, display_name: str) -> None:
        if not extension_id or not extension_id.strip():
            msg = "Extension id cannot be empty."
            raise ValueError(msg)
        if not display_name or not display_name.strip():
            msg = "Extension display name cannot be empty."
            raise ValueError(msg)
        self._id = extension_id
        self._display_name = display_name
        self._selectors: list[ComparisonExtensionSelector] = []
        self._comparators: list[ComparisonExtensionComparator] = []
        self._metadata_keys: list[str] = []

    def add_selector(
        self,
        name: str,
        description: str,
    ) -> ComparisonExtensionBuilder:
        """Register a selector descriptor exposed by the extension."""

        if not name or not name.strip():
            msg = "Selector name cannot be empty."
            raise ValueError(msg)
        if not description or not description.strip():
            msg = "Selector description cannot be empty."
            raise ValueError(msg)
        self._selectors.append(ComparisonExtensionSelector(name, description))
        return self

    def add_comparator(
        self,
        declaration: str,
        description: str,
    ) -> ComparisonExtensionBuilder:
        """Register a comparator declaration exposed by the extension."""

        if not declaration or not declaration.strip():
            msg = "Comparator declaration cannot be empty."
            raise ValueError(msg)
        if not description or not description.strip():
            msg = "Comparator description cannot be empty."
            raise ValueError(msg)
        self._comparators.append(ComparisonExtensionComparator(declaration, description))
        return self

    def add_metadata_key(self, key: str) -> ComparisonExtensionBuilder:
        """Register a metadata key emitted by the extension."""

        if not key or not key.strip():
            msg = "Metadata key cannot be empty."
            raise ValueError(msg)
        self._metadata_keys.append(key)
        return self

    def build(self) -> ComparisonExtensionDescriptor:
        """Build the immutable extension descriptor."""

        return ComparisonExtensionDescriptor(
            self._id,
            self._display_name,
            tuple(self._selectors),
            tuple(self._comparators),
            tuple(self._metadata_keys),
        )


@dataclass(frozen=True, slots=True)
class ComparisonRowFinality:
    """Finality metadata for a materialized comparison row."""

    row_type: str
    row_id: str
    finality: ComparisonFinality
    reason: str
    version: int = 1
    supersedes_row_id: str | None = None


@dataclass(frozen=True, slots=True)
class ComparisonChangelogEntry:
    """One deterministic row-finality change between snapshots."""

    row_type: str
    row_id: str
    version: int
    finality: ComparisonFinality
    supersedes_row_id: str | None
    reason: str


class ComparisonChangelog:
    """Creates and replays row-finality changelogs between snapshots."""

    @staticmethod
    def create(
        previous: Iterable[ComparisonRowFinality],
        current: Iterable[ComparisonRowFinality],
    ) -> tuple[ComparisonChangelogEntry, ...]:
        """Create changelog entries from one finality snapshot to another."""

        previous_by_key = {_finality_key(row): row for row in previous}
        current_by_key = {_finality_key(row): row for row in current}
        entries: list[ComparisonChangelogEntry] = []

        for key, current_row in sorted(current_by_key.items()):
            previous_row = previous_by_key.get(key)
            if previous_row is None:
                entries.append(
                    ComparisonChangelogEntry(
                        current_row.row_type,
                        current_row.row_id,
                        current_row.version,
                        current_row.finality,
                        current_row.supersedes_row_id,
                        current_row.reason,
                    )
                )
                continue
            if (
                previous_row.finality is current_row.finality
                and previous_row.reason == current_row.reason
            ):
                continue
            entries.append(
                ComparisonChangelogEntry(
                    current_row.row_type,
                    current_row.row_id,
                    previous_row.version + 1,
                    ComparisonFinality.REVISED,
                    previous_row.row_id,
                    f"Row metadata changed from {previous_row.finality.value} "
                    f"to {current_row.finality.value}.",
                )
            )

        for key, previous_row in sorted(previous_by_key.items()):
            if key in current_by_key:
                continue
            entries.append(
                ComparisonChangelogEntry(
                    previous_row.row_type,
                    previous_row.row_id,
                    previous_row.version + 1,
                    ComparisonFinality.RETRACTED,
                    previous_row.row_id,
                    "Row was not emitted by the current snapshot.",
                )
            )
        return tuple(entries)

    @staticmethod
    def replay(
        previous: Iterable[ComparisonRowFinality],
        entries: Iterable[ComparisonChangelogEntry],
    ) -> tuple[ComparisonRowFinality, ...]:
        """Replay changelog entries over a previous finality snapshot."""

        active = {_finality_key(row): row for row in previous}
        for entry in sorted(entries, key=lambda item: (item.row_type, item.row_id, item.version)):
            key = f"{entry.row_type}\n{entry.row_id}"
            if entry.finality is ComparisonFinality.RETRACTED:
                active.pop(key, None)
                continue
            active[key] = ComparisonRowFinality(
                entry.row_type,
                entry.row_id,
                ComparisonFinality.FINAL
                if entry.finality is ComparisonFinality.REVISED
                else entry.finality,
                entry.reason,
                entry.version,
                entry.supersedes_row_id,
            )
        return tuple(sorted(active.values(), key=lambda item: (item.row_type, item.row_id)))


@dataclass(frozen=True, slots=True)
class SourceMatrixCell:
    """Directional source-pair comparison summary."""

    target_source: Any
    against_source: Any
    is_diagonal: bool
    target_has_windows: bool
    against_has_windows: bool
    overlap_row_count: int
    residual_row_count: int
    missing_row_count: int
    coverage_row_count: int
    coverage_ratio: float | None


@dataclass(frozen=True, slots=True)
class SourceMatrixResult:
    """Directional pairwise matrix over recorded window sources."""

    name: str
    window_name: str
    sources: tuple[Any, ...]
    cells: tuple[SourceMatrixCell, ...]

    def get_cell(self, target_source: Any, against_source: Any) -> SourceMatrixCell:
        """Return one directional matrix cell."""

        cell = self.try_get_cell(target_source, against_source)
        if cell is None:
            msg = "Source matrix cell was not found."
            raise KeyError(msg)
        return cell

    def try_get_cell(self, target_source: Any, against_source: Any) -> SourceMatrixCell | None:
        """Return one directional matrix cell, or ``None`` when absent."""

        for cell in self.cells:
            if cell.target_source == target_source and cell.against_source == against_source:
                return cell
        return None


@dataclass(frozen=True, slots=True)
class HierarchyComparisonRow:
    """One parent/child temporal contribution segment."""

    kind: HierarchyComparisonRowKind
    source: Any
    partition: Any
    range: TemporalRange
    parent_record_ids: tuple[WindowRecordId, ...]
    child_record_ids: tuple[WindowRecordId, ...]


@dataclass(frozen=True, slots=True)
class HierarchyComparisonResult:
    """Temporal explanation of parent and child recorded windows."""

    name: str
    parent_window_name: str
    child_window_name: str
    rows: tuple[HierarchyComparisonRow, ...]
    diagnostics: tuple[str, ...] = ()


@dataclass(frozen=True, slots=True)
class LeadLagSummary:
    """Summarizes lead/lag measurements for one comparator run."""

    transition: LeadLagTransition
    axis: TemporalAxis
    tolerance_magnitude: int
    row_count: int
    target_lead_count: int
    target_lag_count: int
    equal_count: int
    missing_comparison_count: int
    outside_tolerance_count: int
    minimum_delta_magnitude: int | None
    maximum_delta_magnitude: int | None


@dataclass(frozen=True, slots=True)
class ComparisonDebugHtmlOptions:
    """Configures optional debug HTML export during comparison execution."""

    enabled: bool
    path: str | Path | None = None

    @classmethod
    def disabled(cls) -> ComparisonDebugHtmlOptions:
        """Return options that do not write a debug HTML artifact."""

        return cls(False)

    @classmethod
    def to_file(cls, path: str | Path) -> ComparisonDebugHtmlOptions:
        """Return options that write a debug HTML artifact to a file."""

        return cls(True, _require_export_path(path))

    def export_if_enabled(self, result: ComparisonResult) -> None:
        """Write debug HTML when this option is enabled."""

        if self.enabled:
            result.export_debug_html(_require_export_path(self.path))


@dataclass(frozen=True, slots=True)
class ComparisonLlmContextOptions:
    """Configures optional LLM context export during comparison execution."""

    enabled: bool
    path: str | Path | None = None

    @classmethod
    def disabled(cls) -> ComparisonLlmContextOptions:
        """Return options that do not write an LLM context artifact."""

        return cls(False)

    @classmethod
    def to_file(cls, path: str | Path) -> ComparisonLlmContextOptions:
        """Return options that write deterministic LLM context JSON to a file."""

        return cls(True, _require_export_path(path))

    def export_if_enabled(self, result: ComparisonResult) -> None:
        """Write LLM context JSON when this option is enabled."""

        if self.enabled:
            result.export_llm_context(_require_export_path(self.path))


@dataclass(frozen=True, slots=True)
class ComparisonResult:
    """Structured result produced by a window comparison run."""

    name: str
    overlap_rows: tuple[OverlapRow, ...] = ()
    residual_rows: tuple[ResidualRow, ...] = ()
    missing_rows: tuple[MissingRow, ...] = ()
    coverage_rows: tuple[CoverageRow, ...] = ()
    gap_rows: tuple[GapRow, ...] = ()
    symmetric_difference_rows: tuple[SymmetricDifferenceRow, ...] = ()
    containment_rows: tuple[ContainmentRow, ...] = ()
    lead_lag_rows: tuple[LeadLagRow, ...] = ()
    lead_lag_summaries: tuple[LeadLagSummary, ...] = ()
    as_of_rows: tuple[AsOfRow, ...] = ()
    comparator_summaries: tuple[ComparatorSummary, ...] = ()
    coverage_summaries: tuple[CoverageSummary, ...] = ()
    diagnostics: tuple[str, ...] = ()
    evaluation_horizon: TemporalPoint | None = None
    known_at: TemporalPoint | None = None
    strict: bool = False
    extension_metadata: tuple[ComparisonExtensionMetadata, ...] = ()
    plan: ComparisonPlan | None = None
    prepared: PreparedComparison | None = None
    aligned: Any = None

    def to_json(self, path: str | Path | None = None) -> str:
        """Return a deterministic JSON representation and optionally write it."""

        text = json.dumps(_to_jsonable(self), sort_keys=True, indent=2)
        _write_text_if_requested(path, text)
        return text

    def export_json(self, path: str | Path | None = None) -> str:
        """Return deterministic portable JSON for this comparison result."""

        if self.plan is not None:
            _ensure_exportable(self.plan)
        text = json.dumps(_result_export_payload(self), indent=2, sort_keys=True)
        _write_text_if_requested(path, text)
        return text

    @property
    def row_finalities(self) -> tuple[ComparisonRowFinality, ...]:
        """Return deterministic finality metadata for emitted rows."""

        rows: list[ComparisonRowFinality] = []
        for row_type, values in _result_row_groups(self):
            for index, row in enumerate(values):
                finality = getattr(row, "finality", ComparisonFinality.FINAL)
                rows.append(
                    ComparisonRowFinality(
                        row_type,
                        f"{row_type}[{index}]",
                        finality,
                        "Depends on at least one open window clipped to the evaluation horizon."
                        if finality is ComparisonFinality.PROVISIONAL
                        else "All contributing windows were closed when the row was produced.",
                    )
                )
        return tuple(rows)

    def has_provisional_rows(self) -> bool:
        """Return whether any comparison row is provisional."""

        return any(row.finality is ComparisonFinality.PROVISIONAL for row in self.row_finalities)

    def provisional_row_finalities(self) -> tuple[ComparisonRowFinality, ...]:
        """Return row-finality metadata for provisional rows."""

        return tuple(
            row
            for row in self.row_finalities
            if row.finality is ComparisonFinality.PROVISIONAL
        )

    def final_row_finalities(self) -> tuple[ComparisonRowFinality, ...]:
        """Return row-finality metadata for final rows."""

        return tuple(
            row for row in self.row_finalities if row.finality is ComparisonFinality.FINAL
        )

    @property
    def diagnostic_rows(self) -> tuple[ComparisonDiagnostic, ...]:
        """Return structured diagnostics for this result."""

        return tuple(_diagnostic_from_code(code, strict=self.strict) for code in self.diagnostics)

    def warning_diagnostics(self) -> tuple[ComparisonDiagnostic, ...]:
        """Return diagnostics with warning severity."""

        return tuple(
            diagnostic
            for diagnostic in self.diagnostic_rows
            if diagnostic.severity is ComparisonDiagnosticSeverity.WARNING
        )

    def error_diagnostics(self) -> tuple[ComparisonDiagnostic, ...]:
        """Return diagnostics with error severity."""

        return tuple(
            diagnostic
            for diagnostic in self.diagnostic_rows
            if diagnostic.severity is ComparisonDiagnosticSeverity.ERROR
        )

    def cohort_evidence(self) -> tuple[CohortEvidenceMetadata, ...]:
        """Return parsed cohort evidence metadata emitted by this result."""

        evidence: list[CohortEvidenceMetadata] = []
        for metadata in self.extension_metadata:
            if metadata.extension_id != "spanfold.cohort":
                continue
            parsed = _parse_cohort_evidence(metadata)
            if parsed is not None:
                evidence.append(parsed)
        return tuple(evidence)

    @property
    def is_valid(self) -> bool:
        """Return whether this result has no blocking diagnostics."""

        return not self.error_diagnostics()

    def to_json_lines(self, path: str | Path | None = None) -> str:
        """Return deterministic JSON Lines for the summary and comparison rows."""

        rows: list[dict[str, Any]] = [
            {
                "artifact": "result-summary",
                "name": self.name,
                "comparator_summaries": _to_jsonable(self.comparator_summaries),
                "coverage_summaries": _to_jsonable(self.coverage_summaries),
                "diagnostic_rows": _to_jsonable(self.diagnostic_rows),
                "row_finalities": _to_jsonable(self.row_finalities),
                "extension_metadata": _to_jsonable(self.extension_metadata),
                "is_valid": self.is_valid,
            }
        ]
        for kind, values in (
            ("overlap", self.overlap_rows),
            ("residual", self.residual_rows),
            ("missing", self.missing_rows),
            ("coverage", self.coverage_rows),
            ("gap", self.gap_rows),
            ("symmetric_difference", self.symmetric_difference_rows),
            ("containment", self.containment_rows),
            ("lead_lag", self.lead_lag_rows),
            ("as_of", self.as_of_rows),
        ):
            for index, row in enumerate(values):
                payload = _to_jsonable(row)
                payload["artifact"] = "result-row"
                payload["row_id"] = f"{kind}[{index}]"
                payload["row_type"] = kind
                rows.append(payload)
        text = "\n".join(json.dumps(row, sort_keys=True) for row in rows)
        _write_text_if_requested(path, text + ("\n" if text else ""))
        return text

    def export_json_lines(self, path: str | Path | None = None) -> str:
        """Return deterministic JSON Lines for LLM and artifact pipelines."""

        if self.plan is not None:
            _ensure_exportable(self.plan)
        text = "\n".join(self.iter_export_json_lines())
        _write_text_if_requested(path, text + ("\n" if text else ""))
        return text

    def iter_export_json_lines(self) -> Iterator[str]:
        """Yield deterministic JSON Lines for LLM and artifact pipelines."""

        if self.plan is not None:
            _ensure_exportable(self.plan)
        yield json.dumps(_result_summary_line_payload(self), sort_keys=True)
        for row_type, values in _result_row_groups(self):
            export_type = _export_row_type(row_type)
            for index, row in enumerate(values):
                payload = {
                    "schema": "spanfold.comparison.row",
                    "schemaVersion": 0,
                    "artifact": "result-row",
                    "rowType": export_type,
                    "rowId": f"{export_type}[{index}]",
                    **_row_export_fields(row),
                }
                yield json.dumps(payload, sort_keys=True)

    def to_markdown(self, path: str | Path | None = None) -> str:
        """Return a compact Markdown explanation of comparison output."""

        lines = [f"# {self.name}", ""]
        if self.evaluation_horizon is not None:
            lines.append(f"evaluation horizon: {_point_label(self.evaluation_horizon)}")
        if self.known_at is not None:
            lines.append(f"knownAt={_point_label(self.known_at)}")
        if len(lines) > 2:
            lines.append("")
        lines.append("| Comparator | Rows |")
        lines.append("| --- | ---: |")
        for summary in self.comparator_summaries:
            lines.append(f"| {summary.comparator} | {summary.row_count} |")
        lines.append("")
        for label, rows in (
            ("Overlap", self.overlap_rows),
            ("Residual", self.residual_rows),
            ("Missing", self.missing_rows),
            ("Coverage", self.coverage_rows),
            ("Gap", self.gap_rows),
            ("Symmetric Difference", self.symmetric_difference_rows),
            ("Containment", self.containment_rows),
            ("Lead/Lag", self.lead_lag_rows),
            ("As-Of", self.as_of_rows),
        ):
            if not rows:
                continue
            lines.extend([f"## {label}", "", "| Window | Key | Partition | Range | Finality |"])
            lines.append("| --- | --- | --- | --- | --- |")
            for row in rows:
                lines.append(
                    f"| {row.window_name} | {row.key} | {row.partition or ''} | "
                    f"{_row_range_or_point(row)} | {row.finality.value} |"
                )
            lines.append("")
        text = "\n".join(lines).rstrip() + "\n"
        _write_text_if_requested(path, text)
        return text

    def export_markdown(self, path: str | Path | None = None) -> str:
        """Return deterministic Markdown for this comparison result."""

        return self.to_markdown(path)

    def to_debug_html(self, path: str | Path | None = None) -> str:
        """Return a self-contained debug HTML visualiser."""

        max_rows_per_type = 200
        row_blocks: list[str] = []
        for label, color, rows in (
            ("Overlap", "#2f7d32", self.overlap_rows),
            ("Residual", "#c77800", self.residual_rows),
            ("Missing", "#b3261e", self.missing_rows),
            ("Coverage", "#2551a8", self.coverage_rows),
            ("Gap", "#6b7280", self.gap_rows),
            ("Symmetric Difference", "#8b2fc9", self.symmetric_difference_rows),
            ("Containment", "#007c89", self.containment_rows),
            ("Lead/Lag", "#8f4b00", self.lead_lag_rows),
            ("As-Of", "#6d5dfc", self.as_of_rows),
        ):
            for row in rows[:max_rows_per_type]:
                row_blocks.append(_html_row(label, color, row))
            if len(rows) > max_rows_per_type:
                row_blocks.append(
                    f'<p style="color:#6b7280;font-style:italic">'
                    f"{html.escape(label)}: showing {max_rows_per_type} of {len(rows)} rows.</p>"
                )
        body = "\n".join(row_blocks) or "<p>No comparison rows.</p>"
        summary_rows = "".join(
            f"<tr><td>{html.escape(summary.comparator)}</td>"
            f"<td>{summary.row_count}</td></tr>"
            for summary in self.comparator_summaries
        )
        extension_rows = "".join(
            f"<li>{html.escape(metadata.extension_id)} "
            f"{html.escape(metadata.key)}={html.escape(metadata.value)}</li>"
            for metadata in self.extension_metadata
        )
        extension_block = (
            f"<h2>Extension metadata</h2><ul>{extension_rows}</ul>"
            if extension_rows
            else ""
        )
        diagnostics_block = _debug_html_json_section(
            "Diagnostics",
            self.diagnostics,
            "No diagnostics.",
            limit=120,
        )
        prepared_block = _debug_html_json_section(
            "Prepared",
            self.prepared,
            "No prepared comparison artifact.",
        )
        aligned_block = _debug_html_json_section(
            "Aligned Segments",
            self.aligned,
            "No aligned comparison artifact.",
        )
        text = f"""<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>{html.escape(self.name)} - Spanfold debug</title>
<style>
body {{ font-family: system-ui, sans-serif; margin: 2rem; color: #17202a; }}
.summary {{ border-collapse: collapse; margin-bottom: 1.5rem; }}
.summary th,.summary td {{
  border-bottom: 1px solid #d9dee7; padding: .35rem .6rem;
}}
.row {{
  margin: .7rem 0; display: grid; grid-template-columns: 9rem 1fr;
  gap: .75rem; align-items: center;
}}
.track {{
  position: relative; height: 1.5rem; background: #eef2f6;
  border-radius: 4px; overflow: hidden;
}}
.bar {{ position: absolute; top: 0; bottom: 0; min-width: 2px; }}
.meta {{ font-size: .9rem; }}
pre {{
  white-space: pre-wrap; word-break: break-word; background: #17202a;
  color: #eef2f6; padding: .75rem; border-radius: 4px;
}}
</style>
</head>
<body>
<h1>{html.escape(self.name)}</h1>
<table class="summary"><tr><th>Comparator</th><th>Rows</th></tr>
{summary_rows}
</table>
{extension_block}
{diagnostics_block}
{prepared_block}
{aligned_block}
{body}
</body>
</html>
"""
        if path is not None:
            _write_text_if_requested(path, text)
        return text

    def export_debug_html(self, path: str | Path | None = None) -> str:
        """Return a self-contained debug HTML document and optionally write it."""

        return self.to_debug_html(path)

    def export_llm_context(self, path: str | Path | None = None) -> str:
        """Return deterministic LLM context JSON and optionally write it."""

        if self.plan is not None:
            _ensure_exportable(self.plan)
        row_documents = [
            json.loads(line)
            for line in self.export_json_lines().splitlines()
            if line.strip()
        ]
        payload = {
            "schema": "spanfold.comparison.llm-context",
            "schemaVersion": 0,
            "artifact": "llm-context",
            "purpose": (
                "Portable comparison context for LLMs, coding agents, CI triage, "
                "and support handoff."
            ),
            "analysisInstructions": [
                (
                    "Treat fullResult as the source of truth for exact fields, "
                    "ranges, windows, segments, tags, diagnostics, summaries, "
                    "and row evidence."
                ),
                (
                    "Use resultMarkdown for a concise natural-language orientation "
                    "before drilling into fullResult."
                ),
                (
                    "Use rowDocuments when chunking or streaming row-level analysis; "
                    "rowDocuments[0] is the result summary and later entries are "
                    "individual comparison rows."
                ),
                (
                    "Preserve rowId, recordIds, window ids, temporal ranges, knownAt, "
                    "evaluationHorizon, and finality metadata when citing evidence."
                ),
                (
                    "Do not infer missing source data from absence alone; check "
                    "diagnostics, normalization, excluded windows, and row finalities first."
                ),
            ],
            "summary": _llm_summary_payload(self),
            "resultMarkdown": self.to_markdown(),
            "fullResult": _result_export_payload(self),
            "rowDocuments": row_documents,
        }
        text = json.dumps(payload, indent=2, sort_keys=True)
        _write_text_if_requested(path, text)
        return text

    def to_llm_context(self, path: str | Path | None = None) -> str:
        """Return deterministic LLM context JSON and optionally write it."""

        return self.export_llm_context(path)

    def explain(
        self,
        format: ComparisonExplanationFormat = ComparisonExplanationFormat.MARKDOWN,
        *,
        markdown: bool | None = None,
        path: str | Path | None = None,
    ) -> str:
        """Return deterministic human-readable comparison output."""

        use_markdown = (
            format is ComparisonExplanationFormat.MARKDOWN
            if markdown is None
            else markdown
        )
        prefix = "# " if use_markdown else ""
        lines = [f"{prefix}Comparison Explain: {self.name}", ""]
        if self.diagnostic_rows:
            lines.append("## Diagnostics" if use_markdown else "Diagnostics")
            for index, diagnostic in enumerate(self.diagnostic_rows):
                lines.append(
                    f"- diagnostic[{index}]: {diagnostic.severity.value} "
                    f"{diagnostic.code} path={diagnostic.path}"
                )
            lines.append("")
        if self.extension_metadata:
            lines.append("## Extension Metadata" if use_markdown else "Extension Metadata")
            for index, metadata in enumerate(self.extension_metadata):
                lines.append(
                    f"- extensionMetadata[{index}]: "
                    f"{metadata.extension_id}.{metadata.key}={metadata.value}"
                )
            lines.append("")
        if self.comparator_summaries:
            lines.append("## Summaries" if use_markdown else "Summaries")
            for summary in self.comparator_summaries:
                lines.append(f"- comparator: {summary.comparator}; rows={summary.row_count}")
            lines.append("")
        for row_type, values in _result_row_groups(self):
            for index, row in enumerate(values):
                record_ids = _row_record_ids(row)
                lines.append(
                    f"- {row_type}[{index}]: {_row_range_or_point(row)}; "
                    f"records={','.join(str(record_id) for record_id in record_ids)}"
                )
        text = "\n".join(lines).rstrip() + "\n"
        _write_text_if_requested(path, text)
        return text


@dataclass(frozen=True, slots=True)
class _NormalizedWindow:
    window: WindowRecord
    source_label: str
    range: TemporalRange
    finality: ComparisonFinality
    record_ids: tuple[WindowRecordId, ...] = ()


@dataclass(frozen=True, slots=True)
class _ExecutionSegment:
    window_name: str
    key: Any
    partition: Any
    range: TemporalRange
    target_record_ids: tuple[WindowRecordId, ...]
    against_record_ids: tuple[WindowRecordId, ...]
    finality: ComparisonFinality


@dataclass(frozen=True, slots=True)
class _CohortDefinition:
    name: str
    sources: tuple[Any, ...]
    activity: CohortActivity


@dataclass(frozen=True, slots=True)
class _PreparedRun:
    plan: ComparisonPlan
    target_windows: list[_NormalizedWindow]
    against_windows: list[_NormalizedWindow]
    diagnostics: list[str]
    prepared: PreparedComparison
    extension_metadata: tuple[ComparisonExtensionMetadata, ...]


class _Any:
    pass


_ANY = _Any()


class WindowComparisonBuilder:
    """Staged comparison builder for recorded window history."""

    def __init__(self, history: WindowHistory, name: str) -> None:
        if not name or not name.strip():
            msg = "Comparison name cannot be empty."
            raise ValueError(msg)
        self._history = history
        self._name = name
        self._target: Callable[[WindowRecord], bool] | None = None
        self._target_selector: ComparisonSelector | None = None
        self._against: list[Callable[[WindowRecord], bool]] = []
        self._against_selectors: list[ComparisonSelector] = []
        self._cohorts: list[_CohortDefinition] = []
        self._window_name: str | None = None
        self._key: Any = _ANY
        self._partition: Any = _ANY
        self._segments: dict[str, Any] = {}
        self._tags: dict[str, Any] = {}
        self._comparators: tuple[str, ...] = ("overlap", "residual", "missing")
        self._axis = TemporalAxis.PROCESSING_POSITION
        self._horizon: TemporalPoint | None = None
        self._known_at: TemporalPoint | None = None
        self._missing_event_time_policy = "reject"
        self._coalesce_adjacent = False
        self._duplicate_windows = "preserve"
        self._output = ComparisonOutputOptions.default()
        self._strict = False
        self._extension_metadata: list[ComparisonExtensionMetadata] = []

    def target(
        self,
        source: Any | None = None,
        *,
        predicate: Callable[[WindowRecord], bool] | None = None,
    ) -> WindowComparisonBuilder:
        """Select the target side by source or custom predicate."""

        self._target = (
            predicate if predicate is not None else lambda window: window.source == source
        )
        self._target_selector = (
            ComparisonSelector.runtime_only("target", "runtime target selector", predicate)
            if predicate is not None
            else ComparisonSelector.for_source(source)
        )
        return self

    def against(
        self,
        source: Any | None = None,
        *,
        predicate: Callable[[WindowRecord], bool] | None = None,
    ) -> WindowComparisonBuilder:
        """Add a comparison side by source or custom predicate."""

        selector = (
            ComparisonSelector.runtime_only(
                f"against-{len(self._against) + 1}",
                "runtime comparison selector",
                predicate,
            )
            if predicate is not None
            else ComparisonSelector.for_source(source)
        )
        self._against.append(selector.matches)
        self._against_selectors.append(selector)
        return self

    def against_cohort(
        self,
        name: str,
        *,
        sources: Iterable[Any],
        activity: CohortActivity | None = None,
    ) -> WindowComparisonBuilder:
        """Add a derived comparison lane backed by a source cohort."""

        cohort_sources = tuple(sources)
        if not cohort_sources:
            msg = "Cohort must declare at least one source."
            raise ValueError(msg)
        cohort_activity = activity or CohortActivity.any()
        if (
            cohort_activity.count is not None
            and cohort_activity.count > len(cohort_sources)
        ):
            msg = "Cohort activity count cannot exceed the number of declared sources."
            raise ValueError(msg)
        self._cohorts.append(_CohortDefinition(name, cohort_sources, cohort_activity))
        self._against_selectors.append(
            ComparisonSelector.for_cohort_sources(cohort_sources, cohort_activity).with_name(name)
        )
        return self

    def within(
        self,
        scope: ComparisonScope
        | Callable[[ComparisonScopeBuilder], ComparisonScope]
        | None = None,
        *,
        window_name: str | None = None,
        key: Any = _ANY,
        partition: Any = _ANY,
        segments: Mapping[str, Any] | None = None,
        tags: Mapping[str, Any] | None = None,
    ) -> WindowComparisonBuilder:
        """Limit comparison scope by window, key, or partition."""

        if scope is not None:
            resolved = (
                scope(ComparisonScopeBuilder())
                if callable(scope)
                else scope
            )
            window_name = resolved.window_name
            segments = {item.name: item.value for item in resolved.segment_filters}
            tags = {item.name: item.value for item in resolved.tag_filters}

        self._window_name = window_name
        self._key = key
        self._partition = partition
        self._segments = dict(segments or {})
        self._tags = dict(tags or {})
        return self

    def normalize(
        self,
        policy: ComparisonNormalizationPolicy
        | Callable[[ComparisonNormalizationBuilder], ComparisonNormalizationBuilder]
        | None = None,
        *,
        axis: TemporalAxis = TemporalAxis.PROCESSING_POSITION,
        horizon: TemporalPoint | None = None,
        known_at: TemporalPoint | None = None,
        missing_event_time: str = "reject",
        coalesce_adjacent: bool = False,
        duplicate_windows: str = "preserve",
    ) -> WindowComparisonBuilder:
        """Choose the comparison axis, open-window horizon, and availability point."""

        if policy is not None:
            resolved = (
                policy(ComparisonNormalizationBuilder()).build()
                if callable(policy)
                else policy
            )
            self._apply_normalization_policy(resolved)
            return self
        if missing_event_time not in {"reject", "exclude"}:
            msg = "missing_event_time must be 'reject' or 'exclude'."
            raise ValueError(msg)
        if duplicate_windows not in {"preserve", "reject"}:
            msg = "duplicate_windows must be 'preserve' or 'reject'."
            raise ValueError(msg)
        self._axis = axis
        self._horizon = horizon
        self._known_at = known_at
        self._missing_event_time_policy = missing_event_time
        self._coalesce_adjacent = coalesce_adjacent
        self._duplicate_windows = duplicate_windows
        return self

    def _apply_normalization_policy(
        self,
        policy: ComparisonNormalizationPolicy,
    ) -> None:
        self._axis = policy.time_axis
        self._horizon = policy.open_window_horizon
        self._known_at = policy.known_at
        self._missing_event_time_policy = policy.null_timestamp_policy.value
        self._coalesce_adjacent = policy.coalesce_adjacent_windows
        self._duplicate_windows = policy.duplicate_window_policy.value

    def using(
        self,
        *comparators: str | Callable[[ComparisonComparatorBuilder], ComparisonComparatorBuilder],
    ) -> WindowComparisonBuilder:
        """Choose comparators to run by name."""

        if len(comparators) == 1 and callable(comparators[0]):
            configure = cast(
                Callable[[ComparisonComparatorBuilder], ComparisonComparatorBuilder],
                comparators[0],
            )
            comparators = configure(ComparisonComparatorBuilder()).build()
        self._comparators = (
            tuple(_normalize_comparator(cast(str, comparator)) for comparator in comparators)
            or self._comparators
        )
        return self

    def output(self, options: ComparisonOutputOptions) -> WindowComparisonBuilder:
        """Set comparison result output preferences."""

        _require_not_none(options, "Output options")
        self._output = options
        return self

    def strict(self) -> WindowComparisonBuilder:
        """Promote warning diagnostics to blocking errors for this comparison."""

        self._strict = True
        return self

    def strict_if(self, is_strict: bool) -> WindowComparisonBuilder:
        """Promote warnings only when ``is_strict`` is true."""

        return self.strict() if is_strict else self

    def build(self) -> ComparisonPlan:
        """Build the current comparison plan without executing it."""

        return self._create_plan()

    def validate(self) -> tuple[ComparisonPlanDiagnostic, ...]:
        """Validate the current comparison plan."""

        return self.build().validate()

    def prepare(self) -> PreparedComparison:
        """Select and normalize windows for the current comparison plan."""

        return self._prepare_run().prepared

    def prepare_live(self, horizon: TemporalPoint) -> PreparedComparison:
        """Prepare the comparison with open windows clipped to an evaluation horizon."""

        previous = self._normalization_state()
        try:
            self.normalize(axis=horizon.axis, horizon=horizon)
            return self.prepare()
        finally:
            self._restore_normalization_state(previous)

    def run(
        self,
        export: ComparisonDebugHtmlOptions | ComparisonLlmContextOptions | None = None,
        *,
        debug_html: ComparisonDebugHtmlOptions | None = None,
        llm_context: ComparisonLlmContextOptions | None = None,
    ) -> ComparisonResult:
        """Run the configured comparison."""

        prepared_run = self._prepare_run()
        result = _run_comparison(
            self._name,
            prepared_run.target_windows,
            prepared_run.against_windows,
            self._comparators,
            evaluation_horizon=self._horizon,
            known_at=self._known_at,
            diagnostics=prepared_run.diagnostics,
            strict=self._strict,
            extension_metadata=prepared_run.extension_metadata,
            plan=prepared_run.plan,
            prepared=prepared_run.prepared,
        )
        _export_configured_result(
            result,
            export=export,
            debug_html=debug_html,
            llm_context=llm_context,
        )
        return result

    def _prepare_run(self) -> _PreparedRun:
        if self._target is None:
            msg = "Comparison target selector is required."
            raise ValueError(msg)
        if not self._against and not self._cohorts:
            msg = "At least one against selector is required."
            raise ValueError(msg)

        diagnostics = self._normalization_diagnostics()
        plan = self._create_plan()
        target_windows = self._select(self._target, "target", diagnostics)
        against_windows = [
            item
            for index, selector in enumerate(self._against)
            for item in self._select(selector, f"against-{index + 1}", diagnostics)
        ]
        self._extension_metadata = []
        for cohort in self._cohorts:
            cohort_windows, metadata = self._select_cohort(cohort, target_windows, diagnostics)
            against_windows.extend(cohort_windows)
            self._extension_metadata.extend(metadata)
        target_windows = self._postprocess_normalized(target_windows, diagnostics)
        against_windows = self._postprocess_normalized(against_windows, diagnostics)
        prepared = self._create_prepared(
            plan,
            target_windows,
            against_windows,
            diagnostics=diagnostics,
        )
        return _PreparedRun(
            plan,
            target_windows,
            against_windows,
            diagnostics,
            prepared,
            tuple(self._extension_metadata),
        )

    def run_live(
        self,
        horizon: TemporalPoint,
        export: ComparisonDebugHtmlOptions | ComparisonLlmContextOptions | None = None,
        *,
        debug_html: ComparisonDebugHtmlOptions | None = None,
        llm_context: ComparisonLlmContextOptions | None = None,
    ) -> ComparisonResult:
        """Run the comparison with open windows clipped to an evaluation horizon."""

        previous = self._normalization_state()
        try:
            self.normalize(axis=horizon.axis, horizon=horizon)
            return self.run(
                export,
                debug_html=debug_html,
                llm_context=llm_context,
            )
        finally:
            self._restore_normalization_state(previous)

    def _normalization_state(
        self,
    ) -> tuple[
        TemporalAxis,
        TemporalPoint | None,
        TemporalPoint | None,
        str,
        bool,
        str,
        ComparisonOutputOptions,
    ]:
        return (
            self._axis,
            self._horizon,
            self._known_at,
            self._missing_event_time_policy,
            self._coalesce_adjacent,
            self._duplicate_windows,
            self._output,
        )

    def _restore_normalization_state(
        self,
        state: tuple[
            TemporalAxis,
            TemporalPoint | None,
            TemporalPoint | None,
            str,
            bool,
            str,
            ComparisonOutputOptions,
        ],
    ) -> None:
        (
            self._axis,
            self._horizon,
            self._known_at,
            self._missing_event_time_policy,
            self._coalesce_adjacent,
            self._duplicate_windows,
            self._output,
        ) = state

    def _select(
        self,
        selector: Callable[[WindowRecord], bool],
        label: str,
        diagnostics: list[str],
    ) -> list[_NormalizedWindow]:
        rows: list[_NormalizedWindow] = []
        for window in self._history.windows:
            if self._window_name is not None and window.window_name != self._window_name:
                continue
            if self._key is not _ANY and window.key != self._key:
                continue
            if self._partition is not _ANY and window.partition != self._partition:
                continue
            if not _has_segments(window, self._segments):
                continue
            if not _has_tags(window, self._tags):
                continue
            if not selector(window):
                continue
            if not self._is_known_at(window):
                _append_once(diagnostics, "future_window_excluded")
                continue
            try:
                temporal_range = window.range_for_axis(self._axis, horizon=self._horizon)
            except ValueError:
                _append_once(diagnostics, "invalid_range_duration")
                continue
            if temporal_range is None:
                self._diagnose_unusable_window(window, diagnostics)
                continue
            finality = (
                ComparisonFinality.FINAL
                if temporal_range.is_closed
                else ComparisonFinality.PROVISIONAL
            )
            rows.append(_NormalizedWindow(window, label, temporal_range, finality))
        return rows

    def _select_cohort(
        self,
        cohort: _CohortDefinition,
        targets: list[_NormalizedWindow],
        diagnostics: list[str],
    ) -> tuple[list[_NormalizedWindow], tuple[ComparisonExtensionMetadata, ...]]:
        member_windows = self._select(
            lambda window: window.source in cohort.sources,
            cohort.name,
            diagnostics,
        )
        return _materialize_cohort(cohort, member_windows, targets)

    def _normalization_diagnostics(self) -> list[str]:
        diagnostics: list[str] = []
        if (
            self._known_at is not None
            and self._known_at.axis is not TemporalAxis.PROCESSING_POSITION
        ):
            diagnostics.append("known_at_requires_processing_position")
        if self._known_at is None and any(
            _try_parse_as_of(comparator) is not None for comparator in self._comparators
        ):
            diagnostics.append("future_leakage_risk")
        if (
            self._horizon is not None
            and self._known_at is not None
            and self._horizon.axis is TemporalAxis.TIMESTAMP
            and self._known_at.axis is TemporalAxis.TIMESTAMP
            and self._horizon.clock != self._known_at.clock
        ):
            diagnostics.append("mixed_clock_risk")
        return diagnostics

    def _is_known_at(self, window: WindowRecord) -> bool:
        if self._known_at is None:
            return True
        if self._known_at.axis is not TemporalAxis.PROCESSING_POSITION:
            return True
        available_position = (
            window.end_position if window.end_position is not None else window.start_position
        )
        return available_position <= self._known_at.position

    def _diagnose_unusable_window(
        self,
        window: WindowRecord,
        diagnostics: list[str],
    ) -> None:
        if self._axis is TemporalAxis.TIMESTAMP and _missing_event_time(window):
            if self._missing_event_time_policy == "reject":
                _append_once(diagnostics, "missing_event_time")
            return
        if not window.is_closed and self._horizon is None:
            _append_once(diagnostics, "open_windows_without_policy")
            _append_once(diagnostics, "unbounded_open_duration")

    def _postprocess_normalized(
        self,
        windows: list[_NormalizedWindow],
        diagnostics: list[str],
    ) -> list[_NormalizedWindow]:
        deduplicated = _deduplicate_normalized(
            windows,
            diagnostics,
            reject=self._duplicate_windows == "reject",
        )
        if self._coalesce_adjacent:
            return _coalesce_normalized(deduplicated)
        return deduplicated

    def _create_plan(self) -> ComparisonPlan:
        scope = ComparisonScope(
            self._window_name,
            self._axis,
            tuple(WindowSegmentFilter(name, value) for name, value in self._segments.items()),
            tuple(WindowTagFilter(name, value) for name, value in self._tags.items()),
        )
        return ComparisonPlan(
            self._name,
            self._target_selector,
            tuple(self._against_selectors),
            scope,
            self._normalization_policy(),
            self._comparators,
            self._output,
            self._strict,
        )

    def _normalization_policy(self) -> ComparisonNormalizationPolicy:
        open_policy = (
            ComparisonOpenWindowPolicy.CLIP_TO_HORIZON
            if self._horizon is not None
            else ComparisonOpenWindowPolicy.REQUIRE_CLOSED
        )
        return ComparisonNormalizationPolicy(
            require_closed_windows=self._horizon is None,
            time_axis=self._axis,
            open_window_policy=open_policy,
            open_window_horizon=self._horizon,
            null_timestamp_policy=ComparisonNullTimestampPolicy(self._missing_event_time_policy),
            coalesce_adjacent_windows=self._coalesce_adjacent,
            duplicate_window_policy=ComparisonDuplicateWindowPolicy(self._duplicate_windows),
            known_at=self._known_at,
        )

    def _create_prepared(
        self,
        plan: ComparisonPlan,
        targets: list[_NormalizedWindow],
        against: list[_NormalizedWindow],
        *,
        diagnostics: Iterable[str],
    ) -> PreparedComparison:
        normalized = [
            *(
                _to_normalized_record(item, ComparisonSide.TARGET)
                for item in targets
            ),
            *(
                _to_normalized_record(item, ComparisonSide.AGAINST)
                for item in against
            ),
        ]
        selected = tuple(record.window for record in normalized)
        return PreparedComparison(
            plan,
            (*plan.validate(), *(_plan_diagnostic_from_code(code) for code in diagnostics)),
            selected,
            (),
            tuple(normalized),
        )


def _run_comparison(
    name: str,
    targets: list[_NormalizedWindow],
    against: list[_NormalizedWindow],
    comparators: tuple[str, ...],
    *,
    evaluation_horizon: TemporalPoint | None = None,
    known_at: TemporalPoint | None = None,
    diagnostics: Iterable[str] = (),
    strict: bool = False,
    extension_metadata: tuple[ComparisonExtensionMetadata, ...] = (),
    plan: ComparisonPlan | None = None,
    prepared: PreparedComparison | None = None,
) -> ComparisonResult:
    target_groups = _group(targets)
    against_groups = _group(against)
    scopes = sorted(
        set(target_groups) | set(against_groups), key=lambda item: tuple(map(repr, item))
    )

    overlap_rows: list[OverlapRow] = []
    residual_rows: list[ResidualRow] = []
    missing_rows: list[MissingRow] = []
    coverage_rows: list[CoverageRow] = []
    gap_rows: list[GapRow] = []
    symmetric_difference_rows: list[SymmetricDifferenceRow] = []
    containment_rows: list[ContainmentRow] = []
    lead_lag_rows: list[LeadLagRow] = []
    lead_lag_summaries: list[LeadLagSummary] = []
    as_of_rows: list[AsOfRow] = []
    aligned = prepared.align() if prepared is not None else None
    result_diagnostics = list(diagnostics)
    lead_lag_options: list[tuple[str, _LeadLagOptions]] = []
    as_of_options: list[tuple[str, _AsOfOptions]] = []
    for comparator in comparators:
        if not ComparisonComparatorCatalog.is_known_declaration(comparator):
            _append_once(result_diagnostics, "unknown_comparator")
            continue
        lead_lag_option = _try_parse_lead_lag(comparator)
        if lead_lag_option is not None:
            lead_lag_options.append((comparator, lead_lag_option))
        as_of_option = _try_parse_as_of(comparator)
        if as_of_option is not None:
            as_of_options.append((comparator, as_of_option))

    execution_segments = _execution_segments_by_scope(scopes, target_groups, against_groups)
    for scope in scopes:
        target_items = sorted(target_groups.get(scope, ()), key=lambda item: item.range.start)
        against_items = sorted(against_groups.get(scope, ()), key=lambda item: item.range.start)
        window_name, key, partition = scope
        scope_segments = execution_segments.get(scope, ())

        if "overlap" in comparators or "coverage" in comparators:
            if "overlap" in comparators:
                overlap_rows.extend(_overlap_rows_from_segments(scope_segments))
            if "coverage" in comparators:
                coverage_by_target = _coverage_by_target_id(scope_segments)
                against_ids_by_target = _against_ids_by_target_id(scope_segments)
                for target in target_items:
                    target_ids = _record_ids(target)
                    covered = sum(
                        coverage_by_target.get(record_id, 0.0) for record_id in target_ids
                    )
                    target_against_ids = tuple(
                        dict.fromkeys(
                            record_id
                            for target_id in target_ids
                            for record_id in against_ids_by_target.get(target_id, ())
                        )
                    )
                    coverage_rows.append(
                        CoverageRow(
                            window_name,
                            key,
                            partition,
                            target.range,
                            target.range.magnitude(),
                            covered,
                            target_ids,
                            target_against_ids,
                            ComparisonFinality.PROVISIONAL
                            if target.finality is ComparisonFinality.PROVISIONAL
                            or _contains_provisional_against(target_against_ids, against_items)
                            else ComparisonFinality.FINAL,
                        )
                    )

        if "residual" in comparators:
            residual_rows.extend(_residual_rows_from_segments(scope_segments))

        if "missing" in comparators:
            missing_rows.extend(_missing_rows_from_segments(scope_segments))

        if "gap" in comparators:
            gap_rows.extend(_gap_rows_from_segments(scope_segments))

        if "symmetric_difference" in comparators:
            symmetric_difference_rows.extend(_symmetric_difference_rows_from_segments(scope_segments))

        if "containment" in comparators:
            containment_rows.extend(
                _containment_rows_from_segments(scope_segments, target_items)
            )

    lead_lag_counts: dict[str, int] = {}
    for comparator, lead_lag_option in lead_lag_options:
        before = len(lead_lag_rows)
        for scope in scopes:
            window_name, key, partition = scope
            lead_lag_rows.extend(
                _lead_lag_rows(
                    window_name,
                    key,
                    partition,
                    target_groups.get(scope, ()),
                    against_groups.get(scope, ()),
                    lead_lag_option,
                )
            )
        lead_lag_counts[comparator] = len(lead_lag_rows) - before
        lead_lag_summaries.append(
            _lead_lag_summary(
                lead_lag_option,
                lead_lag_rows[before:],
                lead_lag_counts[comparator],
            )
        )

    as_of_counts: dict[str, int] = {}
    for comparator, as_of_option in as_of_options:
        before = len(as_of_rows)
        for scope in scopes:
            window_name, key, partition = scope
            as_of_rows.extend(
                _as_of_rows(
                    window_name,
                    key,
                    partition,
                    target_groups.get(scope, ()),
                    against_groups.get(scope, ()),
                    as_of_option,
                    result_diagnostics,
                )
            )
        as_of_counts[comparator] = len(as_of_rows) - before

    summaries = tuple(
        ComparatorSummary(
            comparator,
            _count_for(
                comparator,
                overlap_rows,
                residual_rows,
                missing_rows,
                coverage_rows,
                gap_rows,
                symmetric_difference_rows,
                containment_rows,
                lead_lag_rows,
                lead_lag_counts,
                as_of_rows,
                as_of_counts,
            ),
        )
        for comparator in comparators
    )
    coverage_summaries = _coverage_summaries(coverage_rows)
    return ComparisonResult(
        name=name,
        overlap_rows=tuple(overlap_rows),
        residual_rows=tuple(residual_rows),
        missing_rows=tuple(missing_rows),
        coverage_rows=tuple(coverage_rows),
        gap_rows=tuple(gap_rows),
        symmetric_difference_rows=tuple(symmetric_difference_rows),
        containment_rows=tuple(containment_rows),
        lead_lag_rows=tuple(lead_lag_rows),
        lead_lag_summaries=tuple(lead_lag_summaries),
        as_of_rows=tuple(as_of_rows),
        comparator_summaries=summaries,
        coverage_summaries=coverage_summaries,
        diagnostics=tuple(result_diagnostics),
        evaluation_horizon=evaluation_horizon,
        known_at=known_at,
        strict=strict,
        extension_metadata=extension_metadata,
        plan=plan,
        prepared=prepared,
        aligned=aligned,
    )


def _materialize_cohort(
    cohort: _CohortDefinition,
    members: list[_NormalizedWindow],
    targets: list[_NormalizedWindow],
) -> tuple[list[_NormalizedWindow], tuple[ComparisonExtensionMetadata, ...]]:
    grouped = _group(members)
    target_groups = _group(targets)
    materialized: list[_NormalizedWindow] = []
    metadata: list[ComparisonExtensionMetadata] = []
    segment_index = 0
    for scope in sorted(set(grouped) | set(target_groups), key=lambda item: tuple(map(repr, item))):
        window_name, key, partition = scope
        items = grouped.get(scope, [])
        scope_targets = target_groups.get(scope, [])
        boundaries = sorted(
            {
                point
                for item in [*items, *scope_targets]
                for point in (item.range.start, item.range.require_end())
            }
        )
        for start, end in zip(boundaries, boundaries[1:], strict=False):
            if start >= end:
                continue
            active_items = [
                item
                for item in items
                if item.range.start <= start and end <= item.range.require_end()
            ]
            active_sources = {item.window.source for item in active_items}
            is_active = cohort.activity.is_active(len(active_sources), len(cohort.sources))
            if active_items or any(
                item.range.start <= start and end <= item.range.require_end()
                for item in scope_targets
            ):
                metadata.append(
                    ComparisonExtensionMetadata(
                        "spanfold.cohort",
                        f"segment[{segment_index}]",
                        _cohort_evidence_value(cohort, active_sources, is_active),
                    )
                )
                segment_index += 1
            if not is_active:
                continue
            record = ClosedWindow(
                window_name,
                key,
                start.position,
                end.position,
                source=cohort.name,
                partition=partition,
            )
            materialized.append(
                _NormalizedWindow(
                    record,
                    cohort.name,
                    TemporalRange.closed(start, end),
                    ComparisonFinality.PROVISIONAL
                    if any(
                        item.finality is ComparisonFinality.PROVISIONAL
                        for item in active_items
                    )
                    else ComparisonFinality.FINAL,
                    tuple(item.window.id for item in active_items),
                )
            )
    return materialized, tuple(metadata)


def _cohort_evidence_value(
    cohort: _CohortDefinition,
    active_sources: Iterable[Any],
    is_active: bool,
) -> str:
    ordered_sources = sorted((str(source) for source in active_sources), key=repr)
    return (
        f"rule={cohort.activity.name.replace('_', '-')}; "
        f"required={cohort.activity.required_count(len(cohort.sources))}; "
        f"activeCount={len(ordered_sources)}; "
        f"isActive={'true' if is_active else 'false'}; "
        f"activeSources={','.join(ordered_sources)}"
    )


def build_source_matrix(
    history: WindowHistory,
    name: str,
    window_name: str,
    sources: Iterable[Any],
) -> SourceMatrixResult:
    """Build a directional pairwise source matrix for one recorded window."""

    ordered_sources = tuple(sources)
    if not ordered_sources:
        msg = "At least one source is required."
        raise ValueError(msg)

    source_has_windows = {
        source: any(
            window.window_name == window_name and window.source == source
            for window in history.windows
        )
        for source in ordered_sources
    }
    cells: list[SourceMatrixCell] = []

    for target_source in ordered_sources:
        for against_source in ordered_sources:
            target_has_windows = source_has_windows[target_source]
            against_has_windows = source_has_windows[against_source]
            if target_source == against_source:
                cells.append(
                    SourceMatrixCell(
                        target_source,
                        against_source,
                        True,
                        target_has_windows,
                        against_has_windows,
                        0,
                        0,
                        0,
                        0,
                        1.0 if target_has_windows else None,
                    )
                )
                continue

            result = (
                history.compare(f"{name} {target_source} vs {against_source}")
                .target(target_source)
                .against(against_source)
                .within(window_name=window_name)
                .using("overlap", "residual", "missing", "coverage")
                .run()
            )
            cells.append(
                SourceMatrixCell(
                    target_source,
                    against_source,
                    False,
                    target_has_windows,
                    against_has_windows,
                    len(result.overlap_rows),
                    len(result.residual_rows),
                    len(result.missing_rows),
                    len(result.coverage_rows),
                    _aggregate_coverage_ratio(result.coverage_rows),
                )
            )

    return SourceMatrixResult(name, window_name, ordered_sources, tuple(cells))


def build_hierarchy_comparison(
    history: WindowHistory,
    name: str,
    parent_window_name: str,
    child_window_name: str,
) -> HierarchyComparisonResult:
    """Compare parent windows against child contribution windows."""

    parents = [window for window in history.windows if window.window_name == parent_window_name]
    children = [window for window in history.windows if window.window_name == child_window_name]
    diagnostics: list[str] = []
    if not parents:
        diagnostics.append("missing_parent_lineage")
    if not children:
        diagnostics.append("missing_child_lineage")

    rows: list[HierarchyComparisonRow] = []
    scopes = sorted(
        {
            (window.source, window.partition)
            for window in [*parents, *children]
        },
        key=lambda item: (repr(item[0]), repr(item[1])),
    )
    for source, partition in scopes:
        rows.extend(_hierarchy_rows_for_scope(source, partition, parents, children))

    return HierarchyComparisonResult(
        name,
        parent_window_name,
        child_window_name,
        tuple(rows),
        tuple(diagnostics),
    )


def total_position_length(rows: Iterable[Any]) -> int:
    """Return total processing-position length across rows with ranges."""

    return sum(row.range.position_length() for row in rows)


def total_time_duration(rows: Iterable[Any]) -> timedelta:
    """Return total event-time duration across rows with ranges."""

    total = timedelta()
    for row in rows:
        total += row.range.time_duration()
    return total


def total_target_magnitude(rows: Iterable[CoverageRow]) -> float:
    """Return total target denominator magnitude across coverage rows."""

    return sum(row.target_magnitude for row in rows)


def total_covered_magnitude(rows: Iterable[CoverageRow]) -> float:
    """Return total comparison-covered numerator magnitude across coverage rows."""

    return sum(row.covered_magnitude for row in rows)


def _hierarchy_rows_for_scope(
    source: Any,
    partition: Any,
    parents: list[WindowRecord],
    children: list[WindowRecord],
) -> list[HierarchyComparisonRow]:
    scoped_parents = [
        window for window in parents if window.source == source and window.partition == partition
    ]
    scoped_children = [
        window for window in children if window.source == source and window.partition == partition
    ]
    boundaries = sorted(
        {
            point
            for window in [*scoped_parents, *scoped_children]
            if window.end_position is not None
            for point in (
                TemporalPoint.for_position(window.start_position),
                TemporalPoint.for_position(window.end_position),
            )
        }
    )
    rows: list[HierarchyComparisonRow] = []
    for start, end in zip(boundaries, boundaries[1:], strict=False):
        if start >= end:
            continue
        parent_ids = _active_hierarchy_ids(scoped_parents, start, end)
        child_ids = _active_hierarchy_ids(scoped_children, start, end)
        if not parent_ids and not child_ids:
            continue
        rows.append(
            HierarchyComparisonRow(
                _hierarchy_kind(len(parent_ids), len(child_ids)),
                source,
                partition,
                TemporalRange.closed(start, end),
                parent_ids,
                child_ids,
            )
        )
    return rows


def _active_hierarchy_ids(
    windows: list[WindowRecord],
    start: TemporalPoint,
    end: TemporalPoint,
) -> tuple[WindowRecordId, ...]:
    ids: list[WindowRecordId] = []
    for window in windows:
        temporal_range = window.range_for_axis()
        if temporal_range is None:
            continue
        if temporal_range.start <= start and end <= temporal_range.require_end():
            ids.append(window.id)
    return tuple(sorted(ids, key=lambda item: item.value))


def _hierarchy_kind(parent_count: int, child_count: int) -> HierarchyComparisonRowKind:
    if parent_count > 0 and child_count > 0:
        return HierarchyComparisonRowKind.PARENT_EXPLAINED
    if parent_count > 0:
        return HierarchyComparisonRowKind.UNEXPLAINED_PARENT
    return HierarchyComparisonRowKind.ORPHAN_CHILD


def _group(
    items: Iterable[_NormalizedWindow],
) -> dict[tuple[str, Any, Any], list[_NormalizedWindow]]:
    grouped: dict[tuple[str, Any, Any], list[_NormalizedWindow]] = defaultdict(list)
    for item in items:
        grouped[(item.window.window_name, item.window.key, item.window.partition)].append(item)
    return grouped


def _deduplicate_normalized(
    windows: list[_NormalizedWindow],
    diagnostics: list[str],
    *,
    reject: bool,
) -> list[_NormalizedWindow]:
    seen: set[tuple[str, str, str, str, str, TemporalPoint, TemporalPoint | None, str]] = set()
    rows: list[_NormalizedWindow] = []
    for window in windows:
        key = (
            window.source_label,
            window.window.window_name,
            repr(window.window.key),
            repr(window.window.source),
            repr(window.window.partition),
            window.range.start,
            window.range.end,
            window.range.end_status.value,
        )
        if key in seen:
            _append_once(diagnostics, "duplicate_window")
            if reject:
                continue
        else:
            seen.add(key)
        rows.append(window)
    return rows


def _coalesce_normalized(windows: list[_NormalizedWindow]) -> list[_NormalizedWindow]:
    groups: dict[
        tuple[str, str, str, str, str, str, str],
        list[_NormalizedWindow],
    ] = defaultdict(list)
    for window in windows:
        key = (
            window.source_label,
            window.window.window_name,
            repr(window.window.key),
            repr(window.window.source),
            repr(window.window.partition),
            repr(window.window.segments),
            repr(window.window.tags),
        )
        groups[key].append(window)

    rows: list[_NormalizedWindow] = []
    for _, items in sorted(groups.items(), key=lambda item: item[0]):
        ordered = sorted(items, key=lambda item: item.range.start)
        for item in ordered:
            if not rows or not _can_coalesce(rows[-1], item):
                rows.append(item)
                continue
            previous = rows[-1]
            rows[-1] = _NormalizedWindow(
                previous.window,
                previous.source_label,
                TemporalRange(
                    previous.range.start,
                    item.range.end,
                    item.range.end_status,
                ),
                _finality(previous, item),
                (*_record_ids(previous), *_record_ids(item)),
            )
    return rows


def _can_coalesce(first: _NormalizedWindow, second: _NormalizedWindow) -> bool:
    return (
        first.source_label == second.source_label
        and first.window.window_name == second.window.window_name
        and first.window.key == second.window.key
        and first.window.source == second.window.source
        and first.window.partition == second.window.partition
        and first.window.segments == second.window.segments
        and first.window.tags == second.window.tags
        and first.range.end == second.range.start
    )


def _record_ids(window: _NormalizedWindow) -> tuple[WindowRecordId, ...]:
    return window.record_ids or (window.window.id,)


def _to_normalized_record(
    window: _NormalizedWindow,
    side: ComparisonSide,
) -> NormalizedWindowRecord:
    return NormalizedWindowRecord(
        window.window,
        _primary_record_id(window),
        window.source_label,
        side,
        window.range,
        tuple(window.window.segments),
    )


def _primary_record_id(window: _NormalizedWindow) -> WindowRecordId:
    return _record_ids(window)[0]


def _has_segments(window: WindowRecord, required: Mapping[str, Any]) -> bool:
    return all(
        any(segment.name == name and segment.value == value for segment in window.segments)
        for name, value in required.items()
    )


def _has_tags(window: WindowRecord, required: Mapping[str, Any]) -> bool:
    return all(
        any(tag.name == name and tag.value == value for tag in window.tags)
        for name, value in required.items()
    )


def _coverage_row(
    window_name: str,
    key: Any,
    partition: Any,
    target: _NormalizedWindow,
    against: list[_NormalizedWindow],
) -> CoverageRow:
    intersections = [
        intersection
        for item in against
        if (intersection := target.range.intersection(item.range)) is not None
    ]
    covered = sum(_merged_ranges(intersections), start=0.0)
    return CoverageRow(
        window_name,
        key,
        partition,
        target.range,
        target.range.magnitude(),
        covered,
        _record_ids(target),
        tuple(record_id for item in against for record_id in _record_ids(item)),
        ComparisonFinality.PROVISIONAL
        if target.finality is ComparisonFinality.PROVISIONAL
        or any(item.finality is ComparisonFinality.PROVISIONAL for item in against)
        else ComparisonFinality.FINAL,
    )


def _execution_segments_by_scope(
    scopes: Iterable[tuple[str, Any, Any]],
    target_groups: Mapping[tuple[str, Any, Any], list[_NormalizedWindow]],
    against_groups: Mapping[tuple[str, Any, Any], list[_NormalizedWindow]],
) -> dict[tuple[str, Any, Any], tuple[_ExecutionSegment, ...]]:
    return {
        scope: tuple(
            _execution_segments(
                scope,
                target_groups.get(scope, ()),
                against_groups.get(scope, ()),
            )
        )
        for scope in scopes
    }


def _execution_segments(
    scope: tuple[str, Any, Any],
    targets: Iterable[_NormalizedWindow],
    against: Iterable[_NormalizedWindow],
) -> Iterator[_ExecutionSegment]:
    target_items = tuple(targets)
    against_items = tuple(against)
    boundaries = sorted(
        {
            point
            for item in (*target_items, *against_items)
            if item.range.end is not None
            for point in (item.range.start, item.range.end)
        }
    )
    window_name, key, partition = scope
    for start, end in zip(boundaries, boundaries[1:], strict=False):
        if start >= end:
            continue
        active_targets = tuple(
            item
            for item in target_items
            if item.range.end is not None and item.range.start <= start and end <= item.range.end
        )
        active_against = tuple(
            item
            for item in against_items
            if item.range.end is not None and item.range.start <= start and end <= item.range.end
        )
        yield _ExecutionSegment(
            window_name,
            key,
            partition,
            TemporalRange.closed(start, end),
            tuple(record_id for item in active_targets for record_id in _record_ids(item)),
            tuple(record_id for item in active_against for record_id in _record_ids(item)),
            ComparisonFinality.PROVISIONAL
            if any(
                item.finality is ComparisonFinality.PROVISIONAL
                for item in (*active_targets, *active_against)
            )
            else ComparisonFinality.FINAL,
        )


def _overlap_rows_from_segments(segments: Iterable[_ExecutionSegment]) -> list[OverlapRow]:
    return [
        OverlapRow(
            segment.window_name,
            segment.key,
            segment.partition,
            segment.range,
            segment.target_record_ids,
            segment.against_record_ids,
            segment.finality,
        )
        for segment in segments
        if segment.target_record_ids and segment.against_record_ids
    ]


def _coverage_by_target_id(
    segments: Iterable[_ExecutionSegment],
) -> dict[WindowRecordId, float]:
    coverage: dict[WindowRecordId, float] = defaultdict(float)
    for segment in segments:
        if not segment.against_record_ids:
            continue
        magnitude = segment.range.magnitude()
        for record_id in segment.target_record_ids:
            coverage[record_id] += magnitude
    return coverage


def _against_ids_by_target_id(
    segments: Iterable[_ExecutionSegment],
) -> dict[WindowRecordId, tuple[WindowRecordId, ...]]:
    against_ids: dict[WindowRecordId, dict[WindowRecordId, None]] = defaultdict(dict)
    for segment in segments:
        if not segment.against_record_ids:
            continue
        for target_id in segment.target_record_ids:
            for against_id in segment.against_record_ids:
                against_ids[target_id][against_id] = None
    return {target_id: tuple(ids) for target_id, ids in against_ids.items()}


def _contains_provisional_against(
    record_ids: Iterable[WindowRecordId],
    against: Iterable[_NormalizedWindow],
) -> bool:
    provisional_ids = {
        record_id
        for item in against
        if item.finality is ComparisonFinality.PROVISIONAL
        for record_id in _record_ids(item)
    }
    return any(record_id in provisional_ids for record_id in record_ids)


def _residual_rows_from_segments(segments: Iterable[_ExecutionSegment]) -> list[ResidualRow]:
    return [
        ResidualRow(
            segment.window_name,
            segment.key,
            segment.partition,
            segment.range,
            segment.target_record_ids,
            segment.finality,
        )
        for segment in segments
        if segment.target_record_ids and not segment.against_record_ids
    ]


def _missing_rows_from_segments(segments: Iterable[_ExecutionSegment]) -> list[MissingRow]:
    return [
        MissingRow(
            segment.window_name,
            segment.key,
            segment.partition,
            segment.range,
            segment.against_record_ids,
            segment.finality,
        )
        for segment in segments
        if segment.against_record_ids and not segment.target_record_ids
    ]


def _gap_rows_from_segments(segments: Iterable[_ExecutionSegment]) -> list[GapRow]:
    return [
        GapRow(segment.window_name, segment.key, segment.partition, segment.range)
        for segment in segments
        if not segment.target_record_ids and not segment.against_record_ids
    ]


def _symmetric_difference_rows_from_segments(
    segments: Iterable[_ExecutionSegment],
) -> list[SymmetricDifferenceRow]:
    rows = [
        SymmetricDifferenceRow(
            segment.window_name,
            segment.key,
            segment.partition,
            segment.range,
            ComparisonSide.TARGET if segment.target_record_ids else ComparisonSide.AGAINST,
            segment.target_record_ids,
            segment.against_record_ids,
            segment.finality,
        )
        for segment in segments
        if bool(segment.target_record_ids) != bool(segment.against_record_ids)
    ]
    return sorted(rows, key=_row_sort_key)


def _containment_rows_from_segments(
    segments: Iterable[_ExecutionSegment],
    targets: Iterable[_NormalizedWindow],
) -> list[ContainmentRow]:
    scope_segments = tuple(segments)
    target_ids = {
        record_id
        for target in targets
        for record_id in _record_ids(target)
    }
    contained_ranges: dict[WindowRecordId, list[TemporalRange]] = defaultdict(list)
    for segment in scope_segments:
        if not segment.against_record_ids:
            continue
        for record_id in segment.target_record_ids:
            contained_ranges[record_id].append(segment.range)

    rows: list[ContainmentRow] = []
    for segment in scope_segments:
        if not segment.target_record_ids:
            continue
        if segment.against_record_ids:
            rows.append(
                ContainmentRow(
                    segment.window_name,
                    segment.key,
                    segment.partition,
                    segment.range,
                    ContainmentStatus.CONTAINED,
                    segment.target_record_ids,
                    segment.against_record_ids,
                    segment.finality,
                )
            )
            continue
        for record_id in segment.target_record_ids:
            if record_id not in target_ids:
                continue
            rows.append(
                ContainmentRow(
                    segment.window_name,
                    segment.key,
                    segment.partition,
                    segment.range,
                    _containment_status_for_segment(
                        segment.range,
                        contained_ranges.get(record_id, ()),
                    ),
                    (record_id,),
                    (),
                    segment.finality,
                )
            )
    return sorted(rows, key=_row_sort_key)


def _containment_status_for_segment(
    temporal_range: TemporalRange,
    contained_ranges: Iterable[TemporalRange],
) -> ContainmentStatus:
    contained = tuple(contained_ranges)
    if not contained:
        return ContainmentStatus.NOT_CONTAINED
    first_contained_start = min(item.start for item in contained)
    last_contained_end = max(item.require_end() for item in contained)
    if temporal_range.require_end() <= first_contained_start:
        return ContainmentStatus.LEFT_OVERHANG
    if temporal_range.start >= last_contained_end:
        return ContainmentStatus.RIGHT_OVERHANG
    return ContainmentStatus.NOT_CONTAINED


def _aggregate_coverage_ratio(rows: tuple[CoverageRow, ...]) -> float | None:
    target = sum(row.target_magnitude for row in rows)
    if target == 0:
        return None
    covered = sum(row.covered_magnitude for row in rows)
    return covered / target


def _coverage_summaries(rows: list[CoverageRow]) -> tuple[CoverageSummary, ...]:
    groups: dict[tuple[str, Any, Any], list[CoverageRow]] = defaultdict(list)
    for row in rows:
        groups[(row.window_name, row.key, row.partition)].append(row)

    summaries: list[CoverageSummary] = []
    for scope, items in sorted(groups.items(), key=lambda item: tuple(map(repr, item[0]))):
        target = total_target_magnitude(items)
        covered = total_covered_magnitude(items)
        summaries.append(
            CoverageSummary(
                scope[0],
                scope[1],
                scope[2],
                target,
                covered,
                0.0 if target == 0 else covered / target,
            )
        )
    return tuple(summaries)


def _gap_rows(
    window_name: str,
    key: Any,
    partition: Any,
    targets: list[_NormalizedWindow],
    against: list[_NormalizedWindow],
) -> list[GapRow]:
    merged = _merge_temporal_ranges([item.range for item in [*targets, *against]])
    rows: list[GapRow] = []
    for previous, current in zip(merged, merged[1:], strict=False):
        previous_end = previous.require_end()
        if previous_end < current.start:
            rows.append(
                GapRow(
                    window_name,
                    key,
                    partition,
                    TemporalRange.closed(previous_end, current.start),
                )
            )
    return rows


def _symmetric_difference_rows(
    window_name: str,
    key: Any,
    partition: Any,
    targets: list[_NormalizedWindow],
    against: list[_NormalizedWindow],
) -> list[SymmetricDifferenceRow]:
    rows: list[SymmetricDifferenceRow] = []
    against_ranges = [item.range for item in against]
    target_ranges = [item.range for item in targets]

    for target in targets:
        for temporal_range in target.range.residual(against_ranges):
            rows.append(
                SymmetricDifferenceRow(
                    window_name,
                    key,
                    partition,
                    temporal_range,
                    ComparisonSide.TARGET,
                    _record_ids(target),
                    (),
                    target.finality,
                )
            )

    for item in against:
        for temporal_range in item.range.residual(target_ranges):
            rows.append(
                SymmetricDifferenceRow(
                    window_name,
                    key,
                    partition,
                    temporal_range,
                    ComparisonSide.AGAINST,
                    (),
                    _record_ids(item),
                    item.finality,
                )
            )

    return sorted(rows, key=_row_sort_key)


def _containment_rows(
    window_name: str,
    key: Any,
    partition: Any,
    targets: list[_NormalizedWindow],
    containers: list[_NormalizedWindow],
) -> list[ContainmentRow]:
    rows: list[ContainmentRow] = []
    container_ranges = [container.range for container in containers]

    for target in targets:
        contained: list[ContainmentRow] = []
        for container in containers:
            intersection = target.range.intersection(container.range)
            if intersection is None:
                continue
            contained.append(
                ContainmentRow(
                    window_name,
                    key,
                    partition,
                    intersection,
                    ContainmentStatus.CONTAINED,
                    _record_ids(target),
                    _record_ids(container),
                    _finality(target, container),
                )
            )

        residual_ranges = target.range.residual(container_ranges)
        if not contained and not residual_ranges:
            continue
        if not contained:
            rows.append(
                ContainmentRow(
                    window_name,
                    key,
                    partition,
                    target.range,
                    ContainmentStatus.NOT_CONTAINED,
                    _record_ids(target),
                    (),
                    target.finality,
                )
            )
            continue

        rows.extend(contained)
        first_contained_start = min(row.range.start for row in contained)
        last_contained_end = max(row.range.require_end() for row in contained)
        for temporal_range in residual_ranges:
            status = ContainmentStatus.NOT_CONTAINED
            if temporal_range.require_end() <= first_contained_start:
                status = ContainmentStatus.LEFT_OVERHANG
            elif temporal_range.start >= last_contained_end:
                status = ContainmentStatus.RIGHT_OVERHANG
            rows.append(
                ContainmentRow(
                    window_name,
                    key,
                    partition,
                    temporal_range,
                    status,
                    _record_ids(target),
                    (),
                    target.finality,
                )
            )

    return sorted(rows, key=_row_sort_key)


@dataclass(frozen=True, slots=True)
class _LeadLagOptions:
    transition: LeadLagTransition
    axis: TemporalAxis
    tolerance_magnitude: int


@dataclass(frozen=True, slots=True)
class _AsOfOptions:
    direction: AsOfDirection
    axis: TemporalAxis
    tolerance_magnitude: int


@dataclass(frozen=True, slots=True)
class _TransitionPoint:
    record_id: WindowRecordId
    point: TemporalPoint
    finality: ComparisonFinality


def _lead_lag_rows(
    window_name: str,
    key: Any,
    partition: Any,
    targets: Iterable[_NormalizedWindow],
    against: Iterable[_NormalizedWindow],
    options: _LeadLagOptions,
) -> list[LeadLagRow]:
    candidates = sorted(
        (
            _TransitionPoint(_primary_record_id(item), point, item.finality)
            for item in against
            if item.range.axis is options.axis
            if (point := _transition_point(item.range, options.transition)) is not None
        ),
        key=lambda item: item.point,
    )
    rows: list[LeadLagRow] = []

    for target in targets:
        target_point = _transition_point(target.range, options.transition)
        if target.range.axis is not options.axis or target_point is None:
            continue
        if not candidates:
            rows.append(
                LeadLagRow(
                    window_name,
                    key,
                    partition,
                    options.transition,
                    options.axis,
                    target_point,
                    None,
                    None,
                    options.tolerance_magnitude,
                    False,
                    LeadLagDirection.MISSING_COMPARISON,
                    _primary_record_id(target),
                    None,
                    target.finality,
                )
            )
            continue

        nearest = _nearest_transition(candidates, target_point, options.axis)
        delta = _delta_magnitude(target_point, nearest.point, options.axis)
        absolute_delta = abs(delta)
        rows.append(
            LeadLagRow(
                window_name,
                key,
                partition,
                options.transition,
                options.axis,
                target_point,
                nearest.point,
                delta,
                options.tolerance_magnitude,
                absolute_delta <= options.tolerance_magnitude,
                _lead_lag_direction(delta),
                _primary_record_id(target),
                nearest.record_id,
                ComparisonFinality.PROVISIONAL
                if target.finality is ComparisonFinality.PROVISIONAL
                or nearest.finality is ComparisonFinality.PROVISIONAL
                else ComparisonFinality.FINAL,
            )
        )
    return rows


def _lead_lag_summary(
    options: _LeadLagOptions,
    rows: list[LeadLagRow],
    row_count: int,
) -> LeadLagSummary:
    deltas = [row.delta_magnitude for row in rows if row.delta_magnitude is not None]
    return LeadLagSummary(
        options.transition,
        options.axis,
        options.tolerance_magnitude,
        row_count,
        sum(1 for row in rows if row.direction is LeadLagDirection.TARGET_LEADS),
        sum(1 for row in rows if row.direction is LeadLagDirection.TARGET_LAGS),
        sum(1 for row in rows if row.direction is LeadLagDirection.EQUAL),
        sum(1 for row in rows if row.direction is LeadLagDirection.MISSING_COMPARISON),
        sum(1 for row in rows if not row.is_within_tolerance),
        min(deltas) if deltas else None,
        max(deltas) if deltas else None,
    )


def _as_of_rows(
    window_name: str,
    key: Any,
    partition: Any,
    targets: Iterable[_NormalizedWindow],
    against: Iterable[_NormalizedWindow],
    options: _AsOfOptions,
    diagnostics: list[str],
) -> list[AsOfRow]:
    candidates = sorted(
        (
            _TransitionPoint(_primary_record_id(item), item.range.start, item.finality)
            for item in against
            if item.range.axis is options.axis
        ),
        key=lambda item: (item.point, item.record_id.value),
    )
    rows: list[AsOfRow] = []

    for target in targets:
        if target.range.axis is not options.axis:
            continue
        target_point = target.range.start
        if not candidates:
            rows.append(
                _create_as_of_row(target, options, window_name, key, partition, None, None)
            )
            continue

        candidate, ambiguous, future_rejected = _find_as_of_candidate(
            candidates,
            target_point,
            options,
        )
        if candidate is None:
            distance = (
                abs(_delta_magnitude(target_point, future_rejected.point, options.axis))
                if future_rejected is not None
                else None
            )
            rows.append(
                _create_as_of_row(
                    target,
                    options,
                    window_name,
                    key,
                    partition,
                    None,
                    distance,
                    AsOfMatchStatus.FUTURE_REJECTED
                    if future_rejected is not None
                    else AsOfMatchStatus.NO_MATCH,
                )
            )
            continue

        distance = abs(_delta_magnitude(target_point, candidate.point, options.axis))
        if distance > options.tolerance_magnitude:
            rows.append(
                _create_as_of_row(
                    target,
                    options,
                    window_name,
                    key,
                    partition,
                    None,
                    distance,
                    AsOfMatchStatus.NO_MATCH,
                )
            )
            continue

        if ambiguous:
            diagnostics.append("ambiguous_as_of_match")
        status = (
            AsOfMatchStatus.AMBIGUOUS
            if ambiguous
            else AsOfMatchStatus.EXACT
            if distance == 0
            else AsOfMatchStatus.MATCHED
        )
        rows.append(
            _create_as_of_row(
                target,
                options,
                window_name,
                key,
                partition,
                candidate,
                distance,
                status,
            )
        )
    return rows


def _create_as_of_row(
    target: _NormalizedWindow,
    options: _AsOfOptions,
    window_name: str,
    key: Any,
    partition: Any,
    match: _TransitionPoint | None,
    distance: int | None,
    status: AsOfMatchStatus = AsOfMatchStatus.NO_MATCH,
) -> AsOfRow:
    return AsOfRow(
        window_name,
        key,
        partition,
        options.axis,
        options.direction,
        target.range.start,
        match.point if match is not None else None,
        distance,
        options.tolerance_magnitude,
        status,
        _primary_record_id(target),
        match.record_id if match is not None else None,
        ComparisonFinality.PROVISIONAL
        if target.finality is ComparisonFinality.PROVISIONAL
        or (match is not None and match.finality is ComparisonFinality.PROVISIONAL)
        else ComparisonFinality.FINAL,
    )


def _find_as_of_candidate(
    candidates: list[_TransitionPoint],
    target_point: TemporalPoint,
    options: _AsOfOptions,
) -> tuple[_TransitionPoint | None, bool, _TransitionPoint | None]:
    ambiguous = False
    future_rejected: _TransitionPoint | None = None
    best: _TransitionPoint | None = None
    best_distance: int | None = None

    for candidate in candidates:
        comparison = candidate.point.compare_to(target_point)
        if options.direction is AsOfDirection.PREVIOUS and comparison > 0:
            future_rejected = future_rejected or candidate
            continue
        if options.direction is AsOfDirection.NEXT and comparison < 0:
            continue

        distance = abs(_delta_magnitude(target_point, candidate.point, options.axis))
        if best_distance is None or distance < best_distance:
            best = candidate
            best_distance = distance
            ambiguous = False
            continue
        if distance == best_distance:
            ambiguous = True
            if best is not None and candidate.record_id.value < best.record_id.value:
                best = candidate

    return best, ambiguous, future_rejected


def _transition_point(
    temporal_range: TemporalRange,
    transition: LeadLagTransition,
) -> TemporalPoint | None:
    if transition is LeadLagTransition.START:
        return temporal_range.start
    return temporal_range.end


def _nearest_transition(
    candidates: list[_TransitionPoint],
    target_point: TemporalPoint,
    axis: TemporalAxis,
) -> _TransitionPoint:
    best = candidates[0]
    best_distance = abs(_delta_magnitude(target_point, best.point, axis))
    for candidate in candidates[1:]:
        distance = abs(_delta_magnitude(target_point, candidate.point, axis))
        if distance < best_distance:
            best = candidate
            best_distance = distance
    return best


def _delta_magnitude(
    target_point: TemporalPoint,
    comparison_point: TemporalPoint,
    axis: TemporalAxis,
) -> int:
    if axis is TemporalAxis.TIMESTAMP:
        delta = target_point.timestamp - comparison_point.timestamp
        return int(delta.total_seconds() * 1_000_000)
    return target_point.position - comparison_point.position


def _lead_lag_direction(delta: int) -> LeadLagDirection:
    if delta < 0:
        return LeadLagDirection.TARGET_LEADS
    if delta > 0:
        return LeadLagDirection.TARGET_LAGS
    return LeadLagDirection.EQUAL


def _merged_ranges(ranges: list[TemporalRange]) -> list[float]:
    return [item.magnitude() for item in _merge_temporal_ranges(ranges)]


def _merge_temporal_ranges(ranges: list[TemporalRange]) -> list[TemporalRange]:
    if not ranges:
        return []
    ordered = sorted(ranges, key=lambda item: item.start)
    merged: list[TemporalRange] = []
    for item in ordered:
        if not merged:
            merged.append(item)
            continue
        previous = merged[-1]
        if item.start <= previous.require_end():
            end = max(previous.require_end(), item.require_end())
            merged[-1] = TemporalRange(previous.start, end, previous.end_status)
        else:
            merged.append(item)
    return merged


def _finality(*items: _NormalizedWindow) -> ComparisonFinality:
    return (
        ComparisonFinality.PROVISIONAL
        if any(item.finality is ComparisonFinality.PROVISIONAL for item in items)
        else ComparisonFinality.FINAL
    )


def _result_row_groups(result: ComparisonResult) -> tuple[tuple[str, tuple[Any, ...]], ...]:
    return (
        ("overlap", result.overlap_rows),
        ("residual", result.residual_rows),
        ("missing", result.missing_rows),
        ("coverage", result.coverage_rows),
        ("gap", result.gap_rows),
        ("symmetric_difference", result.symmetric_difference_rows),
        ("containment", result.containment_rows),
        ("lead_lag", result.lead_lag_rows),
        ("as_of", result.as_of_rows),
    )


def _finality_key(row: ComparisonRowFinality) -> str:
    return f"{row.row_type}\n{row.row_id}"


def _align_prepared(prepared: PreparedComparison) -> AlignedComparison:
    groups: dict[tuple[str, str, str, str], list[NormalizedWindowRecord]] = defaultdict(list)
    for window in prepared.normalized_windows:
        key = (
            window.window.window_name,
            repr(window.window.key),
            repr(window.window.partition),
            repr(window.segments),
        )
        groups[key].append(window)

    segments: list[AlignedSegment] = []
    for _, windows in sorted(groups.items(), key=lambda item: item[0]):
        boundaries = sorted(
            {
                point
                for window in windows
                if window.range.end is not None
                for point in (window.range.start, window.range.end)
            }
        )
        if len(boundaries) < 2:
            continue
        first = windows[0]
        for start, end in zip(boundaries, boundaries[1:], strict=False):
            if start >= end:
                continue
            target_ids: list[WindowRecordId] = []
            against_ids: list[WindowRecordId] = []
            for window in windows:
                if window.range.end is None:
                    continue
                if not (window.range.start <= start and end <= window.range.end):
                    continue
                if window.side is ComparisonSide.TARGET:
                    target_ids.append(window.record_id)
                else:
                    against_ids.append(window.record_id)
            if not target_ids and not against_ids:
                continue
            segments.append(
                AlignedSegment(
                    first.window.window_name,
                    first.window.key,
                    first.window.partition,
                    TemporalRange.closed(start, end),
                    tuple(target_ids),
                    tuple(against_ids),
                    first.segments,
                )
            )
    return AlignedComparison(prepared, tuple(segments))


def _append_once(values: list[str], value: str) -> None:
    if value not in values:
        values.append(value)


def _diagnostic_from_code(code: str, *, strict: bool) -> ComparisonDiagnostic:
    severity = _diagnostic_severity(code)
    if strict and severity is ComparisonDiagnosticSeverity.WARNING:
        severity = ComparisonDiagnosticSeverity.ERROR
    return ComparisonDiagnostic(
        code,
        _DIAGNOSTIC_MESSAGES.get(code, "Comparison diagnostic."),
        _DIAGNOSTIC_PATHS.get(code, "comparison"),
        severity,
    )


def _plan_diagnostic_from_code(code: str) -> ComparisonPlanDiagnostic:
    try:
        validation_code = ComparisonPlanValidationCode(code)
    except ValueError:
        validation_code = ComparisonPlanValidationCode.UNKNOWN
    return ComparisonPlanDiagnostic(
        validation_code,
        _DIAGNOSTIC_MESSAGES.get(code, "Comparison diagnostic."),
        _DIAGNOSTIC_PATHS.get(code, "comparison"),
        _diagnostic_severity(code),
    )


def _diagnostic_severity(code: str) -> ComparisonDiagnosticSeverity:
    if code in {
        "ambiguous_as_of_match",
        "duplicate_window",
        "future_leakage_risk",
        "future_window_excluded",
        "mixed_clock_risk",
        "unbounded_open_duration",
    }:
        return ComparisonDiagnosticSeverity.WARNING
    return ComparisonDiagnosticSeverity.ERROR


_DIAGNOSTIC_MESSAGES = {
    "ambiguous_as_of_match": "As-of lookup found multiple equally near comparison windows.",
    "duplicate_window": "Duplicate normalized windows were encountered.",
    "future_leakage_risk": (
        "Point-in-time lookup without known-at may use unavailable future records."
    ),
    "future_window_excluded": "A window was not available at the configured known-at point.",
    "invalid_range_duration": "A normalized temporal range had an invalid duration.",
    "known_at_requires_processing_position": (
        "Known-at filtering currently requires processing-position availability."
    ),
    "mixed_clock_risk": "Timestamp horizon and known-at point use different clock identities.",
    "missing_event_time": "Event-time comparison requires recorded event timestamps.",
    "open_windows_without_policy": "Open windows require an explicit evaluation horizon.",
    "unbounded_open_duration": (
        "An open window was excluded because the plan did not bound its duration."
    ),
    "unknown_comparator": "Comparator declaration is not registered.",
}


_DIAGNOSTIC_PATHS = {
    "ambiguous_as_of_match": "comparators.as_of",
    "duplicate_window": "normalization.duplicate_windows",
    "future_leakage_risk": "normalization.known_at",
    "future_window_excluded": "normalization.known_at",
    "invalid_range_duration": "normalization.horizon",
    "known_at_requires_processing_position": "normalization.known_at",
    "mixed_clock_risk": "normalization",
    "missing_event_time": "normalization.axis",
    "open_windows_without_policy": "normalization.horizon",
    "unbounded_open_duration": "normalization.open_window_policy",
    "unknown_comparator": "comparators",
}


def _missing_event_time(window: WindowRecord) -> bool:
    return window.start_time is None or (window.is_closed and window.end_time is None)


def _count_for(
    comparator: str,
    overlap_rows: list[OverlapRow],
    residual_rows: list[ResidualRow],
    missing_rows: list[MissingRow],
    coverage_rows: list[CoverageRow],
    gap_rows: list[GapRow],
    symmetric_difference_rows: list[SymmetricDifferenceRow],
    containment_rows: list[ContainmentRow],
    lead_lag_rows: list[LeadLagRow],
    lead_lag_counts: dict[str, int],
    as_of_rows: list[AsOfRow],
    as_of_counts: dict[str, int],
) -> int:
    if comparator in lead_lag_counts:
        return lead_lag_counts[comparator]
    if comparator in as_of_counts:
        return as_of_counts[comparator]
    return {
        "overlap": len(overlap_rows),
        "residual": len(residual_rows),
        "missing": len(missing_rows),
        "coverage": len(coverage_rows),
        "gap": len(gap_rows),
        "symmetric_difference": len(symmetric_difference_rows),
        "containment": len(containment_rows),
        "lead_lag": len(lead_lag_rows),
        "as_of": len(as_of_rows),
    }.get(comparator, 0)


def _normalize_comparator(comparator: str) -> str:
    return comparator.strip().lower().replace("-", "_")


def _try_parse_lead_lag(comparator: str) -> _LeadLagOptions | None:
    parts = comparator.split(":")
    if len(parts) != 4 or parts[0] != "lead_lag":
        return None

    transition = {
        "start": LeadLagTransition.START,
        "end": LeadLagTransition.END,
    }.get(parts[1].replace("_", ""))
    axis = _parse_axis(parts[2])
    try:
        tolerance = int(parts[3])
    except ValueError:
        return None
    if transition is None or axis is None or tolerance < 0:
        return None
    return _LeadLagOptions(transition, axis, tolerance)


def _try_parse_as_of(comparator: str) -> _AsOfOptions | None:
    parts = comparator.split(":")
    if len(parts) != 4 or parts[0] not in {"asof", "as_of"}:
        return None

    direction = {
        "previous": AsOfDirection.PREVIOUS,
        "next": AsOfDirection.NEXT,
        "nearest": AsOfDirection.NEAREST,
    }.get(parts[1].replace("_", ""))
    axis = _parse_axis(parts[2])
    try:
        tolerance = int(parts[3])
    except ValueError:
        return None
    if direction is None or axis is None or tolerance < 0:
        return None
    return _AsOfOptions(direction, axis, tolerance)


def _parse_axis(value: str) -> TemporalAxis | None:
    normalized = value.replace("_", "")
    if normalized == "processingposition":
        return TemporalAxis.PROCESSING_POSITION
    if normalized == "timestamp":
        return TemporalAxis.TIMESTAMP
    return None


def _row_sort_key(row: Any) -> tuple[str, str, str, TemporalPoint, TemporalPoint]:
    return (
        repr(row.window_name),
        repr(row.key),
        repr(row.partition),
        row.range.start,
        row.range.require_end(),
    )


def _range_label(temporal_range: TemporalRange) -> str:
    end = temporal_range.require_end()
    if temporal_range.axis is TemporalAxis.PROCESSING_POSITION:
        return f"[{temporal_range.start.position}, {end.position})"
    return f"[{temporal_range.start.timestamp.isoformat()}, {end.timestamp.isoformat()})"


def _row_range_or_point(row: Any) -> str:
    if isinstance(row, LeadLagRow | AsOfRow):
        return _point_label(row.target_point)
    return _range_label(row.range)


def _row_record_ids(row: Any) -> tuple[WindowRecordId, ...]:
    ids: list[WindowRecordId] = []
    for name in (
        "target_record_ids",
        "against_record_ids",
        "container_record_ids",
        "parent_record_ids",
        "child_record_ids",
    ):
        ids.extend(getattr(row, name, ()))
    for name in ("target_record_id", "comparison_record_id", "matched_record_id"):
        record_id = getattr(row, name, None)
        if record_id is not None:
            ids.append(record_id)
    return tuple(ids)


def _parse_cohort_evidence(
    metadata: ComparisonExtensionMetadata,
) -> CohortEvidenceMetadata | None:
    prefix = "segment["
    if not metadata.key.startswith(prefix) or not metadata.key.endswith("]"):
        return None
    try:
        segment_index = int(metadata.key[len(prefix) : -1])
    except ValueError:
        return None
    fields = _parse_metadata_fields(metadata.value)
    try:
        rule = fields["rule"]
        required_count = int(fields["required"])
        active_count = int(fields["activeCount"])
        is_active = fields["isActive"].lower() == "true"
    except (KeyError, ValueError):
        return None
    active_sources = tuple(
        source
        for source in fields.get("activeSources", "").split(",")
        if source
    )
    return CohortEvidenceMetadata(
        segment_index,
        rule,
        required_count,
        active_count,
        is_active,
        active_sources,
        metadata.value,
    )


def _parse_metadata_fields(value: str) -> dict[str, str]:
    fields: dict[str, str] = {}
    for part in value.split(";"):
        key, separator, field_value = part.strip().partition("=")
        if not separator or not key:
            continue
        fields[key] = field_value
    return fields


def _require_export_path(path: str | Path | None) -> str | Path:
    if path is None or not str(path).strip():
        msg = "Export path cannot be empty."
        raise ValueError(msg)
    return path


def _write_text_if_requested(path: str | Path | None, text: str) -> None:
    if path is None:
        return
    destination = Path(_require_export_path(path))
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text(text, encoding="utf-8")


def _ensure_exportable(plan: ComparisonPlan) -> None:
    if plan.is_serializable:
        return
    diagnostics = tuple(
        diagnostic
        for diagnostic in plan.validate()
        if diagnostic.code is ComparisonPlanValidationCode.NON_SERIALIZABLE_SELECTOR
    ) or (
        ComparisonPlanDiagnostic(
            ComparisonPlanValidationCode.NON_SERIALIZABLE_SELECTOR,
            (
                "Comparison plan contains runtime-only selectors and cannot be "
                "exported as portable data."
            ),
            "selectors",
            ComparisonDiagnosticSeverity.ERROR,
        ),
    )
    raise ComparisonExportException(
        "Comparison plan contains runtime-only selectors and cannot be exported as portable data.",
        diagnostics,
    )


def _export_configured_result(
    result: ComparisonResult,
    *,
    export: ComparisonDebugHtmlOptions | ComparisonLlmContextOptions | None,
    debug_html: ComparisonDebugHtmlOptions | None,
    llm_context: ComparisonLlmContextOptions | None,
) -> None:
    if isinstance(export, ComparisonDebugHtmlOptions | ComparisonLlmContextOptions):
        export.export_if_enabled(result)
    elif export is not None:
        msg = "export must be ComparisonDebugHtmlOptions, ComparisonLlmContextOptions, or None."
        raise TypeError(msg)
    if debug_html is not None:
        debug_html.export_if_enabled(result)
    if llm_context is not None:
        llm_context.export_if_enabled(result)


def _plan_export_payload(
    plan: ComparisonPlan,
    diagnostics: Iterable[ComparisonPlanDiagnostic],
) -> dict[str, Any]:
    return {
        "schema": "spanfold.comparison.plan",
        "schemaVersion": 0,
        "artifact": "plan",
        "name": plan.name,
        "isStrict": plan.is_strict,
        "isSerializable": plan.is_serializable,
        "target": _selector_export_payload(plan.target) if plan.target is not None else None,
        "against": [_selector_export_payload(selector) for selector in plan.against],
        "scope": _scope_export_payload(plan.scope),
        "normalization": _normalization_export_payload(plan.normalization),
        "comparators": list(plan.comparators),
        "output": _output_export_payload(plan.output),
        "diagnostics": [_plan_diagnostic_export_payload(item) for item in diagnostics],
    }


def _selector_export_payload(selector: ComparisonSelector) -> dict[str, Any]:
    payload: dict[str, Any] = {
        "name": selector.name,
        "description": selector.description,
        "isSerializable": selector.is_serializable,
    }
    if selector.cohort_activity is not None:
        cohort: dict[str, Any] = {
            "activity": selector.cohort_activity.name,
            "sources": [_to_jsonable(source) for source in selector.cohort_sources],
        }
        if selector.cohort_activity.count is not None:
            cohort["count"] = selector.cohort_activity.count
        payload["cohort"] = cohort
    return payload


def _scope_export_payload(scope: ComparisonScope | None) -> dict[str, Any] | None:
    if scope is None:
        return None
    payload: dict[str, Any] = {
        "windowName": scope.window_name,
        "timeAxis": scope.time_axis.value,
    }
    if scope.segment_filters:
        payload["segmentFilters"] = [
            {"name": item.name, "value": _to_jsonable(item.value)}
            for item in scope.segment_filters
        ]
    if scope.tag_filters:
        payload["tagFilters"] = [
            {"name": item.name, "value": _to_jsonable(item.value)}
            for item in scope.tag_filters
        ]
    return payload


def _normalization_export_payload(policy: ComparisonNormalizationPolicy) -> dict[str, Any]:
    return {
        "requireClosedWindows": policy.require_closed_windows,
        "useHalfOpenRanges": policy.use_half_open_ranges,
        "timeAxis": policy.time_axis.value,
        "openWindowPolicy": policy.open_window_policy.value,
        "openWindowHorizon": _to_jsonable(policy.open_window_horizon),
        "nullTimestampPolicy": policy.null_timestamp_policy.value,
        "coalesceAdjacentWindows": policy.coalesce_adjacent_windows,
        "duplicateWindowPolicy": policy.duplicate_window_policy.value,
        "knownAt": _to_jsonable(policy.known_at),
    }


def _output_export_payload(output: ComparisonOutputOptions) -> dict[str, Any]:
    return {
        "includeAlignedSegments": output.include_aligned_segments,
        "includeExplainData": output.include_explain_data,
    }


def _plan_diagnostic_export_payload(diagnostic: ComparisonPlanDiagnostic) -> dict[str, Any]:
    return {
        "code": diagnostic.code.value,
        "message": diagnostic.message,
        "path": diagnostic.path,
        "severity": diagnostic.severity.value,
    }


def _diagnostic_export_payload(diagnostic: ComparisonDiagnostic) -> dict[str, Any]:
    return {
        "code": diagnostic.code,
        "message": diagnostic.message,
        "path": diagnostic.path,
        "severity": diagnostic.severity.value,
    }


def _result_export_payload(result: ComparisonResult) -> dict[str, Any]:
    plan = result.plan or _fallback_result_plan(result)
    plan_diagnostics = result.plan.validate() if result.plan is not None else ()
    return {
        "schema": "spanfold.comparison.result",
        "schemaVersion": 0,
        "artifact": "result",
        "isValid": result.is_valid,
        "knownAt": _to_jsonable(result.known_at),
        "evaluationHorizon": _to_jsonable(result.evaluation_horizon),
        "plan": _plan_export_payload(plan, plan_diagnostics),
        "diagnostics": [_diagnostic_export_payload(item) for item in result.diagnostic_rows],
        "prepared": _prepared_export_payload(result.prepared),
        "aligned": _aligned_export_payload(result.aligned),
        "comparatorSummaries": [
            {"comparatorName": item.comparator, "rowCount": item.row_count}
            for item in result.comparator_summaries
        ],
        "rows": _rows_export_payload(result),
        "rowFinalities": [_row_finality_export_payload(item) for item in result.row_finalities],
        "extensionMetadata": [
            {
                "extensionId": item.extension_id,
                "key": item.key,
                "value": item.value,
            }
            for item in result.extension_metadata
        ],
        "coverageSummaries": _to_jsonable(result.coverage_summaries),
        "leadLagSummaries": _to_jsonable(result.lead_lag_summaries),
    }


def _fallback_result_plan(result: ComparisonResult) -> ComparisonPlan:
    return ComparisonPlan(
        result.name,
        None,
        (),
        None,
        ComparisonNormalizationPolicy.default(),
        tuple(summary.comparator for summary in result.comparator_summaries),
        ComparisonOutputOptions.default(),
        result.strict,
    )


def _prepared_export_payload(prepared: PreparedComparison | None) -> dict[str, Any] | None:
    if prepared is None:
        return None
    return {
        "selectedWindows": [_window_export_payload(window) for window in prepared.selected_windows],
        "excludedWindows": [
            {
                "recordId": str(item.window.id),
                "reason": item.reason,
                "diagnosticCode": item.diagnostic_code.value if item.diagnostic_code else None,
                "window": _window_export_payload(item.window),
            }
            for item in prepared.excluded_windows
        ],
        "normalizedWindows": [
            {
                "recordId": str(item.record_id),
                "selectorName": item.selector_name,
                "side": item.side.value,
                "range": _to_jsonable(item.range),
                "window": _window_export_payload(item.window),
            }
            for item in prepared.normalized_windows
        ],
    }


def _aligned_export_payload(aligned: Any) -> Any:
    if aligned is None:
        return None
    segments = getattr(aligned, "segments", None)
    if segments is None:
        return _to_jsonable(aligned)
    return {
        "segments": [
            {
                "segmentId": f"segment[{index}]",
                "windowName": segment.window_name,
                "key": _to_jsonable(segment.key),
                "partition": _to_jsonable(segment.partition),
                "range": _to_jsonable(segment.range),
                "targetRecordIds": [str(record_id) for record_id in segment.target_record_ids],
                "againstRecordIds": [str(record_id) for record_id in segment.against_record_ids],
            }
            for index, segment in enumerate(segments)
        ]
    }


def _window_export_payload(window: WindowRecord) -> dict[str, Any]:
    return {
        "id": str(window.id),
        "windowName": window.window_name,
        "key": _to_jsonable(window.key),
        "partition": _to_jsonable(window.partition),
        "source": _to_jsonable(window.source),
        "startPosition": window.start_position,
        "endPosition": window.end_position,
        "startTimestamp": window.start_time.isoformat() if window.start_time else None,
        "endTimestamp": window.end_time.isoformat() if window.end_time else None,
        "segments": _to_jsonable(window.segments),
        "tags": _to_jsonable(window.tags),
        "boundaryReason": window.boundary_reason.value if window.boundary_reason else None,
        "boundaryChanges": _to_jsonable(window.boundary_changes),
    }


def _rows_export_payload(result: ComparisonResult) -> dict[str, list[dict[str, Any]]]:
    return {
        _export_row_type(row_type): [
            {
                "rowId": f"{_export_row_type(row_type)}[{index}]",
                "finality": getattr(row, "finality", ComparisonFinality.FINAL).value,
                **_row_export_fields(row),
            }
            for index, row in enumerate(values)
        ]
        for row_type, values in _result_row_groups(result)
    }


def _row_export_fields(row: Any) -> dict[str, Any]:
    payload = _to_jsonable(row)
    if "range" in payload:
        payload["range"] = _to_jsonable(row.range)
    for key in (
        "target_record_ids",
        "against_record_ids",
        "container_record_ids",
    ):
        if key in payload:
            payload[key] = [str(record_id) for record_id in getattr(row, key)]
    return {_camel_case(key): value for key, value in payload.items()}


def _row_finality_export_payload(row: ComparisonRowFinality) -> dict[str, Any]:
    export_type = _export_row_type(row.row_type)
    return {
        "rowType": export_type,
        "rowId": row.row_id.replace(row.row_type, export_type, 1),
        "finality": row.finality.value,
        "reason": row.reason,
        "version": row.version,
        "supersedesRowId": row.supersedes_row_id,
    }


def _result_summary_line_payload(result: ComparisonResult) -> dict[str, Any]:
    counts = _row_counts(result)
    return {
        "schema": "spanfold.comparison.row",
        "schemaVersion": 0,
        "artifact": "result-summary",
        "planName": result.name,
        "isValid": result.is_valid,
        "knownAt": _to_jsonable(result.known_at),
        "evaluationHorizon": _to_jsonable(result.evaluation_horizon),
        "diagnosticCount": len(result.diagnostic_rows),
        "overlapRowCount": counts["overlap"],
        "residualRowCount": counts["residual"],
        "missingRowCount": counts["missing"],
        "coverageRowCount": counts["coverage"],
        "gapRowCount": counts["gap"],
        "symmetricDifferenceRowCount": counts["symmetricDifference"],
        "containmentRowCount": counts["containment"],
        "leadLagRowCount": counts["leadLag"],
        "asOfRowCount": counts["asOf"],
    }


def _llm_summary_payload(result: ComparisonResult) -> dict[str, Any]:
    counts = _row_counts(result)
    aligned_segments = getattr(result.aligned, "segments", ())
    return {
        "planName": result.name,
        "isValid": result.is_valid,
        "knownAt": _to_jsonable(result.known_at),
        "evaluationHorizon": _to_jsonable(result.evaluation_horizon),
        "diagnosticCount": len(result.diagnostic_rows),
        "selectedWindowCount": len(result.prepared.selected_windows) if result.prepared else 0,
        "excludedWindowCount": len(result.prepared.excluded_windows) if result.prepared else 0,
        "normalizedWindowCount": len(result.prepared.normalized_windows) if result.prepared else 0,
        "alignedSegmentCount": len(aligned_segments),
        "rowCounts": counts,
    }


def _row_counts(result: ComparisonResult) -> dict[str, int]:
    return {
        _export_row_type(row_type): len(values)
        for row_type, values in _result_row_groups(result)
    }


def _export_row_type(row_type: str) -> str:
    return {
        "symmetric_difference": "symmetricDifference",
        "lead_lag": "leadLag",
        "as_of": "asOf",
    }.get(row_type, row_type)


def _camel_case(value: str) -> str:
    head, *tail = value.split("_")
    return head + "".join(part[:1].upper() + part[1:] for part in tail)


def _to_jsonable(value: Any) -> Any:
    if isinstance(value, ComparisonResult):
        payload = {
            key: _to_jsonable(item)
            for key, item in asdict(value).items()
        }
        payload["diagnostic_rows"] = _to_jsonable(value.diagnostic_rows)
        payload["row_finalities"] = _to_jsonable(value.row_finalities)
        payload["is_valid"] = value.is_valid
        return payload
    if isinstance(value, ComparisonSelector):
        return {
            "name": value.name,
            "description": value.description,
            "is_serializable": value.is_serializable,
            "cohort_activity": _to_jsonable(value.cohort_activity),
            "cohort_sources": _to_jsonable(value.cohort_sources),
        }
    if isinstance(value, Enum):
        return value.value
    if isinstance(value, WindowRecordId):
        return value.value
    if isinstance(value, TemporalPoint):
        if value.axis is TemporalAxis.PROCESSING_POSITION:
            return {"axis": value.axis.value, "position": value.position}
        return {
            "axis": value.axis.value,
            "timestamp": value.timestamp.isoformat(),
            "clock": value.clock,
        }
    if isinstance(value, TemporalRange):
        return {
            "start": _to_jsonable(value.start),
            "end": _to_jsonable(value.end) if value.end is not None else None,
            "end_status": value.end_status.value,
        }
    if hasattr(value, "__dataclass_fields__"):
        return {key: _to_jsonable(item) for key, item in asdict(value).items()}
    if isinstance(value, tuple | list):
        return [_to_jsonable(item) for item in value]
    if isinstance(value, dict):
        return {str(key): _to_jsonable(item) for key, item in value.items()}
    if callable(value):
        return "<runtime>"
    return value


def _html_row(label: str, color: str, row: Any) -> str:
    if isinstance(row, LeadLagRow | AsOfRow):
        start = (
            row.target_point.position
            if row.target_point.axis is TemporalAxis.PROCESSING_POSITION
            else 0
        )
        end = start + 1
        range_text = _point_label(row.target_point)
    else:
        start = (
            row.range.start.position
            if row.range.axis is TemporalAxis.PROCESSING_POSITION
            else 0
        )
        end = (
            row.range.require_end().position
            if row.range.axis is TemporalAxis.PROCESSING_POSITION
            else 1
        )
        range_text = _range_label(row.range)
    width = max(end - start, 1)
    left = min(start * 4, 92)
    bar_width = min(width * 4, 100 - left)
    return (
        '<div class="row">'
        f'<div class="meta"><strong>{html.escape(label)}</strong><br>'
        f"{html.escape(str(row.window_name))} / {html.escape(str(row.key))}<br>"
        f"{html.escape(range_text)}<br>"
        f"{html.escape(getattr(row, 'finality', ComparisonFinality.FINAL).value)}</div>"
        '<div class="track">'
        f'<div class="bar" style="left:{left}%;width:{bar_width}%;background:{color}"></div>'
        "</div></div>"
    )


def _debug_html_json_section(
    title: str,
    value: Any,
    empty_text: str,
    *,
    limit: int | None = None,
) -> str:
    if value is None or value == () or value == []:
        return f"<h2>{html.escape(title)}</h2><p>{html.escape(empty_text)}</p>"
    if limit is not None and isinstance(value, tuple | list):
        shown = value[:limit]
        suffix = (
            f"<p>{html.escape(title)}: showing {limit} of {len(value)} rows.</p>"
            if len(value) > limit
            else ""
        )
        payload = json.dumps(_to_jsonable(shown), indent=2, sort_keys=True)
        return f"<h2>{html.escape(title)}</h2><pre>{html.escape(payload)}</pre>{suffix}"
    payload = json.dumps(_to_jsonable(value), indent=2, sort_keys=True)
    return f"<h2>{html.escape(title)}</h2><pre>{html.escape(payload)}</pre>"


def _point_label(point: TemporalPoint) -> str:
    if point.axis is TemporalAxis.PROCESSING_POSITION:
        return str(point.position)
    return point.timestamp.isoformat()


def _require_text(value: str, label: str) -> None:
    if not value or not value.strip():
        msg = f"{label} cannot be empty."
        raise ValueError(msg)


def _require_not_none(value: Any, label: str) -> None:
    if value is None:
        msg = f"{label} is required."
        raise ValueError(msg)
