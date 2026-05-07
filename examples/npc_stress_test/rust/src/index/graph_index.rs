use crate::{
    domain::{PersonConnection, PersonId},
    index::WorldIndex,
};

pub fn connections_for<'a>(
    index: &WorldIndex,
    connections: &'a [PersonConnection],
    person_id: PersonId,
) -> Vec<&'a PersonConnection> {
    index
        .connections
        .get(&person_id)
        .into_iter()
        .flat_map(|ids| ids.iter())
        .map(|id| &connections[*id])
        .collect()
}
