use std::{fs, path::PathBuf};

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;

fn fixture_path(name: &str) -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    root.join("../../../dotnet/tests/Spanfold.Tests/Comparison/Fixtures")
        .join(name)
}

#[test]
fn compare_outputs_json_for_basic_overlap_fixture() {
    Command::cargo_bin("spanfold")
        .expect("binary")
        .args([
            "compare",
            fixture_path("basic-overlap.json")
                .to_str()
                .expect("utf8 fixture path"),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "\"schema\": \"spanfold.comparison.result\"",
        ))
        .stdout(predicate::str::contains("\"rowCount\": 1"));
}

#[test]
fn audit_writes_artifact_bundle() {
    let out = tempdir().expect("tempdir");
    Command::cargo_bin("spanfold")
        .expect("binary")
        .args([
            "audit",
            fixture_path("basic-overlap.json")
                .to_str()
                .expect("utf8 fixture path"),
            "--out",
            out.path().to_str().expect("utf8 output path"),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "\"schema\": \"spanfold.audit.bundle\"",
        ));

    assert!(fs::exists(out.path().join("comparison.json")).expect("comparison.json status"));
    assert!(fs::exists(out.path().join("comparison.md")).expect("comparison.md status"));
    assert!(fs::exists(out.path().join("comparison.html")).expect("comparison.html status"));
    assert!(
        fs::exists(out.path().join("comparison.llm.json")).expect("comparison.llm.json status")
    );
    assert!(
        fs::exists(out.path().join("comparison.rows.jsonl")).expect("comparison.rows.jsonl status")
    );
    assert!(fs::exists(out.path().join("manifest.json")).expect("manifest.json status"));
}

#[test]
fn audit_failure_does_not_partially_replace_an_existing_bundle() {
    let out = tempdir().expect("tempdir");
    let comparison_json = out.path().join("comparison.json");
    fs::write(&comparison_json, "existing comparison").expect("existing comparison");
    fs::create_dir(out.path().join("comparison.md")).expect("blocking artifact directory");

    Command::cargo_bin("spanfold")
        .expect("binary")
        .args([
            "audit",
            fixture_path("basic-overlap.json")
                .to_str()
                .expect("utf8 fixture path"),
            "--out",
            out.path().to_str().expect("utf8 output path"),
        ])
        .assert()
        .code(3)
        .stderr(predicate::str::contains("\"code\":\"io\""));

    assert_eq!(
        fs::read_to_string(comparison_json).expect("preserved comparison"),
        "existing comparison"
    );
    assert!(!out.path().join("comparison.llm.json").exists());
    assert!(!out.path().join("comparison.html").exists());
    assert!(!out.path().join("comparison.rows.jsonl").exists());
    assert!(!out.path().join("manifest.json").exists());
}

#[test]
fn compare_outputs_llm_context_with_row_documents() {
    Command::cargo_bin("spanfold")
        .expect("binary")
        .args([
            "compare",
            fixture_path("basic-overlap.json")
                .to_str()
                .expect("utf8 fixture path"),
            "--format",
            "llm-context",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "\"schema\": \"spanfold.comparison.llm-context\"",
        ))
        .stdout(predicate::str::contains("\"artifact\": \"result-summary\""))
        .stdout(predicate::str::contains("\"rowId\": \"overlap:"));
}

#[test]
fn import_events_writes_window_jsonl() {
    let workspace = tempdir().expect("tempdir");
    let events = workspace.path().join("events.jsonl");
    let map = workspace.path().join("map.json");
    let out = workspace.path().join("windows.jsonl");
    write_event_import_files(&events, &map);

    Command::cargo_bin("spanfold")
        .expect("binary")
        .args([
            "import-events",
            events.to_str().expect("utf8 events path"),
            "--map",
            map.to_str().expect("utf8 map path"),
            "--out",
            out.to_str().expect("utf8 output path"),
        ])
        .assert()
        .success();

    let output = fs::read_to_string(out).expect("windows output");
    assert!(output.contains("\"windowName\":\"DeviceOffline\""));
    assert!(output.contains("\"source\":\"provider-a\""));
    assert!(output.contains("\"startPosition\":1"));
    assert!(output.contains("\"endPosition\":5"));
    assert!(output.contains("\"name\":\"region\""));
}

#[test]
fn import_events_updates_tags_without_reopening_unchanged_segments() {
    let workspace = tempdir().expect("tempdir");
    let events = workspace.path().join("events.jsonl");
    let map = workspace.path().join("map.json");
    let out = workspace.path().join("windows.jsonl");
    write_event_import_files(&events, &map);
    fs::write(
        &events,
        [
            r#"{"position":1,"source":"provider-a","deviceId":"device-1","status":"offline","region":"eu","severity":"high"}"#,
            r#"{"position":2,"source":"provider-a","deviceId":"device-1","status":"offline","region":"eu","severity":"critical"}"#,
        ]
        .join("\n"),
    )
    .expect("events file");

    Command::cargo_bin("spanfold")
        .expect("binary")
        .args([
            "import-events",
            events.to_str().expect("utf8 events path"),
            "--map",
            map.to_str().expect("utf8 map path"),
            "--out",
            out.to_str().expect("utf8 output path"),
        ])
        .assert()
        .success();

    let output = fs::read_to_string(out).expect("windows output");
    let windows = output.lines().collect::<Vec<_>>();
    assert_eq!(windows.len(), 1);
    let window: serde_json::Value = serde_json::from_str(windows[0]).expect("window json parse");
    assert_eq!(window["startPosition"], 1);
    assert!(window["endPosition"].is_null());
    assert_eq!(window["segments"][0]["value"], "eu");
    assert_eq!(window["tags"][0]["value"], "critical");
}

#[test]
fn import_events_accepts_csv_with_header_row() {
    let workspace = tempdir().expect("tempdir");
    let events = workspace.path().join("events.csv");
    let map = workspace.path().join("map.json");
    let out = workspace.path().join("windows.jsonl");
    write_event_import_csv_files(&events, &map);

    Command::cargo_bin("spanfold")
        .expect("binary")
        .args([
            "import-events",
            events.to_str().expect("utf8 events path"),
            "--map",
            map.to_str().expect("utf8 map path"),
            "--out",
            out.to_str().expect("utf8 output path"),
        ])
        .assert()
        .success();

    let output = fs::read_to_string(out).expect("windows output");
    assert!(output.contains("\"windowName\":\"DeviceOffline\""));
    assert!(output.contains("\"source\":\"provider-a\""));
    assert!(output.contains("\"endPosition\":5"));
}

#[test]
fn import_events_reports_missing_event_file_as_io_error() {
    let workspace = tempdir().expect("tempdir");
    let events = workspace.path().join("missing-events.jsonl");
    let map = workspace.path().join("map.json");
    let out = workspace.path().join("windows.jsonl");
    write_event_import_map(&map);

    Command::cargo_bin("spanfold")
        .expect("binary")
        .args([
            "import-events",
            events.to_str().expect("utf8 events path"),
            "--map",
            map.to_str().expect("utf8 map path"),
            "--out",
            out.to_str().expect("utf8 output path"),
        ])
        .assert()
        .code(3)
        .stderr(predicate::str::contains("\"code\":\"io\""));
}

#[test]
fn import_events_reports_malformed_json_as_input_error() {
    let workspace = tempdir().expect("tempdir");
    let events = workspace.path().join("events.jsonl");
    let map = workspace.path().join("map.json");
    let out = workspace.path().join("windows.jsonl");
    write_event_import_files(&events, &map);
    fs::write(&events, "{not json}\n").expect("events file");

    Command::cargo_bin("spanfold")
        .expect("binary")
        .args([
            "import-events",
            events.to_str().expect("utf8 events path"),
            "--map",
            map.to_str().expect("utf8 map path"),
            "--out",
            out.to_str().expect("utf8 output path"),
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("\"code\":\"input\""));
}

#[test]
fn audit_events_writes_artifact_bundle() {
    let workspace = tempdir().expect("tempdir");
    let events = workspace.path().join("events.jsonl");
    let map = workspace.path().join("map.json");
    let out = workspace.path().join("audit");
    write_event_import_files(&events, &map);

    Command::cargo_bin("spanfold")
        .expect("binary")
        .args([
            "audit-events",
            events.to_str().expect("utf8 events path"),
            "--map",
            map.to_str().expect("utf8 map path"),
            "--target",
            "provider-a",
            "--against",
            "provider-b",
            "--out",
            out.to_str().expect("utf8 output path"),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "\"schema\": \"spanfold.audit.bundle\"",
        ));

    assert!(fs::exists(out.join("comparison.json")).expect("comparison.json status"));
    let comparison = fs::read_to_string(out.join("comparison.json")).expect("comparison json");
    let comparison: serde_json::Value =
        serde_json::from_str(&comparison).expect("comparison json parse");
    assert_eq!(
        comparison["rows"]["overlap"]
            .as_array()
            .expect("overlap rows")
            .len(),
        1
    );
    assert_eq!(
        comparison["rows"]["residual"]
            .as_array()
            .expect("residual rows")
            .len(),
        1
    );
}

#[test]
fn audit_windows_accepts_custom_comparison_options() {
    let workspace = tempdir().expect("tempdir");
    let windows = workspace.path().join("windows.jsonl");
    let out = workspace.path().join("audit");
    fs::write(
        &windows,
        [
            r#"{"key":"device-1","source":"provider-a","startPosition":1}"#,
            r#"{"key":"device-1","source":"provider-b","startPosition":3,"endPosition":7}"#,
        ]
        .join("\n"),
    )
    .expect("windows file");

    Command::cargo_bin("spanfold")
        .expect("binary")
        .args([
            "audit-windows",
            windows.to_str().expect("utf8 windows path"),
            "--window",
            "DeviceOffline",
            "--target",
            "provider-a",
            "--against",
            "provider-b",
            "--name",
            "Live audit",
            "--comparators",
            "residual",
            "--strict",
            "--live-horizon-position",
            "10",
            "--out",
            out.to_str().expect("utf8 output path"),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"planName\": \"Live audit\""));

    let comparison = fs::read_to_string(out.join("comparison.json")).expect("comparison json");
    let comparison: serde_json::Value =
        serde_json::from_str(&comparison).expect("comparison json parse");
    assert_eq!(
        comparison["comparatorSummaries"][0]["comparatorName"],
        "residual"
    );
    assert_eq!(comparison["rowFinalities"][0]["finality"], "Provisional");
}

fn write_event_import_files(events: &PathBuf, map: &PathBuf) {
    fs::write(
        events,
        [
            r#"{"position":1,"source":"provider-a","deviceId":"device-1","status":"offline","region":"eu","severity":"high"}"#,
            r#"{"position":2,"source":"provider-b","deviceId":"device-1","status":"offline","region":"eu","severity":"high"}"#,
            r#"{"position":5,"source":"provider-a","deviceId":"device-1","status":"online","region":"eu","severity":"high"}"#,
            r#"{"position":6,"source":"provider-b","deviceId":"device-1","status":"online","region":"eu","severity":"high"}"#,
        ]
        .join("\n"),
    )
    .expect("events file");
    write_event_import_map(map);
}

fn write_event_import_map(map: &PathBuf) {
    fs::write(
        map,
        r#"{
  "input": "jsonl",
  "source": "source",
  "position": "position",
  "windows": [
    {
      "name": "DeviceOffline",
      "key": "deviceId",
      "active": { "field": "status", "equals": "offline" },
      "segments": [{ "name": "region", "field": "region" }],
      "tags": [{ "name": "severity", "field": "severity" }]
    }
  ]
}"#,
    )
    .expect("map file");
}

fn write_event_import_csv_files(events: &PathBuf, map: &PathBuf) {
    fs::write(
        events,
        [
            "position,source,deviceId,status,region,severity",
            "1,provider-a,device-1,offline,eu,high",
            "2,provider-b,device-1,offline,eu,high",
            "5,provider-a,device-1,online,eu,high",
            "6,provider-b,device-1,online,eu,high",
        ]
        .join("\n"),
    )
    .expect("events file");
    fs::write(
        map,
        r#"{
  "input": "csv",
  "source": "source",
  "position": "position",
  "windows": [
    {
      "name": "DeviceOffline",
      "key": "deviceId",
      "active": { "field": "status", "equals": "offline" },
      "segments": [{ "name": "region", "field": "region" }],
      "tags": [{ "name": "severity", "field": "severity" }]
    }
  ]
}"#,
    )
    .expect("map file");
}
