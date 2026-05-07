use std::{env, path::PathBuf, time::Instant};

use npc_stress_test::{
    ConnectedPersonLocation, GenerationConfig, PersonId, WorldIndex, activity_for_person_at,
    co_located_with_person_at, connected_people_locations_at, district_occupancy_report,
    export::{html::write_html_dashboard, jsonl::write_world_jsonl, markdown::write_markdown},
    generate_world, people_in_canonical_location_at, people_in_chunk_at, person_timeline_report,
    repeated_contact_markdown, repeated_contact_report, suspicious_pattern_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() {
        print_help();
        return Ok(());
    }
    let mut people = value_after(&args, "--people")
        .and_then(|value| value.parse().ok())
        .unwrap_or(10_000);
    let seed = value_after(&args, "--seed")
        .and_then(|value| value.parse().ok())
        .unwrap_or(42);
    let json = args.iter().any(|arg| arg == "--json");
    if value_after(&args, "--people").is_none()
        && matches!(args.first().map(String::as_str), Some("query" | "report"))
        && let Some(person) =
            value_after(&args, "--person").and_then(|value| value.parse::<u32>().ok())
    {
        people = people.max(person.saturating_add(1));
    }

    let start = Instant::now();
    let world = generate_world(GenerationConfig { people, seed })?;
    let generation_ms = start.elapsed().as_millis();
    let start = Instant::now();
    let index = WorldIndex::build(&world.windows, &world.connections);
    let index_ms = start.elapsed().as_millis();

    match args[0].as_str() {
        "generate" => {
            let artifact_dir = artifact_dir();
            write_world_jsonl(&world, &artifact_dir)?;
            write_default_reports(&world, &index, &artifact_dir)?;
            println!(
                "generated {} people, {} windows, {} connections in {generation_ms}ms; indexed in {index_ms}ms",
                world.people.len(),
                world.windows.len(),
                world.connections.len()
            );
            println!("artifacts: {}", artifact_dir.display());
        }
        "query" => run_query(&args[1..], &world, &index, json)?,
        "report" => run_report(&args[1..], &world, &index)?,
        _ => print_help(),
    }
    Ok(())
}

fn run_query(
    args: &[String],
    world: &npc_stress_test::generation::routines::WorldData,
    index: &WorldIndex,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let Some(command) = args.first().map(String::as_str) else {
        print_help();
        return Ok(());
    };
    match command {
        "chunk-at" => {
            let chunk = value_after(args, "--chunk").unwrap_or("chunk2");
            let tick = parse_tick(args);
            let people = people_in_chunk_at(world, index, chunk, tick);
            emit_people(&people, json)?;
        }
        "district-at" => {
            let district = value_after(args, "--district").unwrap_or("district_23");
            let tick = parse_tick(args);
            let people = people_in_canonical_location_at(world, index, district, tick);
            emit_people(&people, json)?;
        }
        "person-at" => {
            let person = parse_person(args);
            let tick = parse_tick(args);
            let window = activity_for_person_at(world, index, person, tick);
            if json {
                println!("{}", serde_json::to_string_pretty(&window)?);
            } else if let Some(window) = window {
                println!(
                    "person {} at tick {tick}: {:?} in {} / {} / {}",
                    person.0,
                    window.activity_kind,
                    window.location_canonical,
                    window.chunk_id,
                    window.precise_location_id
                );
            } else {
                println!("person {} has no active window at tick {tick}", person.0);
            }
        }
        "connected-at" => {
            let person = parse_person(args);
            let tick = parse_tick(args);
            let rows = connected_people_locations_at(world, index, person, tick);
            emit_connected(&rows, json)?;
        }
        _ => print_help(),
    }
    Ok(())
}

fn run_report(
    args: &[String],
    world: &npc_stress_test::generation::routines::WorldData,
    index: &WorldIndex,
) -> Result<(), Box<dyn std::error::Error>> {
    let artifact_dir = artifact_dir().join("reports");
    let Some(command) = args.first().map(String::as_str) else {
        print_help();
        return Ok(());
    };
    match command {
        "repeated-contact" => {
            let person = parse_person(args);
            let min_count = value_after(args, "--min-count")
                .and_then(|value| value.parse().ok())
                .unwrap_or(3);
            let report = repeated_contact_markdown(world, index, person, min_count);
            let path = artifact_dir.join(format!("repeated_contact_{}.md", person.0));
            write_markdown(&path, &report)?;
            println!("{}", report);
            println!("wrote {}", path.display());
        }
        "person" => {
            let person = parse_person(args);
            let report = person_timeline_report(world, index, person);
            write_markdown(
                artifact_dir.join(format!("person_{}.md", person.0)),
                &report,
            )?;
            println!("{report}");
        }
        "district" => {
            let district = value_after(args, "--district").unwrap_or("district_23");
            let report = district_occupancy_report(world, index, district);
            write_markdown(artifact_dir.join(format!("{district}.md")), &report)?;
            println!("{report}");
        }
        "suspicious" => {
            let person = parse_person(args);
            let report = suspicious_pattern_report(world, index, person);
            write_markdown(
                artifact_dir.join(format!("suspicious_{}.md", person.0)),
                &report,
            )?;
            println!("{report}");
        }
        _ => print_help(),
    }
    Ok(())
}

fn write_default_reports(
    world: &npc_stress_test::generation::routines::WorldData,
    index: &WorldIndex,
    artifact_dir: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let person = PersonId(12_345.min(world.people.len().saturating_sub(1) as u32));
    write_markdown(
        artifact_dir
            .join("reports")
            .join(format!("person_{}.md", person.0)),
        &person_timeline_report(world, index, person),
    )?;
    write_markdown(
        artifact_dir.join("reports").join("district_23.md"),
        &district_occupancy_report(world, index, "district_23"),
    )?;
    write_markdown(
        artifact_dir
            .join("reports")
            .join(format!("repeated_contact_{}.md", person.0)),
        &repeated_contact_markdown(world, index, person, 3),
    )?;
    write_markdown(
        artifact_dir
            .join("reports")
            .join(format!("suspicious_{}.md", person.0)),
        &suspicious_pattern_report(world, index, person),
    )?;
    write_html_dashboard(world, index, artifact_dir, person, "district_23")?;
    let _ = co_located_with_person_at(world, index, person, 38_400);
    let _ = repeated_contact_report(world, index, person, 3, 1);
    Ok(())
}

fn emit_people(people: &[PersonId], json: bool) -> Result<(), serde_json::Error> {
    if json {
        println!("{}", serde_json::to_string_pretty(people)?);
    } else {
        println!("{} people", people.len());
        println!(
            "{}",
            people
                .iter()
                .take(40)
                .map(|id| id.0.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    Ok(())
}

fn emit_connected(rows: &[ConnectedPersonLocation], json: bool) -> Result<(), serde_json::Error> {
    if json {
        println!("{}", serde_json::to_string_pretty(rows)?);
    } else {
        println!("| Person | Kind | Activity | District | Chunk |");
        println!("| --- | --- | --- | --- | --- |");
        for row in rows {
            println!(
                "| {} | {:?} | {:?} | {} | {} |",
                row.person_id.0,
                row.connection_kind,
                row.activity_kind,
                row.location_canonical.as_deref().unwrap_or(""),
                row.chunk_id.as_deref().unwrap_or("")
            );
        }
    }
    Ok(())
}

fn value_after<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.windows(2)
        .find(|pair| pair[0] == flag)
        .map(|pair| pair[1].as_str())
}

fn parse_tick(args: &[String]) -> u32 {
    value_after(args, "--tick")
        .and_then(|value| value.parse().ok())
        .unwrap_or(37_800)
}

fn parse_person(args: &[String]) -> PersonId {
    PersonId(
        value_after(args, "--person")
            .and_then(|value| value.parse().ok())
            .unwrap_or(12_345),
    )
}

fn artifact_dir() -> PathBuf {
    PathBuf::from("artifacts")
}

fn print_help() {
    println!(
        "NPC temporal-window stress test\n\n\
         cargo run -- generate --people 10000 --seed 42\n\
         cargo run -- query chunk-at --chunk chunk2 --tick 37800\n\
         cargo run -- query district-at --district district_23 --tick 37800\n\
         cargo run -- query person-at --person 12345 --tick 37800\n\
         cargo run -- query connected-at --person 12345 --tick 37800 --json\n\
         cargo run -- report repeated-contact --person 12345 --min-count 3"
    );
}
