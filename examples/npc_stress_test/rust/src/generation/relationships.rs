use std::collections::BTreeMap;

use crate::domain::{ConnectionKind, Person, PersonConnection};

pub fn generate_connections(people: &[Person], seed: u64) -> Vec<PersonConnection> {
    let count = people.len() as u32;
    let mut connections = Vec::with_capacity(people.len() * 4);
    for person in people {
        let id = person.id.0;
        let offsets = [1, 7, 31, ((seed as u32) % 97).max(3)];
        for (slot, offset) in offsets.into_iter().enumerate() {
            let target = (id + offset) % count;
            if target == id {
                continue;
            }
            let kind = match slot {
                0 => ConnectionKind::Neighbour,
                1 => ConnectionKind::Friend,
                2 => ConnectionKind::CoWorker,
                _ if id % 997 == 0 => ConnectionKind::Handler,
                _ if id % 89 == 0 => ConnectionKind::KnownAssociate,
                _ => ConnectionKind::Family,
            };
            let mut metadata = BTreeMap::new();
            metadata.insert("deterministic".to_owned(), "true".to_owned());
            connections.push(PersonConnection {
                from_person_id: person.id,
                to_person_id: crate::domain::PersonId(target),
                connection_kind: kind,
                strength: 0.35 + ((id + target + slot as u32) % 65) as f32 / 100.0,
                metadata,
            });
        }
    }
    connections
}
