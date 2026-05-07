use crate::{
    domain::{PersonId, Tick, TimeWindow},
    generation::routines::WorldData,
    index::{WorldIndex, location_index, temporal_index},
};

pub fn people_in_chunk_at(
    world: &WorldData,
    index: &WorldIndex,
    chunk_id: &str,
    tick: Tick,
) -> Vec<PersonId> {
    location_index::people_in_chunk_at(index, &world.windows, chunk_id, tick)
}

pub fn people_in_canonical_location_at(
    world: &WorldData,
    index: &WorldIndex,
    canonical: &str,
    tick: Tick,
) -> Vec<PersonId> {
    location_index::people_in_canonical_at(index, &world.windows, canonical, tick)
}

pub fn windows_for_person<'a>(
    world: &'a WorldData,
    index: &WorldIndex,
    person_id: PersonId,
) -> Vec<&'a TimeWindow> {
    index
        .person_windows
        .get(&person_id)
        .into_iter()
        .flat_map(|ids| ids.iter())
        .map(|id| &world.windows[*id as usize])
        .collect()
}

pub fn activity_for_person_at<'a>(
    world: &'a WorldData,
    index: &WorldIndex,
    person_id: PersonId,
    tick: Tick,
) -> Option<&'a TimeWindow> {
    index
        .person_windows
        .get(&person_id)?
        .iter()
        .map(|id| &world.windows[*id as usize])
        .find(|window| window.active_at(tick))
}

pub fn people_in_location_range(
    world: &WorldData,
    index: &WorldIndex,
    location_key: &str,
    start_tick: Tick,
    end_tick: Tick,
) -> Vec<PersonId> {
    let ids = index
        .canonical_windows
        .get(location_key)
        .or_else(|| index.chunk_windows.get(location_key));
    let Some(ids) = ids else {
        return Vec::new();
    };
    let mut people = temporal_index::overlapping_ids(ids, &world.windows, start_tick, end_tick)
        .into_iter()
        .map(|id| world.windows[id as usize].person_id)
        .collect::<Vec<_>>();
    people.sort_unstable();
    people.dedup();
    people
}
