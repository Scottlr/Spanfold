//! Canonical typed ownership for materialized comparison results.

use super::*;

/// Canonical row storage with finality metadata partitioned by row family.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct ComparisonRowState {
    rows: ComparisonRows,
    overlap: Vec<ComparisonRowFinality>,
    residual: Vec<ComparisonRowFinality>,
    missing: Vec<ComparisonRowFinality>,
    coverage: Vec<ComparisonRowFinality>,
    gap: Vec<ComparisonRowFinality>,
    symmetric_difference: Vec<ComparisonRowFinality>,
    containment: Vec<ComparisonRowFinality>,
    lead_lag: Vec<ComparisonRowFinality>,
    as_of: Vec<ComparisonRowFinality>,
}

impl ComparisonRowState {
    pub(super) fn new(rows: ComparisonRows, finalities: Vec<ComparisonRowFinality>) -> Self {
        let expected_count = rows
            .family_layouts()
            .iter()
            .map(|(_, count)| count)
            .sum::<usize>();
        assert_eq!(
            finalities.len(),
            expected_count,
            "canonical comparison rows and finalities must have equal counts"
        );

        let mut finalities = finalities.into_iter();
        let mut state = Self {
            rows,
            overlap: Vec::new(),
            residual: Vec::new(),
            missing: Vec::new(),
            coverage: Vec::new(),
            gap: Vec::new(),
            symmetric_difference: Vec::new(),
            containment: Vec::new(),
            lead_lag: Vec::new(),
            as_of: Vec::new(),
        };
        macro_rules! partition_finalities {
            ($(($kind:ident, $rows:ident, $compat:ident, $view:ident, $debug:literal, $count:literal),)*) => {
                $(
                    state.$rows = take_finalities(
                        &mut finalities,
                        ComparisonRowKind::$kind,
                        state.rows.$rows.len(),
                    );
                )*
            };
        }
        for_each_comparison_row_family!(partition_finalities);
        debug_assert!(finalities.next().is_none());
        state
    }

    pub(super) fn empty() -> Self {
        Self::new(ComparisonRows::default(), Vec::new())
    }

    pub(super) fn rows(&self) -> &ComparisonRows {
        &self.rows
    }

    pub(super) fn compatibility_finalities(&self) -> Vec<ComparisonRowFinality> {
        let mut finalities = Vec::with_capacity(self.rows.total_count());
        macro_rules! append_finalities {
            ($(($kind:ident, $rows:ident, $compat:ident, $view:ident, $debug:literal, $count:literal),)*) => {
                $(finalities.extend(self.$rows.iter().cloned());)*
            };
        }
        for_each_comparison_row_family!(append_finalities);
        finalities
    }

    pub(super) fn finalities(&self) -> impl Iterator<Item = &ComparisonRowFinality> {
        [
            self.overlap.as_slice(),
            self.residual.as_slice(),
            self.missing.as_slice(),
            self.coverage.as_slice(),
            self.gap.as_slice(),
            self.symmetric_difference.as_slice(),
            self.containment.as_slice(),
            self.lead_lag.as_slice(),
            self.as_of.as_slice(),
        ]
        .into_iter()
        .flatten()
    }

    pub(super) fn family_finalities(&self, kind: ComparisonRowKind) -> &[ComparisonRowFinality] {
        match kind {
            ComparisonRowKind::Overlap => &self.overlap,
            ComparisonRowKind::Residual => &self.residual,
            ComparisonRowKind::Missing => &self.missing,
            ComparisonRowKind::Coverage => &self.coverage,
            ComparisonRowKind::Gap => &self.gap,
            ComparisonRowKind::SymmetricDifference => &self.symmetric_difference,
            ComparisonRowKind::Containment => &self.containment,
            ComparisonRowKind::LeadLag => &self.lead_lag,
            ComparisonRowKind::AsOf => &self.as_of,
        }
    }
}

fn take_finalities(
    finalities: &mut impl Iterator<Item = ComparisonRowFinality>,
    kind: ComparisonRowKind,
    count: usize,
) -> Vec<ComparisonRowFinality> {
    finalities
        .take(count)
        .inspect(|metadata| {
            assert_eq!(
                metadata.row_kind().ok(),
                Some(kind),
                "canonical comparison row finality has the wrong family"
            );
        })
        .collect()
}

/// Complete typed state retained by an in-memory comparison result.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct ComparisonResultState {
    prepared: Option<PreparedComparison>,
    aligned: Option<AlignedComparison>,
    rows: ComparisonRowState,
}

impl ComparisonResultState {
    pub(super) fn new(
        prepared: Option<PreparedComparison>,
        aligned: Option<AlignedComparison>,
        rows: ComparisonRowState,
    ) -> Self {
        Self {
            prepared,
            aligned,
            rows,
        }
    }

    pub(super) fn empty() -> Self {
        Self::new(None, None, ComparisonRowState::empty())
    }

    pub(super) fn prepared(&self) -> Option<&PreparedComparison> {
        self.prepared.as_ref()
    }

    pub(super) fn aligned(&self) -> Option<&AlignedComparison> {
        self.aligned.as_ref()
    }

    pub(super) fn rows(&self) -> &ComparisonRows {
        self.rows.rows()
    }

    pub(super) fn compatibility_finalities(&self) -> Vec<ComparisonRowFinality> {
        self.rows.compatibility_finalities()
    }

    pub(super) fn finalities(&self) -> impl Iterator<Item = &ComparisonRowFinality> {
        self.rows.finalities()
    }

    pub(super) fn family_finalities(&self, kind: ComparisonRowKind) -> &[ComparisonRowFinality] {
        self.rows.family_finalities(kind)
    }
}
