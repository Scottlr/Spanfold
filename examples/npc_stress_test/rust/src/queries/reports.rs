use std::collections::BTreeMap;

use crate::{
    domain::{ActivityKind, PersonId, Tick},
    generation::routines::WorldData,
    index::WorldIndex,
    queries::{
        connected_people_locations_at, people_in_canonical_location_at, repeated_contact_report,
        windows_for_person,
    },
};

pub fn person_timeline_report(
    world: &WorldData,
    index: &WorldIndex,
    person_id: PersonId,
) -> String {
    let person = world.people.iter().find(|person| person.id == person_id);
    let mut out = String::new();
    out.push_str(&format!("# Person {:?} Timeline\n\n", person_id));
    if let Some(person) = person {
        out.push_str(&format!(
            "- Archetype: {:?}\n- Home: {}\n- Work: {}\n\n",
            person.archetype, person.home_district, person.work_district
        ));
    }
    out.push_str("| Start | End | Activity | District | Chunk | Precise |\n| --- | --- | --- | --- | --- | --- |\n");
    for window in windows_for_person(world, index, person_id) {
        out.push_str(&format!(
            "| {} | {} | {:?} | {} | {} | {} |\n",
            window.start_tick,
            window.end_tick,
            window.activity_kind,
            window.location_canonical,
            window.chunk_id,
            window.precise_location_id
        ));
    }
    out
}

pub fn district_occupancy_report(world: &WorldData, index: &WorldIndex, district: &str) -> String {
    let sample_ticks = [
        8 * 3600,
        10 * 3600 + 35 * 60,
        12 * 3600,
        19 * 3600,
        23 * 3600,
    ];
    let mut activity_mix = BTreeMap::<ActivityKind, u32>::new();
    if let Some(ids) = index.canonical_windows.get(district) {
        for id in ids {
            *activity_mix
                .entry(world.windows[*id as usize].activity_kind)
                .or_default() += 1;
        }
    }
    let mut out = format!("# District {district} Occupancy\n\n## Activity Mix\n\n");
    for (activity, count) in activity_mix {
        out.push_str(&format!("- {:?}: {}\n", activity, count));
    }
    out.push_str("\n## Sample Ticks\n\n| Tick | People present | Sample |\n| --- | ---: | --- |\n");
    for tick in sample_ticks {
        let people = people_in_canonical_location_at(world, index, district, tick);
        let sample = people
            .iter()
            .take(8)
            .map(|id| id.0.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!("| {tick} | {} | {sample} |\n", people.len()));
    }
    out
}

pub fn connected_person_report(
    world: &WorldData,
    index: &WorldIndex,
    person_id: PersonId,
    ticks: &[Tick],
) -> String {
    let mut out = format!("# Connected People for {:?}\n\n", person_id);
    for tick in ticks {
        out.push_str(&format!("## Tick {tick}\n\n"));
        out.push_str("| Person | Kind | Strength | Activity | District | Chunk |\n| --- | --- | ---: | --- | --- | --- |\n");
        for location in connected_people_locations_at(world, index, person_id, *tick) {
            out.push_str(&format!(
                "| {} | {:?} | {:.2} | {:?} | {} | {} |\n",
                location.person_id.0,
                location.connection_kind,
                location.strength,
                location.activity_kind,
                location.location_canonical.unwrap_or_default(),
                location.chunk_id.unwrap_or_default()
            ));
        }
    }
    out
}

pub fn repeated_contact_markdown(
    world: &WorldData,
    index: &WorldIndex,
    person_id: PersonId,
    min_count: u32,
) -> String {
    let report = repeated_contact_report(world, index, person_id, min_count, 1);
    let mut out = format!("# Repeated Contact Report for {:?}\n\n", person_id);
    out.push_str("| Person | Overlaps | Duration | Locations |\n| --- | ---: | ---: | --- |\n");
    for contact in report.contacts.iter().take(50) {
        out.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            contact.person_id.0,
            contact.overlap_count,
            contact.total_overlap_duration,
            contact.locations.join(", ")
        ));
    }
    out
}

pub fn suspicious_pattern_report(
    world: &WorldData,
    index: &WorldIndex,
    target: PersonId,
) -> String {
    let repeated = repeated_contact_report(world, index, target, 3, 900);
    let night_people =
        crate::queries::people_in_location_range(world, index, "district_23", 22 * 3600, 86_400);
    let mut out = format!("# Suspicious Pattern Report for {:?}\n\n", target);
    out.push_str("## Repeated nearby people\n\n");
    for contact in repeated.contacts.iter().take(20) {
        out.push_str(&format!(
            "- person {}: {} overlaps, {} seconds\n",
            contact.person_id.0, contact.overlap_count, contact.total_overlap_duration
        ));
    }
    out.push_str("\n## Night activity in district_23\n\n");
    out.push_str(&format!("- Unique people: {}\n", night_people.len()));
    out
}
