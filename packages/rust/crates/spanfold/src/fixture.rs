use std::str::FromStr;

use serde::Deserialize;
use thiserror::Error;

use crate::{
    AgainstSelection, CohortActivity, Comparator, ComparisonDuplicateWindowPolicy, ComparisonPlan,
    OpenWindowPolicy, PrimitiveValue, WindowFilter, WindowHistory, WindowHistoryFixture, compare,
};

/// Fixture loading and validation error.
#[derive(Debug, Error)]
pub enum FixtureError {
    /// JSON parse failure.
    #[error("failed to parse fixture JSON: {0}")]
    Json(#[from] serde_json::Error),
    /// Validation failure.
    #[error("{0}")]
    Validation(String),
    /// Temporal range construction failure.
    #[error("{0}")]
    Temporal(#[from] crate::TemporalRangeError),
}

/// Parsed contract fixture.
#[derive(Clone, Debug)]
pub struct ContractFixture {
    history: WindowHistory,
    plan: ComparisonPlan,
    expected: Option<serde_json::Value>,
}

impl ContractFixture {
    /// Parses a fixture JSON document.
    pub fn parse_json(json: &str) -> Result<Self, FixtureError> {
        let parsed: RawFixture = serde_json::from_str(json)?;
        parsed.validate()?;
        let mut builder = WindowHistoryFixture::new();
        for window in parsed.windows {
            let metadata = |w| apply_window_metadata(w, &window);
            if let Some(end) = window.end_position {
                builder = builder.closed_window(
                    window.window_name.clone(),
                    window.key.clone(),
                    window.start_position,
                    end,
                    metadata,
                )?;
            } else {
                builder = builder.open_window(
                    window.window_name.clone(),
                    window.key.clone(),
                    window.start_position,
                    metadata,
                );
            }
        }

        let expected = parsed.expected.or(parsed.expect);
        Ok(Self {
            history: builder.build(),
            plan: parsed.plan.try_into()?,
            expected,
        })
    }

    /// Returns the fixture history.
    #[must_use]
    pub fn history(&self) -> &WindowHistory {
        &self.history
    }

    /// Returns the fixture plan.
    #[must_use]
    pub fn plan(&self) -> &ComparisonPlan {
        &self.plan
    }

    /// Executes the fixture.
    #[must_use]
    pub fn execute(&self) -> crate::ComparisonResult {
        compare(&self.history, &self.plan)
    }

    /// Executes the fixture and enforces its optional expectation contract.
    pub fn execute_checked(&self) -> Result<crate::ComparisonResult, FixtureError> {
        let result = self.execute();
        if let Some(expected) = &self.expected {
            validate_expectation(expected, &result)?;
        }
        Ok(result)
    }
}

impl FromStr for ContractFixture {
    type Err = FixtureError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse_json(s)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFixture {
    schema: String,
    #[serde(rename = "schemaVersion")]
    schema_version: u32,
    #[serde(rename = "name", default)]
    _name: Option<String>,
    windows: Vec<RawWindow>,
    plan: RawPlan,
    #[serde(rename = "expected", default)]
    expected: Option<serde_json::Value>,
    #[serde(rename = "expect", default)]
    expect: Option<serde_json::Value>,
}

impl RawFixture {
    fn validate(&self) -> Result<(), FixtureError> {
        if self.schema != "spanfold.contract-fixture" {
            return Err(FixtureError::Validation(
                "$.schema must be spanfold.contract-fixture.".to_owned(),
            ));
        }
        if self.schema_version != 1 {
            return Err(FixtureError::Validation(
                "$.schemaVersion must be 1.".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawWindow {
    #[serde(rename = "windowName")]
    window_name: String,
    key: String,
    source: Option<String>,
    partition: Option<String>,
    #[serde(rename = "knownAtPosition")]
    known_at_position: Option<i64>,
    #[serde(rename = "startPosition")]
    start_position: i64,
    #[serde(rename = "endPosition")]
    end_position: Option<i64>,
    #[serde(default)]
    segments: Vec<RawNamedValue>,
    #[serde(default)]
    tags: Vec<RawNamedValue>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawNamedValue {
    name: String,
    value: PrimitiveValue,
    #[serde(rename = "parentName")]
    parent_name: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPlan {
    name: String,
    #[serde(rename = "targetSource")]
    target_source: String,
    #[serde(rename = "againstSources")]
    against_sources: Option<Vec<String>>,
    #[serde(rename = "againstCohort")]
    against_cohort: Option<RawAgainstCohort>,
    #[serde(rename = "scopeWindow")]
    scope_window: Option<String>,
    #[serde(rename = "scopeKey")]
    scope_key: Option<String>,
    #[serde(rename = "scopePartition")]
    scope_partition: Option<String>,
    #[serde(rename = "scopeSegments", default)]
    scope_segments: Vec<RawNamedValue>,
    #[serde(rename = "scopeTags", default)]
    scope_tags: Vec<RawNamedValue>,
    comparators: Vec<String>,
    #[serde(rename = "knownAtPosition")]
    known_at_position: Option<i64>,
    #[serde(rename = "liveHorizonPosition")]
    live_horizon_position: Option<i64>,
    #[serde(rename = "openWindowHorizonPosition")]
    open_window_horizon_position: Option<i64>,
    #[serde(rename = "coalesceAdjacentWindows", default)]
    coalesce_adjacent_windows: bool,
    #[serde(rename = "duplicateWindowPolicy")]
    duplicate_window_policy: Option<String>,
    strict: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAgainstCohort {
    name: String,
    sources: Vec<String>,
    activity: String,
    count: Option<usize>,
}

impl TryFrom<RawPlan> for ComparisonPlan {
    type Error = FixtureError;

    fn try_from(value: RawPlan) -> Result<Self, Self::Error> {
        if value.live_horizon_position.is_some() && value.open_window_horizon_position.is_some() {
            return Err(FixtureError::Validation(
                "liveHorizonPosition and openWindowHorizonPosition are mutually exclusive."
                    .to_owned(),
            ));
        }
        let against = if let Some(cohort) = value.against_cohort {
            if cohort.sources.is_empty() {
                return Err(FixtureError::Validation(
                    "againstCohort must declare at least one source.".to_owned(),
                ));
            }
            let activity = parse_cohort_activity(&cohort.activity, cohort.count)?;
            if activity
                .count()
                .is_some_and(|count| count > cohort.sources.len())
            {
                return Err(FixtureError::Validation(
                    "againstCohort activity count cannot exceed the number of declared sources."
                        .to_owned(),
                ));
            }
            AgainstSelection::Cohort {
                name: cohort.name,
                sources: cohort.sources,
                activity,
            }
        } else if let Some(sources) = value.against_sources {
            AgainstSelection::Sources(sources)
        } else {
            return Err(FixtureError::Validation(
                "$.plan must contain againstSources or againstCohort.".to_owned(),
            ));
        };

        let mut comparators = Vec::new();
        for comparator in value.comparators {
            let parsed = Comparator::parse(&comparator).ok_or_else(|| {
                FixtureError::Validation(format!("unsupported comparator: {comparator}"))
            })?;
            comparators.push(parsed);
        }

        let open_window_horizon = value
            .live_horizon_position
            .or(value.open_window_horizon_position);

        Ok(ComparisonPlan {
            name: value.name,
            target_source: value.target_source,
            against,
            target_selector: None,
            against_selectors: Vec::new(),
            scope_window: value.scope_window,
            scope_key: value.scope_key,
            scope_partition: value.scope_partition,
            scope_segments: into_filters(value.scope_segments),
            scope_tags: into_filters(value.scope_tags),
            comparators,
            require_closed_windows: open_window_horizon.is_none(),
            use_half_open_ranges: true,
            time_axis: crate::TemporalAxis::ProcessingPosition,
            null_timestamp_policy: crate::ComparisonNullTimestampPolicy::Reject,
            known_at: value.known_at_position.map(crate::TemporalPoint::position),
            open_window_policy: if open_window_horizon.is_some() {
                OpenWindowPolicy::ClipToHorizon
            } else {
                OpenWindowPolicy::RequireClosed
            },
            open_window_horizon: open_window_horizon.map(crate::TemporalPoint::position),
            coalesce_adjacent_windows: value.coalesce_adjacent_windows,
            duplicate_window_policy: parse_duplicate_window_policy(
                value.duplicate_window_policy.as_deref(),
            )?,
            output: crate::ComparisonOutputOptions::default_options(),
            strict: value.strict,
        })
    }
}

fn parse_duplicate_window_policy(
    value: Option<&str>,
) -> Result<ComparisonDuplicateWindowPolicy, FixtureError> {
    match value.unwrap_or("Preserve") {
        "Preserve" | "preserve" => Ok(ComparisonDuplicateWindowPolicy::Preserve),
        "Reject" | "reject" => Ok(ComparisonDuplicateWindowPolicy::Reject),
        value => Err(FixtureError::Validation(format!(
            "$.plan.duplicateWindowPolicy '{value}' is not supported."
        ))),
    }
}

fn parse_cohort_activity(
    activity: &str,
    count: Option<usize>,
) -> Result<CohortActivity, FixtureError> {
    match activity {
        "any" => Ok(CohortActivity::Any),
        "all" => Ok(CohortActivity::All),
        "none" => Ok(CohortActivity::None),
        "at-least" => {
            let count = count.ok_or_else(|| {
                FixtureError::Validation("againstCohort at-least requires count.".to_owned())
            })?;
            if count < 1 {
                return Err(FixtureError::Validation(
                    "againstCohort at-least count must be greater than zero.".to_owned(),
                ));
            }
            Ok(CohortActivity::AtLeast { count })
        }
        "at-most" => Ok(CohortActivity::AtMost {
            count: count.unwrap_or(0),
        }),
        "exactly" => Ok(CohortActivity::Exactly {
            count: count.unwrap_or(0),
        }),
        _ => Err(FixtureError::Validation(format!(
            "unsupported againstCohort activity: {activity}"
        ))),
    }
}

fn into_filters(values: Vec<RawNamedValue>) -> Vec<WindowFilter> {
    values
        .into_iter()
        .map(|item| WindowFilter {
            name: item.name,
            value: item.value,
        })
        .collect()
}

fn apply_window_metadata(
    mut builder: crate::WindowHistoryFixtureWindow,
    window: &RawWindow,
) -> crate::WindowHistoryFixtureWindow {
    if let Some(source) = &window.source {
        builder = builder.source(source.clone());
    }
    if let Some(known_at_position) = window.known_at_position {
        builder = builder.known_at_position(known_at_position);
    }
    if let Some(partition) = &window.partition {
        builder = builder.partition(partition.clone());
    }
    for segment in &window.segments {
        builder = if let Some(parent) = &segment.parent_name {
            builder.child_segment(segment.name.clone(), segment.value.clone(), parent.clone())
        } else {
            builder.segment(segment.name.clone(), segment.value.clone())
        };
    }
    for tag in &window.tags {
        builder = builder.tag(tag.name.clone(), tag.value.clone());
    }
    builder
}

fn validate_expectation(
    expected: &serde_json::Value,
    result: &crate::ComparisonResult,
) -> Result<(), FixtureError> {
    let Some(object) = expected.as_object() else {
        return Err(FixtureError::Validation(
            "fixture expectation must be an object".to_owned(),
        ));
    };
    if let Some(expected_valid) = object.get("isValid") {
        let actual_valid = serde_json::Value::Bool(result.is_valid);
        if expected_valid != &actual_valid {
            return Err(FixtureError::Validation(format!(
                "fixture expectation mismatch at isValid: expected {expected_valid}, actual {actual_valid}"
            )));
        }
    }
    if let Some(expected_diagnostics) = object.get("diagnostics") {
        let actual_diagnostics = serde_json::Value::Array(
            result
                .diagnostics
                .iter()
                .map(|diagnostic| serde_json::Value::String(diagnostic.code.clone()))
                .collect(),
        );
        if expected_diagnostics != &actual_diagnostics {
            return Err(FixtureError::Validation(format!(
                "fixture expectation mismatch at diagnostics: expected {expected_diagnostics}, actual {actual_diagnostics}"
            )));
        }
    }
    if let Some(expected_summaries) = object.get("summaries") {
        let actual_summaries = serde_json::to_value(&result.comparator_summaries)?;
        if !json_subset(expected_summaries, &actual_summaries) {
            return Err(FixtureError::Validation(
                "fixture expectation mismatch at summaries".to_owned(),
            ));
        }
    }
    Ok(())
}

fn json_subset(expected: &serde_json::Value, actual: &serde_json::Value) -> bool {
    match (expected, actual) {
        (serde_json::Value::Object(expected), serde_json::Value::Object(actual)) => {
            expected.iter().all(|(key, value)| {
                actual
                    .get(key)
                    .is_some_and(|candidate| json_subset(value, candidate))
            })
        }
        (serde_json::Value::Array(expected), serde_json::Value::Array(actual)) => {
            expected.len() == actual.len()
                && expected
                    .iter()
                    .zip(actual)
                    .all(|(expected, actual)| json_subset(expected, actual))
        }
        _ => expected == actual,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segmented_fixture_filters_against_windows_out_of_scope() {
        let fixture = ContractFixture::parse_json(include_str!(
            "../../../../dotnet/tests/Spanfold.Tests/Comparison/Fixtures/segmented-residual.json"
        ))
        .expect("fixture should parse");
        let result = fixture.execute();

        assert!(result.is_valid);
        assert_eq!(result.residual_rows.len(), 1);
        assert_eq!(result.residual_rows[0].range.start, 1);
        assert_eq!(result.residual_rows[0].range.end, 5);
    }

    #[test]
    fn fixture_plan_accepts_key_and_partition_scope() {
        let fixture = ContractFixture::parse_json(
            r#"{
  "schema": "spanfold.contract-fixture",
  "schemaVersion": 1,
  "name": "key-partition-scope",
  "windows": [
    {
      "windowName": "DeviceOffline",
      "key": "device-1",
      "source": "provider-a",
      "partition": "fleet-a",
      "startPosition": 1,
      "endPosition": 5
    },
    {
      "windowName": "DeviceOffline",
      "key": "device-1",
      "source": "provider-b",
      "partition": "fleet-a",
      "startPosition": 3,
      "endPosition": 6
    },
    {
      "windowName": "DeviceOffline",
      "key": "device-2",
      "source": "provider-a",
      "partition": "fleet-a",
      "startPosition": 1,
      "endPosition": 5
    }
  ],
  "plan": {
    "name": "key-partition-scope",
    "targetSource": "provider-a",
    "againstSources": ["provider-b"],
    "scopeWindow": "DeviceOffline",
    "scopeKey": "device-1",
    "scopePartition": "fleet-a",
    "coalesceAdjacentWindows": true,
    "duplicateWindowPolicy": "Reject",
    "comparators": ["overlap"],
    "strict": false
  },
  "expect": {
    "isValid": true,
    "rowCounts": {
      "overlap": 1
    },
    "diagnostics": []
  }
}"#,
        )
        .expect("fixture should parse");
        let result = fixture.execute();

        assert!(result.is_valid);
        assert_eq!(result.overlap_rows.len(), 1);
        assert_eq!(result.overlap_rows[0].key, "device-1");
        assert_eq!(result.overlap_rows[0].partition.as_deref(), Some("fleet-a"));
        assert!(fixture.plan().coalesce_adjacent_windows);
        assert_eq!(
            fixture.plan().duplicate_window_policy,
            ComparisonDuplicateWindowPolicy::Reject
        );
    }

    #[test]
    fn strict_broad_scope_fixture_emits_broad_selector_diagnostic() {
        let fixture = ContractFixture::parse_json(include_str!(
            "../../../../dotnet/tests/Spanfold.Tests/Comparison/Fixtures/strict-broad-scope.json"
        ))
        .expect("fixture should parse");
        let result = fixture.execute();

        assert!(!result.is_valid);
        assert_eq!(result.diagnostics[0].code, "BroadSelector");
    }

    #[test]
    fn cohort_any_fixture_parses_and_executes() {
        let fixture = ContractFixture::parse_json(include_str!(
            "../../../../dotnet/tests/Spanfold.Tests/Comparison/Fixtures/cohort-any-residual.json"
        ))
        .expect("fixture should parse");
        let result = fixture.execute();

        assert!(result.is_valid);
        assert!(result.residual_rows.is_empty());
    }

    #[test]
    fn bundled_contract_fixtures_enforce_expectations() {
        for json in [
            include_str!(
                "../../../../dotnet/tests/Spanfold.Tests/Comparison/Fixtures/basic-overlap.json"
            ),
            include_str!(
                "../../../../dotnet/tests/Spanfold.Tests/Comparison/Fixtures/cohort-any-residual.json"
            ),
            include_str!(
                "../../../../dotnet/tests/Spanfold.Tests/Comparison/Fixtures/segmented-residual.json"
            ),
            include_str!(
                "../../../../dotnet/tests/Spanfold.Tests/Comparison/Fixtures/strict-broad-scope.json"
            ),
        ] {
            ContractFixture::parse_json(json)
                .expect("fixture should parse")
                .execute_checked()
                .expect("fixture expectation should match");
        }
    }
}
