//! Episode formation over normalized window evidence.

mod formation;
mod graph;
mod identity;
mod metrics;
mod model;
mod relation;

pub use formation::EpisodeFormationBuilder;
pub use model::{
    Episode, EpisodeError, EpisodeFormationPlan, EpisodeFormationPolicy, EpisodeFragment,
    EpisodeId, EpisodeNormalizationFailure, EpisodeSet, TemporalTolerance,
};
pub use relation::{
    EpisodeComparisonBuilder, EpisodeComparisonError, EpisodeComparisonPlan,
    EpisodeComparisonResult, EpisodeRelation, EpisodeRelationKind, EpisodeRelationMetrics,
    EpisodeRelationPolicy,
};

use crate::WindowHistory;

impl WindowHistory {
    /// Starts an episode-formation builder over this history.
    #[must_use]
    pub fn form_episodes(&self, name: impl Into<String>) -> EpisodeFormationBuilder<'_> {
        EpisodeFormationBuilder::new(self, name.into())
    }

    /// Starts an exhaustive comparison between two episode definitions.
    #[must_use]
    pub fn compare_episodes(&self, name: impl Into<String>) -> EpisodeComparisonBuilder<'_> {
        EpisodeComparisonBuilder::new(self, name.into())
    }
}
