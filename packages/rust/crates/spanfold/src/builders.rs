use crate::{
    AgainstSelection, CohortActivity, Comparator, ComparisonDiagnostic, ComparisonPlan,
    DiagnosticSeverity, OpenWindowPolicy, PreparedComparison, PrimitiveValue, TemporalPoint,
    WindowFilter, WindowHistory, align, compare, compare_live, prepare, prepare_live,
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
                scope_window: None,
                scope_segments: Vec::new(),
                scope_tags: Vec::new(),
                comparators: Vec::new(),
                known_at: None,
                open_window_policy: OpenWindowPolicy::RequireClosed,
                open_window_horizon: None,
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
        self
    }

    /// Sets one comparison source lane.
    #[must_use]
    pub fn against_source(mut self, source: impl Into<String>) -> Self {
        self.plan.against = AgainstSelection::Sources(vec![source.into()]);
        self
    }

    /// Sets multiple comparison source lanes.
    #[must_use]
    pub fn against_sources(mut self, sources: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.plan.against =
            AgainstSelection::Sources(sources.into_iter().map(Into::into).collect::<Vec<_>>());
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
        self
    }

    /// Limits the comparison to one window family.
    #[must_use]
    pub fn scope_window(mut self, window_name: impl Into<String>) -> Self {
        self.plan.scope_window = Some(window_name.into());
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
        self.plan.open_window_policy = OpenWindowPolicy::ClipToHorizon;
        self.plan.open_window_horizon = Some(TemporalPoint::position(position));
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
        let mut diagnostics = Vec::new();
        if self.plan.strict && self.plan.scope_window.is_none() {
            diagnostics.push(ComparisonDiagnostic {
                code: "BroadSelector".to_owned(),
                severity: DiagnosticSeverity::Error,
            });
        }
        if self.plan.target_source.is_empty() {
            diagnostics.push(ComparisonDiagnostic {
                code: "MissingTarget".to_owned(),
                severity: DiagnosticSeverity::Error,
            });
        }
        if matches!(&self.plan.against, AgainstSelection::Sources(sources) if sources.is_empty()) {
            diagnostics.push(ComparisonDiagnostic {
                code: "MissingAgainst".to_owned(),
                severity: DiagnosticSeverity::Error,
            });
        }
        diagnostics
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
        debug_html.export_if_enabled(&result)?;
        llm_context.export_if_enabled(&result)?;
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
        debug_html.export_if_enabled(&result)?;
        llm_context.export_if_enabled(&result)?;
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

    use crate::{ComparisonDebugHtmlOptions, ComparisonLlmContextOptions, WindowHistoryFixture};

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

    fn unique_temp_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("{name}-{}", std::process::id()))
    }
}
