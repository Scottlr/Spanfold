use std::collections::HashMap;

use super::rollup::{ParentState, RollupMembership, RollupMembershipKey};
use crate::{TemporalPoint, WindowRecorder, WindowSegment, WindowTag};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) struct RuntimeStateKey {
    pub(super) window_name: String,
    pub(super) key: String,
    pub(super) source: Option<String>,
    pub(super) partition: Option<String>,
    pub(super) segment_context: String,
}

#[derive(Clone)]
pub(super) struct ChildContext<'a> {
    pub(super) lineage: &'a str,
    pub(super) key: &'a str,
    pub(super) membership_context: &'a str,
    pub(super) event_point: TemporalPoint,
    pub(super) is_active: bool,
    pub(super) segments: &'a [WindowSegment],
    pub(super) tags: &'a [WindowTag],
}

pub(super) struct WindowObservation {
    pub(super) state_key: RuntimeStateKey,
    pub(super) window_name: String,
    pub(super) key: String,
    pub(super) event_point: TemporalPoint,
    pub(super) source: Option<String>,
    pub(super) partition: Option<String>,
    pub(super) is_active: bool,
    pub(super) segments: Vec<WindowSegment>,
    pub(super) tags: Vec<WindowTag>,
}

pub(super) struct EventWindowObservation {
    pub(super) state_key: RuntimeStateKey,
    pub(super) key: String,
    pub(super) lifecycle: SourceWindowLifecycle,
    pub(super) segments: Vec<WindowSegment>,
    pub(super) tags: Vec<WindowTag>,
    pub(super) rollups: Vec<EventRollupObservation>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SourceWindowLifecycle {
    Inactive,
    Pending(usize),
    Active,
    Closing,
}

pub(super) struct EventRollupObservation {
    pub(super) key: String,
    pub(super) segments: Vec<WindowSegment>,
    pub(super) segment_context: String,
    pub(super) rollups: Vec<EventRollupObservation>,
}

pub(super) struct PipelineRuntime {
    pub(super) observation_buffer: Vec<EventWindowObservation>,
    pub(super) recorder: WindowRecorder,
    pub(super) pending_confirmations: HashMap<RuntimeStateKey, usize>,
    pub(super) parents: HashMap<RuntimeStateKey, ParentState>,
    pub(super) rollup_memberships: HashMap<RollupMembershipKey, RollupMembership>,
    pub(super) position: i64,
}
