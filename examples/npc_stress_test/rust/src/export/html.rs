use std::{fs, path::Path};

use crate::{
    domain::PersonId,
    generation::routines::WorldData,
    index::WorldIndex,
    queries::{
        connected_person_report, district_occupancy_report, people_in_canonical_location_at,
        people_in_chunk_at, person_timeline_report, repeated_contact_markdown,
        suspicious_pattern_report,
    },
};

pub fn write_html_dashboard(
    world: &WorldData,
    index: &WorldIndex,
    artifact_dir: &Path,
    person_id: PersonId,
    district: &str,
) -> Result<(), std::io::Error> {
    fs::create_dir_all(artifact_dir)?;
    let tick = 38_400;
    let chunk_people = people_in_chunk_at(world, index, "chunk2", tick);
    let district_people = people_in_canonical_location_at(world, index, district, tick);
    let reports = [
        (
            "Person Timeline",
            person_timeline_report(world, index, person_id),
        ),
        (
            "District Occupancy",
            district_occupancy_report(world, index, district),
        ),
        (
            "Connected People",
            connected_person_report(world, index, person_id, &[8 * 3600, tick, 19 * 3600]),
        ),
        (
            "Repeated Contact",
            repeated_contact_markdown(world, index, person_id, 3),
        ),
        (
            "Suspicious Patterns",
            suspicious_pattern_report(world, index, person_id),
        ),
    ];

    let mut body = String::new();
    body.push_str(&format!(
        r#"<section class="hero">
  <p class="eyebrow">Spanfold Rust example</p>
  <h1>NPC Temporal-Window Stress Test</h1>
  <p>Deterministic daily windows for simulated people in a game city, with indexed point-in-time and overlap queries.</p>
  <div class="metrics">
    <div><strong>{}</strong><span>people</span></div>
    <div><strong>{}</strong><span>windows</span></div>
    <div><strong>{}</strong><span>connections</span></div>
    <div><strong>{}</strong><span>chunk2 at tick {tick}</span></div>
    <div><strong>{}</strong><span>{district} at tick {tick}</span></div>
  </div>
</section>"#,
        world.people.len(),
        world.windows.len(),
        world.connections.len(),
        chunk_people.len(),
        district_people.len()
    ));

    body.push_str(
        r#"<section>
  <h2>Generated Artifacts</h2>
  <div class="links">
    <a href="people.jsonl">people.jsonl</a>
    <a href="windows.jsonl">windows.jsonl</a>
    <a href="connections.jsonl">connections.jsonl</a>
    <a href="reports/">Markdown reports</a>
  </div>
</section>"#,
    );

    for (title, markdown) in reports {
        body.push_str(&format!(
            r#"<section class="report">
  <h2>{}</h2>
  {}
</section>"#,
            escape_html(title),
            markdown_to_html(&markdown)
        ));
    }

    fs::write(artifact_dir.join("index.html"), html_shell(&body))
}

fn html_shell(body: &str) -> String {
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Spanfold NPC Stress Test</title>
  <style>
    :root {{
      --paper: #faf8f4;
      --panel: #f5f1e8;
      --rule: #d8d0c2;
      --ink: #1a1714;
      --ink-2: #5a544c;
      --target: #2a2520;
      --against: #c9a87a;
      --overlap: #3d6b4a;
    }}
    * {{ box-sizing: border-box; }}
    body {{
      margin: 0;
      background: var(--paper);
      color: var(--ink);
      font: 14px/1.55 system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    }}
    main {{ max-width: 1120px; margin: 0 auto; padding: 48px 24px 72px; }}
    section {{ border-top: 1px solid var(--rule); padding: 28px 0; }}
    .hero {{ border-top: 0; padding-top: 0; }}
    .eyebrow {{
      margin: 0 0 8px;
      font: 700 11px/1.2 ui-monospace, SFMono-Regular, Menlo, monospace;
      letter-spacing: 1.6px;
      text-transform: uppercase;
      color: var(--ink-2);
    }}
    h1 {{ margin: 0; font-size: 34px; line-height: 1.1; letter-spacing: 0; }}
    h2 {{ margin: 0 0 16px; font-size: 20px; line-height: 1.2; letter-spacing: 0; }}
    h3 {{ margin: 22px 0 10px; font-size: 15px; }}
    p {{ max-width: 760px; color: var(--ink-2); }}
    .metrics {{
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(160px, 1fr));
      gap: 10px;
      margin-top: 24px;
    }}
    .metrics div {{
      background: var(--panel);
      border: 1px solid var(--rule);
      border-radius: 8px;
      padding: 14px;
    }}
    .metrics strong {{ display: block; font-size: 24px; line-height: 1.1; }}
    .metrics span {{ display: block; margin-top: 4px; color: var(--ink-2); font-size: 12px; }}
    .links {{ display: flex; flex-wrap: wrap; gap: 10px; }}
    .links a {{
      color: var(--paper);
      background: var(--target);
      border-bottom: 3px solid #000;
      border-radius: 6px;
      padding: 9px 12px;
      text-decoration: none;
      font-weight: 700;
    }}
    table {{ width: 100%; border-collapse: collapse; margin: 14px 0 20px; font-size: 13px; }}
    th, td {{ border-bottom: 1px solid var(--rule); padding: 7px 8px; text-align: left; vertical-align: top; }}
    th {{ background: var(--panel); font-size: 12px; }}
    ul {{ padding-left: 20px; }}
    code {{ font-family: ui-monospace, SFMono-Regular, Menlo, monospace; }}
  </style>
</head>
<body>
  <main>
    {body}
  </main>
</body>
</html>"#
    )
}

fn markdown_to_html(markdown: &str) -> String {
    let mut out = String::new();
    let mut in_table = false;
    let mut in_list = false;
    for line in markdown.lines() {
        if line.trim().is_empty() {
            close_table(&mut out, &mut in_table);
            close_list(&mut out, &mut in_list);
            continue;
        }
        if let Some(title) = line.strip_prefix("# ") {
            close_table(&mut out, &mut in_table);
            close_list(&mut out, &mut in_list);
            out.push_str(&format!("<h3>{}</h3>", escape_html(title)));
        } else if let Some(title) = line.strip_prefix("## ") {
            close_table(&mut out, &mut in_table);
            close_list(&mut out, &mut in_list);
            out.push_str(&format!("<h3>{}</h3>", escape_html(title)));
        } else if line.starts_with("| ") {
            if line.contains("---") {
                continue;
            }
            close_list(&mut out, &mut in_list);
            if !in_table {
                out.push_str("<table>");
                in_table = true;
            }
            let cells = line
                .trim_matches('|')
                .split('|')
                .map(|cell| format!("<td>{}</td>", escape_html(cell.trim())))
                .collect::<String>();
            out.push_str(&format!("<tr>{cells}</tr>"));
        } else if let Some(item) = line.strip_prefix("- ") {
            close_table(&mut out, &mut in_table);
            if !in_list {
                out.push_str("<ul>");
                in_list = true;
            }
            out.push_str(&format!("<li>{}</li>", escape_html(item)));
        } else {
            close_table(&mut out, &mut in_table);
            close_list(&mut out, &mut in_list);
            out.push_str(&format!("<p>{}</p>", escape_html(line)));
        }
    }
    close_table(&mut out, &mut in_table);
    close_list(&mut out, &mut in_list);
    out
}

fn close_table(out: &mut String, in_table: &mut bool) {
    if *in_table {
        out.push_str("</table>");
        *in_table = false;
    }
}

fn close_list(out: &mut String, in_list: &mut bool) {
    if *in_list {
        out.push_str("</ul>");
        *in_list = false;
    }
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
