use std::collections::{HashMap, HashSet};

use super::ChildActivityView;
use crate::{TemporalPoint, WindowHistory, WindowRecordId, WindowSegment, WindowTag};

pub(super) type RuntimeStateKey = (String, String, Option<String>, Option<String>, String);
pub(super) type RollupMembershipKey = (String, String, Option<String>, Option<String>, String);

#[derive(Clone, Debug, PartialEq)]
pub(super) struct OpenState {
    pub(super) id: WindowRecordId,
    pub(super) start: TemporalPoint,
    pub(super) source: Option<String>,
    pub(super) partition: Option<String>,
    pub(super) segments: Vec<WindowSegment>,
    pub(super) tags: Vec<WindowTag>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct ParentState {
    pub(super) active_children: HashSet<RollupChildId>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) struct RollupChildId {
    pub(super) key: String,
    pub(super) membership_context: String,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct RollupMembership {
    pub(super) parent_key: String,
    pub(super) segment_context: String,
    pub(super) segments: Vec<WindowSegment>,
    pub(super) tags: Vec<WindowTag>,
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
    pub(super) is_active: bool,
    pub(super) segments: Vec<WindowSegment>,
    pub(super) rollups: Vec<EventRollupObservation>,
}

pub(super) struct EventRollupObservation {
    pub(super) segments: Vec<WindowSegment>,
    pub(super) segment_context: String,
    pub(super) rollups: Vec<EventRollupObservation>,
}

impl ParentState {
    pub(super) fn view(&self) -> ChildActivityView {
        ChildActivityView {
            active_count: self.active_children.len(),
            total_count: self.active_children.len(),
        }
    }
}

pub(super) struct PipelineRuntime {
    pub(super) observation_buffer: Vec<EventWindowObservation>,
    pub(super) record_windows: bool,
    pub(super) history: WindowHistory,
    pub(super) active: HashMap<RuntimeStateKey, OpenState>,
    pub(super) parents: HashMap<RuntimeStateKey, ParentState>,
    pub(super) rollup_memberships: HashMap<RollupMembershipKey, RollupMembership>,
    pub(super) position: i64,
    pub(super) next_record_id: u64,
}
