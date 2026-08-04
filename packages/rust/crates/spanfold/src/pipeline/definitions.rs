use std::{collections::BTreeMap, sync::Arc};

use super::{ChildActivityView, RollUpSegmentProjection, WindowEmission};
use crate::{WindowSegment, WindowTag};

pub(super) type KeySelector<T> = Arc<dyn Fn(&T) -> String + Send + Sync + 'static>;
pub(super) type ActivePredicate<T> = Arc<dyn Fn(&T) -> bool + Send + Sync + 'static>;
pub(super) type EventTimeSelector<T> = Arc<dyn Fn(&T) -> i64 + Send + Sync + 'static>;
pub(super) type SegmentSelector<T> = Arc<dyn Fn(&T) -> Vec<WindowSegment> + Send + Sync + 'static>;
pub(super) type TagSelector<T> = Arc<dyn Fn(&T) -> Vec<WindowTag> + Send + Sync + 'static>;
pub(super) type RollupPredicate = Arc<dyn Fn(ChildActivityView) -> bool + Send + Sync + 'static>;
pub(super) type EmissionCallback = Arc<dyn Fn(&WindowEmission) + Send + Sync + 'static>;

#[derive(Clone, Default)]
pub(super) struct WindowCallbackSet {
    pub(super) opened: Vec<EmissionCallback>,
    pub(super) closed: Vec<EmissionCallback>,
}

pub(super) struct RollUpDefinition<T> {
    pub(super) name: String,
    pub(super) key: KeySelector<T>,
    pub(super) is_active: RollupPredicate,
    pub(super) rollups: Vec<RollUpDefinition<T>>,
    pub(super) callbacks: WindowCallbackSet,
    pub(super) segment_projection: RollUpSegmentProjection,
}

pub(super) struct WindowDefinition<T> {
    pub(super) name: String,
    pub(super) key: KeySelector<T>,
    pub(super) is_active: ActivePredicate<T>,
    pub(super) segments: Option<SegmentSelector<T>>,
    pub(super) tags: Option<TagSelector<T>>,
    pub(super) rollups: Vec<RollUpDefinition<T>>,
    pub(super) callbacks: WindowCallbackSet,
}

impl<T> Clone for RollUpDefinition<T> {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            key: Arc::clone(&self.key),
            is_active: Arc::clone(&self.is_active),
            rollups: self.rollups.clone(),
            callbacks: self.callbacks.clone(),
            segment_projection: self.segment_projection.clone(),
        }
    }
}

impl<T> Clone for WindowDefinition<T> {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            key: Arc::clone(&self.key),
            is_active: Arc::clone(&self.is_active),
            segments: self.segments.as_ref().map(Arc::clone),
            tags: self.tags.as_ref().map(Arc::clone),
            rollups: self.rollups.clone(),
            callbacks: self.callbacks.clone(),
        }
    }
}

pub(super) struct PipelineDefinitions<T> {
    pub(super) windows: Vec<WindowDefinition<T>>,
    pub(super) max_new_records: Option<u64>,
    pub(super) event_time: Option<EventTimeSelector<T>>,
    pub(super) emission_callbacks: Vec<EmissionCallback>,
    pub(super) window_callbacks: BTreeMap<String, WindowCallbackSet>,
}
