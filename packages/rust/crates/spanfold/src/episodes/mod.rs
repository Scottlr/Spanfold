//! Episode formation over normalized window evidence.

mod formation;
mod identity;
mod model;

pub use formation::EpisodeFormationBuilder;
pub use model::{
    Episode, EpisodeError, EpisodeFormationPlan, EpisodeFormationPolicy, EpisodeFragment,
    EpisodeId, EpisodeNormalizationFailure, EpisodeSet, TemporalTolerance,
};

use crate::WindowHistory;

impl WindowHistory {
    /// Starts an episode-formation builder over this history.
    #[must_use]
    pub fn form_episodes(&self, name: impl Into<String>) -> EpisodeFormationBuilder<'_> {
        EpisodeFormationBuilder::new(self, name.into())
    }
}
