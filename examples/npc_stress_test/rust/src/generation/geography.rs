use crate::domain::Location;

pub const DISTRICT_COUNT: u32 = 64;
pub const CHUNK_COUNT: u32 = 32;

pub fn district(index: u32) -> String {
    format!("district_{}", index % DISTRICT_COUNT)
}

pub fn chunk(index: u32) -> String {
    format!("chunk{}", index % CHUNK_COUNT)
}

pub fn location_for(district_index: u32, place: &str, person: u32, tick: u32) -> Location {
    let district = district(district_index);
    let chunk = chunk(district_index * 3 + tick / 3_600 + person % 7);
    let precise = format!("{district}_{place}_{}", person % 97);
    let mut location = Location::district(district, chunk, precise);
    if matches!(place, "home" | "work" | "school" | "shop" | "safehouse") {
        location.building_id = Some(format!(
            "building_{}_{}",
            district_index % DISTRICT_COUNT,
            person % 500
        ));
        location.room_id = Some(format!("room_{}", (person + tick / 900) % 30));
    }
    location
}

pub fn commute_location(from: u32, to: u32, person: u32, tick: u32) -> Location {
    let midpoint = (from + to + tick / 1_800) % DISTRICT_COUNT;
    let mut location = location_for(midpoint, "road", person, tick);
    location.precise_location_id = format!(
        "route_{}_{}_{}",
        from % DISTRICT_COUNT,
        to % DISTRICT_COUNT,
        person % 19
    );
    location.building_id = None;
    location.room_id = None;
    location
}
