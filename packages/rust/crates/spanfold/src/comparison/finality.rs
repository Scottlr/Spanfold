//! Deterministic row identity and finality materialization.

use super::*;

pub(super) fn build_row_finalities(
    rows: &ComparisonRows,
    provisional_record_ids: &BTreeSet<String>,
) -> Vec<ComparisonRowFinality> {
    let mut finalities = Vec::new();
    append_overlap_finalities(&mut finalities, &rows.overlap, provisional_record_ids);
    append_residual_finalities(&mut finalities, &rows.residual, provisional_record_ids);
    append_missing_finalities(&mut finalities, &rows.missing, provisional_record_ids);
    append_coverage_finalities(&mut finalities, &rows.coverage, provisional_record_ids);
    append_gap_finalities(&mut finalities, &rows.gap);
    append_symmetric_difference_finalities(
        &mut finalities,
        &rows.symmetric_difference,
        provisional_record_ids,
    );
    append_containment_finalities(&mut finalities, &rows.containment, provisional_record_ids);
    append_lead_lag_finalities(&mut finalities, &rows.lead_lag, provisional_record_ids);
    append_as_of_finalities(&mut finalities, &rows.as_of, provisional_record_ids);
    finalities
}

pub(super) fn stable_row_id<T: Serialize>(row_type: &str, row: &T) -> String {
    let payload = serde_json::to_vec(row).unwrap_or_default();
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in row_type.bytes().chain(payload) {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{row_type}:{hash:016x}")
}

pub(crate) fn stable_row_id_for_export<T: Serialize>(row_type: &str, row: &T) -> String {
    stable_row_id(row_type, row)
}

pub(super) fn append_gap_finalities(finalities: &mut Vec<ComparisonRowFinality>, rows: &[GapRow]) {
    for row in rows {
        finalities.push(ComparisonRowFinality {
            row_type: "gap".to_owned(),
            row_id: stable_row_id("gap", row),
            finality: ComparisonFinality::Final,
            reason: "derived from closed windows".to_owned(),
            version: 1,
            supersedes_row_id: None,
        });
    }
}

pub(super) fn append_overlap_finalities(
    finalities: &mut Vec<ComparisonRowFinality>,
    rows: &[OverlapRow],
    provisional_record_ids: &BTreeSet<String>,
) {
    for row in rows {
        push_finality(
            finalities,
            "overlap",
            stable_row_id("overlap", row),
            row.target_record_ids
                .iter()
                .chain(row.against_record_ids.iter())
                .any(|id| provisional_record_ids.contains(id)),
        );
    }
}

pub(super) fn append_residual_finalities(
    finalities: &mut Vec<ComparisonRowFinality>,
    rows: &[ResidualRow],
    provisional_record_ids: &BTreeSet<String>,
) {
    for row in rows {
        push_finality(
            finalities,
            "residual",
            stable_row_id("residual", row),
            row.target_record_ids
                .iter()
                .any(|id| provisional_record_ids.contains(id)),
        );
    }
}

pub(super) fn append_missing_finalities(
    finalities: &mut Vec<ComparisonRowFinality>,
    rows: &[MissingRow],
    provisional_record_ids: &BTreeSet<String>,
) {
    for row in rows {
        push_finality(
            finalities,
            "missing",
            stable_row_id("missing", row),
            row.against_record_ids
                .iter()
                .any(|id| provisional_record_ids.contains(id)),
        );
    }
}

pub(super) fn append_coverage_finalities(
    finalities: &mut Vec<ComparisonRowFinality>,
    rows: &[CoverageRow],
    provisional_record_ids: &BTreeSet<String>,
) {
    for row in rows {
        push_finality(
            finalities,
            "coverage",
            stable_row_id("coverage", row),
            row.target_record_ids
                .iter()
                .chain(row.against_record_ids.iter())
                .any(|id| provisional_record_ids.contains(id)),
        );
    }
}

pub(super) fn append_symmetric_difference_finalities(
    finalities: &mut Vec<ComparisonRowFinality>,
    rows: &[SymmetricDifferenceRow],
    provisional_record_ids: &BTreeSet<String>,
) {
    for row in rows {
        push_finality(
            finalities,
            "symmetricDifference",
            stable_row_id("symmetricDifference", row),
            row.target_record_ids
                .iter()
                .chain(row.against_record_ids.iter())
                .any(|id| provisional_record_ids.contains(id)),
        );
    }
}

pub(super) fn append_containment_finalities(
    finalities: &mut Vec<ComparisonRowFinality>,
    rows: &[ContainmentRow],
    provisional_record_ids: &BTreeSet<String>,
) {
    for row in rows {
        push_finality(
            finalities,
            "containment",
            stable_row_id("containment", row),
            row.target_record_ids
                .iter()
                .chain(row.container_record_ids.iter())
                .any(|id| provisional_record_ids.contains(id)),
        );
    }
}

pub(super) fn append_lead_lag_finalities(
    finalities: &mut Vec<ComparisonRowFinality>,
    rows: &[LeadLagRow],
    provisional_record_ids: &BTreeSet<String>,
) {
    for row in rows {
        push_finality(
            finalities,
            "leadLag",
            stable_row_id("leadLag", row),
            provisional_record_ids.contains(&row.target_record_id)
                || row
                    .comparison_record_id
                    .as_ref()
                    .is_some_and(|id| provisional_record_ids.contains(id)),
        );
    }
}

pub(super) fn append_as_of_finalities(
    finalities: &mut Vec<ComparisonRowFinality>,
    rows: &[AsOfRow],
    provisional_record_ids: &BTreeSet<String>,
) {
    for row in rows {
        push_finality(
            finalities,
            "asOf",
            stable_row_id("asOf", row),
            provisional_record_ids.contains(&row.target_record_id)
                || row
                    .matched_record_id
                    .as_ref()
                    .is_some_and(|id| provisional_record_ids.contains(id)),
        );
    }
}

pub(super) fn push_finality(
    finalities: &mut Vec<ComparisonRowFinality>,
    row_type: &str,
    row_id: String,
    provisional: bool,
) {
    finalities.push(ComparisonRowFinality {
        row_type: row_type.to_owned(),
        row_id,
        finality: if provisional {
            ComparisonFinality::Provisional
        } else {
            ComparisonFinality::Final
        },
        reason: if provisional {
            "depends on an open window clipped to the evaluation horizon".to_owned()
        } else {
            "derived from closed windows".to_owned()
        },
        version: 1,
        supersedes_row_id: None,
    });
}
