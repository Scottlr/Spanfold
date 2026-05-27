// Six redrawn Spanfold figures using the v2 block-style primitives.
// Every figure ships with a <Key items=[]> explaining its vocabulary.

const PW = 680;   // plot width

// ─── helpers ─────────────────────────────────────────────────────
function Guide({ x, top=0, bottom=260 }) {
  return <line x1={x} y1={top} x2={x} y2={bottom} stroke={SF.rule} strokeWidth={1} strokeDasharray="2 3"/>;
}

// ═══ 01. A window, not a flag ═════════════════════════════════════
function D_Window() {
  const total = 10, u = PW/total;
  return (
    <Figure
      eyebrow="01 · State window"
      title="A predicate is true over a range — not just now. Events frame a half-open window."
      w={780} h={220}
      keyItems={[
        { swatch: SF.target, term: 'window', desc: 'the predicate is active over this range' },
        { mark: '1', term: 'opening event', desc: 'predicate became true → window opens' },
        { mark: '2', term: 'closing event', desc: 'predicate became false → window closes' },
        { mark: '3', term: 'half-open interval', desc: '[a, b) — active at a, no longer active at b' },
      ]}>
      <g transform="translate(60,70)">
        <EventDot x={3*u} label="offline" color={SF.residual.fill}/>
        <EventDot x={7*u} label="online"  color={SF.overlap.fill}/>
        <Mark x={3*u} y={-48} n="1" color={SF.residual.fill}/>
        <Mark x={7*u} y={-48} n="2" color={SF.overlap.fill}/>

        <Lane y={0} label="device-17" sublabel="DeviceOffline" width={PW}>
          <Block x={3*u} w={4*u} color={SF.target} label="offline — 4 positions"/>
        </Lane>

        <g transform={`translate(0,80)`}>
          <Ruler positions={[0,3,7,10]} total={total} width={PW}/>
        </g>

        <g transform={`translate(${3*u},120)`}>
          <Mark x={0} y={0} n="3"/>
          <text x={18} y={4} fontFamily={SF.fontMono} fontSize={13} fontWeight={700} fill={SF.ink}>[3, 7)</text>
          <text x={80} y={4} fontFamily={SF.fontUI} fontSize={11} fill={SF.ink2}>closed on left, open on right</text>
        </g>
      </g>
    </Figure>
  );
}

// ═══ 02. Overlap / Residual / Missing ═════════════════════════════
function D_Compare() {
  const total = 10, u = PW/total;
  return (
    <Figure
      eyebrow="02 · Comparing histories"
      title="One comparison plan emits three row kinds derived from aligned ranges."
      w={780} h={360}
      keyItems={[
        { swatch: SF.target,   term: 'target',   desc: 'the lane being tested' },
        { swatch: SF.against,  term: 'against',  desc: 'the lane expected to explain or cover it' },
        { swatch: SF.residual, term: 'residual', desc: 'target-only — usually under-coverage' },
        { swatch: SF.overlap,  term: 'overlap',  desc: 'both active — agreement evidence' },
        { swatch: SF.missing,  term: 'missing',  desc: 'against-only — target missed it' },
      ]}>
      <g transform="translate(80,10)">
        <Lane y={0} label="Target" sublabel="provider-a" width={PW}>
          <Block x={2*u} w={5*u} color={SF.target} label="offline"/>
        </Lane>
        <Lane y={64} label="Against" sublabel="provider-b" width={PW}>
          <Block x={4*u} w={5*u} color={SF.against} label="offline"/>
        </Lane>

        <line x1={0} y1={126} x2={PW} y2={126} stroke={SF.rule}/>
        <text x={-72} y={144} fontFamily={SF.fontMono} fontSize={9.5} letterSpacing={1.4}
          fontWeight={600} fill={SF.ink3} style={{textTransform:'uppercase'}}>Emits →</text>

        {[2,4,7,9].map(p => <Guide key={p} x={p*u} top={16} bottom={255}/>)}

        <g transform="translate(0,148)">
          <text x={-14} y={16} textAnchor="end" fontFamily={SF.fontUI} fontSize={12} fontWeight={700} fill={SF.residual.fill}>residual</text>
          <Block x={2*u} w={2*u} h={22} color={SF.residual} halfOpen={false}/>
          <text x={4*u+10} y={16} fontFamily={SF.fontMono} fontSize={10.5} fill={SF.ink3}>[2, 4)</text>
        </g>
        <g transform="translate(0,180)">
          <text x={-14} y={16} textAnchor="end" fontFamily={SF.fontUI} fontSize={12} fontWeight={700} fill={SF.overlap.fill}>overlap</text>
          <Block x={4*u} w={3*u} h={22} color={SF.overlap} halfOpen={false}/>
          <text x={7*u+10} y={16} fontFamily={SF.fontMono} fontSize={10.5} fill={SF.ink3}>[4, 7)</text>
        </g>
        <g transform="translate(0,212)">
          <text x={-14} y={16} textAnchor="end" fontFamily={SF.fontUI} fontSize={12} fontWeight={700} fill={SF.missing.edge}>missing</text>
          <Block x={7*u} w={2*u} h={22} color={SF.missing} halfOpen={false}/>
          <text x={9*u+10} y={16} fontFamily={SF.fontMono} fontSize={10.5} fill={SF.ink3}>[7, 9)</text>
        </g>

        <g transform="translate(0,260)">
          <Ruler positions={[0,2,4,7,9,10]} total={total} width={PW}/>
        </g>
      </g>
    </Figure>
  );
}

// ═══ 03. Live vs final (horizon) ══════════════════════════════════
function D_Horizon() {
  const total = 10, u = PW/total, hz = 6.5;
  return (
    <Figure
      eyebrow="03 · Live finality"
      title="The horizon splits final evidence from provisional rows derived from open windows."
      w={780} h={240}
      keyItems={[
        { swatch: SF.final, term: 'final', desc: 'window closed before the horizon — stable evidence' },
        { swatch: SF.provisional, hatched: true, term: 'provisional', desc: 'window still open at the horizon — may change' },
        { mark: 'H', term: 'horizon', desc: 'the moment the live run asks its question' },
      ]}>
      <g transform="translate(80,30)">
        {/* horizon spans behind everything */}
        <rect x={hz*u} y={-20} width={PW - hz*u} height={180} fill="#faf2e4" opacity={0.6}/>
        <line x1={hz*u} y1={-20} x2={hz*u} y2={160} stroke={SF.residual.fill} strokeWidth={1.6} strokeDasharray="4 3"/>
        <g transform={`translate(${hz*u},-28)`}>
          <rect x={-32} y={-10} width={64} height={18} fill={SF.residual.fill}/>
          <text y={3} textAnchor="middle" fontFamily={SF.fontMono} fontSize={10.5}
            fill="#fff" fontWeight={700} letterSpacing={1}>HORIZON</text>
        </g>

        <Lane y={0} label="device-04" sublabel="closed" width={PW}>
          <Block x={1*u} w={3*u} color={SF.final} label="offline · final"/>
        </Lane>
        <Lane y={64} label="device-17" sublabel="open" width={PW}>
          <Block x={4*u} w={(hz-4)*u} color={SF.target} label="known" halfOpen={false}/>
          {/* hatched provisional extension */}
          <g transform={`translate(${hz*u},0)`}>
            <rect x={0} y={31} width={2*u} height={3} fill={SF.provisional.edge}/>
            <rect x={0} y={0} width={2*u} height={31} fill="url(#sf-hatch)"/>
            <rect x={0} y={-2} width={2} height={36} fill={SF.provisional.edge}/>
            <rect x={2*u-2} y={-2} width={2} height={5} fill={SF.provisional.edge}/>
            <rect x={2*u-2} y={29} width={2} height={5} fill={SF.provisional.edge}/>
            <text x={u} y={18} textAnchor="middle" fontFamily={SF.fontUI} fontSize={12} fontWeight={700} fill={SF.ink}>provisional →</text>
          </g>
        </Lane>

        <g transform="translate(0,140)">
          <Ruler positions={[0,1,4,6.5,10]} total={total} width={PW}/>
        </g>
      </g>
    </Figure>
  );
}

// ═══ 04. Segments ═════════════════════════════════════════════════
function D_Segments() {
  const total = 10, u = PW/total;
  const segs = [
    {x: 0, w: 3, label: 'warmup',   bg: '#eef0e8', edge: '#aeb89c'},
    {x: 3, w: 4, label: 'steady',   bg: '#f4ecdc', edge: '#c9a87a'},
    {x: 7, w: 3, label: 'cooldown', bg: '#e8e0e8', edge: '#b89cb8'},
  ];
  return (
    <Figure
      eyebrow="04 · Segments & tags"
      title="Segments are context tinted behind the timeline — windows inherit the segment they open inside."
      w={780} h={220}
      keyItems={[
        { swatch: { fill: '#f4ecdc', edge: '#c9a87a' }, term: 'segment band', desc: 'a phase or period — pure context, not a window' },
        { swatch: SF.target, term: 'window', desc: 'inherits the segment + tags it opened in' },
        { mark: 'T', term: 'tag', desc: 'descriptive metadata preserved through export' },
      ]}>
      <g transform="translate(80,30)">
        {segs.map(s => (
          <g key={s.label}>
            <rect x={s.x*u} y={-10} width={s.w*u} height={64} fill={s.bg}/>
            <rect x={s.x*u} y={-10} width={2} height={64} fill={s.edge}/>
            <text x={s.x*u + s.w*u/2} y={-18} textAnchor="middle"
              fontFamily={SF.fontMono} fontSize={10} fontWeight={700} fill={s.edge}
              letterSpacing={1} style={{textTransform:'uppercase'}}>{s.label}</text>
          </g>
        ))}

        <Lane y={0} label="device-17" sublabel="DeviceOffline" width={PW}>
          <Block x={2*u} w={3*u} color={SF.target} label="offline"/>
          <Block x={7.5*u} w={1.5*u} color={SF.target} label="offline"/>
        </Lane>

        <g transform="translate(0,75)">
          <Ruler positions={[0,3,7,10]} total={total} width={PW}/>
        </g>

        {/* tag row */}
        <g transform="translate(0,120)">
          <text x={-14} y={0} textAnchor="end" fontFamily={SF.fontUI} fontSize={11} fontWeight={700} fill={SF.ink2}>tags</text>
          {[
            {x: 2*u, label: 'region=eu'},
            {x: 2*u+88, label: 'severity=hi'},
            {x: 2*u+196, label: 'team=infra'},
          ].map((t,i) => (
            <g key={i} transform={`translate(${t.x},-12)`}>
              <rect width={84} height={20} fill={SF.panel} stroke={SF.rule}/>
              <text x={10} y={14} fontFamily={SF.fontMono} fontSize={10.5} fill={SF.ink}>{t.label}</text>
            </g>
          ))}
        </g>
      </g>
    </Figure>
  );
}

// ═══ 05. Cohort quorum ════════════════════════════════════════════
function D_Cohort() {
  const total = 12, u = PW/total;
  return (
    <Figure
      eyebrow="05 · Cohorts"
      title="A cohort reduces several contributor lanes to a single derived lane via a quorum rule."
      w={780} h={320}
      keyItems={[
        { swatch: SF.against, term: 'contributor', desc: 'one member of the cohort group' },
        { swatch: SF.overlap, term: 'cohort active', desc: 'quorum rule satisfied — here ≥ 2 of 3' },
        { swatch: SF.target,  term: 'primary', desc: 'compared against the cohort' },
      ]}>
      <g transform="translate(90,10)">
        {[
          {y: 0,   label: 'gw-1', x: 1, w: 4},
          {y: 46,  label: 'gw-2', x: 2, w: 5},
          {y: 92,  label: 'gw-3', x: 3, w: 3},
        ].map((l,i) => (
          <Lane key={i} y={l.y} strong={false} label={l.label} sublabel="contributor" width={PW}>
            <Block x={l.x*u} w={l.w*u} h={28} color={SF.against}/>
          </Lane>
        ))}

        <line x1={0} y1={140} x2={PW} y2={140} stroke={SF.rule}/>
        <text x={-74} y={158} fontFamily={SF.fontMono} fontSize={9.5} letterSpacing={1.4} fontWeight={600}
          fill={SF.ink3} style={{textTransform:'uppercase'}}>Derived →</text>

        <g transform="translate(0,154)">
          <Lane y={0} label="cohort" sublabel="≥ 2 of 3" width={PW}>
            <Block x={2*u} w={4*u} color={SF.overlap} label="quorum active"/>
          </Lane>
        </g>
        <g transform="translate(0,214)">
          <Lane y={0} label="primary" sublabel="compared" width={PW}>
            <Block x={3.5*u} w={2*u} color={SF.target} label="offline"/>
          </Lane>
        </g>

        <g transform="translate(0,275)">
          <Ruler positions={[0,2,6,12]} total={total} width={PW}/>
        </g>
      </g>
    </Figure>
  );
}

// ═══ 06. Roll-ups ═════════════════════════════════════════════════
function D_Rollup() {
  const total = 12, u = PW/total;
  return (
    <Figure
      eyebrow="06 · Roll-ups & hierarchy"
      title="Fold many child windows into a parent summary without losing the constituent rows."
      w={780} h={320}
      keyItems={[
        { swatch: SF.against, term: 'child window', desc: 'leaf-level evidence from one contributor' },
        { swatch: SF.target,  term: 'parent roll-up', desc: 'opens when any child is active, closes when all close' },
        { mark: '↓', term: 'fold path', desc: 'each child keeps a pointer into its parent for drill-down' },
      ]}>
      <g transform="translate(90,10)">
        {[
          {y: 0,  label: 'pod-a', x: 1,   w: 3},
          {y: 46, label: 'pod-b', x: 2.5, w: 2},
          {y: 92, label: 'pod-c', x: 6,   w: 3},
        ].map((l,i) => (
          <Lane key={i} y={l.y} strong={false} label={l.label} sublabel="child" width={PW}>
            <Block x={l.x*u} w={l.w*u} h={28} color={SF.against}/>
          </Lane>
        ))}

        <g stroke={SF.ink3} strokeWidth={1} fill="none" strokeDasharray="2 3">
          <path d={`M${1*u},28 C${1*u},120 ${1*u},160 ${1*u},186`}/>
          <path d={`M${2.5*u},74 C${2.5*u},140 ${2.5*u},170 ${2.5*u},186`}/>
          <path d={`M${6*u},120 C${6*u},150 ${6*u},175 ${6*u},186`}/>
        </g>

        <g transform="translate(0,186)">
          <Lane y={0} label="cluster" sublabel="parent" width={PW}>
            <Block x={1*u} w={8*u} color={SF.target} label="degraded · 3 children"/>
          </Lane>
        </g>

        <g transform="translate(0,260)">
          <Ruler positions={[0,1,4,6,9,12]} total={total} width={PW}/>
        </g>
      </g>
    </Figure>
  );
}

Object.assign(window, { D_Window, D_Compare, D_Horizon, D_Segments, D_Cohort, D_Rollup });
