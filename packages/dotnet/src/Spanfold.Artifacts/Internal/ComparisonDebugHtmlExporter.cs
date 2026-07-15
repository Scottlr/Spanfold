using Spanfold.Comparison;
using System.Globalization;
using System.Text;
using System.Text.Encodings.Web;

using Spanfold;

namespace Spanfold.Artifacts.Internal;

internal static class ComparisonDebugHtmlExporter
{
    private static readonly HtmlEncoder Encoder = HtmlEncoder.Default;

    internal static string Export(ComparisonResult result)
    {
        var builder = new StringBuilder(capacity: 32 * 1024);
        var scale = TimelineScale.Create(result);

        AppendDocumentStart(builder, result);
        AppendHero(builder, result);
        AppendSummary(builder, result);
        AppendTimeline(builder, result, scale);
        AppendSegmentBands(builder, result, scale);
        AppendAlignedSegments(builder, result, scale);
        AppendDiagnostics(builder, result);
        AppendMetadata(builder, result);
        AppendRows(builder, result);
        AppendDocumentEnd(builder);

        return builder.ToString();
    }

    private static void AppendDocumentStart(StringBuilder builder, ComparisonResult result)
    {
        builder
            .AppendLine("<!doctype html>")
            .AppendLine("<html lang=\"en\">")
            .AppendLine("<head>")
            .AppendLine("  <meta charset=\"utf-8\">")
            .AppendLine("  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">")
            .Append("  <title>");
        AppendText(builder, result.Plan.Name);
        builder
            .AppendLine(" - Spanfold debug</title>")
            .AppendLine("  <style>")
            .AppendLine("""
:root {
  color-scheme: light;
  --bg: #F0E9D6;
  --bg-panel: #E8DFC6;
  --bg-card: #F5EFE0;
  --ink: #2B2A26;
  --ink-muted: #6B6659;
  --ink-faint: #9A9484;
  --rule: #D9CFB4;
  --code-bg: #1F1E1A;
  --code-fg: #E8DFC6;
  --code-comment: #9A9484;
  --accent-rust: #B8742A;
  --accent-forest: #3E6B5C;
  --accent-slate: #5E7A9B;
  --accent-amber: #C97A3A;
}

* { box-sizing: border-box; }

body {
  margin: 0;
  background: var(--bg);
  color: var(--ink);
  font: 14px/1.5 ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
}

main {
  width: min(1220px, calc(100% - 32px));
  margin: 0 auto;
  padding: 28px 0 48px;
}

.hero,
.panel {
  background: var(--bg-card);
  border: 1px solid var(--rule);
  border-radius: 8px;
}

.hero {
  padding: 28px;
  border-top: 4px solid var(--accent-forest);
}

.eyebrow {
  color: var(--accent-forest);
  font-size: 12px;
  font-weight: 700;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

h1,
h2,
h3,
p {
  margin: 0;
}

h1 {
  margin-top: 8px;
  font-size: 40px;
  line-height: 1.02;
}

h2 {
  font-size: 22px;
  line-height: 1.2;
}

h3 {
  font-size: 15px;
}

.lead {
  max-width: 760px;
  margin-top: 14px;
  color: var(--ink-muted);
  font-size: 16px;
}

.badges {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-top: 20px;
}

.badge {
  display: inline-flex;
  align-items: center;
  min-height: 28px;
  padding: 4px 10px;
  border: 1px solid var(--rule);
  border-radius: 8px;
  background: var(--bg-panel);
  color: var(--ink);
  font-weight: 650;
}

.badge.valid { border-color: var(--accent-forest); background: color-mix(in srgb, var(--accent-forest) 12%, var(--bg-card)); color: var(--accent-forest); }
.badge.invalid { border-color: var(--accent-rust); background: color-mix(in srgb, var(--accent-rust) 12%, var(--bg-card)); color: var(--accent-rust); }
.badge.live { border-color: var(--accent-amber); background: color-mix(in srgb, var(--accent-amber) 14%, var(--bg-card)); color: var(--accent-rust); }

.grid {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 12px;
  margin-top: 18px;
}

.card {
  min-height: 86px;
  padding: 16px;
  border: 1px solid var(--rule);
  border-radius: 8px;
  background: var(--bg-card);
}

.card .value {
  margin-top: 6px;
  font-size: 30px;
  font-weight: 760;
  line-height: 1;
}

.card .label {
  color: var(--ink-muted);
  font-size: 12px;
  font-weight: 700;
  letter-spacing: 0;
  text-transform: uppercase;
}

.panel {
  margin-top: 18px;
  padding: 22px;
}

.section-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 18px;
}

.section-note {
  max-width: 680px;
  color: var(--ink-muted);
}

.timeline-tools {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 10px;
  margin: 0 0 12px;
  color: var(--ink-muted);
  font-size: 12px;
  font-weight: 700;
}

.timeline-tools input {
  width: auto;
}

body:has(#spanfold-zoom-2x:checked) .timeline-canvas { --timeline-width: 2200px; }
body:has(#spanfold-zoom-5x:checked) .timeline-canvas { --timeline-width: 5500px; }
body:has(#spanfold-zoom-10x:checked) .timeline-canvas { --timeline-width: 11000px; }
body:has(#spanfold-zoom-20x:checked) .timeline-canvas { --timeline-width: 22000px; }

.zoom-presets {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px;
  margin: 0;
  padding: 0;
  border: 0;
}

.zoom-presets legend {
  float: left;
  margin-right: 6px;
  padding: 0;
  font-size: 12px;
  font-weight: 700;
}

.zoom-presets label {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  padding: 2px 8px;
  border: 1px solid var(--rule);
  border-radius: 6px;
  font-size: 11px;
  font-weight: 650;
  cursor: pointer;
}

.zoom-presets label:has(input:checked) {
  border-color: var(--accent-forest);
  background: color-mix(in srgb, var(--accent-forest) 12%, var(--bg-card));
}

.zoom-presets input { width: auto; }

.lane-overflow {
  position: absolute;
  right: 4px;
  top: 4px;
  z-index: 5;
  padding: 1px 7px;
  border: 1px solid var(--rule);
  border-radius: 4px;
  background: var(--bg-card);
  color: var(--ink-muted);
  font-size: 10px;
  font-weight: 650;
  white-space: nowrap;
}

.timeline-shell {
  overflow-x: auto;
  overflow-y: hidden;
  padding-bottom: 10px;
  border: 1px solid color-mix(in srgb, var(--rule) 88%, var(--ink));
  border-radius: 8px;
  background:
    linear-gradient(180deg, color-mix(in srgb, var(--bg-card) 86%, white), var(--bg-card));
  box-shadow:
    inset 0 1px 0 color-mix(in srgb, white 54%, transparent),
    0 10px 24px color-mix(in srgb, var(--ink) 5%, transparent);
}

.timeline-canvas {
  min-width: var(--timeline-width, 1100px);
  padding: 12px;
}

.timeline {
  display: grid;
  gap: 8px;
}

.lane {
  display: grid;
  grid-template-columns: minmax(180px, 250px) minmax(280px, 1fr);
  gap: 8px;
  align-items: center;
}

.lane-title {
  overflow-wrap: anywhere;
  color: var(--ink);
  font-weight: 720;
}

.lane-meta {
  display: block;
  color: var(--ink-muted);
  font-size: 12px;
  font-weight: 500;
}

.track {
  position: relative;
  min-height: 28px;
  overflow: visible;
  border: 1px solid color-mix(in srgb, var(--rule) 72%, var(--ink));
  border-radius: 0;
  background:
    linear-gradient(90deg, color-mix(in srgb, var(--ink) 11%, transparent) 1px, transparent 1px) 0 0 / 12.5% 100%,
    linear-gradient(90deg, transparent 0 99%, color-mix(in srgb, var(--ink) 17%, transparent) 99% 100%) 0 0 / 6.25% 100%,
    color-mix(in srgb, var(--bg-panel) 72%, var(--bg-card));
  box-shadow: none;
}

.track::before {
  position: absolute;
  top: 50%;
  right: 10px;
  left: 10px;
  height: 1px;
  background: color-mix(in srgb, var(--ink) 18%, transparent);
  content: "";
}

.bar {
  position: absolute;
  top: 4px;
  z-index: 2;
  height: 20px;
  min-width: 3px;
  border: 1px solid color-mix(in srgb, var(--ink) 26%, transparent);
  border-radius: 0;
  opacity: 1;
  box-shadow: none;
}

.bar::before,
.bar::after {
  position: absolute;
  top: -8px;
  bottom: -18px;
  width: 0;
  border-left: 1px dotted color-mix(in srgb, var(--ink-muted) 72%, transparent);
  content: "";
}

.bar::before {
  left: -1px;
}

.bar::after {
  right: -1px;
}

.bar.target { background: var(--accent-slate); }
.bar.against { background: var(--accent-forest); }
.bar.overlap { background: var(--accent-forest); }
.bar.residual { background: var(--accent-rust); }
.bar.missing { background: var(--accent-amber); }
.bar.gap { background: var(--ink-faint); }
.bar.open {
  border-style: dashed;
  background-image: repeating-linear-gradient(
    135deg,
    color-mix(in srgb, white 12%, transparent) 0 5px,
    transparent 5px 10px);
}

.boundary-marker {
  position: absolute;
  top: 4px;
  bottom: 4px;
  z-index: 3;
  width: 2px;
  border-radius: 0;
  background: color-mix(in srgb, var(--accent-amber) 78%, var(--ink));
  box-shadow:
    0 0 0 2px color-mix(in srgb, var(--bg-card) 72%, transparent),
    0 0 0 3px color-mix(in srgb, var(--accent-amber) 22%, transparent);
}

.axis {
  display: flex;
  justify-content: space-between;
  margin-top: 12px;
  color: var(--ink-muted);
  font-size: 12px;
}

.legend {
  display: flex;
  flex-wrap: wrap;
  gap: 8px 14px;
  margin-top: 12px;
  color: var(--ink-muted);
  font-size: 12px;
}

.legend-item {
  display: inline-flex;
  align-items: center;
  gap: 6px;
}

.swatch {
  width: 12px;
  height: 12px;
  border-radius: 3px;
}

.band-lane {
  display: grid;
  grid-template-columns: minmax(180px, 250px) minmax(280px, 1fr);
  gap: 8px;
  align-items: center;
}

.band-title {
  overflow-wrap: anywhere;
  font-weight: 720;
}

.band-track {
  position: relative;
  min-height: 32px;
  overflow: visible;
  border: 1px solid color-mix(in srgb, var(--rule) 72%, var(--ink));
  border-radius: 0;
  background:
    linear-gradient(90deg, color-mix(in srgb, var(--ink) 11%, transparent) 1px, transparent 1px) 0 0 / 12.5% 100%,
    color-mix(in srgb, var(--bg-panel) 72%, var(--bg-card));
  box-shadow: none;
}

.band-segment {
  position: absolute;
  top: 4px;
  height: 22px;
  min-width: 4px;
  overflow: hidden;
  padding: 2px 8px;
  border: 1px solid color-mix(in srgb, var(--ink) 14%, transparent);
  border-radius: 0;
  color: var(--ink);
  font-size: 12px;
  font-weight: 700;
  line-height: 18px;
  text-overflow: ellipsis;
  white-space: nowrap;
  box-shadow: none;
}

.band-segment.c0 { background: color-mix(in srgb, var(--accent-slate) 24%, var(--bg-card)); }
.band-segment.c1 { background: color-mix(in srgb, var(--accent-forest) 22%, var(--bg-card)); }
.band-segment.c2 { background: color-mix(in srgb, var(--accent-amber) 24%, var(--bg-card)); }
.band-segment.c3 { background: color-mix(in srgb, var(--accent-rust) 20%, var(--bg-card)); }
.band-segment.c4 { background: var(--bg-panel); }
.band-segment.c5 { background: color-mix(in srgb, var(--ink-faint) 24%, var(--bg-card)); }

.table-wrap {
  overflow-x: auto;
  border: 1px solid var(--rule);
  border-radius: 8px;
}

table {
  width: 100%;
  border-collapse: collapse;
  background: var(--bg-card);
}

th,
td {
  padding: 10px 12px;
  border-bottom: 1px solid var(--rule);
  text-align: left;
  vertical-align: top;
}

th {
  color: var(--ink-muted);
  font-size: 12px;
  font-weight: 750;
  letter-spacing: 0;
  text-transform: uppercase;
}

tr:last-child td { border-bottom: 0; }

.mono {
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", monospace;
  font-size: 12px;
}

.empty {
  padding: 18px;
  border: 1px dashed var(--rule);
  border-radius: 8px;
  color: var(--ink-muted);
  background: var(--bg-card);
}

.severity-error { color: var(--accent-rust); font-weight: 720; }
.severity-warning { color: var(--accent-amber); font-weight: 720; }

@media (max-width: 820px) {
  main { width: min(100% - 20px, 1220px); padding-top: 16px; }
  .hero, .panel { padding: 18px; }
  .grid { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  .timeline-canvas { min-width: var(--timeline-width, 900px); }
}

@media (max-width: 520px) {
  .grid { grid-template-columns: 1fr; }
  h1 { font-size: 30px; }
}
""")
            .AppendLine("  </style>")
            .AppendLine("</head>")
            .AppendLine("<body>")
            .AppendLine("<main>");
    }

    private static void AppendHero(StringBuilder builder, ComparisonResult result)
    {
        builder
            .AppendLine("<section class=\"hero\">")
            .AppendLine("  <div class=\"eyebrow\">Spanfold comparison debug</div>")
            .Append("  <h1>");
        AppendText(builder, result.Plan.Name);
        builder
            .AppendLine("</h1>")
            .AppendLine("  <p class=\"lead\">Inspect which windows were active, where sources overlapped or diverged, and whether live rows are final or provisional.</p>")
            .AppendLine("  <div class=\"badges\">");

        AppendBadge(builder, result.IsValid ? "Valid result" : "Invalid result", result.IsValid ? "valid" : "invalid");

        if (result.EvaluationHorizon.HasValue)
        {
            AppendBadge(builder, "Live horizon " + FormatPoint(result.EvaluationHorizon.Value), "live");
        }

        if (result.KnownAt.HasValue)
        {
            AppendBadge(builder, "Known at " + FormatPoint(result.KnownAt.Value), string.Empty);
        }

        AppendBadge(builder, FormatAxis(result.Plan.Scope?.TimeAxis ?? TemporalAxis.ProcessingPosition), string.Empty);

        builder
            .AppendLine("  </div>")
            .AppendLine("</section>");
    }

    private static void AppendSummary(StringBuilder builder, ComparisonResult result)
    {
        var rowCount = result.OverlapRows.Count
            + result.ResidualRows.Count
            + result.MissingRows.Count
            + result.CoverageRows.Count
            + result.GapRows.Count
            + result.SymmetricDifferenceRows.Count
            + result.ContainmentRows.Count
            + result.LeadLagRows.Count
            + result.AsOfRows.Count;
        var provisionalRows = result.RowFinalities.Count(static row => row.Finality == ComparisonFinality.Provisional);

        builder.AppendLine("<section class=\"grid\" aria-label=\"Comparison summary\">");
        AppendCard(builder, "Selected windows", result.Prepared?.SelectedWindows.Count ?? 0);
        AppendCard(builder, "Normalized windows", result.Prepared?.NormalizedWindows.Count ?? 0);
        AppendCard(builder, "Aligned segments", result.Aligned?.Segments.Count ?? 0);
        AppendCard(builder, "Result rows", rowCount);
        AppendCard(builder, "Diagnostics", result.Diagnostics.Count);
        AppendCard(builder, "Provisional rows", provisionalRows);
        AppendCard(builder, "Comparators", result.ComparatorSummaries.Count);
        AppendCard(builder, "Excluded windows", result.Prepared?.ExcludedWindows.Count ?? 0);
        builder.AppendLine("</section>");
    }

    private static void AppendTimeline(StringBuilder builder, ComparisonResult result, TimelineScale? scale)
    {
        builder
            .AppendLine("<section class=\"panel\">")
            .AppendLine("  <div class=\"section-head\">")
            .AppendLine("    <div>")
            .AppendLine("      <h2>Window Timeline</h2>")
            .AppendLine("      <p class=\"section-note\">Each lane shows normalized windows after selector, scope, known-at, and open-window policy have been applied.</p>")
            .AppendLine("    </div>")
            .AppendLine("  </div>");

        if (result.Prepared?.NormalizedWindows.Count > 0 && scale is not null)
        {
            const int MaxLanes = 200;
            const int MaxBarsPerLane = 200;

            AppendTimelineTools(builder);
            builder
                .AppendLine("  <div class=\"timeline-shell\" data-spanfold-timeline>")
                .AppendLine("    <div class=\"timeline-canvas\">")
                .AppendLine("      <div class=\"timeline\">");

            var allLanes = result.Prepared.NormalizedWindows
                .GroupBy(static record => new TimelineLaneKey(record.Side, record.SelectorName, record.Window.WindowName))
                .OrderBy(static group => group.Key.Side)
                .ThenBy(static group => group.Key.SelectorName, StringComparer.Ordinal)
                .ThenBy(static group => group.Key.WindowName, StringComparer.Ordinal)
                .ToArray();

            for (var i = 0; i < Math.Min(allLanes.Length, MaxLanes); i++)
            {
                var lane = allLanes[i];
                AppendWindowLane(builder, lane.Key, lane.OrderBy(static record => record.Range.Start), lane.Count(), MaxBarsPerLane, scale);
            }

            builder
                .AppendLine("      </div>");
            AppendAxis(builder, scale);
            builder
                .AppendLine("    </div>")
                .AppendLine("  </div>");
            AppendLegend(builder, includeSegments: false);

            if (allLanes.Length > MaxLanes)
            {
                builder
                    .Append("  <div class=\"empty\" style=\"margin-top:18px\">Showing first ")
                    .Append(MaxLanes.ToString(CultureInfo.InvariantCulture))
                    .Append(" of ");
                AppendText(builder, allLanes.Length.ToString(CultureInfo.InvariantCulture));
                builder.AppendLine(" lane groups. Narrow the comparison scope to see more.</div>");
            }

            AppendWindowDetailTable(builder, result);
        }
        else
        {
            builder.AppendLine("  <div class=\"empty\">No normalized windows were available to visualize. Check diagnostics or run a comparison that reaches preparation.</div>");
        }

        builder.AppendLine("</section>");
    }

    private static void AppendWindowLane(
        StringBuilder laneBuilder,
        TimelineLaneKey laneKey,
        IOrderedEnumerable<NormalizedWindowRecord> records,
        int totalRecords,
        int maxBars,
        TimelineScale scale)
    {
        var sideClass = laneKey.Side == ComparisonSide.Target ? "target" : "against";
        laneBuilder
            .AppendLine("    <div class=\"lane\">")
            .Append("      <div class=\"lane-title\">");
        AppendText(laneBuilder, laneKey.Side.ToString());
        laneBuilder.Append(" / ");
        AppendText(laneBuilder, laneKey.SelectorName);
        laneBuilder.Append("<span class=\"lane-meta\">");
        AppendText(laneBuilder, laneKey.WindowName);
        if (totalRecords > maxBars)
        {
            laneBuilder.Append(" — showing ");
            laneBuilder.Append(maxBars.ToString(CultureInfo.InvariantCulture));
            laneBuilder.Append(" of ");
            AppendText(laneBuilder, totalRecords.ToString(CultureInfo.InvariantCulture));
            laneBuilder.Append(" windows");
        }
        laneBuilder
            .AppendLine("</span></div>")
            .AppendLine("      <div class=\"track\">");

        var shown = 0;
        foreach (var record in records)
        {
            if (shown >= maxBars)
            {
                break;
            }

            if (TryGetRangeCss(record.Range, scale, out var left, out var width))
            {
                laneBuilder
                    .Append("        <div class=\"bar ")
                    .Append(sideClass);
                if (record.Range.EndStatus == TemporalRangeEndStatus.OpenAtHorizon || record.Range.EndStatus == TemporalRangeEndStatus.UnknownEnd)
                {
                    laneBuilder.Append(" open");
                }

                laneBuilder
                    .Append("\" style=\"left:")
                    .Append(left)
                    .Append("%;width:")
                    .Append(width)
                    .Append("%\" title=\"");
                AppendAttribute(laneBuilder, FormatWindowTitle(record));
                laneBuilder.AppendLine("\"></div>");
            }

            if (record.Range.End.HasValue
                && record.Window.BoundaryReason is not null
                && TryGetPointCss(record.Range.End.Value, scale, out var boundaryLeft))
            {
                laneBuilder
                    .Append("        <div class=\"boundary-marker\" style=\"left:")
                    .Append(boundaryLeft)
                    .Append("%\" title=\"");
                AppendAttribute(laneBuilder, FormatBoundary(record.Window));
                laneBuilder.AppendLine("\"></div>");
            }

            shown++;
        }

        if (totalRecords > maxBars)
        {
            laneBuilder
                .Append("      <div class=\"lane-overflow\">+")
                .Append((totalRecords - maxBars).ToString(CultureInfo.InvariantCulture))
                .AppendLine(" more</div>");
        }

        laneBuilder
            .AppendLine("      </div>")
            .AppendLine("    </div>");
    }

    private static void AppendTimelineTools(StringBuilder builder)
    {
        builder
            .AppendLine("  <div class=\"timeline-tools\">")
            .AppendLine("    <fieldset class=\"zoom-presets\">")
            .AppendLine("      <legend>Zoom</legend>")
            .AppendLine("      <label><input type=\"radio\" name=\"spanfold-zoom\" value=\"1x\" id=\"spanfold-zoom-1x\" checked> 1&times;</label>")
            .AppendLine("      <label><input type=\"radio\" name=\"spanfold-zoom\" value=\"2x\" id=\"spanfold-zoom-2x\"> 2&times;</label>")
            .AppendLine("      <label><input type=\"radio\" name=\"spanfold-zoom\" value=\"5x\" id=\"spanfold-zoom-5x\"> 5&times;</label>")
            .AppendLine("      <label><input type=\"radio\" name=\"spanfold-zoom\" value=\"10x\" id=\"spanfold-zoom-10x\"> 10&times;</label>")
            .AppendLine("      <label><input type=\"radio\" name=\"spanfold-zoom\" value=\"20x\" id=\"spanfold-zoom-20x\"> 20&times;</label>")
            .AppendLine("    </fieldset>")
            .AppendLine("    <span>Scroll horizontally to inspect dense or long-running lane histories.</span>")
            .AppendLine("  </div>");
    }

    private static void AppendWindowDetailTable(StringBuilder builder, ComparisonResult result)
    {
        if (result.Prepared is null || result.Prepared.NormalizedWindows.Count == 0)
        {
            return;
        }

        builder
            .AppendLine("  <div class=\"table-wrap\" style=\"margin-top:18px\">")
            .AppendLine("    <table>")
            .AppendLine("      <thead>")
            .AppendLine("        <tr><th>Side</th><th>Selector</th><th>Window</th><th>Key</th><th>Range</th><th>Segments</th><th>Tags</th><th>Boundary</th></tr>")
            .AppendLine("      </thead>")
            .AppendLine("      <tbody>");

        foreach (var record in result.Prepared.NormalizedWindows.Take(80))
        {
            builder
                .AppendLine("        <tr>")
                .Append("          <td>");
            AppendText(builder, record.Side.ToString());
            builder.Append("</td><td>");
            AppendText(builder, record.SelectorName);
            builder.Append("</td><td>");
            AppendText(builder, record.Window.WindowName);
            builder.Append("</td><td>");
            AppendText(builder, FormatObject(record.Window.Key));
            builder.Append("</td><td class=\"mono\">");
            AppendText(builder, FormatRange(record.Range));
            builder.Append("</td><td>");
            AppendText(builder, FormatSegments(record.Segments));
            builder.Append("</td><td>");
            AppendText(builder, FormatTags(record.Window.Tags));
            builder.Append("</td><td>");
            AppendText(builder, FormatBoundary(record.Window));
            builder.AppendLine("</td>")
                .AppendLine("        </tr>");
        }

        if (result.Prepared.NormalizedWindows.Count > 80)
        {
            builder
                .AppendLine("        <tr>")
                .Append("          <td colspan=\"8\">Showing first 80 of ");
            AppendText(builder, result.Prepared.NormalizedWindows.Count.ToString(CultureInfo.InvariantCulture));
            builder.AppendLine(" normalized windows.</td>")
                .AppendLine("        </tr>");
        }

        builder
            .AppendLine("      </tbody>")
            .AppendLine("    </table>")
            .AppendLine("  </div>");
    }

    private static void AppendSegmentBands(StringBuilder builder, ComparisonResult result, TimelineScale? scale)
    {
        if (result.Aligned is null || result.Aligned.Segments.Count == 0 || scale is null)
        {
            return;
        }

        var bands = result.Aligned.Segments
            .SelectMany(static segment => segment.Segments.Select(context => new SegmentBand(
                new SegmentBandLaneKey(
                    segment.WindowName,
                    FormatObject(segment.Key),
                    FormatObject(segment.Partition),
                    context.Name,
                    context.ParentName ?? string.Empty),
                FormatObject(context.Value),
                segment.Range)))
            .ToArray();

        if (bands.Length == 0)
        {
            return;
        }

        var laneGroups = bands
            .GroupBy(static band => band.Key)
            .OrderBy(static group => group.Key.WindowName, StringComparer.Ordinal)
            .ThenBy(static group => group.Key.Key, StringComparer.Ordinal)
            .ThenBy(static group => group.Key.Partition, StringComparer.Ordinal)
            .ThenBy(static group => group.Key.ParentName, StringComparer.Ordinal)
            .ThenBy(static group => group.Key.SegmentName, StringComparer.Ordinal)
            .ToArray();

        builder
            .AppendLine("<section class=\"panel\">")
            .AppendLine("  <div class=\"section-head\">")
            .AppendLine("    <div>")
            .AppendLine("      <h2>Segment Context Bands</h2>")
            .AppendLine("      <p class=\"section-note\">Bands show the segment values attached to aligned windows, making phase, period, and other boundary changes visible before inspecting individual rows.</p>")
            .AppendLine("    </div>")
            .AppendLine("  </div>");
        AppendTimelineTools(builder);
        builder
            .AppendLine("  <div class=\"timeline-shell\" data-spanfold-timeline>")
            .AppendLine("    <div class=\"timeline-canvas\">")
            .AppendLine("      <div class=\"timeline\">");

        foreach (var lane in laneGroups.Take(60))
        {
            builder
                .AppendLine("    <div class=\"band-lane\">")
                .Append("      <div class=\"band-title\">");
            AppendText(builder, lane.Key.SegmentName);
            builder.Append("<span class=\"lane-meta\">");
            AppendText(builder, lane.Key.WindowName);
            builder.Append(" / key ");
            AppendText(builder, lane.Key.Key);

            if (!string.IsNullOrEmpty(lane.Key.Partition))
            {
                builder.Append(" / partition ");
                AppendText(builder, lane.Key.Partition);
            }

            if (!string.IsNullOrEmpty(lane.Key.ParentName))
            {
                builder.Append(" / parent ");
                AppendText(builder, lane.Key.ParentName);
            }

            builder
                .AppendLine("</span></div>")
                .AppendLine("      <div class=\"band-track\">");

            foreach (var band in lane.OrderBy(static band => band.Range.Start))
            {
                if (TryGetRangeCss(band.Range, scale, out var left, out var width))
                {
                    builder
                        .Append("        <div class=\"band-segment c")
                        .Append(GetBandColourIndex(lane.Key.SegmentName + "=" + band.Value))
                        .Append("\" style=\"left:")
                        .Append(left)
                        .Append("%;width:")
                        .Append(width)
                        .Append("%\" title=\"");
                    AppendAttribute(builder, FormatBandTitle(lane.Key, band));
                    builder.Append("\">");
                    AppendText(builder, band.Value);
                    builder.AppendLine("</div>");
                }
            }

            builder
                .AppendLine("      </div>")
                .AppendLine("    </div>");
        }

        builder.AppendLine("      </div>");
        AppendAxis(builder, scale);
        builder
            .AppendLine("    </div>")
            .AppendLine("  </div>");

        if (laneGroups.Length > 60)
        {
            builder
                .Append("  <div class=\"empty\" style=\"margin-top:18px\">Showing first 60 of ");
            AppendText(builder, laneGroups.Length.ToString(CultureInfo.InvariantCulture));
            builder.AppendLine(" segment band lanes.</div>");
        }

        builder.AppendLine("</section>");
    }

    private static void AppendAlignedSegments(StringBuilder builder, ComparisonResult result, TimelineScale? scale)
    {
        builder
            .AppendLine("<section class=\"panel\">")
            .AppendLine("  <div class=\"section-head\">")
            .AppendLine("    <div>")
            .AppendLine("      <h2>Aligned Segments</h2>")
            .AppendLine("      <p class=\"section-note\">Segments are the comparison-ready temporal pieces. Their colour shows whether target and comparison windows were both active, target-only, comparison-only, or empty.</p>")
            .AppendLine("    </div>")
            .AppendLine("  </div>");

        if (result.Aligned?.Segments.Count > 0 && scale is not null)
        {
            const int MaxSegmentLanes = 200;
            const int MaxSegmentsPerLane = 200;

            AppendTimelineTools(builder);
            builder
                .AppendLine("  <div class=\"timeline-shell\" data-spanfold-timeline>")
                .AppendLine("    <div class=\"timeline-canvas\">")
                .AppendLine("      <div class=\"timeline\">");

            var allLanes = result.Aligned.Segments
                .GroupBy(static segment => new SegmentLaneKey(
                    segment.WindowName,
                    FormatObject(segment.Key),
                    FormatObject(segment.Partition),
                    FormatSegments(segment.Segments)))
                .OrderBy(static group => group.Key.WindowName, StringComparer.Ordinal)
                .ThenBy(static group => group.Key.Key, StringComparer.Ordinal)
                .ThenBy(static group => group.Key.Partition, StringComparer.Ordinal)
                .ThenBy(static group => group.Key.Segments, StringComparer.Ordinal)
                .ToArray();

            for (var i = 0; i < Math.Min(allLanes.Length, MaxSegmentLanes); i++)
            {
                var lane = allLanes[i];
                var totalSegments = lane.Count();
                builder
                    .AppendLine("    <div class=\"lane\">")
                    .Append("      <div class=\"lane-title\">");
                AppendText(builder, lane.Key.WindowName);
                builder.Append("<span class=\"lane-meta\">key ");
                AppendText(builder, lane.Key.Key);
                if (!string.IsNullOrEmpty(lane.Key.Partition))
                {
                    builder.Append(" / partition ");
                    AppendText(builder, lane.Key.Partition);
                }

                if (!string.IsNullOrEmpty(lane.Key.Segments))
                {
                    builder.Append(" / ");
                    AppendText(builder, lane.Key.Segments);
                }

                if (totalSegments > MaxSegmentsPerLane)
                {
                    builder.Append(" — showing ");
                    builder.Append(MaxSegmentsPerLane.ToString(CultureInfo.InvariantCulture));
                    builder.Append(" of ");
                    AppendText(builder, totalSegments.ToString(CultureInfo.InvariantCulture));
                    builder.Append(" segments");
                }

                builder
                    .AppendLine("</span></div>")
                    .AppendLine("      <div class=\"track\">");

                var shown = 0;
                foreach (var segment in lane.OrderBy(static segment => segment.Range.Start))
                {
                    if (shown >= MaxSegmentsPerLane)
                    {
                        break;
                    }

                    if (TryGetRangeCss(segment.Range, scale, out var left, out var width))
                    {
                        builder
                            .Append("        <div class=\"bar ")
                            .Append(GetSegmentClass(segment))
                            .Append("\" style=\"left:")
                            .Append(left)
                            .Append("%;width:")
                            .Append(width)
                            .Append("%\" title=\"");
                        AppendAttribute(builder, FormatSegmentTitle(segment));
                        builder.AppendLine("\"></div>");
                    }

                    shown++;
                }

                if (totalSegments > MaxSegmentsPerLane)
                {
                    builder
                        .Append("      <div class=\"lane-overflow\">+")
                        .Append((totalSegments - MaxSegmentsPerLane).ToString(CultureInfo.InvariantCulture))
                        .AppendLine(" more</div>");
                }

                builder
                    .AppendLine("      </div>")
                    .AppendLine("    </div>");
            }

            builder.AppendLine("      </div>");
            AppendAxis(builder, scale);
            builder
                .AppendLine("    </div>")
                .AppendLine("  </div>");
            AppendLegend(builder, includeSegments: true);

            if (allLanes.Length > MaxSegmentLanes)
            {
                builder
                    .Append("  <div class=\"empty\" style=\"margin-top:18px\">Showing first ")
                    .Append(MaxSegmentLanes.ToString(CultureInfo.InvariantCulture))
                    .Append(" of ");
                AppendText(builder, allLanes.Length.ToString(CultureInfo.InvariantCulture));
                builder.AppendLine(" segment lane groups. Narrow the comparison scope to see more.</div>");
            }

            AppendSegmentTable(builder, result);
        }
        else
        {
            builder.AppendLine("  <div class=\"empty\">No aligned segments were available to visualize. This usually means validation stopped execution before alignment.</div>");
        }

        builder.AppendLine("</section>");
    }

    private static void AppendSegmentTable(StringBuilder builder, ComparisonResult result)
    {
        if (result.Aligned is null || result.Aligned.Segments.Count == 0)
        {
            return;
        }

        builder
            .AppendLine("  <div class=\"table-wrap\" style=\"margin-top:18px\">")
            .AppendLine("    <table>")
            .AppendLine("      <thead>")
            .AppendLine("        <tr><th>Kind</th><th>Window</th><th>Key</th><th>Segments</th><th>Range</th><th>Target IDs</th><th>Against IDs</th></tr>")
            .AppendLine("      </thead>")
            .AppendLine("      <tbody>");

        foreach (var segment in result.Aligned.Segments.Take(80))
        {
            builder
                .AppendLine("        <tr>")
                .Append("          <td>");
            AppendText(builder, GetSegmentLabel(segment));
            builder.Append("</td><td>");
            AppendText(builder, segment.WindowName);
            builder.Append("</td><td>");
            AppendText(builder, FormatObject(segment.Key));
            builder.Append("</td><td>");
            AppendText(builder, FormatSegments(segment.Segments));
            builder.Append("</td><td class=\"mono\">");
            AppendText(builder, FormatRange(segment.Range));
            builder.Append("</td><td class=\"mono\">");
            AppendText(builder, FormatIds(segment.TargetRecordIds));
            builder.Append("</td><td class=\"mono\">");
            AppendText(builder, FormatIds(segment.AgainstRecordIds));
            builder.AppendLine("</td>")
                .AppendLine("        </tr>");
        }

        if (result.Aligned.Segments.Count > 80)
        {
            builder
                .AppendLine("        <tr>")
                    .Append("          <td colspan=\"7\">Showing first 80 of ");
            AppendText(builder, result.Aligned.Segments.Count.ToString(CultureInfo.InvariantCulture));
            builder.AppendLine(" aligned segments.</td>")
                .AppendLine("        </tr>");
        }

        builder
            .AppendLine("      </tbody>")
            .AppendLine("    </table>")
            .AppendLine("  </div>");
    }

    private static void AppendDiagnostics(StringBuilder builder, ComparisonResult result)
    {
        builder
            .AppendLine("<section class=\"panel\">")
            .AppendLine("  <div class=\"section-head\">")
            .AppendLine("    <div>")
            .AppendLine("      <h2>Diagnostics</h2>")
            .AppendLine("      <p class=\"section-note\">Diagnostics explain validation and execution concerns that affect whether the result can be trusted.</p>")
            .AppendLine("    </div>")
            .AppendLine("  </div>");

        if (result.Diagnostics.Count == 0)
        {
            builder.AppendLine("  <div class=\"empty\">No diagnostics were emitted.</div>");
        }
        else
        {
            builder
                .AppendLine("  <div class=\"table-wrap\">")
                .AppendLine("    <table>")
                .AppendLine("      <thead><tr><th>Severity</th><th>Code</th><th>Path</th><th>Message</th></tr></thead>")
                .AppendLine("      <tbody>");

            foreach (var diagnostic in result.Diagnostics)
            {
                builder
                    .AppendLine("        <tr>")
                    .Append("          <td class=\"severity-")
                    .Append(diagnostic.Severity == ComparisonPlanDiagnosticSeverity.Error ? "error" : "warning")
                    .Append("\">");
                AppendText(builder, diagnostic.Severity.ToString());
                builder.Append("</td><td>");
                AppendText(builder, diagnostic.Code.ToString());
                builder.Append("</td><td class=\"mono\">");
                AppendText(builder, diagnostic.Path);
                builder.Append("</td><td>");
                AppendText(builder, diagnostic.Message);
                builder.AppendLine("</td>")
                    .AppendLine("        </tr>");
            }

            builder
                .AppendLine("      </tbody>")
                .AppendLine("    </table>")
                .AppendLine("  </div>");
        }

        builder.AppendLine("</section>");
    }

    private static void AppendMetadata(StringBuilder builder, ComparisonResult result)
    {
        builder
            .AppendLine("<section class=\"panel\">")
            .AppendLine("  <div class=\"section-head\">")
            .AppendLine("    <div>")
            .AppendLine("      <h2>Metadata</h2>")
            .AppendLine("      <p class=\"section-note\">Compact extension metadata explains derived artifacts such as cohort activity evidence.</p>")
            .AppendLine("    </div>")
            .AppendLine("  </div>");

        if (result.ExtensionMetadata.Count == 0)
        {
            builder.AppendLine("  <div class=\"empty\">No extension metadata was emitted.</div>");
        }
        else
        {
            builder
                .AppendLine("  <div class=\"table-wrap\">")
                .AppendLine("    <table>")
                .AppendLine("      <thead><tr><th>Extension</th><th>Key</th><th>Value</th></tr></thead>")
                .AppendLine("      <tbody>");

            foreach (var item in result.ExtensionMetadata.Take(120))
            {
                builder
                    .AppendLine("        <tr>")
                    .Append("          <td>");
                AppendText(builder, item.ExtensionId);
                builder.Append("</td><td class=\"mono\">");
                AppendText(builder, item.Key);
                builder.Append("</td><td>");
                AppendText(builder, item.Value);
                builder.AppendLine("</td>")
                    .AppendLine("        </tr>");
            }

            if (result.ExtensionMetadata.Count > 120)
            {
                builder
                    .AppendLine("        <tr>")
                    .Append("          <td colspan=\"3\">Showing first 120 of ");
                AppendText(builder, result.ExtensionMetadata.Count.ToString(CultureInfo.InvariantCulture));
                builder.AppendLine(" metadata entries.</td>")
                    .AppendLine("        </tr>");
            }

            builder
                .AppendLine("      </tbody>")
                .AppendLine("    </table>")
                .AppendLine("  </div>");
        }

        builder.AppendLine("</section>");
    }

    private static void AppendRows(StringBuilder builder, ComparisonResult result)
    {
        builder
            .AppendLine("<section class=\"panel\">")
            .AppendLine("  <div class=\"section-head\">")
            .AppendLine("    <div>")
            .AppendLine("      <h2>Rows And Finality</h2>")
            .AppendLine("      <p class=\"section-note\">Comparator row counts show which analytical questions emitted evidence. Finality rows explain which live results may change after open windows close.</p>")
            .AppendLine("    </div>")
            .AppendLine("  </div>")
            .AppendLine("  <div class=\"table-wrap\">")
            .AppendLine("    <table>")
            .AppendLine("      <thead><tr><th>Row family</th><th>Count</th></tr></thead>")
            .AppendLine("      <tbody>");

        AppendRowCount(builder, "overlap", result.OverlapRows.Count);
        AppendRowCount(builder, "residual", result.ResidualRows.Count);
        AppendRowCount(builder, "missing", result.MissingRows.Count);
        AppendRowCount(builder, "coverage", result.CoverageRows.Count);
        AppendRowCount(builder, "gap", result.GapRows.Count);
        AppendRowCount(builder, "symmetric-difference", result.SymmetricDifferenceRows.Count);
        AppendRowCount(builder, "containment", result.ContainmentRows.Count);
        AppendRowCount(builder, "lead-lag", result.LeadLagRows.Count);
        AppendRowCount(builder, "as-of", result.AsOfRows.Count);

        builder
            .AppendLine("      </tbody>")
            .AppendLine("    </table>")
            .AppendLine("  </div>");

        if (result.RowFinalities.Count > 0)
        {
            builder
                .AppendLine("  <div class=\"table-wrap\" style=\"margin-top:18px\">")
                .AppendLine("    <table>")
                .AppendLine("      <thead><tr><th>Row</th><th>Finality</th><th>Reason</th><th>Version</th></tr></thead>")
                .AppendLine("      <tbody>");

            foreach (var row in result.RowFinalities.Take(120))
            {
                builder
                    .AppendLine("        <tr>")
                    .Append("          <td class=\"mono\">");
                AppendText(builder, row.RowType + ":" + row.RowId);
                builder.Append("</td><td>");
                AppendText(builder, row.Finality.ToString());
                builder.Append("</td><td>");
                AppendText(builder, row.Reason);
                builder.Append("</td><td>");
                AppendText(builder, row.Version.ToString(CultureInfo.InvariantCulture));
                builder.AppendLine("</td>")
                    .AppendLine("        </tr>");
            }

            if (result.RowFinalities.Count > 120)
            {
                builder
                    .AppendLine("        <tr>")
                    .Append("          <td colspan=\"4\">Showing first 120 of ");
                AppendText(builder, result.RowFinalities.Count.ToString(CultureInfo.InvariantCulture));
                builder.AppendLine(" finality entries.</td>")
                    .AppendLine("        </tr>");
            }

            builder
                .AppendLine("      </tbody>")
                .AppendLine("    </table>")
                .AppendLine("  </div>");
        }
        else
        {
            builder.AppendLine("  <div class=\"empty\" style=\"margin-top:18px\">No row finality metadata was emitted.</div>");
        }

        builder.AppendLine("</section>");
    }

    private static void AppendDocumentEnd(StringBuilder builder)
    {
        builder
            .AppendLine("</main>")
            .AppendLine("</body>")
            .AppendLine("</html>");
    }

    private static void AppendBadge(StringBuilder builder, string text, string cssClass)
    {
        builder.Append("    <span class=\"badge");
        if (!string.IsNullOrEmpty(cssClass))
        {
            builder.Append(' ').Append(cssClass);
        }

        builder.Append("\">");
        AppendText(builder, text);
        builder.AppendLine("</span>");
    }

    private static void AppendCard(StringBuilder builder, string label, int value)
    {
        builder
            .AppendLine("  <div class=\"card\">")
            .Append("    <div class=\"label\">");
        AppendText(builder, label);
        builder.Append("</div><div class=\"value\">");
        AppendText(builder, value.ToString(CultureInfo.InvariantCulture));
        builder
            .AppendLine("</div>")
            .AppendLine("  </div>");
    }

    private static void AppendAxis(StringBuilder builder, TimelineScale scale)
    {
        builder
            .AppendLine("  <div class=\"axis\">")
            .Append("    <span>");
        AppendText(builder, FormatScalar(scale.Axis, scale.Min));
        builder.Append("</span><span>");
        AppendText(builder, FormatScalar(scale.Axis, scale.Max));
        builder
            .AppendLine("</span>")
            .AppendLine("  </div>");
    }

    private static void AppendLegend(StringBuilder builder, bool includeSegments)
    {
        builder
            .AppendLine("  <div class=\"legend\">")
            .AppendLine("    <span class=\"legend-item\"><span class=\"swatch\" style=\"background:var(--accent-slate)\"></span>Target window</span>")
            .AppendLine("    <span class=\"legend-item\"><span class=\"swatch\" style=\"background:var(--accent-forest)\"></span>Comparison window</span>");

        if (includeSegments)
        {
            builder
                .AppendLine("    <span class=\"legend-item\"><span class=\"swatch\" style=\"background:var(--accent-forest)\"></span>Overlap</span>")
                .AppendLine("    <span class=\"legend-item\"><span class=\"swatch\" style=\"background:var(--accent-rust)\"></span>Target only</span>")
                .AppendLine("    <span class=\"legend-item\"><span class=\"swatch\" style=\"background:var(--accent-amber)\"></span>Comparison only</span>")
                .AppendLine("    <span class=\"legend-item\"><span class=\"swatch\" style=\"background:var(--ink-faint)\"></span>Gap</span>");
        }

        builder.AppendLine("  </div>");
    }

    private static void AppendRowCount(StringBuilder builder, string rowFamily, int count)
    {
        builder
            .AppendLine("        <tr>")
            .Append("          <td>");
        AppendText(builder, rowFamily);
        builder.Append("</td><td>");
        AppendText(builder, count.ToString(CultureInfo.InvariantCulture));
        builder.AppendLine("</td>")
            .AppendLine("        </tr>");
    }

    private static bool TryGetRangeCss(
        TemporalRange range,
        TimelineScale scale,
        out string left,
        out string width)
    {
        if (!TryGetScalar(range.Start, out var start))
        {
            left = string.Empty;
            width = string.Empty;
            return false;
        }

        long end;
        if (range.End.HasValue && TryGetScalar(range.End.Value, out var closedEnd))
        {
            end = closedEnd;
        }
        else
        {
            end = Math.Max(start + 1, scale.Max);
        }

        var span = Math.Max(1d, scale.Max - scale.Min);
        var leftValue = ((start - scale.Min) / span) * 100d;
        var widthValue = Math.Max(0.35d, ((Math.Max(end, start + 1) - start) / span) * 100d);

        left = FormatPercent(leftValue);
        width = FormatPercent(Math.Min(100d - leftValue, widthValue));
        return true;
    }

    private static bool TryGetPointCss(
        TemporalPoint point,
        TimelineScale scale,
        out string left)
    {
        if (!TryGetScalar(point, out var scalar))
        {
            left = string.Empty;
            return false;
        }

        var span = Math.Max(1d, scale.Max - scale.Min);
        left = FormatPercent(((scalar - scale.Min) / span) * 100d);
        return true;
    }

    private static string FormatWindowTitle(NormalizedWindowRecord record)
    {
        return record.Window.WindowName
            + " / key " + FormatObject(record.Window.Key)
            + " / source " + FormatObject(record.Window.Source)
            + " / " + FormatSegments(record.Segments)
            + " / tags " + FormatTags(record.Window.Tags)
            + " / boundary " + FormatBoundary(record.Window)
            + " / " + FormatRange(record.Range)
            + " / id " + ShortId(record.RecordId);
    }

    private static string FormatSegmentTitle(AlignedSegment segment)
    {
        return GetSegmentLabel(segment)
            + " / " + segment.WindowName
            + " / key " + FormatObject(segment.Key)
            + " / " + FormatSegments(segment.Segments)
            + " / " + FormatRange(segment.Range);
    }

    private static string FormatBandTitle(SegmentBandLaneKey key, SegmentBand band)
    {
        return key.SegmentName
            + "=" + band.Value
            + " / " + key.WindowName
            + " / key " + key.Key
            + (string.IsNullOrEmpty(key.ParentName) ? string.Empty : " / parent " + key.ParentName)
            + " / " + FormatRange(band.Range);
    }

    private static int GetBandColourIndex(string value)
    {
        unchecked
        {
            var hash = 17;
            for (var i = 0; i < value.Length; i++)
            {
                hash = (hash * 31) + value[i];
            }

            return (hash & int.MaxValue) % 6;
        }
    }

    private static string GetSegmentClass(AlignedSegment segment)
    {
        return (segment.TargetRecordIds.Count > 0, segment.AgainstRecordIds.Count > 0) switch
        {
            (true, true) => "overlap",
            (true, false) => "residual",
            (false, true) => "missing",
            _ => "gap"
        };
    }

    private static string GetSegmentLabel(AlignedSegment segment)
    {
        return (segment.TargetRecordIds.Count > 0, segment.AgainstRecordIds.Count > 0) switch
        {
            (true, true) => "overlap",
            (true, false) => "target only",
            (false, true) => "comparison only",
            _ => "gap"
        };
    }

    private static string FormatRange(TemporalRange range)
    {
        return "["
            + FormatPoint(range.Start)
            + ", "
            + (range.End.HasValue ? FormatPoint(range.End.Value) : "open")
            + ") "
            + range.EndStatus;
    }

    private static string FormatPoint(TemporalPoint point)
    {
        return point.Axis switch
        {
            TemporalAxis.ProcessingPosition => "pos " + point.Position.ToString(CultureInfo.InvariantCulture),
            TemporalAxis.Timestamp => point.Timestamp.ToUniversalTime().ToString("O", CultureInfo.InvariantCulture),
            _ => "unknown"
        };
    }

    private static string FormatScalar(TemporalAxis axis, long scalar)
    {
        return axis switch
        {
            TemporalAxis.ProcessingPosition => "pos " + scalar.ToString(CultureInfo.InvariantCulture),
            TemporalAxis.Timestamp => new DateTimeOffset(scalar, TimeSpan.Zero).ToString("O", CultureInfo.InvariantCulture),
            _ => scalar.ToString(CultureInfo.InvariantCulture)
        };
    }

    private static string FormatAxis(TemporalAxis axis)
    {
        return axis switch
        {
            TemporalAxis.ProcessingPosition => "processing position",
            TemporalAxis.Timestamp => "event timestamp",
            _ => "unknown axis"
        };
    }

    private static string FormatIds(IReadOnlyList<WindowRecordId> ids)
    {
        if (ids.Count == 0)
        {
            return "-";
        }

        return string.Join(", ", ids.Select(ShortId));
    }

    private static string FormatSegments(IReadOnlyList<WindowSegment> segments)
    {
        if (segments.Count == 0)
        {
            return string.Empty;
        }

        return string.Join(
            ", ",
            segments.Select(static segment => segment.Name + "=" + FormatObject(segment.Value)));
    }

    private static string FormatTags(IReadOnlyList<WindowTag> tags)
    {
        if (tags.Count == 0)
        {
            return string.Empty;
        }

        return string.Join(
            ", ",
            tags.Select(static tag => tag.Name + "=" + FormatObject(tag.Value)));
    }

    private static string FormatBoundary(WindowRecord window)
    {
        if (window.BoundaryReason is null)
        {
            return string.Empty;
        }

        if (window.BoundaryChanges.Count == 0)
        {
            return window.BoundaryReason.ToString() ?? string.Empty;
        }

        return window.BoundaryReason
            + ": "
            + string.Join(
                ", ",
                window.BoundaryChanges.Select(static change =>
                    change.SegmentName
                    + " "
                    + FormatObject(change.PreviousValue)
                    + " -> "
                    + FormatObject(change.CurrentValue)));
    }

    private static string ShortId(WindowRecordId id)
    {
        return id.Value.Length <= 12 ? id.Value : id.Value[..12];
    }

    private static string FormatObject(object? value)
    {
        return value switch
        {
            null => string.Empty,
            IFormattable formattable => formattable.ToString(null, CultureInfo.InvariantCulture) ?? string.Empty,
            _ => value.ToString() ?? string.Empty
        };
    }

    private static string FormatPercent(double value)
    {
        return Math.Clamp(value, 0d, 100d).ToString("0.###", CultureInfo.InvariantCulture);
    }

    private static bool TryGetScalar(TemporalPoint point, out long scalar)
    {
        switch (point.Axis)
        {
            case TemporalAxis.ProcessingPosition:
                scalar = point.Position;
                return true;
            case TemporalAxis.Timestamp:
                scalar = point.Timestamp.ToUniversalTime().Ticks;
                return true;
            default:
                scalar = 0;
                return false;
        }
    }

    private static void AppendText(StringBuilder builder, object? value)
    {
        builder.Append(Encoder.Encode(Convert.ToString(value, CultureInfo.InvariantCulture) ?? string.Empty));
    }

    private static void AppendAttribute(StringBuilder builder, string value)
    {
        builder.Append(Encoder.Encode(value));
    }

    private readonly record struct TimelineLaneKey(ComparisonSide Side, string SelectorName, string WindowName);

    private readonly record struct SegmentLaneKey(string WindowName, string Key, string Partition, string Segments);

    private readonly record struct SegmentBandLaneKey(
        string WindowName,
        string Key,
        string Partition,
        string SegmentName,
        string ParentName);

    private readonly record struct SegmentBand(SegmentBandLaneKey Key, string Value, TemporalRange Range);

    private sealed record TimelineScale(TemporalAxis Axis, long Min, long Max)
    {
        internal static TimelineScale? Create(ComparisonResult result)
        {
            var candidates = new List<TemporalRange>();

            if (result.Prepared is not null)
            {
                candidates.AddRange(result.Prepared.NormalizedWindows.Select(static record => record.Range));
            }

            if (result.Aligned is not null)
            {
                candidates.AddRange(result.Aligned.Segments.Select(static segment => segment.Range));
            }

            if (candidates.Count == 0)
            {
                return null;
            }

            long? min = null;
            long? max = null;
            TemporalAxis axis = TemporalAxis.Unknown;

            foreach (var range in candidates)
            {
                if (!TryGetScalar(range.Start, out var start))
                {
                    continue;
                }

                if (axis == TemporalAxis.Unknown)
                {
                    axis = range.Axis;
                }

                var effectiveEnd = start + 1;
                if (range.End.HasValue && TryGetScalar(range.End.Value, out var end))
                {
                    effectiveEnd = Math.Max(end, start + 1);
                }

                min = min.HasValue ? Math.Min(min.Value, start) : start;
                max = max.HasValue ? Math.Max(max.Value, effectiveEnd) : effectiveEnd;
            }

            if (!min.HasValue || !max.HasValue)
            {
                return null;
            }

            if (max.Value <= min.Value)
            {
                max = min.Value + 1;
            }

            return new TimelineScale(axis, min.Value, max.Value);
        }
    }
}
