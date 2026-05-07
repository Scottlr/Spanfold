use std::collections::BTreeMap;

use thiserror::Error;

use crate::{
    ComparisonDiagnostic, ComparisonResult, TemporalPoint, WindowHistoryFixture,
    WindowHistoryFixtureWindow,
};

/// Alias matching the cross-language fixture-builder naming.
pub type WindowHistoryFixtureBuilder = WindowHistoryFixture;

/// Alias matching the cross-language fixture-window-builder naming.
pub type WindowHistoryFixtureWindowBuilder = WindowHistoryFixtureWindow;

/// Assertion failure returned by Spanfold testing helpers.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
#[error("{message}")]
pub struct SpanfoldAssertionError {
    message: String,
}

impl SpanfoldAssertionError {
    /// Creates an assertion error with a concise message.
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    /// Returns the failure message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// Framework-neutral assertions for Spanfold comparison artifacts.
pub struct SpanfoldAssert;

impl SpanfoldAssert {
    /// Asserts that a comparison result is valid.
    pub fn is_valid(result: &ComparisonResult) -> Result<(), SpanfoldAssertionError> {
        if result.is_valid {
            return Ok(());
        }
        Err(SpanfoldAssertionError::new(
            "Expected a valid Spanfold result.",
        ))
    }

    /// Asserts that a comparison result contains no diagnostics.
    pub fn has_no_diagnostics(result: &ComparisonResult) -> Result<(), SpanfoldAssertionError> {
        if result.diagnostics.is_empty() {
            return Ok(());
        }
        Err(SpanfoldAssertionError::new(format!(
            "Expected no Spanfold diagnostics, found {}.",
            result.diagnostics.len()
        )))
    }

    /// Asserts that a comparison result contains a diagnostic code.
    pub fn has_diagnostic<'a>(
        result: &'a ComparisonResult,
        code: &str,
    ) -> Result<&'a ComparisonDiagnostic, SpanfoldAssertionError> {
        result
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == code)
            .ok_or_else(|| {
                SpanfoldAssertionError::new(format!("Expected Spanfold diagnostic {code}."))
            })
    }

    /// Asserts that a named row collection contains an expected count.
    pub fn has_row_count(
        result: &ComparisonResult,
        row_type: &str,
        expected_count: usize,
    ) -> Result<(), SpanfoldAssertionError> {
        let actual = row_count(result, row_type)?;
        if actual == expected_count {
            return Ok(());
        }
        Err(SpanfoldAssertionError::new(format!(
            "Expected {expected_count} {row_type} rows, found {actual}."
        )))
    }

    /// Asserts that no row is provisional.
    pub fn has_no_provisional_rows(
        result: &ComparisonResult,
    ) -> Result<(), SpanfoldAssertionError> {
        if !result.has_provisional_rows() {
            return Ok(());
        }
        Err(SpanfoldAssertionError::new(format!(
            "Expected no provisional Spanfold rows, found {}.",
            result.provisional_row_finalities().len()
        )))
    }

    /// Asserts that at least one row is provisional.
    pub fn has_provisional_rows(result: &ComparisonResult) -> Result<(), SpanfoldAssertionError> {
        if result.has_provisional_rows() {
            return Ok(());
        }
        Err(SpanfoldAssertionError::new(
            "Expected at least one provisional Spanfold row.",
        ))
    }
}

/// Snapshot normalization helpers for Spanfold artifacts.
pub struct SpanfoldSnapshot;

impl SpanfoldSnapshot {
    /// Normalizes line endings, trailing whitespace, and volatile record IDs.
    #[must_use]
    pub fn normalize(value: &str, normalize_record_ids: bool) -> String {
        let mut normalized = value.replace("\r\n", "\n").replace('\r', "\n");
        normalized = normalized
            .lines()
            .map(str::trim_end)
            .collect::<Vec<_>>()
            .join("\n")
            .trim_end()
            .to_owned();
        if normalize_record_ids {
            normalized = normalize_hex_record_ids(&normalized);
        }
        normalized.push('\n');
        normalized
    }

    /// Asserts that two snapshot strings are equal after normalization.
    pub fn assert_equal(expected: &str, actual: &str) -> Result<(), SpanfoldAssertionError> {
        let normalized_expected = Self::normalize(expected, true);
        let normalized_actual = Self::normalize(actual, true);
        if normalized_expected == normalized_actual {
            return Ok(());
        }
        Err(SpanfoldAssertionError::new("Spanfold snapshot mismatch."))
    }
}

/// Deterministic processing-position clock for comparison tests.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VirtualComparisonClock {
    position: i64,
}

impl VirtualComparisonClock {
    /// Creates a clock at an initial processing position.
    pub fn new(initial_position: i64) -> Result<Self, SpanfoldAssertionError> {
        if initial_position < 0 {
            return Err(SpanfoldAssertionError::new(
                "Initial position cannot be negative.",
            ));
        }
        Ok(Self {
            position: initial_position,
        })
    }

    /// Creates a clock at position zero.
    #[must_use]
    pub const fn zero() -> Self {
        Self { position: 0 }
    }

    /// Returns the current position.
    #[must_use]
    pub const fn position(self) -> i64 {
        self.position
    }

    /// Returns the current processing-position horizon.
    #[must_use]
    pub const fn horizon(self) -> TemporalPoint {
        TemporalPoint::position(self.position)
    }

    /// Advances by a non-negative position delta.
    pub fn advance_by(&mut self, positions: i64) -> Result<TemporalPoint, SpanfoldAssertionError> {
        if positions < 0 {
            return Err(SpanfoldAssertionError::new(
                "Position delta cannot be negative.",
            ));
        }
        self.position += positions;
        Ok(self.horizon())
    }

    /// Advances to an absolute processing position.
    pub fn advance_to(&mut self, position: i64) -> Result<TemporalPoint, SpanfoldAssertionError> {
        if position < self.position {
            return Err(SpanfoldAssertionError::new(
                "Virtual comparison clocks cannot move backwards.",
            ));
        }
        self.position = position;
        Ok(self.horizon())
    }
}

fn row_count(result: &ComparisonResult, row_type: &str) -> Result<usize, SpanfoldAssertionError> {
    let normalized = row_type.replace(['-', '_'], "").to_lowercase();
    match normalized.as_str() {
        "overlap" => Ok(result.overlap_rows.len()),
        "residual" => Ok(result.residual_rows.len()),
        "missing" => Ok(result.missing_rows.len()),
        "coverage" => Ok(result.coverage_rows.len()),
        "gap" => Ok(result.gap_rows.len()),
        "symmetricdifference" => Ok(result.symmetric_difference_rows.len()),
        "containment" => Ok(result.containment_rows.len()),
        "leadlag" => Ok(result.lead_lag_rows.len()),
        "asof" => Ok(result.as_of_rows.len()),
        _ => Err(SpanfoldAssertionError::new(format!(
            "Unknown Spanfold row type: {row_type}"
        ))),
    }
}

fn normalize_hex_record_ids(value: &str) -> String {
    let mut ids: BTreeMap<String, String> = BTreeMap::new();
    let mut next_id = 1;
    let mut output = String::with_capacity(value.len());
    let mut token = String::new();

    for character in value.chars() {
        if character.is_ascii_hexdigit() {
            token.push(character);
            continue;
        }
        flush_token(&mut output, &mut token, &mut ids, &mut next_id);
        output.push(character);
    }
    flush_token(&mut output, &mut token, &mut ids, &mut next_id);
    output
}

fn flush_token(
    output: &mut String,
    token: &mut String,
    ids: &mut BTreeMap<String, String>,
    next_id: &mut usize,
) {
    if token.len() >= 16 && token.len() <= 64 {
        let replacement = ids.entry(token.clone()).or_insert_with(|| {
            let replacement = format!("<record-id:{next_id}>");
            *next_id += 1;
            replacement
        });
        output.push_str(replacement);
    } else {
        output.push_str(token);
    }
    token.clear();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AgainstSelection, Comparator, ComparisonPlan, OpenWindowPolicy, WindowHistoryFixture,
    };

    #[test]
    fn fixture_aliases_can_create_comparison_history() {
        let history = WindowHistoryFixtureBuilder::new()
            .closed_window("DeviceOffline", "device-1", 1, 5, |window| {
                window.source("provider-a")
            })
            .expect("closed window")
            .closed_window("DeviceOffline", "device-1", 3, 7, |window| {
                window.source("provider-b")
            })
            .expect("closed window")
            .build();
        let plan = ComparisonPlan {
            name: "fixture helper".to_owned(),
            target_source: "provider-a".to_owned(),
            against: AgainstSelection::Sources(vec!["provider-b".to_owned()]),
            scope_window: Some("DeviceOffline".to_owned()),
            scope_segments: Vec::new(),
            scope_tags: Vec::new(),
            comparators: vec![Comparator::Overlap, Comparator::Residual],
            known_at: None,
            open_window_policy: OpenWindowPolicy::RequireClosed,
            open_window_horizon: None,
            strict: false,
        };

        let result = crate::compare(&history, &plan);

        SpanfoldAssert::is_valid(&result).expect("valid result");
        SpanfoldAssert::has_row_count(&result, "overlap", 1).expect("overlap rows");
        SpanfoldAssert::has_row_count(&result, "residual", 1).expect("residual rows");
    }

    #[test]
    fn snapshot_helper_normalizes_record_ids_and_line_endings() {
        let expected = "id=0123456789abcdef\r\nsame=0123456789abcdef  \n";
        let actual = "id=fedcba9876543210\nsame=fedcba9876543210\n";

        SpanfoldSnapshot::assert_equal(expected, actual).expect("normalized snapshots");
    }

    #[test]
    fn virtual_clock_produces_deterministic_horizons() {
        let mut clock = VirtualComparisonClock::zero();

        assert_eq!(clock.horizon(), TemporalPoint::position(0));
        assert_eq!(
            clock.advance_by(5).expect("advance by"),
            TemporalPoint::position(5)
        );
        assert_eq!(
            clock.advance_to(9).expect("advance to"),
            TemporalPoint::position(9)
        );
        assert!(clock.advance_to(8).is_err());
    }

    #[test]
    fn assertion_reports_unknown_row_type() {
        let history = WindowHistoryFixture::new().build();
        let plan = ComparisonPlan {
            name: "empty".to_owned(),
            target_source: "provider-a".to_owned(),
            against: AgainstSelection::Sources(vec!["provider-b".to_owned()]),
            scope_window: None,
            scope_segments: Vec::new(),
            scope_tags: Vec::new(),
            comparators: vec![Comparator::Overlap],
            known_at: None,
            open_window_policy: OpenWindowPolicy::RequireClosed,
            open_window_horizon: None,
            strict: false,
        };
        let result = crate::compare(&history, &plan);

        let error = SpanfoldAssert::has_row_count(&result, "not-a-row", 0)
            .expect_err("unknown rows should fail");

        assert!(error.message().contains("Unknown Spanfold row type"));
    }
}
