use std::{
    fs::{self, OpenOptions},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use crate::CliError;

pub(crate) fn validate_import_paths(
    events: &Path,
    map: &Path,
    output: &Path,
) -> Result<(), CliError> {
    if output.is_dir() {
        return Err(CliError::io(format!(
            "import-events: output path '{}' is a directory",
            output.display()
        )));
    }

    let paths = [
        ("events", events, resolve_import_path("events", events)?),
        ("map", map, resolve_import_path("map", map)?),
        ("output", output, resolve_import_path("output", output)?),
    ];
    for (index, (left_label, left_path, left_resolved)) in paths.iter().enumerate() {
        for (right_label, right_path, right_resolved) in paths.iter().skip(index + 1) {
            if left_resolved == right_resolved {
                return Err(CliError::input(format!(
                    "import-events: {left_label} path '{}' resolves to the same path as {right_label} path '{}'; canonical path aliases must be distinct (hard-linked files are supported by staged publication)",
                    left_path.display(),
                    right_path.display()
                )));
            }
        }
    }
    Ok(())
}

fn resolve_import_path(label: &str, path: &Path) -> Result<PathBuf, CliError> {
    match fs::canonicalize(path) {
        Ok(path) => Ok(path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let parent = path
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."));
            let parent = fs::canonicalize(parent).map_err(|error| {
                CliError::io(format!(
                    "import-events: resolve {label} path '{}': {error}",
                    path.display()
                ))
            })?;
            let file_name = path.file_name().ok_or_else(|| {
                CliError::io(format!(
                    "import-events: resolve {label} path '{}': path has no file name",
                    path.display()
                ))
            })?;
            Ok(parent.join(file_name))
        }
        Err(error) => Err(CliError::io(format!(
            "import-events: resolve {label} path '{}': {error}",
            path.display()
        ))),
    }
}

static IMPORT_TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

pub(crate) fn create_import_stage(output: &Path) -> Result<(PathBuf, fs::File), CliError> {
    let file_name = output
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("windows.jsonl");
    for _ in 0..100 {
        let temporary = output.with_file_name(format!(
            ".{file_name}.{}.{}.tmp",
            std::process::id(),
            IMPORT_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => return Ok((temporary, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(CliError::io(format!(
                    "import-events: create staging output '{}': {error}",
                    temporary.display()
                )));
            }
        }
    }
    Err(CliError::io(format!(
        "import-events: create staging output beside '{}' failed after repeated name collisions",
        output.display()
    )))
}

pub(crate) fn publish_import_stage(temporary: &Path, output: &Path) -> Result<(), std::io::Error> {
    // The staged file is a sibling, so rename is an atomic directory-entry
    // replacement on Unix. If replacement fails, the existing destination is
    // untouched. Windows reports an error when the destination already exists;
    // that leaves the existing destination intact and is surfaced to the CLI.
    fs::rename(temporary, output)
}
