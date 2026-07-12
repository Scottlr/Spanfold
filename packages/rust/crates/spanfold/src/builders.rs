use crate::{
    AgainstSelection, CohortActivity, Comparator, ComparisonDiagnostic,
    ComparisonDuplicateWindowPolicy, ComparisonNormalizationPolicy, ComparisonOutputOptions,
    ComparisonPlan, ComparisonScope, ComparisonSelector, OpenWindowPolicy, PreparedComparison,
    PrimitiveValue, TemporalPoint, WindowFilter, WindowHistory, align, compare, compare_live,
    prepare, prepare_live,
};

/// Fluent comparison builder over an existing recorded history.
#[derive(Clone, Debug)]
pub struct WindowComparisonBuilder<'a> {
    history: &'a WindowHistory,
    plan: ComparisonPlan,
}

impl WindowHistory {
    /// Starts a fluent comparison builder over the recorded history.
    #[must_use]
    pub fn compare(&self, name: impl Into<String>) -> WindowComparisonBuilder<'_> {
        WindowComparisonBuilder {
            history: self,
            plan: ComparisonPlan {
                name: name.into(),
                target_source: String::new(),
                against: AgainstSelection::Sources(Vec::new()),
                target_selector: None,
                against_selectors: Vec::new(),
                scope_window: None,
                scope_key: None,
                scope_partition: None,
                scope_segments: Vec::new(),
                scope_tags: Vec::new(),
                comparators: Vec::new(),
                require_closed_windows: true,
                use_half_open_ranges: true,
                time_axis: crate::TemporalAxis::ProcessingPosition,
                null_timestamp_policy: crate::ComparisonNullTimestampPolicy::Reject,
                known_at: None,
                open_window_policy: OpenWindowPolicy::RequireClosed,
                open_window_horizon: None,
                coalesce_adjacent_windows: false,
                duplicate_window_policy: ComparisonDuplicateWindowPolicy::Preserve,
                output: crate::ComparisonOutputOptions::default_options(),
                strict: false,
            },
        }
    }
}

impl<'a> WindowComparisonBuilder<'a> {
    /// Returns the current comparison plan.
    #[must_use]
    pub fn plan(&self) -> &ComparisonPlan {
        &self.plan
    }

    /// Sets the target source lane.
    #[must_use]
    pub fn target_source(mut self, source: impl Into<String>) -> Self {
        self.plan.target_source = source.into();
        self.plan.target_selector = None;
        self
    }

    /// Sets the target selector for the comparison.
    #[must_use]
    pub fn target_selector(mut self, selector: ComparisonSelector) -> Self {
        self.plan.target_source = selector.name.clone();
        self.plan.target_selector = Some(selector);
        self
    }

    /// Sets one comparison source lane.
    #[must_use]
    pub fn against_source(mut self, source: impl Into<String>) -> Self {
        self.plan.against = AgainstSelection::Sources(vec![source.into()]);
        self.plan.against_selectors.clear();
        self
    }

    /// Sets multiple comparison source lanes.
    #[must_use]
    pub fn against_sources(mut self, sources: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.plan.against =
            AgainstSelection::Sources(sources.into_iter().map(Into::into).collect::<Vec<_>>());
        self.plan.against_selectors.clear();
        self
    }

    /// Adds a comparison selector.
    #[must_use]
    pub fn against_selector(mut self, selector: ComparisonSelector) -> Self {
        self.plan.against_selectors.push(selector);
        self
    }

    /// Configures a cohort-based comparison side.
    #[must_use]
    pub fn against_cohort(
        mut self,
        name: impl Into<String>,
        sources: impl IntoIterator<Item = impl Into<String>>,
        activity: CohortActivity,
    ) -> Self {
        self.plan.against = AgainstSelection::Cohort {
            name: name.into(),
            sources: sources.into_iter().map(Into::into).collect::<Vec<_>>(),
            activity,
        };
        self.plan.against_selectors.clear();
        self
    }

    /// Limits the comparison to one window family.
    #[must_use]
    pub fn scope_window(mut self, window_name: impl Into<String>) -> Self {
        self.plan.scope_window = Some(window_name.into());
        self
    }

    /// Applies a reusable comparison scope.
    #[must_use]
    pub fn scope(mut self, scope: ComparisonScope) -> Self {
        self.plan.scope_window = scope.window_name;
        self.plan.scope_key = scope.key;
        self.plan.scope_partition = scope.partition;
        self.plan.time_axis = scope.time_axis;
        self.plan.scope_segments = scope.segment_filters;
        self.plan.scope_tags = scope.tag_filters;
        self
    }

    /// Limits the comparison to one logical key.
    #[must_use]
    pub fn scope_key(mut self, key: impl Into<String>) -> Self {
        self.plan.scope_key = Some(key.into());
        self
    }

    /// Limits the comparison to one partition.
    #[must_use]
    pub fn scope_partition(mut self, partition: impl Into<String>) -> Self {
        self.plan.scope_partition = Some(partition.into());
        self
    }

    /// Adds a segment equality filter.
    #[must_use]
    pub fn scope_segment(
        mut self,
        name: impl Into<String>,
        value: impl Into<PrimitiveValue>,
    ) -> Self {
        self.plan.scope_segments.push(WindowFilter {
            name: name.into(),
            value: value.into(),
        });
        self
    }

    /// Adds a tag equality filter.
    #[must_use]
    pub fn scope_tag(mut self, name: impl Into<String>, value: impl Into<PrimitiveValue>) -> Self {
        self.plan.scope_tags.push(WindowFilter {
            name: name.into(),
            value: value.into(),
        });
        self
    }

    /// Sets a known-at processing position.
    #[must_use]
    pub fn known_at_position(mut self, position: i64) -> Self {
        self.plan.known_at = Some(TemporalPoint::position(position));
        self
    }

    /// Clips open windows to a processing-position horizon.
    #[must_use]
    pub fn clip_open_windows_to_position(mut self, position: i64) -> Self {
        self.plan.require_closed_windows = false;
        self.plan.open_window_policy = OpenWindowPolicy::ClipToHorizon;
        self.plan.open_window_horizon = Some(TemporalPoint::position(position));
        self
    }

    /// Applies a reusable normalization policy.
    #[must_use]
    pub fn normalization(mut self, policy: ComparisonNormalizationPolicy) -> Self {
        self.plan.require_closed_windows = policy.require_closed_windows;
        self.plan.use_half_open_ranges = policy.use_half_open_ranges;
        self.plan.time_axis = policy.time_axis;
        self.plan.null_timestamp_policy = policy.null_timestamp_policy;
        self.plan.known_at = policy.known_at;
        self.plan.open_window_policy = policy.open_window_policy;
        self.plan.open_window_horizon = policy.open_window_horizon;
        self.plan.coalesce_adjacent_windows = policy.coalesce_adjacent_windows;
        self.plan.duplicate_window_policy = policy.duplicate_window_policy;
        self
    }

    /// Coalesces adjacent normalized windows with identical comparison scope.
    #[must_use]
    pub fn coalesce_adjacent_windows(mut self) -> Self {
        self.plan.coalesce_adjacent_windows = true;
        self
    }

    /// Excludes duplicate normalized windows and emits a diagnostic.
    #[must_use]
    pub fn reject_duplicate_windows(mut self) -> Self {
        self.plan.duplicate_window_policy = ComparisonDuplicateWindowPolicy::Reject;
        self
    }

    /// Sets comparison result output preferences.
    #[must_use]
    pub fn output(mut self, output: ComparisonOutputOptions) -> Self {
        self.plan.output = output;
        self
    }

    /// Adds a comparator declaration.
    #[must_use]
    pub fn use_comparator(mut self, comparator: Comparator) -> Self {
        self.plan.comparators.push(comparator);
        self
    }

    /// Adds overlap rows.
    #[must_use]
    pub fn overlap(self) -> Self {
        self.use_comparator(Comparator::Overlap)
    }

    /// Adds residual rows.
    #[must_use]
    pub fn residual(self) -> Self {
        self.use_comparator(Comparator::Residual)
    }

    /// Adds missing rows.
    #[must_use]
    pub fn missing(self) -> Self {
        self.use_comparator(Comparator::Missing)
    }

    /// Adds coverage rows.
    #[must_use]
    pub fn coverage(self) -> Self {
        self.use_comparator(Comparator::Coverage)
    }

    /// Adds gap rows.
    #[must_use]
    pub fn gap(self) -> Self {
        self.use_comparator(Comparator::Gap)
    }

    /// Adds symmetric-difference rows.
    #[must_use]
    pub fn symmetric_difference(self) -> Self {
        self.use_comparator(Comparator::SymmetricDifference)
    }

    /// Adds containment rows.
    #[must_use]
    pub fn containment(self) -> Self {
        self.use_comparator(Comparator::Containment)
    }

    /// Enables strict execution.
    #[must_use]
    pub fn strict(mut self) -> Self {
        self.plan.strict = true;
        self
    }

    /// Returns plan diagnostics without running comparators.
    #[must_use]
    pub fn validate(&self) -> Vec<ComparisonDiagnostic> {
        self.plan.validate()
    }

    /// Prepares the comparison.
    #[must_use]
    pub fn prepare(&self) -> PreparedComparison {
        prepare(self.history, &self.plan)
    }

    /// Prepares a live comparison at the supplied evaluation horizon.
    #[must_use]
    pub fn prepare_live(&self, evaluation_horizon: TemporalPoint) -> PreparedComparison {
        prepare_live(self.history, &self.plan, evaluation_horizon)
    }

    /// Executes the comparison.
    #[must_use]
    pub fn run(&self) -> crate::ComparisonResult {
        compare(self.history, &self.plan)
    }

    /// Executes the comparison and optionally writes configured export artifacts.
    pub fn run_with_exports(
        &self,
        debug_html: &crate::ComparisonDebugHtmlOptions,
        llm_context: &crate::ComparisonLlmContextOptions,
    ) -> Result<crate::ComparisonResult, crate::ComparisonExportError> {
        let result = self.run();
        crate::export::export_configured_bundle(&result, debug_html, llm_context)?;
        Ok(result)
    }

    /// Executes the comparison and optionally writes a debug HTML artifact.
    pub fn run_with_debug_html(
        &self,
        debug_html: &crate::ComparisonDebugHtmlOptions,
    ) -> Result<crate::ComparisonResult, crate::ComparisonExportError> {
        self.run_with_exports(debug_html, &crate::ComparisonLlmContextOptions::disabled())
    }

    /// Executes the comparison and optionally writes an LLM context artifact.
    pub fn run_with_llm_context(
        &self,
        llm_context: &crate::ComparisonLlmContextOptions,
    ) -> Result<crate::ComparisonResult, crate::ComparisonExportError> {
        self.run_with_exports(&crate::ComparisonDebugHtmlOptions::disabled(), llm_context)
    }

    /// Executes a live comparison at the supplied evaluation horizon.
    #[must_use]
    pub fn run_live(&self, evaluation_horizon: TemporalPoint) -> crate::ComparisonResult {
        compare_live(self.history, &self.plan, evaluation_horizon)
    }

    /// Executes a live comparison and optionally writes configured export artifacts.
    pub fn run_live_with_exports(
        &self,
        evaluation_horizon: TemporalPoint,
        debug_html: &crate::ComparisonDebugHtmlOptions,
        llm_context: &crate::ComparisonLlmContextOptions,
    ) -> Result<crate::ComparisonResult, crate::ComparisonExportError> {
        let result = self.run_live(evaluation_horizon);
        crate::export::export_configured_bundle(&result, debug_html, llm_context)?;
        Ok(result)
    }

    /// Executes a live comparison and optionally writes a debug HTML artifact.
    pub fn run_live_with_debug_html(
        &self,
        evaluation_horizon: TemporalPoint,
        debug_html: &crate::ComparisonDebugHtmlOptions,
    ) -> Result<crate::ComparisonResult, crate::ComparisonExportError> {
        self.run_live_with_exports(
            evaluation_horizon,
            debug_html,
            &crate::ComparisonLlmContextOptions::disabled(),
        )
    }

    /// Executes a live comparison and optionally writes an LLM context artifact.
    pub fn run_live_with_llm_context(
        &self,
        evaluation_horizon: TemporalPoint,
        llm_context: &crate::ComparisonLlmContextOptions,
    ) -> Result<crate::ComparisonResult, crate::ComparisonExportError> {
        self.run_live_with_exports(
            evaluation_horizon,
            &crate::ComparisonDebugHtmlOptions::disabled(),
            llm_context,
        )
    }

    /// Aligns the prepared comparison.
    #[must_use]
    pub fn align(&self) -> crate::AlignedComparison {
        align(&self.prepare())
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use crate::{
        ComparisonDebugHtmlOptions, ComparisonLlmContextOptions, ComparisonNormalizationPolicy,
        ComparisonNullTimestampPolicy, ComparisonScope, ComparisonSelector, TemporalAxis,
        TemporalPoint, WindowHistoryFixture,
    };

    #[test]
    fn builder_can_write_configured_debug_and_llm_exports() {
        let history = WindowHistoryFixture::new()
            .closed_window("DeviceOffline", "device-1", 1, 5, |w| {
                w.source("provider-a")
            })
            .expect("target")
            .closed_window("DeviceOffline", "device-1", 3, 6, |w| {
                w.source("provider-b")
            })
            .expect("against")
            .build();
        let directory = unique_temp_dir("spanfold-builder-exports");
        let debug_path = directory.join("debug").join("provider-qa.html");
        let llm_path = directory.join("llm").join("provider-qa.llm.json");

        let result = history
            .compare("Provider QA")
            .target_source("provider-a")
            .against_source("provider-b")
            .scope_window("DeviceOffline")
            .overlap()
            .run_with_exports(
                &ComparisonDebugHtmlOptions::to_file(&debug_path),
                &ComparisonLlmContextOptions::to_file(&llm_path),
            )
            .expect("configured exports");

        assert!(result.is_valid);
        assert!(
            fs::read_to_string(&debug_path)
                .expect("debug html")
                .contains("Provider QA")
        );
        assert!(
            fs::read_to_string(&llm_path)
                .expect("llm context")
                .contains("spanfold.comparison.llm-context")
        );
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn builder_accepts_reusable_scope_and_normalization_policy() {
        let history = WindowHistoryFixture::new().build();
        let plan = history
            .compare("Provider QA")
            .target_source("provider-a")
            .against_source("provider-b")
            .scope(
                ComparisonScope::window("DeviceOffline")
                    .key("device-1")
                    .partition("region-a")
                    .segment("period", "first")
                    .tag("venue", "A"),
            )
            .normalization(
                ComparisonNormalizationPolicy::clip_open_windows_to(TemporalPoint::position(10))
                    .with_known_at(TemporalPoint::position(9))
                    .coalescing_adjacent_windows()
                    .rejecting_duplicate_windows(),
            )
            .overlap()
            .plan()
            .clone();

        assert_eq!(plan.scope_window.as_deref(), Some("DeviceOffline"));
        assert_eq!(plan.scope_key.as_deref(), Some("device-1"));
        assert_eq!(plan.scope_partition.as_deref(), Some("region-a"));
        assert_eq!(plan.scope_segments.len(), 1);
        assert_eq!(plan.scope_tags.len(), 1);
        assert_eq!(plan.open_window_horizon, Some(TemporalPoint::position(10)));
        assert_eq!(plan.time_axis, TemporalAxis::ProcessingPosition);
        assert_eq!(plan.known_at, Some(TemporalPoint::position(9)));
        assert!(plan.coalesce_adjacent_windows);
        assert_eq!(
            plan.duplicate_window_policy,
            crate::ComparisonDuplicateWindowPolicy::Reject
        );
    }

    #[test]
    fn builder_applies_reusable_event_time_normalization_policy() {
        let history = WindowHistoryFixture::new().build();
        let plan = history
            .compare("Event-time QA")
            .target_source("provider-a")
            .against_source("provider-b")
            .scope_window("DeviceOffline")
            .normalization(
                ComparisonNormalizationPolicy::event_time().excluding_missing_event_time(),
            )
            .overlap()
            .plan()
            .clone();

        assert_eq!(plan.time_axis, TemporalAxis::Timestamp);
        assert_eq!(
            plan.null_timestamp_policy,
            ComparisonNullTimestampPolicy::Exclude
        );
    }

    #[test]
    fn selector_builder_creates_composable_selectors() {
        let selector = ComparisonSelector::for_window_name("DeviceOffline")
            .and(ComparisonSelector::for_source("provider-a"));
        let window = crate::WindowRecord::Closed(crate::ClosedWindow {
            id: crate::WindowRecordId::new("record-1").expect("record id"),
            window_name: "DeviceOffline".to_owned(),
            key: "device-1".to_owned(),
            range: crate::TemporalRange::positions(1, 5).expect("range"),
            known_at: None,
            source: Some("provider-a".to_owned()),
            partition: None,
            segments: Vec::new(),
            tags: Vec::new(),
            boundary_reason: None,
            boundary_changes: Vec::new(),
        });

        assert!(selector.matches(&window));
        assert!(ComparisonSelector::for_position_range(5, Some(1)).is_err());
    }

    #[test]
    fn builder_can_scope_by_key_and_partition() {
        let history = WindowHistoryFixture::new()
            .closed_window("DeviceOffline", "device-1", 1, 5, |w| {
                w.source("provider-a").partition("fleet-a")
            })
            .expect("target")
            .closed_window("DeviceOffline", "device-1", 3, 6, |w| {
                w.source("provider-b").partition("fleet-a")
            })
            .expect("against")
            .closed_window("DeviceOffline", "device-2", 1, 5, |w| {
                w.source("provider-a").partition("fleet-a")
            })
            .expect("other key")
            .build();

        let result = history
            .compare("Scoped QA")
            .target_source("provider-a")
            .against_source("provider-b")
            .scope_window("DeviceOffline")
            .scope_key("device-1")
            .scope_partition("fleet-a")
            .overlap()
            .run();

        assert_eq!(result.overlap_rows.len(), 1);
        assert_eq!(result.overlap_rows[0].key, "device-1");
        assert_eq!(result.overlap_rows[0].partition.as_deref(), Some("fleet-a"));
    }

    #[test]
    fn builder_can_set_normalization_duplicate_and_coalesce_policy() {
        let history = WindowHistoryFixture::new()
            .closed_window("DeviceOffline", "device-1", 1, 3, |w| {
                w.source("provider-a")
            })
            .expect("target first")
            .closed_window("DeviceOffline", "device-1", 3, 5, |w| {
                w.source("provider-a")
            })
            .expect("target second")
            .closed_window("DeviceOffline", "device-1", 1, 5, |w| {
                w.source("provider-b")
            })
            .expect("against")
            .build();

        let result = history
            .compare("Normalization QA")
            .target_source("provider-a")
            .against_source("provider-b")
            .scope_window("DeviceOffline")
            .coalesce_adjacent_windows()
            .reject_duplicate_windows()
            .overlap()
            .run();

        assert_eq!(result.overlap_rows.len(), 1);
        assert_eq!(result.overlap_rows[0].target_record_ids.len(), 2);
    }

    fn unique_temp_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("{name}-{}", std::process::id()))
    }
}
