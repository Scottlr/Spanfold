//! Comparison scope, normalization policy, and plan configuration.

use std::{borrow::Cow, collections::BTreeSet, fmt};

use crate::{PrimitiveValue, TemporalAxis, TemporalPoint};

use super::diagnostics::plan_diagnostic;
use super::{
    AgainstSelection, CohortActivity, Comparator, ComparisonDiagnostic, ComparisonSelector,
    DiagnosticSeverity,
};

/// Equality filter over tags or segments.
#[derive(Clone, Debug, PartialEq)]
pub struct WindowFilter {
    /// Filter name.
    pub name: String,
    /// Filter value.
    pub value: PrimitiveValue,
}

/// Scope used to select the windows compared by a plan.
#[derive(Clone, Debug, PartialEq)]
pub struct ComparisonScope {
    /// Optional window family scope.
    pub window_name: Option<String>,
    /// Optional logical key scope.
    pub key: Option<String>,
    /// Optional partition scope.
    pub partition: Option<String>,
    /// Time axis used by scoped temporal comparators.
    pub time_axis: crate::TemporalAxis,
    /// Segment filters.
    pub segment_filters: Vec<WindowFilter>,
    /// Tag filters.
    pub tag_filters: Vec<WindowFilter>,
}

impl Default for ComparisonScope {
    fn default() -> Self {
        Self::all()
    }
}

impl ComparisonScope {
    /// Creates an unrestricted processing-position scope.
    #[must_use]
    pub fn all() -> Self {
        Self {
            window_name: None,
            key: None,
            partition: None,
            time_axis: crate::TemporalAxis::ProcessingPosition,
            segment_filters: Vec::new(),
            tag_filters: Vec::new(),
        }
    }

    /// Creates a scope restricted to one window family.
    #[must_use]
    pub fn window(window_name: impl Into<String>) -> Self {
        Self {
            window_name: Some(window_name.into()),
            ..Self::all()
        }
    }

    /// Returns this scope using event-time normalization.
    #[must_use]
    pub const fn on_event_time(mut self) -> Self {
        self.time_axis = crate::TemporalAxis::Timestamp;
        self
    }

    /// Returns this scope using processing-position normalization.
    #[must_use]
    pub const fn on_position(mut self) -> Self {
        self.time_axis = crate::TemporalAxis::ProcessingPosition;
        self
    }

    /// Restricts the scope to one logical key.
    #[must_use]
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }

    /// Restricts the scope to one partition.
    #[must_use]
    pub fn partition(mut self, partition: impl Into<String>) -> Self {
        self.partition = Some(partition.into());
        self
    }

    /// Adds a segment equality filter.
    #[must_use]
    pub fn segment(mut self, name: impl Into<String>, value: impl Into<PrimitiveValue>) -> Self {
        self.segment_filters.push(WindowFilter {
            name: name.into(),
            value: value.into(),
        });
        self
    }

    /// Adds a tag equality filter.
    #[must_use]
    pub fn tag(mut self, name: impl Into<String>, value: impl Into<PrimitiveValue>) -> Self {
        self.tag_filters.push(WindowFilter {
            name: name.into(),
            value: value.into(),
        });
        self
    }
}

/// Handling for records that do not have timestamps in event-time comparisons.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComparisonNullTimestampPolicy {
    /// Emit a diagnostic for records without event timestamps.
    Reject,
    /// Exclude records without event timestamps.
    Exclude,
}

/// Describes how recorded windows are normalized before comparison.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComparisonNormalizationPolicy {
    /// Whether open windows must be closed.
    pub require_closed_windows: bool,
    /// Whether ranges use start-inclusive/end-exclusive semantics.
    pub use_half_open_ranges: bool,
    /// Normalization axis.
    pub time_axis: crate::TemporalAxis,
    /// Open-window handling.
    pub open_window_policy: OpenWindowPolicy,
    /// Horizon used when clipping open windows.
    pub open_window_horizon: Option<crate::TemporalPoint>,
    /// Missing timestamp handling in event-time mode.
    pub null_timestamp_policy: ComparisonNullTimestampPolicy,
    /// Whether adjacent normalized windows can be coalesced.
    pub coalesce_adjacent_windows: bool,
    /// Duplicate normalized-window handling.
    pub duplicate_window_policy: ComparisonDuplicateWindowPolicy,
    /// Availability point used for known-at filtering.
    pub known_at: Option<crate::TemporalPoint>,
}

impl Default for ComparisonNormalizationPolicy {
    fn default() -> Self {
        Self::default_policy()
    }
}

impl ComparisonNormalizationPolicy {
    /// Returns the default historical comparison normalization policy.
    #[must_use]
    pub fn default_policy() -> Self {
        Self {
            require_closed_windows: false,
            use_half_open_ranges: true,
            time_axis: crate::TemporalAxis::ProcessingPosition,
            open_window_policy: OpenWindowPolicy::RequireClosed,
            open_window_horizon: None,
            null_timestamp_policy: ComparisonNullTimestampPolicy::Reject,
            coalesce_adjacent_windows: false,
            duplicate_window_policy: ComparisonDuplicateWindowPolicy::Preserve,
            known_at: None,
        }
    }

    /// Returns a policy that excludes open windows from historical comparison.
    #[must_use]
    pub fn require_closed() -> Self {
        Self::default_policy()
    }

    /// Returns a policy that clips open windows to an explicit horizon.
    #[must_use]
    pub fn clip_open_windows_to(horizon: crate::TemporalPoint) -> Self {
        Self {
            require_closed_windows: false,
            time_axis: horizon.axis(),
            open_window_policy: OpenWindowPolicy::ClipToHorizon,
            open_window_horizon: Some(horizon),
            ..Self::default_policy()
        }
    }

    /// Returns a policy that normalizes on the event-time axis.
    #[must_use]
    pub fn event_time() -> Self {
        Self {
            time_axis: crate::TemporalAxis::Timestamp,
            ..Self::default_policy()
        }
    }

    /// Returns this policy with missing event timestamps rejected.
    #[must_use]
    pub const fn rejecting_missing_event_time(mut self) -> Self {
        self.null_timestamp_policy = ComparisonNullTimestampPolicy::Reject;
        self
    }

    /// Returns this policy with missing event timestamps excluded.
    #[must_use]
    pub const fn excluding_missing_event_time(mut self) -> Self {
        self.null_timestamp_policy = ComparisonNullTimestampPolicy::Exclude;
        self
    }

    /// Returns this policy with a known-at availability point.
    #[must_use]
    pub fn with_known_at(mut self, point: crate::TemporalPoint) -> Self {
        self.known_at = Some(point);
        self
    }

    /// Returns this policy with adjacent-window coalescing enabled.
    #[must_use]
    pub const fn coalescing_adjacent_windows(mut self) -> Self {
        self.coalesce_adjacent_windows = true;
        self
    }

    /// Returns this policy with duplicate normalized windows rejected.
    #[must_use]
    pub const fn rejecting_duplicate_windows(mut self) -> Self {
        self.duplicate_window_policy = ComparisonDuplicateWindowPolicy::Reject;
        self
    }
}

/// Describes output preferences for a comparison plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ComparisonOutputOptions {
    /// Whether result output should include aligned segment details.
    pub include_aligned_segments: bool,
    /// Whether result output should include explain data.
    pub include_explain_data: bool,
}

impl Default for ComparisonOutputOptions {
    fn default() -> Self {
        Self::default_options()
    }
}

impl ComparisonOutputOptions {
    /// Returns the default comparison output options.
    #[must_use]
    pub const fn default_options() -> Self {
        Self {
            include_aligned_segments: true,
            include_explain_data: true,
        }
    }
}

/// Typed comparison plan.
#[non_exhaustive]
#[derive(Clone, PartialEq)]
pub struct ComparisonPlan {
    /// Comparison name.
    pub name: String,
    pub(crate) selection: ComparisonSelection,
    /// Optional window family scope.
    pub(crate) scope_window: Option<String>,
    /// Optional logical key scope.
    pub(crate) scope_key: Option<String>,
    /// Optional partition scope.
    pub(crate) scope_partition: Option<String>,
    /// Segment filters.
    pub(crate) scope_segments: Vec<WindowFilter>,
    /// Tag filters.
    pub(crate) scope_tags: Vec<WindowFilter>,
    /// Comparator declarations.
    pub(crate) comparators: Vec<Comparator>,
    /// Whether open windows must be closed during normalization.
    pub(crate) require_closed_windows: bool,
    /// Whether ranges use start-inclusive/end-exclusive semantics.
    pub(crate) use_half_open_ranges: bool,
    /// Temporal axis requested for normalization.
    pub(crate) time_axis: TemporalAxis,
    /// Missing timestamp handling in event-time mode.
    pub(crate) null_timestamp_policy: ComparisonNullTimestampPolicy,
    /// Availability point used for known-at filtering.
    pub(crate) known_at: Option<crate::TemporalPoint>,
    /// How open windows are handled.
    pub(crate) open_window_policy: OpenWindowPolicy,
    /// Exclusive horizon used when clipping open windows.
    pub(crate) open_window_horizon: Option<crate::TemporalPoint>,
    /// Whether adjacent normalized windows can be coalesced.
    pub(crate) coalesce_adjacent_windows: bool,
    /// Duplicate normalized-window handling.
    pub(crate) duplicate_window_policy: ComparisonDuplicateWindowPolicy,
    /// Result output preferences.
    pub(crate) output: ComparisonOutputOptions,
    /// Whether strict validation is enabled.
    pub(crate) strict: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ComparisonSelection {
    target: TargetSelection,
    against: ComparisonAgainstSelection,
}

#[derive(Clone, Debug, PartialEq)]
enum TargetSelection {
    Source(String),
    Selector(ComparisonSelector),
}

#[derive(Clone, Debug, PartialEq)]
enum ComparisonAgainstSelection {
    Legacy(AgainstSelection),
    Selectors(Vec<ComparisonSelector>),
    Contradictory {
        legacy: AgainstSelection,
        selectors: Vec<ComparisonSelector>,
    },
}

impl ComparisonSelection {
    pub(crate) fn legacy(target_source: impl Into<String>, against: AgainstSelection) -> Self {
        Self {
            target: TargetSelection::Source(target_source.into()),
            against: ComparisonAgainstSelection::Legacy(against),
        }
    }

    fn target_source(&self) -> &str {
        match &self.target {
            TargetSelection::Source(source) => source,
            TargetSelection::Selector(selector) => &selector.name,
        }
    }

    fn target_selector(&self) -> Option<&ComparisonSelector> {
        match &self.target {
            TargetSelection::Selector(selector) => Some(selector),
            TargetSelection::Source(_) => None,
        }
    }

    fn set_target_source(&mut self, source: String) {
        self.target = TargetSelection::Source(source);
    }

    fn set_target_selector(&mut self, selector: ComparisonSelector) {
        self.target = TargetSelection::Selector(selector);
    }

    fn legacy_against(&self) -> Option<&AgainstSelection> {
        match &self.against {
            ComparisonAgainstSelection::Legacy(against)
            | ComparisonAgainstSelection::Contradictory {
                legacy: against, ..
            } => Some(against),
            ComparisonAgainstSelection::Selectors(_) => None,
        }
    }

    fn against_selectors(&self) -> &[ComparisonSelector] {
        match &self.against {
            ComparisonAgainstSelection::Selectors(selectors)
            | ComparisonAgainstSelection::Contradictory { selectors, .. } => selectors,
            ComparisonAgainstSelection::Legacy(_) => &[],
        }
    }

    fn set_legacy_against(&mut self, against: AgainstSelection) {
        self.against = ComparisonAgainstSelection::Legacy(against);
    }

    fn push_against_selector(&mut self, selector: ComparisonSelector) {
        match &mut self.against {
            ComparisonAgainstSelection::Legacy(AgainstSelection::Sources(sources))
                if sources.is_empty() =>
            {
                self.against = ComparisonAgainstSelection::Selectors(vec![selector]);
            }
            ComparisonAgainstSelection::Legacy(_) => {
                let ComparisonAgainstSelection::Legacy(legacy) = std::mem::replace(
                    &mut self.against,
                    ComparisonAgainstSelection::Selectors(Vec::new()),
                ) else {
                    unreachable!("matched legacy comparison selection")
                };
                self.against = ComparisonAgainstSelection::Contradictory {
                    legacy,
                    selectors: vec![selector],
                };
            }
            ComparisonAgainstSelection::Selectors(selectors)
            | ComparisonAgainstSelection::Contradictory { selectors, .. } => {
                selectors.push(selector);
            }
        }
    }
}

impl fmt::Debug for ComparisonPlan {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let empty_against = AgainstSelection::Sources(Vec::new());
        formatter
            .debug_struct("ComparisonPlan")
            .field("name", &self.name)
            .field("target_source", &self.selection.target_source())
            .field(
                "against",
                self.selection.legacy_against().unwrap_or(&empty_against),
            )
            .field("target_selector", &self.selection.target_selector())
            .field("against_selectors", &self.selection.against_selectors())
            .field("scope_window", &self.scope_window)
            .field("scope_key", &self.scope_key)
            .field("scope_partition", &self.scope_partition)
            .field("scope_segments", &self.scope_segments)
            .field("scope_tags", &self.scope_tags)
            .field("comparators", &self.comparators)
            .field("require_closed_windows", &self.require_closed_windows)
            .field("use_half_open_ranges", &self.use_half_open_ranges)
            .field("time_axis", &self.time_axis)
            .field("null_timestamp_policy", &self.null_timestamp_policy)
            .field("known_at", &self.known_at)
            .field("open_window_policy", &self.open_window_policy)
            .field("open_window_horizon", &self.open_window_horizon)
            .field("coalesce_adjacent_windows", &self.coalesce_adjacent_windows)
            .field("duplicate_window_policy", &self.duplicate_window_policy)
            .field("output", &self.output)
            .field("strict", &self.strict)
            .finish()
    }
}

impl ComparisonPlan {
    /// Creates a comparison plan with validated-shape defaults.
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        target_source: impl Into<String>,
        against: AgainstSelection,
        comparators: Vec<Comparator>,
    ) -> Self {
        Self {
            name: name.into(),
            selection: ComparisonSelection::legacy(target_source, against),
            scope_window: None,
            scope_key: None,
            scope_partition: None,
            scope_segments: Vec::new(),
            scope_tags: Vec::new(),
            comparators,
            require_closed_windows: true,
            use_half_open_ranges: true,
            time_axis: TemporalAxis::ProcessingPosition,
            null_timestamp_policy: ComparisonNullTimestampPolicy::Reject,
            known_at: None,
            open_window_policy: OpenWindowPolicy::RequireClosed,
            open_window_horizon: None,
            coalesce_adjacent_windows: false,
            duplicate_window_policy: ComparisonDuplicateWindowPolicy::Preserve,
            output: ComparisonOutputOptions::default_options(),
            strict: false,
        }
    }

    /// Restricts the plan to one window family.
    #[must_use]
    pub fn with_scope_window(mut self, window_name: Option<String>) -> Self {
        self.scope_window = window_name;
        self
    }

    /// Configures whether open windows are allowed during normalization.
    #[must_use]
    pub fn with_require_closed_windows(mut self, require_closed_windows: bool) -> Self {
        self.require_closed_windows = require_closed_windows;
        self
    }

    /// Configures open-window handling and its optional clipping horizon.
    #[must_use]
    pub fn with_open_window_policy(
        mut self,
        policy: OpenWindowPolicy,
        horizon: Option<TemporalPoint>,
    ) -> Self {
        self.open_window_policy = policy;
        self.open_window_horizon = horizon;
        self
    }

    /// Enables strict validation diagnostics.
    #[must_use]
    pub fn with_strict(mut self, strict: bool) -> Self {
        self.strict = strict;
        self
    }

    pub(crate) fn effective_target_selector(&self) -> Cow<'_, ComparisonSelector> {
        self.selection.target_selector().map_or_else(
            || {
                Cow::Owned(ComparisonSelector::for_source(
                    self.selection.target_source(),
                ))
            },
            Cow::Borrowed,
        )
    }

    pub(crate) fn effective_against_selectors(&self) -> Vec<Cow<'_, ComparisonSelector>> {
        let selectors = self.selection.against_selectors();
        if !selectors.is_empty() {
            return selectors.iter().map(Cow::Borrowed).collect();
        }
        match self.selection.legacy_against() {
            None => Vec::new(),
            Some(AgainstSelection::Sources(sources)) => sources
                .iter()
                .map(|source| Cow::Owned(ComparisonSelector::for_source(source)))
                .collect(),
            Some(AgainstSelection::Cohort {
                name,
                sources,
                activity,
            }) => vec![Cow::Owned(
                ComparisonSelector::for_cohort_sources(sources.clone(), activity.clone())
                    .with_name(name.clone()),
            )],
        }
    }

    pub(crate) fn target_source(&self) -> &str {
        self.selection.target_source()
    }

    pub(crate) fn legacy_against(&self) -> Option<&AgainstSelection> {
        self.selection.legacy_against()
    }

    pub(crate) fn against_for_alignment(&self) -> Cow<'_, AgainstSelection> {
        self.selection.legacy_against().map_or_else(
            || Cow::Owned(AgainstSelection::Sources(Vec::new())),
            Cow::Borrowed,
        )
    }

    pub(crate) fn explicit_target_selector(&self) -> Option<&ComparisonSelector> {
        self.selection.target_selector()
    }

    pub(crate) fn explicit_against_selectors(&self) -> &[ComparisonSelector] {
        self.selection.against_selectors()
    }

    pub(crate) fn set_target_source(&mut self, source: String) {
        self.selection.set_target_source(source);
    }

    pub(crate) fn set_target_selector(&mut self, selector: ComparisonSelector) {
        self.selection.set_target_selector(selector);
    }

    pub(crate) fn set_legacy_against(&mut self, against: AgainstSelection) {
        self.selection.set_legacy_against(against);
    }

    pub(crate) fn push_against_selector(&mut self, selector: ComparisonSelector) {
        self.selection.push_against_selector(selector);
    }

    /// Returns whether every effective selector can be exported as portable data.
    #[must_use]
    pub fn is_serializable(&self) -> bool {
        self.effective_target_selector().is_serializable
            && self
                .effective_against_selectors()
                .iter()
                .all(|selector| selector.is_serializable)
    }

    /// Returns structural plan validation diagnostics without reading history.
    #[must_use]
    pub fn validate(&self) -> Vec<ComparisonDiagnostic> {
        let mut diagnostics = Vec::new();
        if self.name.trim().is_empty() {
            diagnostics.push(plan_diagnostic("MissingName", DiagnosticSeverity::Error));
        }

        if self.selection.target_selector().is_none()
            && self.selection.target_source().trim().is_empty()
        {
            diagnostics.push(plan_diagnostic("MissingTarget", DiagnosticSeverity::Error));
        }

        let against = self.effective_against_selectors();
        if against.is_empty() {
            diagnostics.push(plan_diagnostic("MissingAgainst", DiagnosticSeverity::Error));
        }

        if self.comparators.is_empty() {
            diagnostics.push(plan_diagnostic(
                "MissingComparator",
                DiagnosticSeverity::Error,
            ));
        }

        if self
            .scope_window
            .as_deref()
            .is_some_and(|value| value.trim().is_empty())
        {
            diagnostics.push(plan_diagnostic("EmptyScope", DiagnosticSeverity::Error));
        }

        let mut comparator_declarations = BTreeSet::new();
        for comparator in &self.comparators {
            let declaration = comparator.declaration();
            if !comparator_declarations.insert(declaration) {
                diagnostics.push(plan_diagnostic(
                    "DuplicateComparator",
                    DiagnosticSeverity::Error,
                ));
            }
            if matches!(
                comparator,
                Comparator::LeadLag {
                    tolerance_magnitude,
                    ..
                } | Comparator::AsOf {
                    tolerance_magnitude,
                    ..
                } if *tolerance_magnitude < 0
            ) {
                diagnostics.push(plan_diagnostic(
                    "NegativeTolerance",
                    DiagnosticSeverity::Error,
                ));
            }
        }

        if let Some(selector) = self.selection.target_selector()
            && selector.name.trim().is_empty()
        {
            diagnostics.push(plan_diagnostic(
                "EmptyTargetSelectorName",
                DiagnosticSeverity::Error,
            ));
        }
        if matches!(
            &self.selection.against,
            ComparisonAgainstSelection::Contradictory { .. }
        ) {
            diagnostics.push(plan_diagnostic(
                "ContradictoryAgainstSelection",
                DiagnosticSeverity::Error,
            ));
        }

        if self.selection.against_selectors().is_empty() {
            match self.selection.legacy_against().expect("legacy selection") {
                AgainstSelection::Sources(sources) => {
                    validate_source_list(sources, false, &mut diagnostics);
                }
                AgainstSelection::Cohort {
                    name,
                    sources,
                    activity,
                } => {
                    if name.trim().is_empty() {
                        diagnostics.push(plan_diagnostic(
                            "EmptyCohortName",
                            DiagnosticSeverity::Error,
                        ));
                    }
                    validate_source_list(sources, true, &mut diagnostics);
                    if activity.count().is_some_and(|count| count > sources.len()) {
                        diagnostics.push(plan_diagnostic(
                            "InvalidCohortCount",
                            DiagnosticSeverity::Error,
                        ));
                    }
                    if matches!(activity, CohortActivity::AtLeast { count: 0 }) {
                        diagnostics.push(plan_diagnostic(
                            "InvalidCohortCount",
                            DiagnosticSeverity::Error,
                        ));
                    }
                }
            }
        }

        let mut selector_names = BTreeSet::new();
        for selector in self
            .selection
            .target_selector()
            .into_iter()
            .chain(self.selection.against_selectors())
        {
            if selector.name.trim().is_empty() {
                diagnostics.push(plan_diagnostic(
                    "EmptySelectorName",
                    DiagnosticSeverity::Error,
                ));
            }
            if !selector_names.insert(selector.name.as_str()) {
                diagnostics.push(plan_diagnostic(
                    "DuplicateSelectorName",
                    DiagnosticSeverity::Error,
                ));
            }
        }
        validate_filters(&self.scope_segments, "Segment", &mut diagnostics);
        validate_filters(&self.scope_tags, "Tag", &mut diagnostics);

        if !self.use_half_open_ranges {
            diagnostics.push(plan_diagnostic(
                "UnsupportedRangeSemantics",
                DiagnosticSeverity::Error,
            ));
        }
        if let Some(horizon) = &self.open_window_horizon
            && horizon.axis() != self.time_axis
        {
            diagnostics.push(plan_diagnostic(
                "HorizonAxisMismatch",
                DiagnosticSeverity::Error,
            ));
        }
        if self.open_window_horizon.is_some()
            && self.open_window_policy != OpenWindowPolicy::ClipToHorizon
        {
            diagnostics.push(plan_diagnostic(
                "UnusedOpenWindowHorizon",
                DiagnosticSeverity::Error,
            ));
        }
        diagnostics
    }
}

fn validate_source_list(
    sources: &[String],
    cohort: bool,
    diagnostics: &mut Vec<ComparisonDiagnostic>,
) {
    if sources.is_empty() {
        diagnostics.push(plan_diagnostic(
            if cohort {
                "EmptyCohort"
            } else {
                "EmptyAgainstSources"
            },
            DiagnosticSeverity::Error,
        ));
    }
    let mut seen = BTreeSet::<&str>::new();
    for source in sources {
        if source.trim().is_empty() {
            diagnostics.push(plan_diagnostic("EmptySource", DiagnosticSeverity::Error));
        }
        if !seen.insert(source.as_str()) {
            diagnostics.push(plan_diagnostic(
                "DuplicateSource",
                DiagnosticSeverity::Error,
            ));
        }
    }
}

fn validate_filters(
    filters: &[WindowFilter],
    kind: &str,
    diagnostics: &mut Vec<ComparisonDiagnostic>,
) {
    let mut names = BTreeSet::new();
    for filter in filters {
        if filter.name.trim().is_empty() {
            diagnostics.push(plan_diagnostic(
                &format!("Empty{kind}FilterName"),
                DiagnosticSeverity::Error,
            ));
        }
        if !names.insert(filter.name.as_str()) {
            diagnostics.push(plan_diagnostic(
                &format!("Duplicate{kind}FilterName"),
                DiagnosticSeverity::Error,
            ));
        }
    }
}

/// Open-window normalization policy.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpenWindowPolicy {
    /// Open windows are rejected.
    RequireClosed,
    /// Open windows are clipped to the configured horizon.
    ClipToHorizon,
}

/// Duplicate normalized-window handling policy.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComparisonDuplicateWindowPolicy {
    /// Preserve duplicate normalized windows.
    Preserve,
    /// Exclude duplicate normalized windows and emit a diagnostic.
    Reject,
}
