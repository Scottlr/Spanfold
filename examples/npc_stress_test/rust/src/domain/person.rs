use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct PersonId(pub u32);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Archetype {
    OfficeWorker,
    ShopWorker,
    Student,
    Retired,
    NightShiftWorker,
    Courier,
    EmergencyWorker,
    CriminalActor,
    SurveillanceTarget,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Person {
    pub id: PersonId,
    pub archetype: Archetype,
    pub home_district: String,
    pub work_district: String,
    pub home_chunk: String,
}
