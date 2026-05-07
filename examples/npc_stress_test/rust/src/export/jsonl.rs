use std::{
    fs::{self, File},
    io::{BufWriter, Write},
    path::Path,
};

use serde::Serialize;

use crate::generation::routines::WorldData;

pub fn write_world_jsonl(
    world: &WorldData,
    artifact_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(artifact_dir)?;
    write_jsonl(artifact_dir.join("people.jsonl"), &world.people)?;
    write_jsonl(artifact_dir.join("windows.jsonl"), &world.windows)?;
    write_jsonl(artifact_dir.join("connections.jsonl"), &world.connections)?;
    Ok(())
}

pub fn write_jsonl<T: Serialize>(
    path: impl AsRef<Path>,
    rows: &[T],
) -> Result<(), Box<dyn std::error::Error>> {
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);
    for row in rows {
        serde_json::to_writer(&mut writer, row)?;
        writer.write_all(b"\n")?;
    }
    Ok(())
}
