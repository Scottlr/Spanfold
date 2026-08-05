use std::collections::HashSet;

use super::ChildActivityView;
use crate::{WindowSegment, WindowTag};

#[derive(Clone, Debug, Default)]
pub(super) struct ParentState {
    known_children: HashSet<RollupChildId>,
    active_children: HashSet<RollupChildId>,
}

impl ParentState {
    pub(super) fn set_child_activity(
        &mut self,
        child_id: RollupChildId,
        is_active: bool,
    ) -> ChildActivityView {
        self.known_children.insert(child_id.clone());
        if is_active {
            self.active_children.insert(child_id);
        } else {
            self.active_children.remove(&child_id);
        }
        self.view()
    }

    pub(super) fn remove_child(&mut self, child_id: &RollupChildId) -> Option<ChildActivityView> {
        if !self.known_children.remove(child_id) {
            return None;
        }
        self.active_children.remove(child_id);
        Some(self.view())
    }

    fn view(&self) -> ChildActivityView {
        ChildActivityView {
            active_count: self.active_children.len(),
            total_count: self.known_children.len(),
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) struct RollupChildId {
    pub(super) key: String,
    pub(super) membership_context: String,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) struct RollupMembershipKey {
    pub(super) lineage: String,
    pub(super) child_key: String,
    pub(super) source: Option<String>,
    pub(super) partition: Option<String>,
    pub(super) membership_context: String,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct RollupMembership {
    pub(super) parent_key: String,
    pub(super) segment_context: String,
    pub(super) segments: Vec<WindowSegment>,
    pub(super) tags: Vec<WindowTag>,
}
