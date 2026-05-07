use crate::{
    domain::{PersonId, Tick, TimeWindow},
    index::{WorldIndex, temporal_index},
};

pub fn people_in_chunk_at(
    index: &WorldIndex,
    windows: &[TimeWindow],
    chunk: &str,
    tick: Tick,
) -> Vec<PersonId> {
    people_from_ids(index.chunk_windows.get(chunk), windows, tick)
}

pub fn people_in_canonical_at(
    index: &WorldIndex,
    windows: &[TimeWindow],
    canonical: &str,
    tick: Tick,
) -> Vec<PersonId> {
    people_from_ids(index.canonical_windows.get(canonical), windows, tick)
}

fn people_from_ids(ids: Option<&Vec<u32>>, windows: &[TimeWindow], tick: Tick) -> Vec<PersonId> {
    let Some(ids) = ids else {
        return Vec::new();
    };
    let mut people = temporal_index::active_ids_at(ids, windows, tick)
        .into_iter()
        .map(|id| windows[id as usize].person_id)
        .collect::<Vec<_>>();
    people.sort_unstable();
    people.dedup();
    people
}
