// Style guide artboard — documents the complete visual system so any
// diagram across the site can pull from the same vocabulary.

function StyleGuide() {
  const S = {
    paper: '#faf8f4', panel: '#f5f1e8', ink: '#1a1714',
    ink2: '#5a544c', ink3: '#8a837a', rule: '#d8d0c2',
  };
  const Eyebrow = ({children}) => (
    <div style={{display:'flex', alignItems:'center', gap:10, marginBottom:10, marginTop:28}}>
      <div style={{width:6, height:6, background: S.ink}}/>
      <span style={{fontFamily:'"JetBrains Mono", ui-monospace, Menlo, monospace', fontSize:10,
        letterSpacing:1.6, textTransform:'uppercase', color: S.ink2, fontWeight:600}}>{children}</span>
      <div style={{flex:1, height:1, background: S.rule}}/>
    </div>
  );
  const Row = ({c, name, hex, edge, use}) => (
    <div style={{display:'grid', gridTemplateColumns: '64px 1fr 180px 1fr', alignItems:'center',
      gap:16, padding:'10px 0', borderTop:`1px solid ${S.rule}`}}>
      <div style={{position:'relative', height:26}}>
        <div style={{position:'absolute', inset:0, background: c.fill}}/>
        <div style={{position:'absolute', left:0, right:0, bottom:0, height:4, background: c.edge}}/>
      </div>
      <div>
        <div style={{fontSize:13, fontWeight:700, color: S.ink}}>{name}</div>
        <div style={{fontSize:11, color: S.ink3, fontFamily:'"JetBrains Mono", ui-monospace, Menlo, monospace', marginTop:2}}>{hex} · edge {edge}</div>
      </div>
      <div style={{fontSize:11, color: S.ink2, fontFamily:'"JetBrains Mono", ui-monospace, Menlo, monospace', letterSpacing:0.5}}>{use}</div>
    </div>
  );
  const colors = [
    { c: SF.target,      name: 'Target',       hex:'#2a2520', edge:'#000000', use:'bars under test · primary lane' },
    { c: SF.against,     name: 'Against',      hex:'#c9a87a', edge:'#8f6f3e', use:'comparison lane · cohort contributor' },
    { c: SF.overlap,     name: 'Overlap',      hex:'#3d6b4a', edge:'#224a30', use:'agreement · cohort quorum active · final rows' },
    { c: SF.residual,    name: 'Residual',    hex:'#b85a3a', edge:'#843a1c', use:'target-only evidence · horizon line · warnings' },
    { c: SF.missing,     name: 'Missing',      hex:'#d4b860', edge:'#8f7524', use:'against-only evidence · gaps in target' },
    { c: SF.coverage,    name: 'Coverage',     hex:'#6b8e9a', edge:'#40606a', use:'coverage ratio · as-of queries · reference bands' },
    { c: SF.provisional, name: 'Provisional',  hex:'hatched', edge:'#8f6f3e', use:'still-open windows past the horizon' },
    { c: SF.neutral,     name: 'Neutral',      hex:'#e8e2d4', edge:'#b8ae99', use:'placeholder / informational bars' },
  ];

  return (
    <div style={{width:780, padding:'36px 40px', background: S.paper, border:`1px solid ${S.rule}`,
      fontFamily:'ui-sans-serif, -apple-system, system-ui, sans-serif', color: S.ink}}>
      <div style={{fontFamily:'"JetBrains Mono", ui-monospace, Menlo, monospace', fontSize:10,
        letterSpacing:1.6, textTransform:'uppercase', color: S.ink3, marginBottom:10}}>Style guide · v2</div>
      <div style={{fontSize:24, fontWeight:700, letterSpacing:-0.4, marginBottom:8}}>Spanfold diagram system</div>
      <div style={{fontSize:13, color: S.ink2, lineHeight:1.55, maxWidth:640}}>
        One palette, one type ramp, one set of primitives — every diagram on the site pulls from this sheet.
        If a figure introduces a new colour or a new bar style, it's a bug, not a feature.
      </div>

      <Eyebrow>Palette · bars</Eyebrow>
      <div style={{fontSize:12, color: S.ink3, marginBottom:4, fontStyle:'italic'}}>Eight roles, two tones each (fill + darker bottom edge).</div>
      {colors.map(c => <Row key={c.name} {...c}/>)}
      <div style={{height:1, background: S.rule}}/>

      <Eyebrow>Palette · surfaces & text</Eyebrow>
      <div style={{display:'grid', gridTemplateColumns:'repeat(3, 1fr)', gap:10, marginTop:8}}>
        {[
          {bg:'#faf8f4', label:'Paper', hex:'#faf8f4'},
          {bg:'#f5f1e8', label:'Panel', hex:'#f5f1e8'},
          {bg:'#d8d0c2', label:'Rule',  hex:'#d8d0c2'},
          {bg:'#1a1714', label:'Ink',   hex:'#1a1714', ink:'#fff'},
          {bg:'#5a544c', label:'Ink-2', hex:'#5a544c', ink:'#fff'},
          {bg:'#8a837a', label:'Ink-3', hex:'#8a837a', ink:'#fff'},
        ].map(s => (
          <div key={s.label} style={{background: s.bg, color: s.ink||S.ink, padding:'12px 14px',
            border: `1px solid ${S.rule}`}}>
            <div style={{fontSize:12, fontWeight:700}}>{s.label}</div>
            <div style={{fontSize:10, fontFamily:'"JetBrains Mono", ui-monospace, Menlo, monospace', marginTop:2, opacity:0.8}}>{s.hex}</div>
          </div>
        ))}
      </div>

      <Eyebrow>Bar anatomy</Eyebrow>
      <svg width={700} height={120} viewBox="0 0 700 120" style={{overflow:'visible'}}>
        <Defs/>
        <Block x={40} w={180} h={36} color={SF.target} label="[a, b)"/>
        <g fontFamily="ui-sans-serif" fontSize={10.5} fill={S.ink2}>
          <line x1={40} y1={46} x2={40} y2={74} stroke={S.ink3} strokeWidth={0.8}/>
          <text x={40} y={86} textAnchor="middle">solid left · active at a</text>
          <line x1={220} y1={46} x2={220} y2={74} stroke={S.ink3} strokeWidth={0.8}/>
          <text x={220} y={86} textAnchor="middle">bracket right · open at b</text>
          <line x1={130} y1={40} x2={130} y2={74} stroke={S.ink3} strokeWidth={0.8}/>
          <text x={130} y={98} textAnchor="middle">darker 3px bottom edge</text>
          <line x1={130} y1={10} x2={130} y2={-2} stroke={S.ink3} strokeWidth={0.8}/>
          <text x={130} y={-8} textAnchor="middle">flat top face — no gradients</text>
        </g>

        {/* hatched variant */}
        <g transform="translate(320,0)">
          <rect x={0} y={0} width={180} height={33} fill="url(#sf-hatch)"/>
          <rect x={0} y={33} width={180} height={3} fill="#8f6f3e"/>
          <rect x={0} y={-2} width={2} height={40} fill="#8f6f3e"/>
          <rect x={178} y={-2} width={2} height={5} fill="#8f6f3e"/>
          <rect x={178} y={31} width={2} height={5} fill="#8f6f3e"/>
          <text x={90} y={22} textAnchor="middle" fontFamily="ui-sans-serif" fontSize={12} fontWeight={700} fill={S.ink}>provisional</text>
          <text x={90} y={60} textAnchor="middle" fontFamily="ui-sans-serif" fontSize={10.5} fill={S.ink2}>hatched fill + same bottom edge</text>
        </g>

        {/* thin derived row */}
        <g transform="translate(560,8)">
          <Block x={0} w={120} h={22} color={SF.overlap} halfOpen={false}/>
          <text x={60} y={50} textAnchor="middle" fontFamily="ui-sans-serif" fontSize={10.5} fill={S.ink2}>derived row · 22px tall</text>
          <text x={60} y={64} textAnchor="middle" fontFamily="ui-sans-serif" fontSize={10.5} fill={S.ink2}>no bracket (closed interval)</text>
        </g>
      </svg>

      <Eyebrow>Type ramp</Eyebrow>
      <div style={{display:'grid', gridTemplateColumns:'200px 1fr 140px', rowGap:10, fontSize:13, marginTop:8}}>
        {[
          {s:{fontFamily:'"JetBrains Mono", ui-monospace, Menlo, monospace', fontSize:10, letterSpacing:1.6, textTransform:'uppercase', color: S.ink2, fontWeight:600}, label:'Eyebrow', val:'10 mono · 1.6 tracked'},
          {s:{fontSize:17, fontWeight:700, letterSpacing:-0.3, color: S.ink}, label:'Figure title', val:'17 ui · 700 · -0.3'},
          {s:{fontSize:13, color: S.ink2, lineHeight:1.55}, label:'Body / caption', val:'13 ui · 400 · 1.55'},
          {s:{fontSize:12, fontWeight:700, color: S.ink}, label:'Lane label', val:'12 ui · 700'},
          {s:{fontFamily:'"JetBrains Mono", ui-monospace, Menlo, monospace', fontSize:9.5, letterSpacing:1.4, textTransform:'uppercase', color: S.ink3, fontWeight:600}, label:'Lane sublabel', val:'9.5 mono · 1.4'},
          {s:{fontFamily:'"JetBrains Mono", ui-monospace, Menlo, monospace', fontSize:11, color: S.ink2, fontWeight:600}, label:'Axis tick · interval', val:'11 mono · 600'},
          {s:{fontFamily:'"JetBrains Mono", ui-monospace, Menlo, monospace', fontSize:10.5, color: S.ink3}, label:'Inline caption', val:'10.5 mono'},
        ].map((r,i) => (
          <React.Fragment key={i}>
            <div style={{fontSize:11, color: S.ink3, fontFamily:'"JetBrains Mono", ui-monospace, Menlo, monospace'}}>{r.label}</div>
            <div style={r.s}>The quick brown fox {r.label === 'Axis tick · interval' && '0 3 7 10'}</div>
            <div style={{fontSize:10, color: S.ink3, fontFamily:'"JetBrains Mono", ui-monospace, Menlo, monospace', textAlign:'right'}}>{r.val}</div>
          </React.Fragment>
        ))}
      </div>

      <Eyebrow>Auxiliary elements</Eyebrow>
      <svg width={700} height={110} viewBox="0 0 700 110" style={{overflow:'visible'}}>
        {/* numbered mark */}
        <g transform="translate(30,40)">
          <Mark x={0} y={0} n="1"/>
          <text x={24} y={4} fontFamily="ui-sans-serif" fontSize={11} fill={S.ink2}>numbered callout · ink fill</text>
        </g>
        {/* event dot */}
        <g transform="translate(270,30)">
          <EventDot x={0} label="event"/>
          <text x={24} y={24} fontFamily="ui-sans-serif" fontSize={11} fill={S.ink2}>event dot + tick</text>
        </g>
        {/* guide line */}
        <g transform="translate(460,10)">
          <line x1={0} y1={0} x2={0} y2={60} stroke={S.rule} strokeWidth={1} strokeDasharray="2 3"/>
          <text x={8} y={30} fontFamily="ui-sans-serif" fontSize={11} fill={S.ink2}>dashed column guide</text>
        </g>
        {/* horizon line */}
        <g transform="translate(30,100)">
          <line x1={0} y1={-50} x2={0} y2={-10} stroke={SF.residual.fill} strokeWidth={1.6} strokeDasharray="4 3"/>
          <rect x={-24} y={-62} width={48} height={14} fill={SF.residual.fill}/>
          <text x={0} y={-52} textAnchor="middle" fontFamily='"JetBrains Mono", ui-monospace, Menlo, monospace' fontSize={9.5} fill="#fff" fontWeight={700} letterSpacing={1}>HORIZON</text>
          <text x={40} y={-28} fontFamily="ui-sans-serif" fontSize={11} fill={S.ink2}>horizon line · residual red</text>
        </g>
        {/* segment band */}
        <g transform="translate(330,60)">
          <rect x={0} y={0} width={140} height={28} fill="#f4ecdc"/>
          <rect x={0} y={0} width={2} height={28} fill="#c9a87a"/>
          <text x={70} y={18} textAnchor="middle" fontFamily='"JetBrains Mono", ui-monospace, Menlo, monospace' fontSize={10} fontWeight={700} fill="#8f6f3e" letterSpacing={1}>SEGMENT</text>
          <text x={150} y={18} fontFamily="ui-sans-serif" fontSize={11} fill={S.ink2}>tinted band + left rule</text>
        </g>
      </svg>

      <Eyebrow>Apply to existing graph types</Eyebrow>
      <div style={{fontSize:12, color: S.ink2, lineHeight:1.6, marginTop:4}}>
        <b>Any timeline-style figure</b> on the site maps directly onto this vocabulary. A few concrete translations:
      </div>
      <ul style={{fontSize:12, color: S.ink2, lineHeight:1.7, paddingLeft:18, marginTop:8}}>
        <li><b>state-windows, temporal-model</b> → single-lane figure, target fill, ruler with only open/close positions labelled.</li>
        <li><b>comparing-histories, use-cases, index</b> → two lanes (target + against) stacked, derived rows (residual/overlap/missing) underneath with a "Emits →" divider.</li>
        <li><b>live-finality, querying-history (as-of)</b> → add the horizon line + red tag; any bar crossing it switches from solid fill to hatched.</li>
        <li><b>segments</b> → context as tinted bands behind bars; never peer bars.</li>
        <li><b>cohorts, source-matrix</b> → N contributor rows (against fill), derived rows below the rule using overlap or coverage fill.</li>
        <li><b>rollups, hierarchy</b> → child rows above, dashed fold paths, parent row below using target fill.</li>
        <li><b>advanced-analytics (lead/lag, containment)</b> → reuse the two-lane + derived-row pattern; add a signed offset annotation in mono above the overlap bar.</li>
      </ul>
    </div>
  );
}

Object.assign(window, { StyleGuide });
