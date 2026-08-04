//! Self-contained debug HTML rendering.

use super::*;

/// Exports a self-contained comparison debug page as deterministic HTML.
pub fn export_result_debug_html(result: &ComparisonResult) -> String {
    let mut html = String::with_capacity(32 * 1024);
    let mut row_count = 0;
    macro_rules! count_rows {
        ($(($kind:ident, $rows:ident, $compat:ident, $view:ident, $debug:literal, $count:literal),)*) => {
            $(
                row_count += result.canonical_rows().$rows.len();
            )*
        };
    }
    crate::comparison::for_each_comparison_row_family!(count_rows);
    let provisional_rows = result
        .canonical_row_finalities()
        .filter(|row| row.finality == ComparisonFinality::Provisional)
        .count();

    write!(
        html,
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>{} - Spanfold debug</title><style>{}</style></head><body><main>",
        escape_html(&result.plan_name),
        DEBUG_HTML_STYLE
    )
    .expect("write debug html");
    write!(
        html,
        "<section class=\"hero\"><div class=\"eyebrow\">Spanfold comparison debug</div><h1>{}</h1><p class=\"lead\">Inspect selected windows, aligned segments, comparator rows, finality, diagnostics, and extension metadata.</p><div class=\"badges\"><span class=\"badge {}\">{}</span>",
        escape_html(&result.plan_name),
        if result.is_valid { "valid" } else { "invalid" },
        if result.is_valid {
            "Valid result"
        } else {
            "Invalid result"
        }
    )
    .expect("write debug html");
    if let Some(horizon) = result.evaluation_horizon.as_ref() {
        write!(
            html,
            "<span class=\"badge live\">Live horizon {}</span>",
            horizon.magnitude
        )
        .expect("write debug html");
    }
    if let Some(known_at) = result.known_at.as_ref() {
        write!(
            html,
            "<span class=\"badge\">Known at {}</span>",
            known_at.magnitude
        )
        .expect("write debug html");
    }
    html.push_str("</div></section>");

    html.push_str("<section class=\"grid\" aria-label=\"Comparison summary\">");
    append_debug_card(
        &mut html,
        "Selected windows",
        prepared_len(result, "selectedWindows"),
    );
    append_debug_card(
        &mut html,
        "Normalized windows",
        prepared_len(result, "normalizedWindows"),
    );
    append_debug_card(&mut html, "Aligned segments", aligned_len(result));
    append_debug_card(&mut html, "Result rows", row_count);
    append_debug_card(&mut html, "Diagnostics", result.diagnostics.len());
    append_debug_card(&mut html, "Provisional rows", provisional_rows);
    append_debug_card(&mut html, "Comparators", result.comparator_summaries.len());
    append_debug_card(
        &mut html,
        "Excluded windows",
        prepared_len(result, "excludedWindows"),
    );
    html.push_str("</section>");

    append_debug_json_section(
        &mut html,
        "Window Timeline",
        "Normalized windows after selector, scope, known-at, and open-window policy have been applied.",
        result
            .prepared
            .as_ref()
            .and_then(|value| value.get("normalizedWindows")),
        80,
        "normalized windows",
    );
    append_debug_json_section(
        &mut html,
        "Aligned Segments",
        "Prepared target and against windows after boundary alignment.",
        result
            .aligned
            .as_ref()
            .and_then(|value| value.get("segments")),
        80,
        "aligned segments",
    );
    append_debug_diagnostics(&mut html, result);
    append_debug_metadata(&mut html, result);
    append_debug_rows(&mut html, result);
    append_debug_pretty_json(&mut html, "Prepared", result.prepared.as_ref());
    append_debug_pretty_json(&mut html, "Aligned", result.aligned.as_ref());
    write!(
        html,
        "<section class=\"panel\"><h2>Markdown Summary</h2><pre>{}</pre></section>",
        escape_html(&export_result_markdown(result))
    )
    .expect("write debug html");
    html.push_str("</main></body></html>");
    html
}

const DEBUG_HTML_STYLE: &str = r#"
body{margin:0;background:#f6f4ee;color:#26231f;font:14px/1.5 ui-sans-serif,system-ui,-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif}
main{width:min(1220px,calc(100% - 32px));margin:0 auto;padding:28px 0 48px}
.hero,.panel{background:#fffaf0;border:1px solid #d8cfb9;border-radius:8px}.hero{padding:28px;border-top:4px solid #3e6b5c}
.eyebrow{color:#3e6b5c;font-size:12px;font-weight:700;letter-spacing:.08em;text-transform:uppercase}h1,h2,h3,p{margin:0}h1{margin-top:8px;font-size:40px;line-height:1.02}.lead{max-width:760px;margin-top:14px;color:#6b6659;font-size:16px}
.badges{display:flex;flex-wrap:wrap;gap:8px;margin-top:20px}.badge{display:inline-flex;min-height:28px;align-items:center;padding:4px 10px;border:1px solid #d8cfb9;border-radius:8px;background:#eee5cf;font-weight:650}.badge.valid{border-color:#3e6b5c;color:#3e6b5c}.badge.invalid{border-color:#b8742a;color:#b8742a}.badge.live{border-color:#c97a3a;color:#9c5525}
.grid{display:grid;grid-template-columns:repeat(4,minmax(0,1fr));gap:12px;margin-top:18px}.card{min-height:86px;padding:16px;border:1px solid #d8cfb9;border-radius:8px;background:#fffaf0}.value{margin-top:6px;font-size:30px;font-weight:760;line-height:1}.label{color:#6b6659;font-size:12px;font-weight:700;text-transform:uppercase}
.panel{margin-top:18px;padding:22px}.section-note{max-width:760px;margin-top:6px;color:#6b6659}.table-wrap{overflow-x:auto;border:1px solid #d8cfb9;border-radius:8px}table{width:100%;border-collapse:collapse;background:#fffaf0}th,td{padding:10px 12px;border-bottom:1px solid #d8cfb9;text-align:left;vertical-align:top}th{color:#6b6659;font-size:12px;font-weight:750;text-transform:uppercase}tr:last-child td{border-bottom:0}
.mono,pre{font-family:ui-monospace,SFMono-Regular,Menlo,Monaco,Consolas,"Liberation Mono",monospace;font-size:12px}pre{white-space:pre-wrap;word-break:break-word;background:#25231f;color:#f3ead6;border-radius:8px;padding:14px}.empty{padding:18px;border:1px dashed #d8cfb9;border-radius:8px;color:#6b6659;background:#fffaf0}.severity-error{color:#b8742a;font-weight:720}.severity-warning{color:#9c5525;font-weight:720}
@media(max-width:820px){main{width:min(100% - 20px,1220px);padding-top:16px}.hero,.panel{padding:18px}.grid{grid-template-columns:repeat(2,minmax(0,1fr))}}@media(max-width:520px){.grid{grid-template-columns:1fr}h1{font-size:30px}}
"#;

fn append_debug_card(html: &mut String, label: &str, value: usize) {
    write!(
        html,
        "<div class=\"card\"><div class=\"label\">{}</div><div class=\"value\">{}</div></div>",
        escape_html(label),
        value
    )
    .expect("write debug html");
}

fn append_debug_json_section(
    html: &mut String,
    title: &str,
    note: &str,
    value: Option<&Value>,
    max_rows: usize,
    unit: &str,
) {
    write!(
        html,
        "<section class=\"panel\"><h2>{}</h2><p class=\"section-note\">{}</p>",
        escape_html(title),
        escape_html(note)
    )
    .expect("write debug html");
    match value.and_then(Value::as_array) {
        Some(rows) if !rows.is_empty() => {
            html.push_str("<div class=\"table-wrap\" style=\"margin-top:18px\"><table><tbody>");
            for row in rows.iter().take(max_rows) {
                write!(
                    html,
                    "<tr><td class=\"mono\"><pre>{}</pre></td></tr>",
                    escape_html(
                        &serde_json::to_string_pretty(row)
                            .unwrap_or_else(|_| "<serialization-error>".to_owned())
                    )
                )
                .expect("write debug html");
            }
            if rows.len() > max_rows {
                write!(
                    html,
                    "<tr><td>Showing first {} of {} {}.</td></tr>",
                    max_rows,
                    rows.len(),
                    escape_html(unit)
                )
                .expect("write debug html");
            }
            html.push_str("</tbody></table></div>");
        }
        _ => {
            html.push_str("<div class=\"empty\" style=\"margin-top:18px\">No data available.</div>")
        }
    }
    html.push_str("</section>");
}

fn append_debug_diagnostics(html: &mut String, result: &ComparisonResult) {
    html.push_str("<section class=\"panel\"><h2>Diagnostics</h2>");
    if result.diagnostics.is_empty() {
        html.push_str(
            "<div class=\"empty\" style=\"margin-top:18px\">No diagnostics.</div></section>",
        );
        return;
    }
    html.push_str("<div class=\"table-wrap\" style=\"margin-top:18px\"><table><tbody>");
    for diagnostic in result.diagnostics.iter().take(120) {
        let value = serde_json::to_value(diagnostic).unwrap_or(Value::Null);
        write!(
            html,
            "<tr><td class=\"mono\"><pre>{}</pre></td></tr>",
            escape_html(
                &serde_json::to_string_pretty(&value)
                    .unwrap_or_else(|_| "<serialization-error>".to_owned())
            )
        )
        .expect("write debug html");
    }
    if result.diagnostics.len() > 120 {
        write!(
            html,
            "<tr><td>Showing first 120 of {} diagnostics.</td></tr>",
            result.diagnostics.len()
        )
        .expect("write debug html");
    }
    html.push_str("</tbody></table></div></section>");
}

fn append_debug_metadata(html: &mut String, result: &ComparisonResult) {
    html.push_str("<section class=\"panel\"><h2>Extension Metadata</h2>");
    if result.extension_metadata.is_empty() {
        html.push_str(
            "<div class=\"empty\" style=\"margin-top:18px\">No extension metadata.</div></section>",
        );
        return;
    }
    html.push_str("<div class=\"table-wrap\" style=\"margin-top:18px\"><table><tbody>");
    for item in result.extension_metadata.iter().take(120) {
        let value = serde_json::to_value(item).unwrap_or(Value::Null);
        write!(
            html,
            "<tr><td class=\"mono\"><pre>{}</pre></td></tr>",
            escape_html(
                &serde_json::to_string_pretty(&value)
                    .unwrap_or_else(|_| "<serialization-error>".to_owned())
            )
        )
        .expect("write debug html");
    }
    if result.extension_metadata.len() > 120 {
        write!(
            html,
            "<tr><td>Showing first 120 of {} metadata items.</td></tr>",
            result.extension_metadata.len()
        )
        .expect("write debug html");
    }
    html.push_str("</tbody></table></div></section>");
}

fn append_debug_rows(html: &mut String, result: &ComparisonResult) {
    html.push_str("<section class=\"panel\"><h2>Comparator Rows</h2>");
    macro_rules! append_tables {
        ($(($kind:ident, $rows:ident, $compat:ident, $view:ident, $debug:literal, $count:literal),)*) => {
            $(
                append_debug_row_table(html, $debug, &result.canonical_rows().$rows);
            )*
        };
    }
    crate::comparison::for_each_comparison_row_family!(append_tables);
    let finalities = result.canonical_row_finalities().collect::<Vec<_>>();
    if finalities.is_empty() {
        html.push_str("<div class=\"empty\" style=\"margin-top:18px\">No row finalities.</div>");
    } else {
        append_debug_row_table(html, "row-finality", &finalities);
    }
    html.push_str("</section>");
}

fn append_debug_row_table<T: Serialize>(html: &mut String, label: &str, rows: &[T]) {
    const MAX_ROWS_PER_TYPE: usize = 200;
    if rows.is_empty() {
        return;
    }
    write!(
        html,
        "<h3 style=\"margin-top:18px\">{}</h3><div class=\"table-wrap\"><table><tbody>",
        escape_html(label)
    )
    .expect("write debug html");
    for row in rows.iter().take(MAX_ROWS_PER_TYPE) {
        let value = serde_json::to_value(row).unwrap_or(Value::Null);
        write!(
            html,
            "<tr><td class=\"mono\"><pre>{}</pre></td></tr>",
            escape_html(
                &serde_json::to_string_pretty(&value)
                    .unwrap_or_else(|_| "<serialization-error>".to_owned())
            )
        )
        .expect("write debug html");
    }
    if rows.len() > MAX_ROWS_PER_TYPE {
        write!(
            html,
            "<tr><td>{}: showing {} of {} rows.</td></tr>",
            escape_html(label),
            MAX_ROWS_PER_TYPE,
            rows.len()
        )
        .expect("write debug html");
    }
    html.push_str("</tbody></table></div>");
}

fn append_debug_pretty_json(html: &mut String, title: &str, value: Option<&Value>) {
    let payload = value.cloned().unwrap_or(Value::Null);
    write!(
        html,
        "<section class=\"panel\"><h2>{}</h2><pre>{}</pre></section>",
        escape_html(title),
        escape_html(
            &serde_json::to_string_pretty(&payload)
                .unwrap_or_else(|_| "<serialization-error>".to_owned())
        )
    )
    .expect("write debug html");
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
