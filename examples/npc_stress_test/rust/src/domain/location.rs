use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Location {
    pub chunk_id: String,
    pub location_canonical: String,
    pub precise_location_id: String,
    pub building_id: Option<String>,
    pub room_id: Option<String>,
}

impl Location {
    pub fn district(
        district: impl Into<String>,
        chunk: impl Into<String>,
        precise: impl Into<String>,
    ) -> Self {
        Self {
            chunk_id: chunk.into(),
            location_canonical: district.into(),
            precise_location_id: precise.into(),
            building_id: None,
            room_id: None,
        }
    }
}
