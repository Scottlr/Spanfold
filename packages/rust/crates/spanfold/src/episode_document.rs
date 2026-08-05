//! Versioned language-neutral Episode analysis documents and deterministic results.

use std::cmp::Ordering;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    ComparisonFinality, ComparisonNormalizationPolicy, ComparisonScope, ComparisonSelector,
    Episode, EpisodeComparisonError, EpisodeComparisonResult, EpisodeRelationKind, EpisodeSet,
    TemporalAxis, TemporalPoint, TemporalTolerance, WindowHistory,
};

const PLAN_SCHEMA: &str = "spanfold.episode.analysis";
const RESULT_SCHEMA: &str = "spanfold.episode.analysis.result";
const SCHEMA_VERSION: u32 = 1;

/// One named source participating in a portable Episode analysis.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EpisodeAnalysisSource {
    name: String,
    source: String,
}

impl EpisodeAnalysisSource {
    /// Returns the side display name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the exact source identity.
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }
}

/// Versioned portable definition for comparing two Episode sources.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EpisodeAnalysisDocument {
    name: String,
    target: EpisodeAnalysisSource,
    against: EpisodeAnalysisSource,
    window_name: String,
    normalization_axis: TemporalAxis,
    stitch_tolerance: i64,
    relation_tolerance: i64,
    live_horizon: Option<i64>,
}

impl EpisodeAnalysisDocument {
    /// Parses and validates a versioned Episode analysis document.
    pub fn parse_json(json: &str) -> Result<Self, EpisodeAnalysisDocumentError> {
        let raw: RawEpisodeAnalysisDocument = serde_json::from_str(json)?;
        if raw.schema != PLAN_SCHEMA {
            return Err(EpisodeAnalysisDocumentError::UnsupportedSchema(raw.schema));
        }
        if raw.schema_version != SCHEMA_VERSION {
            return Err(EpisodeAnalysisDocumentError::UnsupportedVersion(
                raw.schema_version,
            ));
        }
        require_non_empty("$.name", &raw.name)?;
        require_non_empty("$.target.name", &raw.target.name)?;
        require_non_empty("$.target.source", &raw.target.source)?;
        require_non_empty("$.against.name", &raw.against.name)?;
        require_non_empty("$.against.source", &raw.against.source)?;
        require_non_empty("$.windowName", &raw.window_name)?;
        if raw.target.source == raw.against.source {
            return Err(EpisodeAnalysisDocumentError::SameSource);
        }
        if raw.normalization_axis != "processingPosition" {
            return Err(EpisodeAnalysisDocumentError::UnsupportedAxis(
                raw.normalization_axis,
            ));
        }
        if raw.stitch_tolerance < 0 {
            return Err(EpisodeAnalysisDocumentError::NegativeTolerance {
                field: "stitchTolerance",
                value: raw.stitch_tolerance,
            });
        }
        if raw.relation_tolerance < 0 {
            return Err(EpisodeAnalysisDocumentError::NegativeTolerance {
                field: "relationTolerance",
                value: raw.relation_tolerance,
            });
        }
        if raw.live_horizon.is_some_and(|value| value < 0) {
            return Err(EpisodeAnalysisDocumentError::NegativeLiveHorizon);
        }

        Ok(Self {
            name: raw.name,
            target: raw.target,
            against: raw.against,
            window_name: raw.window_name,
            normalization_axis: TemporalAxis::ProcessingPosition,
            stitch_tolerance: raw.stitch_tolerance,
            relation_tolerance: raw.relation_tolerance,
            live_horizon: raw.live_horizon,
        })
    }

    /// Returns the analytical comparison name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the target source definition.
    #[must_use]
    pub const fn target(&self) -> &EpisodeAnalysisSource {
        &self.target
    }

    /// Returns the against source definition.
    #[must_use]
    pub const fn against(&self) -> &EpisodeAnalysisSource {
        &self.against
    }

    /// Returns the one named window family.
    #[must_use]
    pub fn window_name(&self) -> &str {
        &self.window_name
    }

    /// Returns the normalization axis.
    #[must_use]
    pub const fn normalization_axis(&self) -> TemporalAxis {
        self.normalization_axis
    }

    /// Returns the maximum same-side gap magnitude.
    #[must_use]
    pub const fn stitch_tolerance(&self) -> i64 {
        self.stitch_tolerance
    }

    /// Returns the maximum cross-side gap magnitude.
    #[must_use]
    pub const fn relation_tolerance(&self) -> i64 {
        self.relation_tolerance
    }

    /// Returns the optional live evaluation-horizon magnitude.
    #[must_use]
    pub const fn live_horizon(&self) -> Option<i64> {
        self.live_horizon
    }

    /// Executes this document through the existing Episode comparison module.
    pub fn execute(
        &self,
        history: &WindowHistory,
    ) -> Result<EpisodeAnalysisResultDocument, EpisodeAnalysisDocumentError> {
        let builder = history
            .compare_episodes(self.name.clone())
            .target(
                self.target.name.clone(),
                ComparisonSelector::for_source(self.target.source.clone()),
            )
            .against(
                self.against.name.clone(),
                ComparisonSelector::for_source(self.against.source.clone()),
            )
            .scope(ComparisonScope::window(self.window_name.clone()).on_position())
            .normalization(ComparisonNormalizationPolicy::default_policy())
            .stitch_gaps_up_to(TemporalTolerance::processing_positions(
                self.stitch_tolerance,
            )?)
            .relate_within(TemporalTolerance::processing_positions(
                self.relation_tolerance,
            )?);
        let result = if let Some(horizon) = self.live_horizon {
            builder.run_live(TemporalPoint::position(horizon))?
        } else {
            builder.run()?
        };
        Ok(EpisodeAnalysisResultDocument {
            document: self.clone(),
            result,
        })
    }
}

/// A portable Episode definition coupled to its materialized analytical result.
#[derive(Clone, Debug, PartialEq)]
pub struct EpisodeAnalysisResultDocument {
    document: EpisodeAnalysisDocument,
    result: EpisodeComparisonResult,
}

impl EpisodeAnalysisResultDocument {
    /// Returns the portable analysis definition.
    #[must_use]
    pub const fn document(&self) -> &EpisodeAnalysisDocument {
        &self.document
    }

    /// Returns the materialized Episode comparison.
    #[must_use]
    pub const fn result(&self) -> &EpisodeComparisonResult {
        &self.result
    }

    /// Exports deterministic portable JSON without runtime-specific Episode IDs.
    pub fn export_json(&self) -> Result<String, EpisodeAnalysisDocumentError> {
        let json = serde_json::to_string_pretty(&PortableResult::from(self))?;
        Ok(canonicalize_json_strings(&json))
    }

    /// Exports deterministic portable Markdown without runtime-specific Episode IDs.
    #[must_use]
    pub fn export_markdown(&self) -> String {
        let target_episodes = portable_episode_order(self.result.target_episodes());
        let against_episodes = portable_episode_order(self.result.against_episodes());
        let relations = portable_relations(&self.result, &target_episodes, &against_episodes);
        let mut text = String::new();
        text.push_str("# Episode analysis: ");
        text.push_str(&self.document.name);
        text.push_str("\n\n");
        append_fact(&mut text, "Window", &self.document.window_name);
        append_fact(&mut text, "Normalization axis", "processingPosition");
        append_fact(
            &mut text,
            "Stitch tolerance",
            &self.document.stitch_tolerance.to_string(),
        );
        append_fact(
            &mut text,
            "Relation tolerance",
            &self.document.relation_tolerance.to_string(),
        );
        append_fact(
            &mut text,
            "Evaluation horizon",
            &optional(self.document.live_horizon),
        );
        text.push('\n');
        append_summary(&mut text, self.result.summary());
        append_episodes(&mut text, "Target", &self.document.target, &target_episodes);
        append_episodes(
            &mut text,
            "Against",
            &self.document.against,
            &against_episodes,
        );
        append_relations(&mut text, &relations);
        text
    }
}

/// Failure while decoding, executing, or exporting a portable Episode document.
#[derive(Debug, Error)]
pub enum EpisodeAnalysisDocumentError {
    /// The document is not valid JSON for the v1 shape.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    /// The schema name is unsupported.
    #[error("unsupported Episode analysis schema: {0}")]
    UnsupportedSchema(String),
    /// The schema version is unsupported.
    #[error("unsupported Episode analysis schemaVersion: {0}")]
    UnsupportedVersion(u32),
    /// A required string is empty.
    #[error("{0} cannot be empty")]
    EmptyField(&'static str),
    /// Target and against identify the same source.
    #[error("$.target.source and $.against.source must be different")]
    SameSource,
    /// Version 1 does not carry the clock contract required for timestamp analysis.
    #[error(
        "Episode analysis schemaVersion 1 supports only the 'processingPosition' normalizationAxis, not '{0}'"
    )]
    UnsupportedAxis(String),
    /// A tolerance is negative.
    #[error("$.{field} must be non-negative, got {value}")]
    NegativeTolerance {
        /// Field containing the value.
        field: &'static str,
        /// Rejected magnitude.
        value: i64,
    },
    /// The optional live horizon is negative.
    #[error("$.liveHorizon must be a non-negative integer or null")]
    NegativeLiveHorizon,
    /// The compiled Episode plan or execution failed.
    #[error(transparent)]
    Analysis(#[from] EpisodeComparisonError),
    /// The Episode tolerance was invalid.
    #[error(transparent)]
    Episode(#[from] crate::EpisodeError),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawEpisodeAnalysisDocument {
    schema: String,
    schema_version: u32,
    name: String,
    target: EpisodeAnalysisSource,
    against: EpisodeAnalysisSource,
    window_name: String,
    normalization_axis: String,
    stitch_tolerance: i64,
    relation_tolerance: i64,
    live_horizon: Option<i64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PortableResult<'a> {
    schema: &'static str,
    schema_version: u32,
    analysis_name: &'a str,
    window_name: &'a str,
    normalization_axis: &'static str,
    stitch_tolerance: i64,
    relation_tolerance: i64,
    evaluation_horizon: Option<i64>,
    target: PortableSide<'a>,
    against: PortableSide<'a>,
    summary: PortableComparisonSummary,
    relations: Vec<PortableRelation>,
}

impl<'a> From<&'a EpisodeAnalysisResultDocument> for PortableResult<'a> {
    fn from(document: &'a EpisodeAnalysisResultDocument) -> Self {
        let result = &document.result;
        let target_episodes = portable_episode_order(result.target_episodes());
        let against_episodes = portable_episode_order(result.against_episodes());
        Self {
            schema: RESULT_SCHEMA,
            schema_version: SCHEMA_VERSION,
            analysis_name: &document.document.name,
            window_name: &document.document.window_name,
            normalization_axis: "processingPosition",
            stitch_tolerance: document.document.stitch_tolerance,
            relation_tolerance: document.document.relation_tolerance,
            evaluation_horizon: document.document.live_horizon,
            target: PortableSide::new(
                &document.document.target,
                result.target_episodes(),
                &target_episodes,
            ),
            against: PortableSide::new(
                &document.document.against,
                result.against_episodes(),
                &against_episodes,
            ),
            summary: PortableComparisonSummary::from(result.summary()),
            relations: portable_relations(result, &target_episodes, &against_episodes),
        }
    }
}

#[derive(Serialize)]
struct PortableSide<'a> {
    name: &'a str,
    source: &'a str,
    summary: PortableSetSummary,
    episodes: Vec<PortableEpisode<'a>>,
}

impl<'a> PortableSide<'a> {
    fn new(
        source: &'a EpisodeAnalysisSource,
        set: &'a EpisodeSet,
        episodes: &[&'a Episode],
    ) -> Self {
        Self {
            name: &source.name,
            source: &source.source,
            summary: PortableSetSummary::from(set.summary()),
            episodes: episodes
                .iter()
                .enumerate()
                .map(|(index, &episode)| PortableEpisode {
                    index,
                    key: episode.key(),
                    partition: episode.partition(),
                    start: episode.envelope().start().magnitude(),
                    end: episode.envelope().end().magnitude(),
                    fragment_count: episode.fragments().len(),
                    active_magnitude: episode.active_magnitude(),
                    elapsed_magnitude: episode.elapsed_magnitude(),
                    internal_gap_magnitude: episode.internal_gap_magnitude(),
                    finality: finality_name(episode.finality()),
                })
                .collect(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PortableSetSummary {
    episode_count: usize,
    final_episode_count: usize,
    provisional_episode_count: usize,
    total_active_magnitude: i64,
    total_elapsed_magnitude: i64,
    total_internal_gap_magnitude: i64,
}

impl From<&crate::EpisodeSetSummary> for PortableSetSummary {
    fn from(summary: &crate::EpisodeSetSummary) -> Self {
        Self {
            episode_count: summary.episode_count(),
            final_episode_count: summary.final_episode_count(),
            provisional_episode_count: summary.provisional_episode_count(),
            total_active_magnitude: summary.total_active_magnitude(),
            total_elapsed_magnitude: summary.total_elapsed_magnitude(),
            total_internal_gap_magnitude: summary.total_internal_gap_magnitude(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PortableEpisode<'a> {
    index: usize,
    key: &'a str,
    partition: Option<&'a str>,
    start: i64,
    end: i64,
    fragment_count: usize,
    active_magnitude: i64,
    elapsed_magnitude: i64,
    internal_gap_magnitude: i64,
    finality: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PortableComparisonSummary {
    target_episode_count: usize,
    against_episode_count: usize,
    matched_target_episode_count: usize,
    matched_against_episode_count: usize,
    unmatched_target_episode_count: usize,
    unmatched_against_episode_count: usize,
    one_to_one_relation_count: usize,
    split_relation_count: usize,
    merge_relation_count: usize,
    complex_relation_count: usize,
    total_overlap_magnitude: i64,
}

impl From<&crate::EpisodeComparisonSummary> for PortableComparisonSummary {
    fn from(summary: &crate::EpisodeComparisonSummary) -> Self {
        Self {
            target_episode_count: summary.target_episode_count(),
            against_episode_count: summary.against_episode_count(),
            matched_target_episode_count: summary.matched_target_episode_count(),
            matched_against_episode_count: summary.matched_against_episode_count(),
            unmatched_target_episode_count: summary.unmatched_target_episode_count(),
            unmatched_against_episode_count: summary.unmatched_against_episode_count(),
            one_to_one_relation_count: summary.one_to_one_relation_count(),
            split_relation_count: summary.split_relation_count(),
            merge_relation_count: summary.merge_relation_count(),
            complex_relation_count: summary.complex_relation_count(),
            total_overlap_magnitude: summary.total_overlap_magnitude(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PortableRelation {
    kind: &'static str,
    target_episode_indexes: Vec<usize>,
    against_episode_indexes: Vec<usize>,
    finality: &'static str,
    overlap_magnitude: i64,
    minimum_gap_magnitude: Option<i64>,
    onset_delta_magnitude: Option<i64>,
    recovery_delta_magnitude: Option<i64>,
}

fn require_non_empty(field: &'static str, value: &str) -> Result<(), EpisodeAnalysisDocumentError> {
    if value.trim().is_empty() {
        return Err(EpisodeAnalysisDocumentError::EmptyField(field));
    }
    Ok(())
}

fn portable_episode_order(set: &EpisodeSet) -> Vec<&Episode> {
    let mut episodes = set.episodes().iter().collect::<Vec<_>>();
    episodes.sort_by(|left, right| compare_portable_episodes(left, right));
    episodes
}

fn compare_portable_episodes(left: &Episode, right: &Episode) -> Ordering {
    left.key()
        .as_bytes()
        .cmp(right.key().as_bytes())
        .then_with(|| {
            left.partition()
                .map(str::as_bytes)
                .cmp(&right.partition().map(str::as_bytes))
        })
        .then_with(|| {
            left.envelope()
                .start()
                .magnitude()
                .cmp(&right.envelope().start().magnitude())
        })
        .then_with(|| {
            left.envelope()
                .end()
                .magnitude()
                .cmp(&right.envelope().end().magnitude())
        })
        .then_with(|| left.fragments().len().cmp(&right.fragments().len()))
        .then_with(|| left.active_magnitude().cmp(&right.active_magnitude()))
        .then_with(|| left.elapsed_magnitude().cmp(&right.elapsed_magnitude()))
        .then_with(|| {
            left.internal_gap_magnitude()
                .cmp(&right.internal_gap_magnitude())
        })
        .then_with(|| finality_rank(left.finality()).cmp(&finality_rank(right.finality())))
}

fn portable_relations(
    result: &EpisodeComparisonResult,
    target_order: &[&Episode],
    against_order: &[&Episode],
) -> Vec<PortableRelation> {
    let mut relations = result
        .relations()
        .iter()
        .map(|relation| PortableRelation {
            kind: relation_kind_name(relation.kind()),
            target_episode_indexes: episode_indexes(relation.target_episodes(), target_order),
            against_episode_indexes: episode_indexes(relation.against_episodes(), against_order),
            finality: finality_name(relation.finality()),
            overlap_magnitude: relation.metrics().overlap_magnitude(),
            minimum_gap_magnitude: relation.metrics().minimum_gap_magnitude(),
            onset_delta_magnitude: relation.metrics().onset_delta_magnitude(),
            recovery_delta_magnitude: relation.metrics().recovery_delta_magnitude(),
        })
        .collect::<Vec<_>>();
    relations.sort_by(|left, right| {
        left.target_episode_indexes
            .cmp(&right.target_episode_indexes)
            .then_with(|| {
                left.against_episode_indexes
                    .cmp(&right.against_episode_indexes)
            })
            .then_with(|| left.kind.cmp(right.kind))
            .then_with(|| left.finality.cmp(right.finality))
    });
    relations
}

fn episode_indexes(episodes: &[Episode], order: &[&Episode]) -> Vec<usize> {
    let mut indexes = episodes
        .iter()
        .map(|episode| {
            order
                .iter()
                .position(|candidate| candidate.id() == episode.id())
                .expect("relation episodes belong to their materialized side")
        })
        .collect::<Vec<_>>();
    indexes.sort_unstable();
    indexes
}

fn append_summary(text: &mut String, summary: &crate::EpisodeComparisonSummary) {
    text.push_str("## Summary\n\n");
    text.push_str("| Target episodes | Against episodes | Matched target | Matched against | Unmatched target | Unmatched against | One-to-one | Splits | Merges | Complex | Total overlap |\n");
    text.push_str(
        "| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |\n",
    );
    text.push_str(&format!(
        "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n\n",
        summary.target_episode_count(),
        summary.against_episode_count(),
        summary.matched_target_episode_count(),
        summary.matched_against_episode_count(),
        summary.unmatched_target_episode_count(),
        summary.unmatched_against_episode_count(),
        summary.one_to_one_relation_count(),
        summary.split_relation_count(),
        summary.merge_relation_count(),
        summary.complex_relation_count(),
        summary.total_overlap_magnitude(),
    ));
}

fn append_episodes(
    text: &mut String,
    label: &str,
    source: &EpisodeAnalysisSource,
    episodes: &[&Episode],
) {
    text.push_str("## ");
    text.push_str(label);
    text.push_str(" episodes: ");
    text.push_str(&source.name);
    text.push_str("\n\nSource: `");
    text.push_str(&escape_code(&source.source));
    text.push_str("`\n\n");
    text.push_str("| Index | Key | Partition | Start | End | Fragments | Active | Elapsed | Internal gap | Finality |\n");
    text.push_str("| ---: | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |\n");
    for (index, &episode) in episodes.iter().enumerate() {
        text.push_str(&format!(
            "| {index} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            escape_cell(&identity_json_literal(Some(episode.key()))),
            escape_cell(&identity_json_literal(episode.partition())),
            episode.envelope().start().magnitude(),
            episode.envelope().end().magnitude(),
            episode.fragments().len(),
            episode.active_magnitude(),
            episode.elapsed_magnitude(),
            episode.internal_gap_magnitude(),
            finality_name(episode.finality()),
        ));
    }
    text.push('\n');
}

fn append_relations(text: &mut String, relations: &[PortableRelation]) {
    text.push_str("## Relations\n\n");
    text.push_str("| Kind | Target indexes | Against indexes | Finality | Overlap | Minimum gap | Onset delta | Recovery delta |\n");
    text.push_str("| --- | --- | --- | --- | ---: | ---: | ---: | ---: |\n");
    for relation in relations {
        let targets = join_indexes(&relation.target_episode_indexes);
        let against = join_indexes(&relation.against_episode_indexes);
        text.push_str(&format!(
            "| {} | {targets} | {against} | {} | {} | {} | {} | {} |\n",
            relation.kind,
            relation.finality,
            relation.overlap_magnitude,
            optional(relation.minimum_gap_magnitude),
            optional(relation.onset_delta_magnitude),
            optional(relation.recovery_delta_magnitude),
        ));
    }
    text.push('\n');
}

fn join_indexes(indexes: &[usize]) -> String {
    indexes
        .iter()
        .map(|index| index.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

fn identity_json_literal(value: Option<&str>) -> String {
    serde_json::to_string(&value).expect("portable string identity")
}

fn canonicalize_json_strings(json: &str) -> String {
    let mut canonical = String::with_capacity(json.len());
    let mut characters = json.chars();
    let mut is_inside_string = false;
    while let Some(character) = characters.next() {
        if !is_inside_string {
            canonical.push(character);
            is_inside_string = character == '"';
            continue;
        }

        if character == '"' {
            canonical.push(character);
            is_inside_string = false;
            continue;
        }

        if character != '\\' {
            canonical.push(character);
            continue;
        }

        match characters
            .next()
            .expect("serde_json emitted an incomplete string escape")
        {
            '"' => canonical.push_str("\\\""),
            '\\' => canonical.push_str("\\\\"),
            '/' => canonical.push('/'),
            'b' => canonical.push_str("\\b"),
            'f' => canonical.push_str("\\f"),
            'n' => canonical.push_str("\\n"),
            'r' => canonical.push_str("\\r"),
            't' => canonical.push_str("\\t"),
            'u' => append_canonical_escaped_scalar(&mut canonical, &mut characters),
            _ => unreachable!("serde_json emitted an unsupported string escape"),
        }
    }
    canonical
}

fn append_canonical_escaped_scalar(
    canonical: &mut String,
    characters: &mut impl Iterator<Item = char>,
) {
    let high = read_json_hex_code_unit(characters);
    if !(0xd800..=0xdbff).contains(&high) {
        append_canonical_scalar(canonical, u32::from(high));
        return;
    }

    assert_eq!(characters.next(), Some('\\'));
    assert_eq!(characters.next(), Some('u'));
    let low = read_json_hex_code_unit(characters);
    assert!((0xdc00..=0xdfff).contains(&low));
    let scalar = 0x10000 + ((u32::from(high) - 0xd800) << 10) + (u32::from(low) - 0xdc00);
    append_canonical_scalar(canonical, scalar);
}

fn read_json_hex_code_unit(characters: &mut impl Iterator<Item = char>) -> u16 {
    (0..4).fold(0, |value, _| {
        let digit = characters
            .next()
            .and_then(|character| character.to_digit(16))
            .expect("serde_json emitted an invalid Unicode escape");
        value * 16 + u16::try_from(digit).expect("hex digit fits in u16")
    })
}

fn append_canonical_scalar(canonical: &mut String, scalar: u32) {
    match scalar {
        0x08 => canonical.push_str("\\b"),
        0x09 => canonical.push_str("\\t"),
        0x0a => canonical.push_str("\\n"),
        0x0c => canonical.push_str("\\f"),
        0x0d => canonical.push_str("\\r"),
        0x22 => canonical.push_str("\\\""),
        0x5c => canonical.push_str("\\\\"),
        0x00..=0x1f => canonical.push_str(&format!("\\u{scalar:04x}")),
        _ => canonical
            .push(char::from_u32(scalar).expect("serde_json emitted a valid Unicode scalar")),
    }
}

fn append_fact(text: &mut String, label: &str, value: &str) {
    text.push_str("- ");
    text.push_str(label);
    text.push_str(": `");
    text.push_str(&escape_code(value));
    text.push_str("`\n");
}

fn finality_name(finality: &ComparisonFinality) -> &'static str {
    match finality {
        ComparisonFinality::Final => "final",
        ComparisonFinality::Provisional => "provisional",
        ComparisonFinality::Revised => "revised",
        ComparisonFinality::Retracted => "retracted",
    }
}

const fn finality_rank(finality: &ComparisonFinality) -> u8 {
    match finality {
        ComparisonFinality::Final => 0,
        ComparisonFinality::Provisional => 1,
        ComparisonFinality::Revised => 2,
        ComparisonFinality::Retracted => 3,
    }
}

const fn relation_kind_name(kind: EpisodeRelationKind) -> &'static str {
    match kind {
        EpisodeRelationKind::OneToOne => "oneToOne",
        EpisodeRelationKind::Split => "split",
        EpisodeRelationKind::Merge => "merge",
        EpisodeRelationKind::Complex => "complex",
        EpisodeRelationKind::UnmatchedTarget => "unmatchedTarget",
        EpisodeRelationKind::UnmatchedAgainst => "unmatchedAgainst",
    }
}

fn optional(value: Option<i64>) -> String {
    value.map_or_else(|| "-".to_owned(), |value| value.to_string())
}

fn escape_cell(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace(['\r', '\n'], " ")
}

fn escape_code(value: &str) -> String {
    value.replace('`', "\\`")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WindowHistoryFixture;

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct SharedWindow {
        window_name: String,
        key: String,
        source: String,
        partition: Option<String>,
        start_position: i64,
        end_position: Option<i64>,
    }

    fn shared_history(json_lines: &str) -> crate::WindowHistory {
        let mut history = WindowHistoryFixture::new();
        for line in json_lines.lines() {
            let row: SharedWindow = serde_json::from_str(line).expect("shared window");
            let source = row.source.clone();
            let partition = row.partition.clone();
            history = if let Some(end) = row.end_position {
                history
                    .closed_window(
                        row.window_name,
                        row.key,
                        row.start_position,
                        end,
                        |window| {
                            let window = window.source(source);
                            if let Some(value) = partition {
                                window.partition(value)
                            } else {
                                window
                            }
                        },
                    )
                    .expect("closed window")
            } else {
                history
                    .open_window(row.window_name, row.key, row.start_position, |window| {
                        let window = window.source(source);
                        if let Some(value) = partition {
                            window.partition(value)
                        } else {
                            window
                        }
                    })
                    .expect("open window")
            };
        }
        history.build()
    }

    #[test]
    fn shared_provider_detector_fixture_matches_portable_result_contract() {
        let document = EpisodeAnalysisDocument::parse_json(include_str!(
            "../../../../../features/episodes/fixtures/portable-provider-detector-plan.json"
        ))
        .expect("shared plan");
        let history = shared_history(include_str!(
            "../../../../../features/episodes/fixtures/portable-provider-detector-windows.jsonl"
        ));

        let result = document.execute(&history).expect("execute");
        let expected = include_str!(
            "../../../../../features/episodes/fixtures/portable-provider-detector-result.json"
        );

        assert_eq!(result.export_json().expect("json").trim(), expected.trim());
        let markdown = result.export_markdown();
        assert!(markdown.contains("## Target episodes: provider"));
        assert!(markdown.contains("| oneToOne | 1 | 1 | provisional |"));
        assert!(!markdown.contains("episodeId"));
    }

    #[test]
    fn document_rejects_negative_live_horizon() {
        let error = EpisodeAnalysisDocument::parse_json(
            r#"{
                "schema":"spanfold.episode.analysis",
                "schemaVersion":1,
                "name":"comparison",
                "target":{"name":"provider","source":"provider-a"},
                "against":{"name":"detector","source":"detector-b"},
                "windowName":"Offline",
                "normalizationAxis":"processingPosition",
                "stitchTolerance":0,
                "relationTolerance":0,
                "liveHorizon":-1
            }"#,
        )
        .expect_err("negative horizon");

        assert!(matches!(
            &error,
            EpisodeAnalysisDocumentError::NegativeLiveHorizon
        ));
        assert_eq!(
            error.to_string(),
            "$.liveHorizon must be a non-negative integer or null"
        );
    }

    #[test]
    fn document_rejects_unknown_top_level_fields() {
        let error = EpisodeAnalysisDocument::parse_json(
            r#"{
                "schema":"spanfold.episode.analysis",
                "schemaVersion":1,
                "name":"comparison",
                "target":{"name":"provider","source":"provider-a"},
                "against":{"name":"detector","source":"detector-b"},
                "windowName":"Offline",
                "normalizationAxis":"processingPosition",
                "stitchTolerance":0,
                "relationTolerance":0,
                "liveHorizn":5
            }"#,
        )
        .expect_err("unknown top-level field");

        assert!(matches!(error, EpisodeAnalysisDocumentError::Json(_)));
    }

    #[test]
    fn document_rejects_unknown_source_fields() {
        let error = EpisodeAnalysisDocument::parse_json(
            r#"{
                "schema":"spanfold.episode.analysis",
                "schemaVersion":1,
                "name":"comparison",
                "target":{
                    "name":"provider",
                    "source":"provider-a",
                    "liveHorizon":5
                },
                "against":{"name":"detector","source":"detector-b"},
                "windowName":"Offline",
                "normalizationAxis":"processingPosition",
                "stitchTolerance":0,
                "relationTolerance":0
            }"#,
        )
        .expect_err("unknown source field");

        assert!(matches!(error, EpisodeAnalysisDocumentError::Json(_)));
    }

    #[test]
    fn unicode_identity_uses_portable_utf8_order_and_json_markdown_literals() {
        let document = EpisodeAnalysisDocument::parse_json(include_str!(
            "../../../../../features/episodes/fixtures/portable-provider-detector-plan.json"
        ))
        .expect("shared plan");
        let history = shared_history(include_str!(
            "../../../../../features/episodes/fixtures/portable-unicode-order-windows.jsonl"
        ));

        let result = document.execute(&history).expect("execute");
        let exported = result.export_json().expect("json");
        let expected = include_str!(
            "../../../../../features/episodes/fixtures/portable-unicode-order-result.json"
        );
        assert_eq!(exported, expected.trim_end_matches(['\r', '\n']));
        let json: serde_json::Value = serde_json::from_str(&exported).expect("result json");
        assert_eq!(json["target"]["episodes"][0]["key"], "same");
        assert_eq!(
            json["target"]["episodes"][0]["partition"],
            serde_json::Value::Null
        );
        assert_eq!(json["target"]["episodes"][1]["key"], "same");
        assert_eq!(json["target"]["episodes"][1]["partition"], "null");
        assert_eq!(json["target"]["episodes"][2]["key"], "");
        assert_eq!(json["target"]["episodes"][3]["key"], "😀");
        assert_eq!(json["relations"][0]["targetEpisodeIndexes"][0], 0);
        assert_eq!(json["relations"][3]["targetEpisodeIndexes"][0], 3);

        let markdown = result.export_markdown();
        assert!(markdown.contains("| 0 | \"same\" | null |"));
        assert!(markdown.contains("| 1 | \"same\" | \"null\" |"));
        assert!(markdown.contains("| 2 | \"\" | null |"));
        assert!(markdown.contains("| 3 | \"😀\" | \"null\" |"));
    }

    #[test]
    fn document_rejects_timestamp_axis_without_a_clock_contract() {
        let error = EpisodeAnalysisDocument::parse_json(
            r#"{
                "schema":"spanfold.episode.analysis",
                "schemaVersion":1,
                "name":"comparison",
                "target":{"name":"provider","source":"provider-a"},
                "against":{"name":"detector","source":"detector-b"},
                "windowName":"Offline",
                "normalizationAxis":"timestamp",
                "stitchTolerance":0,
                "relationTolerance":0
            }"#,
        )
        .expect_err("timestamp must require a later schema");

        assert!(matches!(
            error,
            EpisodeAnalysisDocumentError::UnsupportedAxis(_)
        ));
    }

    #[test]
    fn result_omits_runtime_episode_ids() {
        let document = EpisodeAnalysisDocument::parse_json(
            r#"{
                "schema":"spanfold.episode.analysis",
                "schemaVersion":1,
                "name":"comparison",
                "target":{"name":"provider","source":"provider-a"},
                "against":{"name":"detector","source":"detector-b"},
                "windowName":"Offline",
                "normalizationAxis":"processingPosition",
                "stitchTolerance":0,
                "relationTolerance":0
            }"#,
        )
        .expect("document");
        let history = WindowHistoryFixture::new()
            .closed_window("Offline", "device-1", 1, 5, |window| {
                window.source("provider-a")
            })
            .expect("target")
            .closed_window("Offline", "device-1", 1, 5, |window| {
                window.source("detector-b")
            })
            .expect("against")
            .build();

        let json = document
            .execute(&history)
            .expect("execute")
            .export_json()
            .expect("json");

        assert!(!json.contains("episodeId"));
        assert!(json.contains("targetEpisodeIndexes"));
    }
}
