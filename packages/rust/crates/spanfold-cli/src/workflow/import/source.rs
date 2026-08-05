use std::{
    collections::BTreeMap,
    fs,
    io::{BufRead, BufReader},
    path::Path,
};

use super::super::{close_remaining_imported_windows, process_import_event};
use super::{CompiledImportMap, ImportError, ImportedWindowSink};

pub(crate) fn import_events_jsonl(
    path: &Path,
    import_map: &CompiledImportMap,
    sink: &mut impl ImportedWindowSink,
) -> Result<(), ImportError> {
    let path_label = path.display().to_string();
    let file = fs::File::open(path).map_err(|error| {
        ImportError::io(format!(
            "import-events: read events '{path_label}': {error}"
        ))
    })?;
    let reader = BufReader::new(file);
    let mut active = BTreeMap::new();
    let mut last_position: Option<i64> = None;

    for (index, line) in reader.lines().enumerate() {
        let line = line.map_err(|error| {
            ImportError::io(format!(
                "import-events: read event record '{path_label}':{}: {error}",
                index + 1
            ))
        })?;
        if line.trim().is_empty() {
            continue;
        }
        let event: serde_json::Value = serde_json::from_str(&line).map_err(|error| {
            format!(
                "import-events: parse event record '{path_label}':{}: {error}",
                index + 1
            )
        })?;
        process_import_event(
            &event,
            import_map,
            &path_label,
            index + 1,
            &mut active,
            sink,
            &mut last_position,
        )?;
    }

    close_remaining_imported_windows(active, sink)?;
    Ok(())
}

pub(crate) fn import_events_csv(
    path: &Path,
    import_map: &CompiledImportMap,
    sink: &mut impl ImportedWindowSink,
) -> Result<(), ImportError> {
    let path_label = path.display().to_string();
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        .from_path(path)
        .map_err(|error| ImportError::csv(&path_label, error))?;
    let headers = reader
        .headers()
        .map_err(|error| ImportError::csv(&format!("{path_label}:1"), error))?
        .iter()
        .enumerate()
        .map(|(index, header)| {
            if index == 0 {
                header.trim_start_matches('\u{feff}').to_owned()
            } else {
                header.to_owned()
            }
        })
        .collect::<Vec<_>>();
    if headers.is_empty() {
        return Err(format!(
            "import-events: parse CSV header '{path_label}:1': CSV header must contain at least one column"
        )
        .into());
    }
    let mut header_names = std::collections::BTreeSet::new();
    for header in &headers {
        if header.trim().is_empty() || !header_names.insert(header.as_str()) {
            return Err(format!(
                "import-events: parse CSV header '{path_label}:1': CSV headers must be non-empty and unique"
            )
            .into());
        }
    }

    let mut active = BTreeMap::new();
    let mut last_position: Option<i64> = None;

    for (index, record) in reader.records().enumerate() {
        let line_number = index + 2;
        let record = record
            .map_err(|error| ImportError::csv(&format!("{path_label}:{line_number}"), error))?;
        let mut event = serde_json::Map::new();
        for (header, field) in headers.iter().zip(record.iter()) {
            event.insert(header.clone(), serde_json::Value::String(field.to_owned()));
        }
        process_import_event(
            &serde_json::Value::Object(event),
            import_map,
            &path_label,
            line_number,
            &mut active,
            sink,
            &mut last_position,
        )?;
    }

    close_remaining_imported_windows(active, sink)?;
    Ok(())
}
