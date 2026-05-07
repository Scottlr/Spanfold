use serde::Serialize;

use crate::{
    domain::{PersonConnection, PersonId, Tick},
    generation::routines::WorldData,
    index::{WorldIndex, graph_index},
    queries::point_in_time::activity_for_person_at,
};

#[derive(Clone, Debug, Serialize)]
pub struct ConnectedPersonLocation {
    pub person_id: PersonId,
    pub connection_kind: crate::domain::ConnectionKind,
    pub strength: f32,
    pub activity_kind: Option<crate::domain::ActivityKind>,
    pub chunk_id: Option<String>,
    pub location_canonical: Option<String>,
    pub precise_location_id: Option<String>,
}

pub fn connected_people<'a>(
    world: &'a WorldData,
    index: &WorldIndex,
    person_id: PersonId,
) -> Vec<&'a PersonConnection> {
    graph_index::connections_for(index, &world.connections, person_id)
}

pub fn connected_people_locations_at(
    world: &WorldData,
    index: &WorldIndex,
    person_id: PersonId,
    tick: Tick,
) -> Vec<ConnectedPersonLocation> {
    connected_people(world, index, person_id)
        .into_iter()
        .map(|connection| {
            let window = activity_for_person_at(world, index, connection.to_person_id, tick);
            ConnectedPersonLocation {
                person_id: connection.to_person_id,
                connection_kind: connection.connection_kind,
                strength: connection.strength,
                activity_kind: window.map(|window| window.activity_kind),
                chunk_id: window.map(|window| window.chunk_id.clone()),
                location_canonical: window.map(|window| window.location_canonical.clone()),
                precise_location_id: window.map(|window| window.precise_location_id.clone()),
            }
        })
        .collect()
}

pub fn connected_people_near_person_at(
    world: &WorldData,
    index: &WorldIndex,
    person_id: PersonId,
    tick: Tick,
) -> Vec<ConnectedPersonLocation> {
    let Some(target) = activity_for_person_at(world, index, person_id, tick) else {
        return Vec::new();
    };
    connected_people_locations_at(world, index, person_id, tick)
        .into_iter()
        .filter(|location| {
            location.location_canonical.as_deref() == Some(&target.location_canonical)
        })
        .collect()
}
