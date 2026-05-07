pub mod graph_index;
pub mod location_index;
pub mod temporal_index;

use std::collections::BTreeMap;

use crate::domain::{ActivityKind, PersonConnection, PersonId, TimeWindow, WindowId};

#[derive(Clone, Debug)]
pub struct WorldIndex {
    pub person_windows: BTreeMap<PersonId, Vec<WindowId>>,
    pub chunk_windows: BTreeMap<String, Vec<WindowId>>,
    pub canonical_windows: BTreeMap<String, Vec<WindowId>>,
    pub activity_windows: BTreeMap<ActivityKind, Vec<WindowId>>,
    pub connections: BTreeMap<PersonId, Vec<usize>>,
}

impl WorldIndex {
    pub fn build(windows: &[TimeWindow], connections: &[PersonConnection]) -> Self {
        let mut index = Self {
            person_windows: BTreeMap::new(),
            chunk_windows: BTreeMap::new(),
            canonical_windows: BTreeMap::new(),
            activity_windows: BTreeMap::new(),
            connections: BTreeMap::new(),
        };

        for window in windows {
            index
                .person_windows
                .entry(window.person_id)
                .or_default()
                .push(window.window_id);
            index
                .chunk_windows
                .entry(window.chunk_id.clone())
                .or_default()
                .push(window.window_id);
            index
                .canonical_windows
                .entry(window.location_canonical.clone())
                .or_default()
                .push(window.window_id);
            index
                .activity_windows
                .entry(window.activity_kind)
                .or_default()
                .push(window.window_id);
        }
        for ids in index.person_windows.values_mut() {
            ids.sort_by_key(|id| windows[*id as usize].start_tick);
        }
        for ids in index.chunk_windows.values_mut() {
            ids.sort_by_key(|id| windows[*id as usize].start_tick);
        }
        for ids in index.canonical_windows.values_mut() {
            ids.sort_by_key(|id| windows[*id as usize].start_tick);
        }
        for ids in index.activity_windows.values_mut() {
            ids.sort_by_key(|id| windows[*id as usize].start_tick);
        }
        for (connection_index, connection) in connections.iter().enumerate() {
            index
                .connections
                .entry(connection.from_person_id)
                .or_default()
                .push(connection_index);
        }
        index
    }
}
