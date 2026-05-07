use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::{
    domain::{PersonId, Tick, TimeWindow},
    generation::routines::WorldData,
    index::WorldIndex,
    queries::point_in_time::{
        activity_for_person_at, people_in_canonical_location_at, windows_for_person,
    },
};

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct OverlapResult {
    pub person_a: PersonId,
    pub person_b: PersonId,
    pub start_tick: Tick,
    pub end_tick: Tick,
    pub duration: Tick,
    pub location_canonical: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ContactSummary {
    pub person_id: PersonId,
    pub overlap_count: u32,
    pub total_overlap_duration: Tick,
    pub locations: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RepeatedContactReport {
    pub target_person: PersonId,
    pub contacts: Vec<ContactSummary>,
}

pub fn co_located_with_person_at(
    world: &WorldData,
    index: &WorldIndex,
    person_id: PersonId,
    tick: Tick,
) -> Vec<PersonId> {
    let Some(target) = activity_for_person_at(world, index, person_id, tick) else {
        return Vec::new();
    };
    let mut people =
        people_in_canonical_location_at(world, index, &target.location_canonical, tick);
    people.retain(|id| *id != person_id);
    people
}

pub fn overlap_windows_for_people(
    world: &WorldData,
    index: &WorldIndex,
    person_a: PersonId,
    person_b: PersonId,
) -> Vec<OverlapResult> {
    let a_windows = windows_for_person(world, index, person_a);
    let b_windows = windows_for_person(world, index, person_b);
    let mut overlaps = Vec::new();
    for left in &a_windows {
        for right in &b_windows {
            if same_place(left, right) && left.overlap_duration(right) > 0 {
                let start = left.start_tick.max(right.start_tick);
                let end = left.end_tick.min(right.end_tick);
                overlaps.push(OverlapResult {
                    person_a,
                    person_b,
                    start_tick: start,
                    end_tick: end,
                    duration: end - start,
                    location_canonical: left.location_canonical.clone(),
                });
            }
        }
    }
    overlaps
}

pub fn repeated_contact_report(
    world: &WorldData,
    index: &WorldIndex,
    person_id: PersonId,
    min_overlap_count: u32,
    min_overlap_duration: Tick,
) -> RepeatedContactReport {
    let mut summaries: BTreeMap<PersonId, ContactSummary> = BTreeMap::new();
    for target_window in windows_for_person(world, index, person_id) {
        let ids = index
            .canonical_windows
            .get(&target_window.location_canonical)
            .into_iter()
            .flat_map(|ids| ids.iter());
        for id in ids {
            let window = &world.windows[*id as usize];
            if window.person_id == person_id || !same_place(target_window, window) {
                continue;
            }
            let duration = target_window.overlap_duration(window);
            if duration == 0 {
                continue;
            }
            let entry = summaries
                .entry(window.person_id)
                .or_insert_with(|| ContactSummary {
                    person_id: window.person_id,
                    overlap_count: 0,
                    total_overlap_duration: 0,
                    locations: Vec::new(),
                });
            entry.overlap_count += 1;
            entry.total_overlap_duration += duration;
            if !entry.locations.contains(&window.location_canonical) {
                entry.locations.push(window.location_canonical.clone());
            }
        }
    }
    let mut contacts = summaries
        .into_values()
        .filter(|summary| {
            summary.overlap_count >= min_overlap_count
                && summary.total_overlap_duration >= min_overlap_duration
        })
        .collect::<Vec<_>>();
    contacts.sort_by_key(|summary| std::cmp::Reverse(summary.total_overlap_duration));
    RepeatedContactReport {
        target_person: person_id,
        contacts,
    }
}

pub fn connected_people_converged_on_location(
    world: &WorldData,
    index: &WorldIndex,
    person_id: PersonId,
    location: &str,
    tick: Tick,
) -> Vec<PersonId> {
    let connected = crate::queries::connected_people(world, index, person_id)
        .into_iter()
        .map(|connection| connection.to_person_id)
        .collect::<BTreeSet<_>>();
    people_in_canonical_location_at(world, index, location, tick)
        .into_iter()
        .filter(|id| connected.contains(id))
        .collect()
}

fn same_place(left: &TimeWindow, right: &TimeWindow) -> bool {
    left.location_canonical == right.location_canonical
}
