use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{Location, PersonId};

pub type Tick = u32;
pub type WindowId = u32;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum ActivityKind {
    Sleeping,
    Commuting,
    Working,
    Eating,
    Shopping,
    Socialising,
    Loitering,
    Exercising,
    Travelling,
    EmergencyResponse,
    CriminalActivity,
    SurveillanceTarget,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TimeWindow {
    pub window_id: WindowId,
    pub person_id: PersonId,
    pub start_tick: Tick,
    pub end_tick: Tick,
    pub activity_kind: ActivityKind,
    pub chunk_id: String,
    pub location_canonical: String,
    pub precise_location_id: String,
    pub building_id: Option<String>,
    pub room_id: Option<String>,
    pub tags: Vec<String>,
    pub metadata: BTreeMap<String, String>,
}

impl TimeWindow {
    pub fn new(
        window_id: WindowId,
        person_id: PersonId,
        start_tick: Tick,
        end_tick: Tick,
        activity_kind: ActivityKind,
        location: Location,
    ) -> Result<Self, String> {
        if start_tick >= end_tick {
            return Err(format!(
                "invalid zero/negative window {window_id}: [{start_tick}, {end_tick})"
            ));
        }
        Ok(Self {
            window_id,
            person_id,
            start_tick,
            end_tick,
            activity_kind,
            chunk_id: location.chunk_id,
            location_canonical: location.location_canonical,
            precise_location_id: location.precise_location_id,
            building_id: location.building_id,
            room_id: location.room_id,
            tags: Vec::new(),
            metadata: BTreeMap::new(),
        })
    }

    pub fn active_at(&self, tick: Tick) -> bool {
        self.start_tick <= tick && tick < self.end_tick
    }

    pub fn overlaps_range(&self, start_tick: Tick, end_tick: Tick) -> bool {
        self.start_tick < end_tick && start_tick < self.end_tick
    }

    pub fn overlap_duration(&self, other: &Self) -> Tick {
        let start = self.start_tick.max(other.start_tick);
        let end = self.end_tick.min(other.end_tick);
        end.saturating_sub(start)
    }
}
