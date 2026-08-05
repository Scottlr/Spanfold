//! Portable and runtime comparison selector expressions.

use std::{fmt, sync::Arc};

use crate::TemporalAxis;

/// Comparison-side selection.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AgainstSelection {
    /// One or more source lanes.
    Sources(Vec<String>),
    /// Cohort activity across sources.
    Cohort {
        /// Exported cohort name.
        name: String,
        /// Participating sources.
        sources: Vec<String>,
        /// Activity rule.
        activity: CohortActivity,
    },
}

type SelectorPredicate = Arc<dyn Fn(&crate::WindowRecord) -> bool + Send + Sync>;

#[derive(Clone)]
enum ComparisonSelectorKind {
    Any,
    WindowName(String),
    Key(String),
    Source(String),
    Sources(Vec<String>),
    Partition(String),
    PositionRange {
        start_inclusive: i64,
        end_exclusive: Option<i64>,
    },
    TimeRange {
        start_inclusive: i64,
        end_exclusive: Option<i64>,
    },
    Runtime(SelectorPredicate),
    And(Box<ComparisonSelectorKind>, Box<ComparisonSelectorKind>),
    Or(Box<ComparisonSelectorKind>, Box<ComparisonSelectorKind>),
}

impl fmt::Debug for ComparisonSelectorKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Any => formatter.write_str("Any"),
            Self::WindowName(value) => formatter.debug_tuple("WindowName").field(value).finish(),
            Self::Key(value) => formatter.debug_tuple("Key").field(value).finish(),
            Self::Source(value) => formatter.debug_tuple("Source").field(value).finish(),
            Self::Sources(value) => formatter.debug_tuple("Sources").field(value).finish(),
            Self::Partition(value) => formatter.debug_tuple("Partition").field(value).finish(),
            Self::PositionRange {
                start_inclusive,
                end_exclusive,
            } => formatter
                .debug_struct("PositionRange")
                .field("start_inclusive", start_inclusive)
                .field("end_exclusive", end_exclusive)
                .finish(),
            Self::TimeRange {
                start_inclusive,
                end_exclusive,
            } => formatter
                .debug_struct("TimeRange")
                .field("start_inclusive", start_inclusive)
                .field("end_exclusive", end_exclusive)
                .finish(),
            Self::Runtime(_) => formatter.write_str("Runtime(<predicate>)"),
            Self::And(left, right) => formatter
                .debug_tuple("And")
                .field(left)
                .field(right)
                .finish(),
            Self::Or(left, right) => formatter
                .debug_tuple("Or")
                .field(left)
                .field(right)
                .finish(),
        }
    }
}

impl ComparisonSelectorKind {
    fn matches(&self, window: &crate::WindowRecord) -> bool {
        match self {
            Self::Any => true,
            Self::WindowName(window_name) => window.window_name() == window_name,
            Self::Key(key) => window.key() == key,
            Self::Source(source) => window.source() == Some(source.as_str()),
            Self::Sources(sources) => sources
                .iter()
                .any(|source| window.source() == Some(source.as_str())),
            Self::Partition(partition) => window.partition() == Some(partition.as_str()),
            Self::PositionRange {
                start_inclusive,
                end_exclusive,
            } => {
                let start = window.start();
                start.axis() == TemporalAxis::ProcessingPosition
                    && start.magnitude() >= *start_inclusive
                    && end_exclusive.is_none_or(|end| start.magnitude() < end)
            }
            Self::TimeRange {
                start_inclusive,
                end_exclusive,
            } => {
                let start = window.start();
                start.axis() == TemporalAxis::Timestamp
                    && start.magnitude() >= *start_inclusive
                    && end_exclusive.is_none_or(|end| start.magnitude() < end)
            }
            Self::Runtime(predicate) => predicate(window),
            Self::And(left, right) => left.matches(window) && right.matches(window),
            Self::Or(left, right) => left.matches(window) || right.matches(window),
        }
    }
}

/// Describes a selection used by a window comparison plan.
#[derive(Clone, Debug)]
pub struct ComparisonSelector {
    /// Stable selector name used in output and diagnostics.
    pub(crate) name: String,
    /// Human-readable selector description.
    pub description: String,
    /// Whether this selector can be exported as plan data.
    pub is_serializable: bool,
    /// Cohort activity rule when this selector represents a cohort.
    pub cohort_activity: Option<CohortActivity>,
    /// Source identities that belong to this cohort selector.
    pub cohort_sources: Vec<String>,
    kind: ComparisonSelectorKind,
}

impl PartialEq for ComparisonSelector {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.description == other.description
            && self.is_serializable == other.is_serializable
            && self.cohort_activity == other.cohort_activity
            && self.cohort_sources == other.cohort_sources
    }
}

impl ComparisonSelector {
    /// Creates a serializable selector descriptor that matches every window.
    #[must_use]
    pub fn serializable(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            is_serializable: true,
            cohort_activity: None,
            cohort_sources: Vec::new(),
            kind: ComparisonSelectorKind::Any,
        }
    }

    /// Creates a selector for a configured window name.
    #[must_use]
    pub fn for_window_name(window_name: impl Into<String>) -> Self {
        let window_name = window_name.into();
        Self {
            name: format!("window:{window_name}"),
            description: format!("window name = {window_name}"),
            is_serializable: true,
            cohort_activity: None,
            cohort_sources: Vec::new(),
            kind: ComparisonSelectorKind::WindowName(window_name),
        }
    }

    /// Creates a selector for a recorded window key.
    #[must_use]
    pub fn for_key(key: impl Into<String>) -> Self {
        let key = key.into();
        Self {
            name: format!("key:{key}"),
            description: format!("key = {key}"),
            is_serializable: true,
            cohort_activity: None,
            cohort_sources: Vec::new(),
            kind: ComparisonSelectorKind::Key(key),
        }
    }

    /// Creates a selector for a source identity.
    #[must_use]
    pub fn for_source(source: impl Into<String>) -> Self {
        let source = source.into();
        Self {
            name: format!("source:{source}"),
            description: format!("source = {source}"),
            is_serializable: true,
            cohort_activity: None,
            cohort_sources: Vec::new(),
            kind: ComparisonSelectorKind::Source(source),
        }
    }

    /// Creates a selector for any of several source identities.
    #[must_use]
    pub fn for_sources(sources: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self::for_sources_core(sources, None)
    }

    /// Creates a selector for a cohort of source identities.
    #[must_use]
    pub fn for_cohort_sources(
        sources: impl IntoIterator<Item = impl Into<String>>,
        activity: CohortActivity,
    ) -> Self {
        Self::for_sources_core(sources, Some(activity))
    }

    fn for_sources_core(
        sources: impl IntoIterator<Item = impl Into<String>>,
        activity: Option<CohortActivity>,
    ) -> Self {
        let sources = sources.into_iter().map(Into::into).collect::<Vec<_>>();
        let name = format!("sources:{}", sources.join(","));
        let description = format!("source in [{}]", sources.join(", "));
        Self {
            name,
            description,
            is_serializable: true,
            cohort_activity: activity,
            cohort_sources: sources.clone(),
            kind: ComparisonSelectorKind::Sources(sources),
        }
    }

    /// Creates a selector for a partition identity.
    #[must_use]
    pub fn for_partition(partition: impl Into<String>) -> Self {
        let partition = partition.into();
        Self {
            name: format!("partition:{partition}"),
            description: format!("partition = {partition}"),
            is_serializable: true,
            cohort_activity: None,
            cohort_sources: Vec::new(),
            kind: ComparisonSelectorKind::Partition(partition),
        }
    }

    /// Creates a selector for a half-open processing-position start range.
    pub fn for_position_range(
        start_inclusive: i64,
        end_exclusive: Option<i64>,
    ) -> Result<Self, ComparisonSelectorError> {
        if end_exclusive.is_some_and(|end| end < start_inclusive) {
            return Err(ComparisonSelectorError::RangeEndBeforeStart);
        }
        let end_label = end_exclusive.map_or_else(|| "*".to_owned(), |end| end.to_string());
        Ok(Self {
            name: format!("position:{start_inclusive}..{end_label}"),
            description: format!("start position in [{start_inclusive}, {end_label})"),
            is_serializable: true,
            cohort_activity: None,
            cohort_sources: Vec::new(),
            kind: ComparisonSelectorKind::PositionRange {
                start_inclusive,
                end_exclusive,
            },
        })
    }

    /// Creates a selector for a half-open event-time start range.
    pub fn for_time_range(
        start_inclusive: i64,
        end_exclusive: Option<i64>,
    ) -> Result<Self, ComparisonSelectorError> {
        if end_exclusive.is_some_and(|end| end < start_inclusive) {
            return Err(ComparisonSelectorError::RangeEndBeforeStart);
        }
        let end_label = end_exclusive.map_or_else(|| "*".to_owned(), |end| end.to_string());
        Ok(Self {
            name: format!("time:{start_inclusive}..{end_label}"),
            description: format!("start time in [{start_inclusive}, {end_label})"),
            is_serializable: true,
            cohort_activity: None,
            cohort_sources: Vec::new(),
            kind: ComparisonSelectorKind::TimeRange {
                start_inclusive,
                end_exclusive,
            },
        })
    }

    /// Creates a runtime-only selector backed by a predicate.
    #[must_use]
    pub fn runtime_only(
        name: impl Into<String>,
        description: impl Into<String>,
        predicate: impl Fn(&crate::WindowRecord) -> bool + Send + Sync + 'static,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            is_serializable: false,
            cohort_activity: None,
            cohort_sources: Vec::new(),
            kind: ComparisonSelectorKind::Runtime(Arc::new(predicate)),
        }
    }

    /// Creates a copy of this selector with a different display name.
    #[must_use]
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Creates a selector that requires both selectors to match.
    #[must_use]
    pub fn and(self, other: Self) -> Self {
        Self {
            name: format!("{}&{}", self.name, other.name),
            description: format!("({}) and ({})", self.description, other.description),
            is_serializable: self.is_serializable && other.is_serializable,
            cohort_activity: None,
            cohort_sources: Vec::new(),
            kind: ComparisonSelectorKind::And(Box::new(self.kind), Box::new(other.kind)),
        }
    }

    /// Creates a selector that allows either selector to match.
    #[must_use]
    pub fn or(self, other: Self) -> Self {
        Self {
            name: format!("{}|{}", self.name, other.name),
            description: format!("({}) or ({})", self.description, other.description),
            is_serializable: self.is_serializable && other.is_serializable,
            cohort_activity: None,
            cohort_sources: Vec::new(),
            kind: ComparisonSelectorKind::Or(Box::new(self.kind), Box::new(other.kind)),
        }
    }

    /// Returns whether this selector matches a recorded window.
    #[must_use]
    pub fn matches(&self, window: &crate::WindowRecord) -> bool {
        self.kind.matches(window)
    }

    /// Returns the portable selector expression used by plan exports.
    pub(crate) fn export_expression(&self) -> serde_json::Value {
        fn encode(kind: &ComparisonSelectorKind) -> serde_json::Value {
            match kind {
                ComparisonSelectorKind::Any => serde_json::json!({"kind": "any"}),
                ComparisonSelectorKind::WindowName(value) => {
                    serde_json::json!({"kind": "windowName", "value": value})
                }
                ComparisonSelectorKind::Key(value) => {
                    serde_json::json!({"kind": "key", "value": value})
                }
                ComparisonSelectorKind::Source(value) => {
                    serde_json::json!({"kind": "source", "value": value})
                }
                ComparisonSelectorKind::Sources(values) => {
                    serde_json::json!({"kind": "sources", "values": values})
                }
                ComparisonSelectorKind::Partition(value) => {
                    serde_json::json!({"kind": "partition", "value": value})
                }
                ComparisonSelectorKind::PositionRange {
                    start_inclusive,
                    end_exclusive,
                } => serde_json::json!({
                    "kind": "positionRange",
                    "startInclusive": start_inclusive,
                    "endExclusive": end_exclusive
                }),
                ComparisonSelectorKind::TimeRange {
                    start_inclusive,
                    end_exclusive,
                } => serde_json::json!({
                    "kind": "timeRange",
                    "startInclusive": start_inclusive,
                    "endExclusive": end_exclusive
                }),
                ComparisonSelectorKind::Runtime(_) => serde_json::json!({"kind": "runtime"}),
                ComparisonSelectorKind::And(left, right) => serde_json::json!({
                    "kind": "and",
                    "left": encode(left),
                    "right": encode(right)
                }),
                ComparisonSelectorKind::Or(left, right) => serde_json::json!({
                    "kind": "or",
                    "left": encode(left),
                    "right": encode(right)
                }),
            }
        }

        encode(&self.kind)
    }
}

/// Selector construction error.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ComparisonSelectorError {
    /// Range end is before the start.
    #[error("selector range end cannot be earlier than the start")]
    RangeEndBeforeStart,
}

/// Cohort activity rule.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CohortActivity {
    /// Any active source makes the cohort active.
    Any,
    /// All declared sources must be active.
    All,
    /// No declared source may be active.
    None,
    /// At least `count` sources must be active.
    AtLeast {
        /// Required active-member count.
        count: usize,
    },
    /// At most `count` sources may be active.
    AtMost {
        /// Maximum active-member count.
        count: usize,
    },
    /// Exactly `count` sources must be active.
    Exactly {
        /// Exact active-member count.
        count: usize,
    },
}

impl CohortActivity {
    /// Returns the export rule name.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Any => "any",
            Self::All => "all",
            Self::None => "none",
            Self::AtLeast { .. } => "at-least",
            Self::AtMost { .. } => "at-most",
            Self::Exactly { .. } => "exactly",
        }
    }

    /// Returns the configured threshold, when any.
    #[must_use]
    pub const fn count(&self) -> Option<usize> {
        match self {
            Self::Any | Self::All | Self::None => None,
            Self::AtLeast { count } | Self::AtMost { count } | Self::Exactly { count } => {
                Some(*count)
            }
        }
    }

    /// Evaluates activity for a given active-member count.
    #[must_use]
    pub fn is_active(&self, active_count: usize, member_count: usize) -> bool {
        match self {
            Self::Any => active_count >= 1,
            Self::All => active_count == member_count,
            Self::None => active_count == 0,
            Self::AtLeast { count } => active_count >= *count,
            Self::AtMost { count } => active_count <= *count,
            Self::Exactly { count } => active_count == *count,
        }
    }
}
