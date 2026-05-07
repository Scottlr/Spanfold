use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::PersonId;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum ConnectionKind {
    Family,
    Friend,
    CoWorker,
    Neighbour,
    Supplier,
    Rival,
    Handler,
    SurveillanceSubject,
    KnownAssociate,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PersonConnection {
    pub from_person_id: PersonId,
    pub to_person_id: PersonId,
    pub connection_kind: ConnectionKind,
    pub strength: f32,
    pub metadata: BTreeMap<String, String>,
}
