/* sf-diagrams.js — Spanfold v2 diagram system
 * Translates the block-style design from spanfold-graph-designs into
 * inline SVG for the static docs site.  Every [data-fig] element on the
 * page is replaced at DOMContentLoaded with the matching figure.
 */
(function () {
  'use strict';

  // ── Palette ────────────────────────────────────────────────────────────
  var SF = {
    paper:    '#faf8f4',
    panel:    '#f5f1e8',
    ink:      '#1a1714',
    ink2:     '#5a544c',
    ink3:     '#8a837a',
    rule:     '#d8d0c2',
    ruleSoft: '#e8e2d4',

    target:      { fill: '#2a2520', edge: '#000000', ink: '#ffffff' },
    against:     { fill: '#c9a87a', edge: '#8f6f3e', ink: '#2a2520' },
    overlap:     { fill: '#3d6b4a', edge: '#224a30', ink: '#ffffff' },
    residual:    { fill: '#b85a3a', edge: '#843a1c', ink: '#ffffff' },
    missing:     { fill: '#d4b860', edge: '#8f7524', ink: '#2a2520' },
    coverage:    { fill: '#6b8e9a', edge: '#40606a', ink: '#ffffff' },
    provisional: { fill: '#e8dcc0', edge: '#8f6f3e', ink: '#2a2520' },
    neutral:     { fill: '#e8e2d4', edge: '#b8ae99', ink: '#5a544c' },

    fontUI:   'ui-sans-serif, -apple-system, system-ui, sans-serif',
    fontMono: '"JetBrains Mono", "SF Mono", ui-monospace, Menlo, monospace',
  };

  // ── SVG defs (hatch pattern) ───────────────────────────────────────────
  var DEFS = '<defs>'
    + '<pattern id="sf-hatch" patternUnits="userSpaceOnUse" width="6" height="6" patternTransform="rotate(45)">'
    + '<rect width="6" height="6" fill="#e8dcc0"/>'
    + '<line x1="0" y1="0" x2="0" y2="6" stroke="#8f6f3e" stroke-width="1.6"/>'
    + '</pattern>'
    + '</defs>';

  // ── Utility ────────────────────────────────────────────────────────────
  function esc(s) {
    return String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
  }

  // ── Primitives ─────────────────────────────────────────────────────────

  // Block — two-tone window bar with solid left edge and bracket right edge
  function Block(x, w, opts) {
    opts = opts || {};
    var h   = opts.h   || 34;
    var c   = opts.color || SF.target;
    var lbl = opts.label || '';
    var open = opts.halfOpen !== false;
    var edge = 3;
    return '<g transform="translate(' + x + ',0)">'
      + '<rect x="1" y="' + (h+1) + '" width="' + w + '" height="3" fill="rgba(26,23,20,0.12)"/>'
      + '<rect x="0" y="' + (h-edge) + '" width="' + w + '" height="' + edge + '" fill="' + c.edge + '"/>'
      + '<rect x="0" y="0" width="' + w + '" height="' + (h-edge) + '" fill="' + c.fill + '"/>'
      + '<rect x="2" y="1" width="' + (w-4) + '" height="1" fill="rgba(255,255,255,0.18)"/>'
      + '<rect x="0" y="-2" width="2" height="' + (h+4) + '" fill="' + c.edge + '"/>'
      + (open
          ? '<rect x="' + (w-2) + '" y="-2" width="2" height="5" fill="' + c.edge + '"/>'
            + '<rect x="' + (w-2) + '" y="' + (h-3) + '" width="2" height="5" fill="' + c.edge + '"/>'
          : '')
      + (lbl
          ? '<text x="' + (w/2) + '" y="' + (h/2+1) + '" text-anchor="middle" dominant-baseline="middle"'
            + ' font-family="' + SF.fontUI + '" font-size="12" font-weight="600" fill="' + c.ink + '">'
            + esc(lbl) + '</text>'
          : '')
      + '</g>';
  }

  // Hatched provisional block
  function Provisional(x, w, opts) {
    opts = opts || {};
    var h = opts.h || 34;
    var edge = 3;
    var lbl = opts.label || '';
    return '<g transform="translate(' + x + ',0)">'
      + '<rect x="1" y="' + (h+1) + '" width="' + w + '" height="3" fill="rgba(26,23,20,0.12)"/>'
      + '<rect x="0" y="' + (h-edge) + '" width="' + w + '" height="' + edge + '" fill="#8f6f3e"/>'
      + '<rect x="0" y="0" width="' + w + '" height="' + (h-edge) + '" fill="url(#sf-hatch)"/>'
      + '<rect x="0" y="-2" width="2" height="' + (h+4) + '" fill="#8f6f3e"/>'
      + '<rect x="' + (w-2) + '" y="-2" width="2" height="5" fill="#8f6f3e"/>'
      + '<rect x="' + (w-2) + '" y="' + (h-3) + '" width="2" height="5" fill="#8f6f3e"/>'
      + (lbl
          ? '<text x="' + (w/2) + '" y="' + (h/2+1) + '" text-anchor="middle" dominant-baseline="middle"'
            + ' font-family="' + SF.fontUI + '" font-size="12" font-weight="600" fill="' + SF.ink2 + '">'
            + esc(lbl) + '</text>'
          : '')
      + '</g>';
  }

  // Lane — labelled row with baseline rule; returns SVG g string
  function Lane(y, label, sublabel, width, inner, strong) {
    strong = strong !== false;
    return '<g transform="translate(0,' + y + ')">'
      + '<text x="-14" y="20" text-anchor="end" font-family="' + SF.fontUI + '" font-size="12"'
        + ' font-weight="' + (strong ? 700 : 500) + '" fill="' + SF.ink + '">' + esc(label) + '</text>'
      + (sublabel
          ? '<text x="-14" y="33" text-anchor="end" font-family="' + SF.fontMono + '" font-size="9" fill="' + SF.ink3 + '"'
            + ' letter-spacing="0.4" style="text-transform:uppercase">' + esc(sublabel) + '</text>'
          : '')
      + '<line x1="0" y1="17" x2="' + width + '" y2="17" stroke="' + SF.ruleSoft + '" stroke-width="1"/>'
      + (inner || '')
      + '</g>';
  }

  // Ruler — shared position axis
  function Ruler(positions, total, width) {
    var u = width / total;
    var minors = '';
    for (var i = 0; i <= total; i++) {
      minors += '<line x1="' + (i*u) + '" y1="0" x2="' + (i*u) + '" y2="3" stroke="' + SF.rule + '" stroke-width="1"/>';
    }
    var majors = positions.map(function(p) {
      return '<g transform="translate(' + (p*u) + ',0)">'
        + '<line y1="-2" y2="6" stroke="' + SF.ink2 + '" stroke-width="1.4"/>'
        + '<text y="20" text-anchor="middle" font-family="' + SF.fontMono + '" font-size="11" fill="' + SF.ink2 + '" font-weight="600">' + p + '</text>'
        + '</g>';
    }).join('');
    return '<g>'
      + '<line x1="0" y1="0" x2="' + width + '" y2="0" stroke="' + SF.rule + '" stroke-width="1.2"/>'
      + minors + majors + '</g>';
  }

  // Mark — numbered callout circle
  function Mark(x, y, n, color) {
    color = color || SF.ink;
    return '<g transform="translate(' + x + ',' + y + ')">'
      + '<circle r="10" fill="' + color + '" stroke="#faf8f4" stroke-width="2"/>'
      + '<text y="1" text-anchor="middle" dominant-baseline="middle"'
        + ' font-family="' + SF.fontMono + '" font-size="11" font-weight="700" fill="#faf8f4">' + esc(n) + '</text>'
      + '</g>';
  }

  // EventDot — small dot + tick + label
  function EventDot(x, label, direction, color) {
    color = color || SF.ink;
    var up = direction !== 'down';
    return '<g transform="translate(' + x + ',0)">'
      + '<line x1="0" y1="' + (up ? -4 : 4) + '" x2="0" y2="' + (up ? -16 : 16) + '" stroke="' + color + '" stroke-width="1.2"/>'
      + '<circle cx="0" cy="' + (up ? -20 : 20) + '" r="3.5" fill="' + color + '"/>'
      + '<text x="0" y="' + (up ? -30 : 38) + '" text-anchor="middle"'
        + ' font-family="' + SF.fontUI + '" font-size="11" font-weight="600" fill="' + color + '">' + esc(label) + '</text>'
      + '</g>';
  }

  // Dashed guide line
  function Guide(x, top, bottom) {
    top = top || 0; bottom = bottom || 260;
    return '<line x1="' + x + '" y1="' + top + '" x2="' + x + '" y2="' + bottom
      + '" stroke="' + SF.rule + '" stroke-width="1" stroke-dasharray="2 3"/>';
  }

  // Horizon line + HORIZON tag
  function Horizon(x, top, bottom) {
    top = top || -20; bottom = bottom || 200;
    return '<g>'
      + '<rect x="' + (x-36) + '" y="' + (top-28) + '" width="72" height="18" fill="' + SF.residual.fill + '"/>'
      + '<text x="' + x + '" y="' + (top-16) + '" text-anchor="middle"'
        + ' font-family="' + SF.fontMono + '" font-size="9.5" fill="#fff" font-weight="700" letter-spacing="1">HORIZON</text>'
      + '<line x1="' + x + '" y1="' + top + '" x2="' + x + '" y2="' + bottom
        + '" stroke="' + SF.residual.fill + '" stroke-width="1.6" stroke-dasharray="4 3"/>'
      + '</g>';
  }

  // Divider rule between input lanes and derived rows, with label
  function Emits(y, width, label) {
    label = label || 'Emits \u2192';
    return '<g transform="translate(0,' + y + ')">'
      + '<line x1="0" y1="0" x2="' + width + '" y2="0" stroke="' + SF.rule + '"/>'
      + '<text x="-14" y="16" text-anchor="end" font-family="' + SF.fontMono + '" font-size="9" letter-spacing="1.4"'
        + ' font-weight="600" fill="' + SF.ink3 + '" style="text-transform:uppercase">' + esc(label) + '</text>'
      + '</g>';
  }

  // ── Key strip (HTML, not SVG) ──────────────────────────────────────────
  function keyItem(it) {
    var swatch = '';
    if (it.swatch) {
      swatch = '<div style="flex:0 0 auto;width:20px;height:14px;margin-top:2px;position:relative;border-radius:1px;">'
        + '<div style="position:absolute;inset:0;background:' + it.swatch.fill + ';border-radius:1px;"></div>'
        + '<div style="position:absolute;left:0;right:0;bottom:0;height:3px;background:' + it.swatch.edge + ';border-radius:0 0 1px 1px;"></div>'
        + (it.hatched ? '<div style="position:absolute;inset:0;border-radius:1px;background-image:repeating-linear-gradient(45deg,' + it.swatch.edge + ' 0 1.5px,transparent 1.5px 5px);"></div>' : '')
        + '</div>';
    } else if (it.mark !== undefined) {
      swatch = '<div style="flex:0 0 auto;width:20px;height:20px;margin-top:-1px;display:flex;align-items:center;justify-content:center;border-radius:10px;background:' + SF.ink + ';color:#faf8f4;font-family:' + SF.fontMono + ';font-size:11px;font-weight:700;">' + it.mark + '</div>';
    }
    return '<div style="display:flex;align-items:flex-start;gap:10px;max-width:240px;font-size:11.5px;line-height:1.45;color:' + SF.ink2 + ';">'
      + swatch
      + '<div><span style="font-weight:700;color:' + SF.ink + ';">' + it.term + '</span>'
      + (it.desc ? '<span style="color:' + SF.ink2 + ';"> — ' + it.desc + '</span>' : '')
      + '</div></div>';
  }

  function KeyStrip(items) {
    return '<div style="display:flex;flex-wrap:wrap;gap:14px 28px;padding:14px 20px 10px;border-top:1px solid ' + SF.rule + ';background:' + SF.panel + ';font-family:' + SF.fontUI + ';">'
      + items.map(keyItem).join('')
      + '</div>';
  }

  // ── FigCompact wrapper ─────────────────────────────────────────────────
  // Compact variant (no eyebrow/title) for use inside existing page sections.
  function FigCompact(svgContent, keyItems, svgW, svgH) {
    svgW = svgW || 700; svgH = svgH || 260;
    // Keep the authored SVG width available on narrow screens so axis labels do
    // not become unreadable. The scroll region only overflows when its card is
    // narrower than the diagram; on larger screens this remains the same
    // full-width rendering as before.
    var canvasW = svgW + 48; // 24px horizontal padding on each side
    return '<div style="background:' + SF.paper + ';'
      + 'background-image:radial-gradient(rgba(26,23,20,0.035) 1px,transparent 1px);'
      + 'background-size:3px 3px;border:1px solid ' + SF.rule + ';overflow:hidden;">'
      + '<div data-sf-scroll-region role="region" tabindex="0"'
      + ' aria-label="Timeline diagram. Scroll horizontally to read the full axis and labels."'
      + ' style="overflow-x:auto;overflow-y:hidden;-webkit-overflow-scrolling:touch;">'
      + '<div data-sf-diagram-canvas style="--sf-diagram-width:' + canvasW + 'px;box-sizing:border-box;padding:28px 24px 20px;">'
      + '<svg viewBox="0 0 ' + svgW + ' ' + svgH
      + '" style="width:100%;height:auto;overflow:visible;display:block;" aria-hidden="true">'
      + DEFS + svgContent
      + '</svg>'
      + '</div>'
      + '</div>'
      + '<div data-sf-scroll-cue hidden style="padding:0 20px 10px;font-family:' + SF.fontUI + ';font-size:11px;line-height:1.4;color:' + SF.ink2 + ';">'
      + '\u2194 Scroll horizontally to read the full timeline</div>'
      + (keyItems && keyItems.length ? KeyStrip(keyItems) : '')
      + '</div>';
  }

  // ── Shared layout constants ────────────────────────────────────────────
  var PW = 580;     // plot width in SVG coords
  var LX = 90;      // left margin for lane labels
  var TOT = 10;     // default total positions
  var U  = PW / TOT; // = 58px per position unit

  // Lane heights
  var BH  = 32;  // block height (standard window bar)
  var DBH = 22;  // derived row block height
  var LH  = BH + 20; // lane vertical pitch

  // ══════════════════════════════════════════════════════════════════════
  // Figures
  // ══════════════════════════════════════════════════════════════════════

  var figs = {};

  // ── D_Window — single lane, one half-open window ──────────────────────
  figs.D_Window = function () {
    var svgH = 210;
    var g = '<g transform="translate(' + LX + ',60)">';

    // Event dots above lane
    g += EventDot(3*U, 'offline', 'up', SF.residual.fill);
    g += EventDot(7*U, 'online',  'up', SF.overlap.fill);
    g += Mark(3*U, -46, '1', SF.residual.fill);
    g += Mark(7*U, -46, '2', SF.overlap.fill);

    // Lane with block
    g += Lane(0, 'device-17', 'DeviceOffline', PW,
      Block(3*U, 4*U, { h: BH, color: SF.target, label: 'offline — 4 positions' })
    );

    // Ruler
    g += '<g transform="translate(0,' + (BH+24) + ')">' + Ruler([0,3,7,10], TOT, PW) + '</g>';

    // Half-open annotation
    g += Mark(3*U, BH+60, '3');
    g += '<text x="' + (3*U+16) + '" y="' + (BH+64) + '" font-family="' + SF.fontMono + '" font-size="13" font-weight="700" fill="' + SF.ink + '">[3, 7)</text>';
    g += '<text x="' + (3*U+80) + '" y="' + (BH+64) + '" font-family="' + SF.fontUI + '" font-size="11" fill="' + SF.ink2 + '">closed on left, open on right</text>';

    g += '</g>';

    return FigCompact(g, [
      { swatch: SF.target, term: 'window', desc: 'the predicate is active over this range' },
      { mark: '1', term: 'opening event', desc: 'predicate became true — window opens' },
      { mark: '2', term: 'closing event', desc: 'predicate became false — window closes' },
      { mark: '3', term: 'half-open interval', desc: '[a, b) — active at a, no longer active at b' },
    ], PW + LX + 20, svgH);
  };

  // ── D_Compare — two lanes, three derived row kinds ────────────────────
  figs.D_Compare = function () {
    var svgH = 330;
    var g = '<g transform="translate(' + LX + ',12)">';

    // Input lanes
    g += Lane(0,  'Target',  'provider-a', PW, Block(2*U, 5*U, { h: BH, color: SF.target, label: 'offline' }));
    g += Lane(LH, 'Against', 'provider-b', PW, Block(4*U, 5*U, { h: BH, color: SF.against, label: 'offline' }));

    // Guide lines at transitions
    [2,4,7,9].forEach(function(p) { g += Guide(p*U, 0, 220); });

    // Divider
    g += Emits(2*LH + 4, PW);

    // Derived rows
    var dy = 2*LH + 22;
    g += '<g transform="translate(0,' + dy + ')">';
    g += '<text x="-14" y="16" text-anchor="end" font-family="' + SF.fontUI + '" font-size="12" font-weight="700" fill="' + SF.residual.fill + '">residual</text>';
    g += Block(2*U, 2*U, { h: DBH, color: SF.residual, halfOpen: false });
    g += '<text x="' + (4*U+8) + '" y="15" font-family="' + SF.fontMono + '" font-size="10" fill="' + SF.ink3 + '">[2, 4)</text>';
    g += '</g>';

    g += '<g transform="translate(0,' + (dy+32) + ')">';
    g += '<text x="-14" y="16" text-anchor="end" font-family="' + SF.fontUI + '" font-size="12" font-weight="700" fill="' + SF.overlap.fill + '">overlap</text>';
    g += Block(4*U, 3*U, { h: DBH, color: SF.overlap, halfOpen: false });
    g += '<text x="' + (7*U+8) + '" y="15" font-family="' + SF.fontMono + '" font-size="10" fill="' + SF.ink3 + '">[4, 7)</text>';
    g += '</g>';

    g += '<g transform="translate(0,' + (dy+64) + ')">';
    g += '<text x="-14" y="16" text-anchor="end" font-family="' + SF.fontUI + '" font-size="12" font-weight="700" fill="' + SF.missing.edge + '">missing</text>';
    g += Block(7*U, 2*U, { h: DBH, color: SF.missing, halfOpen: false });
    g += '<text x="' + (9*U+8) + '" y="15" font-family="' + SF.fontMono + '" font-size="10" fill="' + SF.ink3 + '">[7, 9)</text>';
    g += '</g>';

    // Ruler
    g += '<g transform="translate(0,' + (dy+100) + ')">' + Ruler([0,2,4,7,9,10], TOT, PW) + '</g>';

    g += '</g>';

    return FigCompact(g, [
      { swatch: SF.target,   term: 'target',   desc: 'the lane under test' },
      { swatch: SF.against,  term: 'against',  desc: 'the lane expected to cover or explain it' },
      { swatch: SF.residual, term: 'residual', desc: 'target-only evidence — under-coverage' },
      { swatch: SF.overlap,  term: 'overlap',  desc: 'both active — agreement' },
      { swatch: SF.missing,  term: 'missing',  desc: 'against-only — target missed it' },
    ], PW + LX + 20, svgH);
  };

  // ── D_Horizon — live finality horizon ─────────────────────────────────
  figs.D_Horizon = function () {
    var svgH = 240;
    var hz = 6.5;
    var hzX = hz * U;
    var g = '<g transform="translate(' + LX + ',38)">';

    // Horizon shading
    g += '<rect x="' + hzX + '" y="-24" width="' + (PW - hzX) + '" height="170" fill="#faf2e4" opacity="0.7"/>';
    g += Horizon(hzX, -24, 148);

    // Lane 1 — closed (final)
    g += Lane(0,  'device-04', 'closed', PW,
      Block(1*U, 3*U, { h: BH, color: SF.overlap, label: 'offline · final' })
    );

    // Lane 2 — open + provisional extension
    var knownW = (hz - 4) * U;
    var provW  = 2 * U;
    g += Lane(LH, 'device-17', 'open', PW,
      Block(4*U, knownW, { h: BH, color: SF.target, label: 'known', halfOpen: false })
      + Provisional(hzX, provW, { h: BH, label: 'provisional \u2192' })
    );

    // Ruler
    g += '<g transform="translate(0,' + (2*LH + 10) + ')">' + Ruler([0,1,4,6.5,10], TOT, PW) + '</g>';

    g += '</g>';

    return FigCompact(g, [
      { swatch: SF.overlap, term: 'final', desc: 'window closed — stable evidence' },
      { swatch: SF.target, term: 'known span', desc: 'open window up to the horizon' },
      { swatch: SF.provisional, hatched: true, term: 'provisional', desc: 'still open at horizon — may change' },
      { mark: 'H', term: 'horizon', desc: 'moment the live run asks its question' },
    ], PW + LX + 20, svgH);
  };

  // ── D_Segments — tinted bands behind windows ──────────────────────────
  figs.D_Segments = function () {
    var svgH = 200;
    var segs = [
      { x: 0, w: 3, label: 'warmup',   bg: '#eef0e8', edge: '#aeb89c' },
      { x: 3, w: 4, label: 'steady',   bg: '#f4ecdc', edge: '#c9a87a' },
      { x: 7, w: 3, label: 'cooldown', bg: '#e8e0e8', edge: '#b89cb8' },
    ];
    var g = '<g transform="translate(' + LX + ',40)">';

    segs.forEach(function(s) {
      g += '<rect x="' + (s.x*U) + '" y="-8" width="' + (s.w*U) + '" height="' + (BH+16) + '" fill="' + s.bg + '"/>';
      g += '<rect x="' + (s.x*U) + '" y="-8" width="2" height="' + (BH+16) + '" fill="' + s.edge + '"/>';
      g += '<text x="' + ((s.x + s.w/2)*U) + '" y="-16" text-anchor="middle"'
        + ' font-family="' + SF.fontMono + '" font-size="9.5" font-weight="700" fill="' + s.edge + '"'
        + ' letter-spacing="1" style="text-transform:uppercase">' + s.label + '</text>';
    });

    g += Lane(0, 'device-17', 'DeviceOffline', PW,
      Block(2*U, 3*U, { h: BH, color: SF.target, label: 'offline' })
      + Block(7.5*U, 1.5*U, { h: BH, color: SF.target, label: 'offline' })
    );

    g += '<g transform="translate(0,' + (BH+24) + ')">' + Ruler([0,3,7,10], TOT, PW) + '</g>';

    // Tag row
    var tags = [
      { x: 2*U, lbl: 'region=eu' },
      { x: 2*U + 88, lbl: 'severity=hi' },
      { x: 2*U + 184, lbl: 'team=infra' },
    ];
    g += '<text x="-14" y="' + (BH+74) + '" text-anchor="end" font-family="' + SF.fontUI + '" font-size="11" font-weight="700" fill="' + SF.ink2 + '">tags</text>';
    tags.forEach(function(t) {
      g += '<rect x="' + t.x + '" y="' + (BH+60) + '" width="80" height="20" fill="' + SF.panel + '" stroke="' + SF.rule + '"/>';
      g += '<text x="' + (t.x+8) + '" y="' + (BH+74) + '" font-family="' + SF.fontMono + '" font-size="10" fill="' + SF.ink + '">' + esc(t.lbl) + '</text>';
    });

    g += '</g>';

    return FigCompact(g, [
      { swatch: { fill: '#f4ecdc', edge: '#c9a87a' }, term: 'segment band', desc: 'a phase or period — context, not a boundary' },
      { swatch: SF.target, term: 'window', desc: 'inherits the segment it opened inside' },
      { mark: 'T', term: 'tag', desc: 'descriptive metadata preserved through export' },
    ], PW + LX + 20, svgH);
  };

  // ── D_Cohort — three contributors → derived quorum lane ───────────────
  figs.D_Cohort = function () {
    var T = 12, u = PW / T;
    var svgH = 330;
    var contributors = [
      { y: 0,    label: 'gw-1', x: 1, w: 4 },
      { y: LH,   label: 'gw-2', x: 2, w: 5 },
      { y: 2*LH, label: 'gw-3', x: 3, w: 3 },
    ];
    var g = '<g transform="translate(' + LX + ',10)">';

    contributors.forEach(function(l) {
      g += Lane(l.y, l.label, 'contributor', PW,
        Block(l.x*u, l.w*u, { h: BH, color: SF.against }), false
      );
    });

    g += Emits(3*LH + 4, PW, 'Derived \u2192');

    g += Lane(3*LH + 22, 'cohort', '\u2265 2 of 3', PW,
      Block(2*u, 4*u, { h: BH, color: SF.overlap, label: 'quorum active' })
    );
    g += Lane(4*LH + 22, 'primary', 'compared', PW,
      Block(3.5*u, 2*u, { h: BH, color: SF.target, label: 'offline' })
    );

    g += '<g transform="translate(0,' + (5*LH + 26) + ')">' + Ruler([0,2,6,12], T, PW) + '</g>';

    g += '</g>';

    return FigCompact(g, [
      { swatch: SF.against, term: 'contributor', desc: 'one member of the cohort group' },
      { swatch: SF.overlap, term: 'cohort active', desc: 'quorum rule satisfied — here \u2265 2 of 3' },
      { swatch: SF.target,  term: 'primary', desc: 'compared against the derived cohort lane' },
    ], PW + LX + 20, svgH);
  };

  // ── D_Rollup — child windows fold into a parent ────────────────────────
  figs.D_Rollup = function () {
    var T = 12, u = PW / T;
    var svgH = 320;
    var children = [
      { y: 0,    label: 'pod-a', x: 1,   w: 3 },
      { y: LH,   label: 'pod-b', x: 2.5, w: 2 },
      { y: 2*LH, label: 'pod-c', x: 6,   w: 3 },
    ];
    var g = '<g transform="translate(' + LX + ',10)">';

    children.forEach(function(c) {
      g += Lane(c.y, c.label, 'child', PW,
        Block(c.x*u, c.w*u, { h: BH, color: SF.against }), false
      );
    });

    // Dashed fold paths from child mid-points down to parent
    var parentY = 3*LH + 20;
    var pathData = [
      [1*u + BH/2, 28, 1*u + BH/2, parentY],
      [2.5*u + BH/2, 74, 2.5*u + BH/2, parentY],
      [6*u + BH/2, 120, 6*u + BH/2, parentY],
    ];
    g += '<g stroke="' + SF.ink3 + '" stroke-width="1" fill="none" stroke-dasharray="2 3">';
    pathData.forEach(function(p) {
      g += '<path d="M' + p[0] + ',' + p[1] + ' C' + p[0] + ',' + ((p[1]+p[3])/2) + ' ' + p[2] + ',' + ((p[1]+p[3])/2) + ' ' + p[2] + ',' + p[3] + '"/>';
    });
    g += '</g>';

    g += Lane(parentY, 'cluster', 'parent', PW,
      Block(1*u, 8*u, { h: BH, color: SF.target, label: 'degraded \u00b7 3 children' })
    );

    g += '<g transform="translate(0,' + (parentY + BH + 36) + ')">' + Ruler([0,1,4,6,9,12], T, PW) + '</g>';

    g += '</g>';

    return FigCompact(g, [
      { swatch: SF.against, term: 'child window', desc: 'leaf-level evidence from one contributor' },
      { swatch: SF.target,  term: 'parent roll-up', desc: 'opens when any child is active' },
      { mark: '\u2193', term: 'fold path', desc: 'child keeps a pointer to the parent for drill-down' },
    ], PW + LX + 20, svgH);
  };

  // ── D_TemporalModel — three axes ──────────────────────────────────────
  figs.D_TemporalModel = function () {
    var svgH = 294;
    // Horizon() badge renders 28px above `top`; extra 24px gap prevents overlap with Position block.
    var hzLaneY = 2*LH + 24;
    var g = '<g transform="translate(' + LX + ',40)">';

    // Event time lane
    g += Lane(0, 'Event time', null, PW,
      Block(2*U, 4*U, { h: BH, color: SF.target, label: '10:03 \u2013 10:11' })
    );

    // Processing position lane
    g += Lane(LH, 'Position', null, PW,
      Block(3.5*U, 4*U, { h: BH, color: SF.against, label: 'positions 47\u202662' })
    );

    // Known-at lane with horizon
    var hzX = 6.5 * U;
    g += '<rect x="' + hzX + '" y="' + (hzLaneY - 8) + '" width="' + (PW-hzX) + '" height="' + (BH+16) + '" fill="#faf2e4" opacity="0.7"/>';
    g += Lane(hzLaneY, 'Known-at', null, PW,
      Block(5*U, 1.5*U, { h: BH, color: SF.missing, label: 'visible' })
    );
    g += Horizon(hzX, hzLaneY - 8, hzLaneY + BH + 16);
    g += '<text x="' + (hzX + 8) + '" y="' + (hzLaneY + BH + 14) + '" font-family="' + SF.fontUI + '" font-size="11" fill="' + SF.ink2 + '">decision @ pos 50</text>';

    g += '<g transform="translate(0,' + (hzLaneY + LH + 10) + ')">' + Ruler([0,2,5,6.5,10], TOT, PW) + '</g>';

    g += '</g>';

    return FigCompact(g, [
      { swatch: SF.target,  term: 'event time', desc: 'when the source says something happened' },
      { swatch: SF.against, term: 'processing position', desc: 'when the pipeline saw it' },
      { swatch: SF.missing, term: 'known-at visible', desc: 'available before the decision point' },
      { mark: 'H', term: 'known-at horizon', desc: 'position 50 — future evidence excluded' },
    ], PW + LX + 20, svgH);
  };

  // ── D_QueryHistory — closed and open windows ───────────────────────────
  figs.D_QueryHistory = function () {
    var svgH = 230;
    var g = '<g transform="translate(' + LX + ',28)">';

    g += Lane(0,  'Closed', null, PW,
      Block(1.5*U, 2.8*U, { h: BH, color: SF.target, label: 'past incident' })
      + Block(5.5*U, 2*U, { h: BH, color: SF.against, label: 'short blip' })
    );

    // Open dashed block for open window
    var ox = 7.8*U, ow = 1.8*U;
    g += Lane(LH, 'Open', null, PW,
      // Use a custom dashed-border open window
      '<g transform="translate(' + ox + ',0)">'
      + '<rect x="0" y="0" width="' + ow + '" height="' + (BH-3) + '" fill="' + SF.panel + '" stroke="' + SF.rule + '" stroke-width="1.5" stroke-dasharray="4 2"/>'
      + '<rect x="0" y="' + (BH-3) + '" width="' + ow + '" height="3" fill="' + SF.rule + '"/>'
      + '<rect x="0" y="-2" width="2" height="' + (BH+4) + '" fill="' + SF.ink3 + '"/>'
      + '<text x="' + (ow/2) + '" y="' + (BH/2+1) + '" text-anchor="middle" dominant-baseline="middle"'
        + ' font-family="' + SF.fontUI + '" font-size="11" fill="' + SF.ink2 + '">still active</text>'
      + '</g>'
    );

    g += '<g transform="translate(0,' + (2*LH + 14) + ')">' + Ruler([0,1.5,4.3,5.5,7.5,7.8,10], TOT, PW) + '</g>';

    // Axis labels
    var axLabels = [
      { x: 1.5*U, lbl: 'opened' }, { x: 4.3*U, lbl: 'closed' },
      { x: 5.5*U, lbl: 'opened' }, { x: 7.5*U, lbl: 'closed' },
      { x: 7.8*U, lbl: 'opened' }, { x: 10*U, lbl: 'now' },
    ];
    g += '<g transform="translate(0,' + (2*LH + 14) + ')" font-family="' + SF.fontUI + '" font-size="9.5" fill="' + SF.ink3 + '">';
    axLabels.forEach(function(al) {
      g += '<text x="' + al.x + '" y="36" text-anchor="middle">' + al.lbl + '</text>';
    });
    g += '</g>';

    g += '</g>';

    return FigCompact(g, [
      { swatch: SF.target,  term: 'closed window', desc: 'start and end recorded — stable evidence' },
      { swatch: SF.against, term: 'closed window', desc: 'a second past incident' },
      { swatch: SF.neutral, term: 'open window', desc: 'start only — still active' },
    ], PW + LX + 20, svgH);
  };

  // ── D_SegmentSplit — predicate truth vs split windows ─────────────────
  figs.D_SegmentSplit = function () {
    var svgH = 270;
    var g = '<g transform="translate(' + LX + ',12)">';

    // One truth span
    g += Lane(0, 'Predicate', null, PW,
      Block(1.5*U, 7*U, { h: BH, color: SF.target, label: 'offline (one truth span)' })
    );

    // Two recorded windows from segment split
    g += Lane(LH, 'Recorded', null, PW,
      Block(1.5*U, 3.5*U, { h: BH, color: SF.target, label: 'segment: triage' })
      + Block(5*U, 3.5*U, { h: BH, color: SF.against, label: 'segment: escalated' })
    );

    g += Emits(2*LH + 4, PW, 'Spans \u2192');

    // Measure bands
    var my = 2*LH + 22;
    g += '<g transform="translate(0,' + my + ')">';
    g += '<text x="-14" y="14" text-anchor="end" font-family="' + SF.fontUI + '" font-size="11" fill="' + SF.ink2 + '">window 1</text>';
    g += Block(1.5*U, 3.5*U, { h: DBH, color: SF.target, halfOpen: false });
    g += '<text x="' + (1.5*U + 8) + '" y="14" font-family="' + SF.fontMono + '" font-size="10" fill="' + SF.ink3 + '">triage</text>';
    g += '</g>';
    g += '<g transform="translate(0,' + (my + 32) + ')">';
    g += '<text x="-14" y="14" text-anchor="end" font-family="' + SF.fontUI + '" font-size="11" fill="' + SF.ink2 + '">window 2</text>';
    g += Block(5*U, 3.5*U, { h: DBH, color: SF.against, halfOpen: false });
    g += '<text x="' + (5*U + 8) + '" y="14" font-family="' + SF.fontMono + '" font-size="10" fill="' + SF.ink3 + '">escalated</text>';
    g += '</g>';

    g += '<g transform="translate(0,' + (my + 68) + ')">' + Ruler([0,1.5,5,8.5], TOT, PW) + '</g>';

    g += '</g>';

    return FigCompact(g, [
      { swatch: SF.target,  term: 'triage window', desc: 'first segment — measurable on its own' },
      { swatch: SF.against, term: 'escalated window', desc: 'second segment — opens when segment changes' },
    ], PW + LX + 20, svgH);
  };

  // ── D_FinalityRows — one run, two finality kinds ───────────────────────
  figs.D_FinalityRows = function () {
    var svgH = 270;
    var hzX = 7.2 * U;
    var g = '<g transform="translate(' + LX + ',38)">';

    g += '<rect x="' + hzX + '" y="-24" width="' + (PW-hzX) + '" height="' + (3*LH+20) + '" fill="#faf2e4" opacity="0.65"/>';
    g += Horizon(hzX, -24, 3*LH + 20);

    g += Lane(0,  'Closed', null, PW,
      Block(1*U, 2.5*U, { h: BH, color: SF.overlap, label: 'final [12,28)' })
    );
    g += Lane(LH, 'Open', null, PW,
      Block(4.5*U, 2.7*U, { h: BH, color: SF.target, label: 'known', halfOpen: false })
      + Provisional(hzX, 1.5*U, { h: BH, label: 'provisional' })
    );
    g += Lane(2*LH, 'Live run', null, PW,
      Block(1*U, 2.5*U, { h: BH, color: SF.overlap, halfOpen: false })
      + Block(4.5*U, 2.7*U, { h: BH, color: SF.target, halfOpen: false })
      + Provisional(hzX, 1.5*U, { h: BH })
    );

    g += '<g transform="translate(0,' + (3*LH + 14) + ')">' + Ruler([0,1,3.5,5,7.2,10], TOT, PW) + '</g>';

    g += '</g>';

    return FigCompact(g, [
      { swatch: SF.overlap,     term: 'final', desc: 'from closed window — stable under replay' },
      { swatch: SF.target,      term: 'known span', desc: 'open window up to the horizon' },
      { swatch: SF.provisional, hatched: true, term: 'provisional', desc: 'clipped at horizon — may change' },
    ], PW + LX + 20, svgH);
  };

  // ── D_CohortPipeline — members → aligned → rule → derived ─────────────
  figs.D_CohortPipeline = function () {
    var svgH = 310;
    var g = '<g transform="translate(' + LX + ',10)">';

    // Members
    g += Lane(0, 'Members', null, PW,
      Block(1.5*U, 2.2*U, { h: BH, color: SF.target })
      + Block(3*U, 2.8*U, { h: BH, color: SF.against })
      + Block(5*U, 2.4*U, { h: BH, color: SF.neutral })
      + '<text x="' + (3.4*U) + '" y="-8" font-family="' + SF.fontUI + '" font-size="10" fill="' + SF.ink2 + '">overlap windows</text>'
    );

    g += Lane(LH, 'Aligned', null, PW,
      Block(1.5*U, 1.5*U, { h: BH, color: SF.neutral })
      + Block(3*U, 0.5*U, { h: BH, color: SF.against })
      + Block(3.5*U, 1.5*U, { h: BH, color: SF.against })
      + Block(5*U, 0.8*U, { h: BH, color: SF.neutral })
      + Block(5.8*U, 1.6*U, { h: BH, color: SF.neutral })
      + '<text x="' + (3.4*U) + '" y="-8" font-family="' + SF.fontUI + '" font-size="10" fill="' + SF.ink2 + '">cut at every transition</text>'
    );

    g += Lane(2*LH, 'Rule (\u22652)', null, PW,
      Block(1.5*U, 1.5*U, { h: BH, color: SF.residual, label: 'fail' })
      + Block(3*U, 2.4*U, { h: BH, color: SF.overlap, label: 'pass' })
      + Block(5.4*U, 2*U, { h: BH, color: SF.residual, label: 'fail' })
    );

    g += Lane(3*LH, 'Cohort', null, PW,
      Block(3*U, 2.4*U, { h: BH, color: SF.overlap, label: 'one derived lane' })
    );

    g += '<g transform="translate(0,' + (4*LH + 14) + ')">' + Ruler([0,1.5,3,5.4,7.4], TOT, PW) + '</g>';

    g += '</g>';

    return FigCompact(g, [
      { swatch: SF.against,  term: 'aligned segments', desc: 'sliced at every member transition' },
      { swatch: SF.residual, term: 'rule failed', desc: 'not enough active members' },
      { swatch: SF.overlap,  term: 'cohort active', desc: 'quorum passed — emitted as derived lane' },
    ], PW + LX + 20, svgH);
  };

  // ── D_SourceMatrix — directional A→B and B→A ──────────────────────────
  figs.D_SourceMatrix = function () {
    var svgH = 250;
    var g = '<g transform="translate(' + LX + ',20)">';

    [2,4,7,9].forEach(function(p) { g += Guide(p*U, 0, 200); });

    // A → B row
    g += Lane(0, 'A \u2192 B', null, PW,
      Block(1.8*U, 2*U, { h: BH, color: SF.residual, label: 'A only' })
      + Block(3.8*U, 2.6*U, { h: BH, color: SF.overlap, label: 'covered' })
    );

    // B → A row
    g += Lane(LH, 'B \u2192 A', null, PW,
      Block(3.8*U, 2.6*U, { h: BH, color: SF.overlap, label: 'covered' })
      + Block(6.4*U, 1.8*U, { h: BH, color: SF.missing, label: 'B only' })
    );

    // Measure bands
    g += Emits(2*LH + 4, PW, 'Cells \u2192');
    var my = 2*LH + 22;
    g += '<g transform="translate(0,' + my + ')">';
    g += '<text x="-14" y="14" text-anchor="end" font-family="' + SF.fontUI + '" font-size="10" fill="' + SF.ink2 + '">A\u2192B</text>';
    g += Block(1.8*U, 2*U, { h: DBH, color: SF.residual, halfOpen: false });
    g += Block(3.8*U, 2.6*U, { h: DBH, color: SF.overlap, halfOpen: false });
    g += '</g>';
    g += '<g transform="translate(0,' + (my+30) + ')">';
    g += '<text x="-14" y="14" text-anchor="end" font-family="' + SF.fontUI + '" font-size="10" fill="' + SF.ink2 + '">B\u2192A</text>';
    g += Block(3.8*U, 2.6*U, { h: DBH, color: SF.overlap, halfOpen: false });
    g += Block(6.4*U, 1.8*U, { h: DBH, color: SF.missing, halfOpen: false });
    g += '</g>';

    g += '<g transform="translate(0,' + (my+64) + ')">' + Ruler([0,1.8,3.8,6.4,8.2], TOT, PW) + '</g>';

    g += '</g>';

    return FigCompact(g, [
      { swatch: SF.residual, term: 'residual (A→B)', desc: 'A had target-only duration B did not cover' },
      { swatch: SF.overlap,  term: 'overlap', desc: 'same range in both cells — agreement evidence' },
      { swatch: SF.missing,  term: 'residual (B→A)', desc: 'B had duration A did not cover' },
    ], PW + LX + 20, svgH);
  };

  // ── D_Hierarchy — parent + children + explained/orphan ────────────────
  figs.D_Hierarchy = function () {
    var svgH = 300;
    var g = '<g transform="translate(' + LX + ',12)">';

    // Parent full span
    g += Lane(0, 'Parent', null, PW,
      Block(1.2*U, 6.4*U, { h: BH, color: SF.target, label: 'region impacted' })
    );

    // child-1 explains left portion
    g += Lane(LH, 'child-1', null, PW,
      Block(1.2*U, 3*U, { h: BH, color: SF.against, label: 'explains' })
    );

    // child-2 explains right portion
    g += Lane(2*LH, 'child-2', null, PW,
      Block(6*U, 1.6*U, { h: BH, color: SF.against, label: 'explains' })
    );

    // gap — unexplained parent range
    var gapX = 4.2*U, gapW = 1.8*U;
    g += Lane(3*LH, 'gap', null, PW,
      '<g transform="translate(' + gapX + ',0)">'
      + '<rect x="0" y="0" width="' + gapW + '" height="' + (BH-3) + '" fill="' + SF.residual.fill + '" opacity="0.35"/>'
      + '<rect x="0" y="' + (BH-3) + '" width="' + gapW + '" height="3" fill="' + SF.residual.edge + '" opacity="0.5"/>'
      + '<rect x="0" y="0" width="2" height="' + BH + '" fill="' + SF.residual.fill + '"/>'
      + '<text x="' + (gapW/2) + '" y="' + (BH/2+1) + '" text-anchor="middle" dominant-baseline="middle"'
        + ' font-family="' + SF.fontUI + '" font-size="10" fill="' + SF.residual.fill + '">unexplained</text>'
      + '</g>'
    );

    // orphan child — past end of parent
    g += Lane(4*LH, 'orphan', null, PW,
      Block(8.5*U, 1.2*U, { h: BH, color: SF.missing, label: 'child only' })
    );

    g += '<g transform="translate(0,' + (5*LH + 10) + ')">' + Ruler([0,1.2,4.2,6,7.6,8.5,9.7], TOT, PW) + '</g>';

    g += '</g>';

    return FigCompact(g, [
      { swatch: SF.target,   term: 'parent', desc: 'region-level roll-up window' },
      { swatch: SF.against,  term: 'explained', desc: 'child contribution overlaps the parent range' },
      { swatch: SF.residual, term: 'unexplained', desc: 'parent active — no child evidence found' },
      { swatch: SF.missing,  term: 'orphan child', desc: 'child active after parent closed' },
    ], PW + LX + 20, svgH);
  };

  // ── D_Advanced — complex multi-lane audit scenario ─────────────────────
  figs.D_Advanced = function () {
    var svgH = 390;
    var g = '<g transform="translate(' + LX + ',12)">';

    // Maintenance band
    var mX = 2.8*U, mW = 1.4*U;
    g += '<rect x="' + mX + '" y="-8" width="' + mW + '" height="' + (BH+16) + '" fill="#e8f0f4" opacity="0.9"/>';
    g += '<rect x="' + mX + '" y="-8" width="2" height="' + (BH+16) + '" fill="#6b8e9a"/>';
    g += Lane(0, 'maintenance', null, PW,
      Block(mX, mW, { h: BH, color: SF.coverage, label: 'scheduled' })
    );

    // Primary
    g += Lane(LH, 'primary', null, PW,
      Block(1.4*U, 6.8*U, { h: BH, color: SF.target, label: 'outage' })
    );

    // Backup (two spans — drops briefly)
    g += Lane(2*LH, 'backup', null, PW,
      Block(2.2*U, 4*U, { h: BH, color: SF.against, label: 'covers' })
      + Block(6.8*U, 1.4*U, { h: BH, color: SF.against, label: 'resumes' })
    );

    // Lifecycle segment bands behind triage/escalated labels
    var triageW = 3.4*U, escalW = 3.4*U;
    g += '<rect x="' + (1.4*U) + '" y="' + (3*LH-8) + '" width="' + triageW + '" height="' + (BH+16) + '" fill="#f4ecdc" opacity="0.8"/>';
    g += '<rect x="' + (4.8*U) + '" y="' + (3*LH-8) + '" width="' + escalW  + '" height="' + (BH+16) + '" fill="#f0e4e4" opacity="0.8"/>';
    g += Lane(3*LH, 'lifecycle', null, PW,
      Block(1.4*U, triageW, { h: BH, color: SF.neutral, label: 'triage' })
      + Block(4.8*U, escalW, { h: BH, color: SF.residual, label: 'escalated' })
    );

    // SLA measure band
    g += Emits(4*LH + 4, PW, 'SLA band \u2192');
    var my = 4*LH + 22;
    var bands = [
      { x: 1.4*U, w: 0.8*U, color: SF.residual, lbl: 'pre-backup' },
      { x: 2.2*U, w: 0.6*U, color: SF.overlap, lbl: 'covered' },
      { x: 2.8*U, w: 1.4*U, color: SF.coverage, lbl: 'excluded' },
      { x: 4.2*U, w: 2*U,   color: SF.overlap, lbl: 'covered' },
      { x: 6.2*U, w: 0.6*U, color: SF.residual, lbl: 'gap' },
      { x: 6.8*U, w: 1.4*U, color: SF.overlap, lbl: 'covered' },
    ];
    g += '<g transform="translate(0,' + my + ')">';
    bands.forEach(function(b) {
      g += Block(b.x, b.w, { h: DBH, color: b.color, halfOpen: false });
    });
    g += '</g>';

    g += '<g transform="translate(0,' + (my + DBH + 20) + ')">' + Ruler([0,1.4,2.2,2.8,4.2,6.2,6.8,8.2], TOT, PW) + '</g>';

    g += '</g>';

    return FigCompact(g, [
      { swatch: SF.coverage,  term: 'maintenance', desc: 'excluded from SLA denominator' },
      { swatch: SF.target,    term: 'primary', desc: 'the window under test' },
      { swatch: SF.against,   term: 'backup', desc: 'expected to cover the primary' },
      { swatch: SF.overlap,   term: 'covered', desc: 'eligible and covered by backup' },
      { swatch: SF.residual,  term: 'gap', desc: 'eligible but not covered' },
      { swatch: SF.neutral,   term: 'triage / escalated', desc: 'lifecycle segment context' },
    ], PW + LX + 20, svgH);
  };

  // ── D_ConceptsFlow — Record / Compare / Derive overview ───────────────
  figs.D_ConceptsFlow = function () {
    var svgH = 200;
    var g = '<g transform="translate(' + LX + ',12)">';

    g += Lane(0, '1. Record', null, PW,
      Block(1.2*U, 3*U, { h: BH, color: SF.target, label: 'predicate true' })
    );
    g += Lane(LH, '2. Compare', null, PW,
      Block(1.2*U, 3.6*U, { h: BH, color: SF.target })
      + Block(3*U, 3.6*U, { h: BH, color: SF.against })
    );

    // Derived row labels inline
    var dy = 2*LH;
    g += '<text x="-14" y="' + (dy+16) + '" text-anchor="end" font-family="' + SF.fontUI + '" font-size="12" font-weight="700" fill="' + SF.ink + '">3. Derive</text>';
    g += Block(1.2*U, 1.8*U, { h: DBH, color: SF.residual, halfOpen: false });
    g += Block(3*U, 1.8*U,   { h: DBH, color: SF.overlap,  halfOpen: false });
    g += Block(4.8*U, 1.8*U, { h: DBH, color: SF.missing,  halfOpen: false });
    g += '<text x="' + (1.2*U + 4) + '" y="' + (dy+16) + '" font-family="' + SF.fontUI + '" font-size="10" fill="' + SF.residual.fill + '">residual</text>';
    g += '<text x="' + (3*U + 4) + '" y="' + (dy+16) + '" font-family="' + SF.fontUI + '" font-size="10" fill="' + SF.ink + '">overlap</text>';
    g += '<text x="' + (4.8*U + 4) + '" y="' + (dy+16) + '" font-family="' + SF.fontUI + '" font-size="10" fill="' + SF.missing.edge + '">missing</text>';

    g += '<g transform="translate(0,' + (dy + DBH + 28) + ')">' + Ruler([0,1.2,3,4.8,6.6], TOT, PW) + '</g>';

    g += '</g>';

    return FigCompact(g, [
      { swatch: SF.target,   term: 'record', desc: 'predicate held true for this key and source' },
      { swatch: SF.residual, term: 'residual', desc: 'target-only — against side did not cover it' },
      { swatch: SF.overlap,  term: 'overlap',  desc: 'both active — agreement evidence' },
      { swatch: SF.missing,  term: 'missing',  desc: 'against-only — target missed it' },
    ], PW + LX + 20, svgH);
  };

  // ── D_GetStarted — get-started worked example ─────────────────────────
  figs.D_GetStarted = function () {
    var svgH = 320;
    var g = '<g transform="translate(' + LX + ',12)">';

    [3,4,5,6].forEach(function(p) { g += Guide(p*U, 0, 230); });

    g += Lane(0,  'provider-a', null, PW,
      Block(3*U, 3*U, { h: BH, color: SF.target, label: 'offline [3, 6)' })
    );
    g += Lane(LH, 'provider-b', null, PW,
      Block(4*U, 1*U, { h: BH, color: SF.against, label: 'offline [4, 5)' })
    );

    g += Emits(2*LH + 4, PW);

    var dy = 2*LH + 22;
    g += '<g transform="translate(0,' + dy + ')">';
    g += '<text x="-14" y="15" text-anchor="end" font-family="' + SF.fontUI + '" font-size="12" font-weight="700" fill="' + SF.residual.fill + '">residual</text>';
    g += Block(3*U, 1*U, { h: DBH, color: SF.residual, halfOpen: false });
    g += '<text x="' + (4*U+8) + '" y="15" font-family="' + SF.fontMono + '" font-size="10" fill="' + SF.ink3 + '">[3, 4)</text>';
    g += '</g>';

    g += '<g transform="translate(0,' + (dy+32) + ')">';
    g += '<text x="-14" y="15" text-anchor="end" font-family="' + SF.fontUI + '" font-size="12" font-weight="700" fill="' + SF.overlap.fill + '">overlap</text>';
    g += Block(4*U, 1*U, { h: DBH, color: SF.overlap, halfOpen: false });
    g += '<text x="' + (5*U+8) + '" y="15" font-family="' + SF.fontMono + '" font-size="10" fill="' + SF.ink3 + '">[4, 5)</text>';
    g += '</g>';

    g += '<g transform="translate(0,' + (dy+64) + ')">';
    g += '<text x="-14" y="15" text-anchor="end" font-family="' + SF.fontUI + '" font-size="12" font-weight="700" fill="' + SF.residual.fill + '">residual</text>';
    g += Block(5*U, 1*U, { h: DBH, color: SF.residual, halfOpen: false });
    g += '<text x="' + (6*U+8) + '" y="15" font-family="' + SF.fontMono + '" font-size="10" fill="' + SF.ink3 + '">[5, 6)</text>';
    g += '</g>';

    g += '<g transform="translate(0,' + (dy+100) + ')">' + Ruler([0,1,3,4,5,6,7], TOT, PW) + '</g>';

    g += '</g>';

    return FigCompact(g, [
      { swatch: SF.target,   term: 'provider-a', desc: 'offline [3, 6) — the target lane' },
      { swatch: SF.against,  term: 'provider-b', desc: 'offline [4, 5) — the against lane' },
      { swatch: SF.residual, term: 'residual', desc: 'provider-a only — not covered by provider-b' },
      { swatch: SF.overlap,  term: 'overlap', desc: 'both active — agreement' },
    ], PW + LX + 20, svgH);
  };

  // ── Use-case figures ──────────────────────────────────────────────────

  figs.UC_Monitoring = function () {
    var svgH = 220;
    var g = '<g transform="translate(' + LX + ',20)">';
    [1.8, 3.5, 6.5].forEach(function (p) { g += Guide(p * U, 0, 3 * LH + 8); });
    g += Lane(0,      'Primary',   null, PW, Block(1.8 * U, 4.8 * U, { h: BH, color: SF.target,  label: 'Outage' }));
    g += Lane(LH,     'Backup',    null, PW, Block(3.5 * U, 4.2 * U, { h: BH, color: SF.against, label: 'Missed' }));
    g += Lane(2 * LH, 'Heartbeat', null, PW, Block(6.5 * U, 1.8 * U, { h: BH, color: SF.missing, label: 'Flapping' }));
    g += '<g transform="translate(0,' + (3 * LH + 14) + ')">' + Ruler([0, 1.8, 3.5, 6.5, 10], TOT, PW) + '</g>';
    g += '</g>';
    return FigCompact(g, [
      { swatch: SF.target,  term: 'Primary',   desc: 'outage window — primary provider' },
      { swatch: SF.against, term: 'Backup',    desc: 'missed early window — comparison provider' },
      { swatch: SF.missing, term: 'Heartbeat', desc: 'absent signal — flapping' },
    ], PW + LX + 20, svgH);
  };

  figs.UC_IoT = function () {
    var svgH = 220;
    var g = '<g transform="translate(' + LX + ',20)">';
    g += '<rect x="0" y="' + (2 * LH) + '" width="' + (4.8 * U) + '" height="' + BH + '" fill="' + SF.neutral.fill + '"/>';
    g += '<rect x="' + (4.8 * U) + '" y="' + (2 * LH) + '" width="' + (5.2 * U) + '" height="' + BH + '" fill="' + SF.coverage.fill + '" opacity="0.55"/>';
    g += Lane(0, 'Sensor A', null, PW,
      Block(1.2 * U, 2.4 * U, { h: BH, color: SF.target })
      + Block(6.0 * U, 2.5 * U, { h: BH, color: SF.target, label: 'Over-temp' }));
    g += Lane(LH, 'Sensor B', null, PW,
      Block(1.8 * U, 1.6 * U, { h: BH, color: SF.against })
      + Block(6.4 * U, 1.7 * U, { h: BH, color: SF.against }));
    g += Lane(2 * LH, 'Mode', null, PW,
      '<text x="' + (2.4 * U) + '" y="' + (BH / 2 + 4) + '" text-anchor="middle" dominant-baseline="middle"'
      + ' font-family="' + SF.fontUI + '" font-size="11" font-weight="600" fill="' + SF.ink2 + '">Idle</text>'
      + '<text x="' + (7.4 * U) + '" y="' + (BH / 2 + 4) + '" text-anchor="middle" dominant-baseline="middle"'
      + ' font-family="' + SF.fontUI + '" font-size="11" font-weight="600" fill="' + SF.ink + '">Calibration</text>');
    g += '<g transform="translate(0,' + (3 * LH + 14) + ')">' + Ruler([0, 1.2, 4.8, 6.0, 10], TOT, PW) + '</g>';
    g += '</g>';
    return FigCompact(g, [
      { swatch: SF.target,                                 term: 'Sensor A',    desc: 'over-temp windows — two events' },
      { swatch: SF.against,                                term: 'Sensor B',    desc: 'corroborating sensor readings' },
      { swatch: { fill: SF.coverage.fill, edge: SF.rule }, term: 'Calibration', desc: 'maintenance phase — risky mode' },
    ], PW + LX + 20, svgH);
  };

  figs.UC_Weather = function () {
    var svgH = 220;
    var g = '<g transform="translate(' + LX + ',20)">';
    [3.2, 6.1].forEach(function (p) { g += Guide(p * U, 0, 3 * LH + 8); });
    g += Lane(0, 'Station', null, PW,
      Block(1.4 * U, 1.8 * U, { h: BH, color: SF.neutral })
      + Block(3.2 * U, 2.9 * U, { h: BH, color: SF.missing, label: 'Above 23.5\u00b0C' })
      + Block(6.1 * U, 1.6 * U, { h: BH, color: SF.neutral }));
    g += Lane(LH, 'Forecast', null, PW,
      Block(2.5 * U, 4.7 * U, { h: BH, color: SF.against, label: 'Forecast miss' }));
    g += Lane(2 * LH, 'Window', null, PW,
      Block(3.2 * U, 2.9 * U, { h: BH, color: SF.overlap, label: 'Active' }));
    g += '<g transform="translate(0,' + (3 * LH + 14) + ')">' + Ruler([0, 1.4, 2.5, 3.2, 6.1, 10], TOT, PW) + '</g>';
    g += '</g>';
    return FigCompact(g, [
      { swatch: SF.neutral, term: 'Within range',    desc: 'reading below threshold' },
      { swatch: SF.missing, term: 'Above 23.5\u00b0C', desc: 'threshold predicate true' },
      { swatch: SF.against, term: 'Forecast',        desc: 'expected range — missed the exceedance' },
      { swatch: SF.overlap, term: 'Window',          desc: 'derived threshold-crossing window' },
    ], PW + LX + 20, svgH);
  };

  figs.UC_Payments = function () {
    var svgH = 220;
    var dpX = 4.8 * U;
    var g = '<g transform="translate(' + LX + ',20)">';
    g += Guide(dpX, -16, 3 * LH + 8);
    g += '<text x="' + (dpX + 6) + '" y="-6" font-family="' + SF.fontUI + '" font-size="10" fill="' + SF.ink2 + '">decision</text>';
    g += Lane(0, 'Risk Flag', null, PW,
      Block(2.0 * U, 4.5 * U, { h: BH, color: SF.target, label: 'High risk' }));
    g += Lane(LH, 'Annotation', null, PW,
      Block(5.2 * U, 2.0 * U, { h: BH, color: SF.against, label: 'Late' }));
    g += Lane(2 * LH, 'Known-at', null, PW,
      Block(2.0 * U, 2.8 * U, { h: BH, color: SF.missing, label: 'Knowable' }));
    g += '<g transform="translate(0,' + (3 * LH + 14) + ')">' + Ruler([0, 2.0, 4.8, 6.5, 10], TOT, PW) + '</g>';
    g += '</g>';
    return FigCompact(g, [
      { swatch: SF.target,  term: 'Risk Flag',  desc: 'active at the decision point' },
      { swatch: SF.missing, term: 'Known-at',   desc: 'evidence visible before decision' },
      { swatch: SF.against, term: 'Annotation', desc: 'arrived after — not knowable at decision' },
    ], PW + LX + 20, svgH);
  };

  figs.UC_Logistics = function () {
    var svgH = 220;
    var g = '<g transform="translate(' + LX + ',20)">';
    [1.0, 2.6, 5.0].forEach(function (p) { g += Guide(p * U, 0, 3 * LH + 8); });
    g += Lane(0,      'Carrier',   null, PW, Block(1.0 * U, 3.2 * U, { h: BH, color: SF.target,   label: 'Held' }));
    g += Lane(LH,     'Warehouse', null, PW, Block(2.6 * U, 4.6 * U, { h: BH, color: SF.against,  label: 'Scan gap' }));
    g += Lane(2 * LH, 'Portal',    null, PW, Block(5.0 * U, 2.8 * U, { h: BH, color: SF.residual, label: 'Delayed' }));
    g += '<g transform="translate(0,' + (3 * LH + 14) + ')">' + Ruler([0, 1.0, 2.6, 5.0, 7.8, 10], TOT, PW) + '</g>';
    g += '</g>';
    return FigCompact(g, [
      { swatch: SF.target,   term: 'Carrier',   desc: 'held window — carrier view' },
      { swatch: SF.against,  term: 'Warehouse', desc: 'exception window — scan gap' },
      { swatch: SF.residual, term: 'Portal',    desc: 'delayed — customer-facing view' },
    ], PW + LX + 20, svgH);
  };

  figs.UC_Distributed = function () {
    var svgH = 220;
    var g = '<g transform="translate(' + LX + ',20)">';
    [1.8, 2.2, 3.5, 7.0].forEach(function (p) { g += Guide(p * U, 0, 3 * LH + 8); });
    g += Lane(0,      'Node A', null, PW, Block(1.8 * U, 5.2 * U, { h: BH, color: SF.target,  label: 'Leader' }));
    g += Lane(LH,     'Node B', null, PW, Block(2.2 * U, 4.8 * U, { h: BH, color: SF.against, label: 'Quorum lost' }));
    g += Lane(2 * LH, 'Node C', null, PW, Block(3.5 * U, 4.2 * U, { h: BH, color: SF.against, label: 'Quorum lost' }));
    g += '<g transform="translate(0,' + (3 * LH + 14) + ')">' + Ruler([0, 1.8, 2.2, 3.5, 7.0, 10], TOT, PW) + '</g>';
    g += '</g>';
    return FigCompact(g, [
      { swatch: SF.target,  term: 'Node A',     desc: 'primary — full leader window' },
      { swatch: SF.against, term: 'Node B / C', desc: 'members — quorum lost at staggered times' },
    ], PW + LX + 20, svgH);
  };

  figs.UC_Pipelines = function () {
    var svgH = 220;
    var g = '<g transform="translate(' + LX + ',20)">';
    [1.2, 2.4, 3.8].forEach(function (p) { g += Guide(p * U, 0, 3 * LH + 8); });
    g += Lane(0,      'Ingest',  null, PW, Block(1.2 * U, 5.5 * U, { h: BH, color: SF.target,  label: 'Active' }));
    g += Lane(LH,     'Enrich',  null, PW, Block(2.4 * U, 4.7 * U, { h: BH, color: SF.against, label: 'Lag' }));
    g += Lane(2 * LH, 'Publish', null, PW, Block(3.8 * U, 3.5 * U, { h: BH, color: SF.overlap, label: 'Gap' }));
    g += '<g transform="translate(0,' + (3 * LH + 14) + ')">' + Ruler([0, 1.2, 2.4, 3.8, 7.1, 10], TOT, PW) + '</g>';
    g += '</g>';
    return FigCompact(g, [
      { swatch: SF.target,  term: 'Ingest',  desc: 'upstream active window' },
      { swatch: SF.against, term: 'Enrich',  desc: 'lag — started after ingest' },
      { swatch: SF.overlap, term: 'Publish', desc: 'gap — narrower than upstream' },
    ], PW + LX + 20, svgH);
  };

  // ── Auto-render ────────────────────────────────────────────────────────
  function updateScrollCues() {
    var regions = document.querySelectorAll('[data-sf-scroll-region]');
    for (var i = 0; i < regions.length; i++) {
      var region = regions[i];
      var cue = region.parentElement.querySelector('[data-sf-scroll-cue]');
      if (cue) {
        cue.hidden = region.scrollWidth <= region.clientWidth;
      }
    }
  }

  function render() {
    var els = document.querySelectorAll('[data-fig]');
    for (var i = 0; i < els.length; i++) {
      var el   = els[i];
      var name = el.getAttribute('data-fig');
      if (typeof figs[name] === 'function') {
        el.innerHTML = figs[name]();
      }
    }
    updateScrollCues();
  }

  window.addEventListener('resize', updateScrollCues);

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', render);
  } else {
    render();
  }

})();
