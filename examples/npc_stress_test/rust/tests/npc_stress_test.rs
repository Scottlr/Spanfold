use std::{fs, path::PathBuf};

use npc_stress_test::{
    ActivityKind, Archetype, ConnectionKind, GenerationConfig, Location, Person, PersonConnection,
    PersonId, TimeWindow, WorldIndex,
    export::jsonl::write_world_jsonl,
    generation::routines::{WorldData, generate_world},
    queries::{
        activity_for_person_at, co_located_with_person_at, connected_people,
        connected_people_locations_at, people_in_canonical_location_at, people_in_chunk_at,
        repeated_contact_report,
    },
};

#[test]
fn active_at_tick_uses_half_open_ranges() {
    let window = test_window(0, PersonId(1), 10, 20, "chunk2", "district_23");
    assert!(window.active_at(10));
    assert!(window.active_at(19));
    assert!(!window.active_at(20));
    assert!(!window.active_at(9));
}

#[test]
fn range_overlap_uses_half_open_logic() {
    let window = test_window(0, PersonId(1), 10, 20, "chunk2", "district_23");
    assert!(window.overlaps_range(0, 11));
    assert!(window.overlaps_range(19, 30));
    assert!(!window.overlaps_range(20, 30));
    assert!(!window.overlaps_range(0, 10));
}

#[test]
fn generated_windows_are_sorted_and_non_overlapping() {
    let world = generate_world(GenerationConfig {
        people: 1_000,
        seed: 42,
    })
    .expect("world");
    let index = WorldIndex::build(&world.windows, &world.connections);
    for ids in index.person_windows.values() {
        for pair in ids.windows(2) {
            let first = &world.windows[pair[0] as usize];
            let second = &world.windows[pair[1] as usize];
            assert!(first.start_tick <= second.start_tick);
            assert!(first.end_tick <= second.start_tick);
            assert!(first.start_tick < first.end_tick);
        }
    }
}

#[test]
fn deterministic_generation_from_seed() {
    let a = generate_world(GenerationConfig {
        people: 100,
        seed: 7,
    })
    .expect("world");
    let b = generate_world(GenerationConfig {
        people: 100,
        seed: 7,
    })
    .expect("world");
    assert_eq!(a.people, b.people);
    assert_eq!(a.windows, b.windows);
    assert_eq!(a.connections, b.connections);
}

#[test]
fn indexed_location_queries_return_expected_people() {
    let world = small_world();
    let index = WorldIndex::build(&world.windows, &world.connections);

    assert_eq!(
        people_in_chunk_at(&world, &index, "chunk2", 10),
        vec![PersonId(1), PersonId(2)]
    );
    assert_eq!(
        people_in_canonical_location_at(&world, &index, "district_23", 10),
        vec![PersonId(1), PersonId(2)]
    );
    assert!(people_in_chunk_at(&world, &index, "chunk2", 20).is_empty());
}

#[test]
fn person_activity_and_connections_work() {
    let world = small_world();
    let index = WorldIndex::build(&world.windows, &world.connections);

    let activity = activity_for_person_at(&world, &index, PersonId(1), 10).expect("activity");
    assert_eq!(activity.activity_kind, ActivityKind::Working);
    assert_eq!(activity.location_canonical, "district_23");

    let connections = connected_people(&world, &index, PersonId(1));
    assert_eq!(connections.len(), 1);
    assert_eq!(connections[0].to_person_id, PersonId(2));

    let locations = connected_people_locations_at(&world, &index, PersonId(1), 10);
    assert_eq!(locations[0].person_id, PersonId(2));
    assert_eq!(
        locations[0].location_canonical.as_deref(),
        Some("district_23")
    );
}

#[test]
fn co_location_and_repeated_contact_report_count_overlaps() {
    let world = small_world();
    let index = WorldIndex::build(&world.windows, &world.connections);

    assert_eq!(
        co_located_with_person_at(&world, &index, PersonId(1), 10),
        vec![PersonId(2)]
    );
    let report = repeated_contact_report(&world, &index, PersonId(1), 1, 5);
    assert_eq!(report.contacts.len(), 1);
    assert_eq!(report.contacts[0].person_id, PersonId(2));
    assert_eq!(report.contacts[0].overlap_count, 2);
    assert_eq!(report.contacts[0].total_overlap_duration, 30);
}

#[test]
fn export_output_is_valid_jsonl() {
    let world = small_world();
    let dir = unique_temp_dir();
    let _ = fs::remove_dir_all(&dir);
    write_world_jsonl(&world, &dir).expect("jsonl");

    for file_name in ["people.jsonl", "windows.jsonl", "connections.jsonl"] {
        let content = fs::read_to_string(dir.join(file_name)).expect("content");
        for line in content.lines() {
            serde_json::from_str::<serde_json::Value>(line).expect("valid json");
        }
    }
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn zero_length_window_is_rejected_and_no_result_queries_are_empty() {
    assert!(
        TimeWindow::new(
            99,
            PersonId(1),
            10,
            10,
            ActivityKind::Working,
            Location::district("district_x", "chunk_x", "nowhere"),
        )
        .is_err()
    );

    let world = small_world();
    let index = WorldIndex::build(&world.windows, &world.connections);
    assert!(people_in_chunk_at(&world, &index, "missing", 10).is_empty());
}

fn small_world() -> WorldData {
    let people = vec![
        Person {
            id: PersonId(1),
            archetype: Archetype::OfficeWorker,
            home_district: "district_1".to_owned(),
            work_district: "district_23".to_owned(),
            home_chunk: "chunk1".to_owned(),
        },
        Person {
            id: PersonId(2),
            archetype: Archetype::ShopWorker,
            home_district: "district_2".to_owned(),
            work_district: "district_23".to_owned(),
            home_chunk: "chunk2".to_owned(),
        },
    ];
    let windows = vec![
        test_window(0, PersonId(1), 0, 20, "chunk2", "district_23"),
        test_window(1, PersonId(1), 20, 40, "chunk3", "district_24"),
        test_window(2, PersonId(2), 5, 15, "chunk2", "district_23"),
        test_window(3, PersonId(2), 15, 40, "chunk4", "district_24"),
    ];
    let connections = vec![PersonConnection {
        from_person_id: PersonId(1),
        to_person_id: PersonId(2),
        connection_kind: ConnectionKind::Friend,
        strength: 0.8,
        metadata: Default::default(),
    }];
    WorldData {
        seed: 1,
        people,
        windows,
        connections,
    }
}

fn test_window(
    id: u32,
    person_id: PersonId,
    start: u32,
    end: u32,
    chunk: &str,
    district: &str,
) -> TimeWindow {
    TimeWindow::new(
        id,
        person_id,
        start,
        end,
        ActivityKind::Working,
        Location::district(district, chunk, format!("{district}_site")),
    )
    .expect("window")
}

fn unique_temp_dir() -> PathBuf {
    std::env::temp_dir().join(format!("npc-stress-test-{}", std::process::id()))
}
