// Spanfold diagram primitives v2 — block-style, keyed, numbered read-order.
// Every figure has: title strip, SVG plot, and a legend strip. All figures
// share the same ruler, lane height, palette, and type ramp.

const SF = {
  paper:     '#faf8f4',
  panel:     '#f5f1e8',
  ink:       '#1a1714',
  ink2:      '#5a544c',
  ink3:      '#8a837a',
  rule:      '#d8d0c2',
  ruleSoft:  '#e8e2d4',

  // semantic fills — flat, block-style, with a slightly darker bottom edge
  target:    { fill: '#2a2520', edge: '#000', ink: '#ffffff', label: 'target' },
  against:   { fill: '#c9a87a', edge: '#8f6f3e', ink: '#2a2520', label: 'against' },
  overlap:   { fill: '#3d6b4a', edge: '#224a30', ink: '#ffffff', label: 'overlap' },
  residual:  { fill: '#b85a3a', edge: '#843a1c', ink: '#ffffff', label: 'residual' },
  missing:   { fill: '#d4b860', edge: '#8f7524', ink: '#2a2520', label: 'missing' },
  coverage:  { fill: '#6b8e9a', edge: '#40606a', ink: '#ffffff', label: 'coverage' },
  provisional:{ fill: '#e8dcc0', edge: '#8f6f3e', ink: '#2a2520', label: 'provisional' },
  final:     { fill: '#3d6b4a', edge: '#224a30', ink: '#ffffff', label: 'final' },
  neutral:   { fill: '#e8e2d4', edge: '#b8ae99', ink: '#5a544c', label: 'neutral' },

  fontUI: 'ui-sans-serif, -apple-system, "Inter", "Segoe UI", system-ui, sans-serif',
  fontMono: '"JetBrains Mono", "SF Mono", ui-monospace, Menlo, monospace',
};

// ═══ Ruler — one shared position axis ═══════════════════════════════
function Ruler({ positions, total = 10, width }) {
  const unit = width / total;
  return (
    <g>
      <line x1={0} y1={0} x2={width} y2={0} stroke={SF.rule} strokeWidth={1.2}/>
      {Array.from({length: total+1}).map((_,i) => (
        <line key={i} x1={i*unit} y1={0} x2={i*unit} y2={3} stroke={SF.rule} strokeWidth={1}/>
      ))}
      {positions.map(p => (
        <g key={p} transform={`translate(${p*unit},0)`}>
          <line y1={-2} y2={6} stroke={SF.ink2} strokeWidth={1.4}/>
          <text y={20} textAnchor="middle" fontFamily={SF.fontMono} fontSize={11} fill={SF.ink2} fontWeight={600}>{p}</text>
        </g>
      ))}
    </g>
  );
}

// ═══ Block-style window bar ═══════════════════════════════════════
// Flat top face + 3px bottom edge + inner highlight line + soft shadow
// below — reads as a physical block that lifts off the paper.
function Block({ x, w, h = 34, color = SF.target, label, halfOpen = true }) {
  const edge = 3;
  return (
    <g transform={`translate(${x},0)`}>
      {/* soft shadow below */}
      <rect x={1} y={h+1} width={w} height={3} fill="rgba(26,23,20,0.12)"/>
      {/* darker bottom edge */}
      <rect x={0} y={h-edge} width={w} height={edge} fill={color.edge}/>
      {/* top face */}
      <rect x={0} y={0} width={w} height={h-edge} fill={color.fill}/>
      {/* inner highlight line — 1px inner top */}
      <rect x={2} y={1} width={w-4} height={1} fill="rgba(255,255,255,0.18)"/>
      {/* solid opening edge */}
      <rect x={0} y={-2} width={2} height={h+4} fill={color.edge}/>
      {halfOpen && (
        <>
          {/* open closing edge — bracket made of two short tabs, not a full edge */}
          <rect x={w-2} y={-2} width={2} height={5} fill={color.edge}/>
          <rect x={w-2} y={h-3} width={2} height={5} fill={color.edge}/>
        </>
      )}
      {label && (
        <text x={w/2} y={h/2+1} textAnchor="middle" dominantBaseline="middle"
          fontFamily={SF.fontUI} fontSize={12} fontWeight={600} fill={color.ink}
          letterSpacing={0.1}>
          {label}
        </text>
      )}
    </g>
  );
}

// ═══ Numbered reading-order circle (① ② ③) ══════════════════════
function Mark({ x, y, n, color = SF.ink }) {
  return (
    <g transform={`translate(${x},${y})`}>
      <circle r={10} fill={color} stroke="#faf8f4" strokeWidth={2}/>
      <text y={1} textAnchor="middle" dominantBaseline="middle"
        fontFamily={SF.fontMono} fontSize={11} fontWeight={700} fill="#faf8f4">{n}</text>
    </g>
  );
}

// ═══ Event marker — small dot + tick from timeline ═════════════════
function EventDot({ x, label, direction = 'up', color = SF.ink }) {
  const up = direction === 'up';
  return (
    <g transform={`translate(${x},0)`}>
      <line x1={0} y1={up ? -4 : 4} x2={0} y2={up ? -16 : 16} stroke={color} strokeWidth={1.2}/>
      <circle cx={0} cy={up ? -20 : 20} r={3.5} fill={color}/>
      <text x={0} y={up ? -30 : 38} textAnchor="middle"
        fontFamily={SF.fontUI} fontSize={11} fontWeight={600} fill={color}>{label}</text>
    </g>
  );
}

// ═══ Lane — one labelled row ═══════════════════════════════════════
function Lane({ y, label, sublabel, children, width, strong = true }) {
  return (
    <g transform={`translate(0,${y})`}>
      <text x={-14} y={20} textAnchor="end" fontFamily={SF.fontUI} fontSize={12}
        fontWeight={strong ? 700 : 500} fill={SF.ink}>{label}</text>
      {sublabel && <text x={-14} y={34} textAnchor="end"
        fontFamily={SF.fontMono} fontSize={9.5} fill={SF.ink3} letterSpacing={0.4}
        style={{textTransform:'uppercase'}}>{sublabel}</text>}
      <line x1={0} y1={17} x2={width} y2={17} stroke={SF.ruleSoft} strokeWidth={1}/>
      {children}
    </g>
  );
}

// ═══ Legend strip — keyed explanations docked under every plot ═══
function Key({ items }) {
  return (
    <div style={{
      display: 'flex', flexWrap: 'wrap', gap: '14px 28px',
      padding: '16px 32px 4px',
      borderTop: `1px solid ${SF.rule}`,
      background: SF.panel,
      fontFamily: SF.fontUI,
    }}>
      {items.map((it, i) => (
        <div key={i} style={{display:'flex', alignItems:'flex-start', gap:10, maxWidth: 220, fontSize:11.5, lineHeight:1.45, color: SF.ink2}}>
          {it.swatch && (
            <div style={{flex:'0 0 auto', width:20, height:14, marginTop:2, position:'relative'}}>
              <div style={{position:'absolute', inset:0, background: it.swatch.fill}}/>
              <div style={{position:'absolute', left:0, right:0, bottom:0, height:3, background: it.swatch.edge}}/>
              {it.hatched && <div style={{position:'absolute', inset:0,
                backgroundImage: 'repeating-linear-gradient(45deg, #8f6f3e 0 1.5px, transparent 1.5px 5px)'}}/>}
            </div>
          )}
          {it.mark && (
            <div style={{flex:'0 0 auto', width:20, height:20, marginTop:-1, display:'flex', alignItems:'center', justifyContent:'center',
              borderRadius:10, background: SF.ink, color:'#faf8f4',
              fontFamily: SF.fontMono, fontSize:11, fontWeight:700}}>{it.mark}</div>
          )}
          <div>
            <span style={{fontWeight:700, color: SF.ink}}>{it.term}</span>
            {it.desc && <span style={{color: SF.ink2}}> — {it.desc}</span>}
          </div>
        </div>
      ))}
    </div>
  );
}

// ═══ Figure frame — eyebrow, title, SVG plot, legend strip ═════════
function Figure({ eyebrow, title, children, keyItems, w = 780, h = 280, plotInsetY = 40 }) {
  return (
    <div style={{
      position: 'relative',
      background: SF.paper,
      backgroundImage: 'radial-gradient(rgba(26,23,20,0.04) 1px, transparent 1px)',
      backgroundSize: '3px 3px',
      border: `1px solid ${SF.rule}`,
      fontFamily: SF.fontUI,
      overflow: 'hidden',
    }}>
      {/* corner accent tick — top-right */}
      <div style={{position:'absolute', top:14, right:14, display:'flex', gap:3, alignItems:'center'}}>
        <div style={{width:10, height:2, background: SF.residual.fill}}/>
        <div style={{width:2, height:2, background: SF.residual.fill}}/>
        <div style={{width:2, height:2, background: SF.residual.fill}}/>
      </div>
      <div style={{padding: '22px 32px 0', position:'relative'}}>
        <div style={{display:'flex', alignItems:'center', gap:10, marginBottom:10}}>
          <div style={{width:6, height:6, background: SF.ink}}/>
          <span style={{fontFamily: SF.fontMono, fontSize:10, letterSpacing:1.6,
            textTransform:'uppercase', color: SF.ink2, fontWeight:600}}>{eyebrow}</span>
        </div>
        <div style={{fontSize:17, fontWeight:700, color: SF.ink, letterSpacing:-0.3,
          lineHeight:1.3, maxWidth: 640, marginBottom:4}}>{title}</div>
        {/* signature underline — 3-segment rule in accent colours */}
        <div style={{display:'flex', gap:3, marginTop:12}}>
          <div style={{width:22, height:3, background: SF.target.fill}}/>
          <div style={{width:6, height:3, background: SF.overlap.fill}}/>
          <div style={{width:3, height:3, background: SF.residual.fill}}/>
        </div>
      </div>
      <div style={{padding: `${plotInsetY}px 32px 32px`, display:'flex', justifyContent:'center'}}>
        <svg width={w-64} height={h} viewBox={`0 0 ${w-64} ${h}`} style={{overflow:'visible'}}>
          <Defs/>
          {children}
        </svg>
      </div>
      {keyItems && keyItems.length > 0 && <Key items={keyItems}/>}
    </div>
  );
}

function Defs() {
  return (
    <defs>
      <pattern id="sf-hatch" patternUnits="userSpaceOnUse" width={6} height={6} patternTransform="rotate(45)">
        <rect width={6} height={6} fill="#e8dcc0"/>
        <line x1={0} y1={0} x2={0} y2={6} stroke="#8f6f3e" strokeWidth={1.6}/>
      </pattern>
    </defs>
  );
}

Object.assign(window, { SF, Ruler, Block, Mark, EventDot, Lane, Key, Figure, Defs });
