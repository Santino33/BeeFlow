const { useState: useSt } = React;

function Chevron({ open }) {
  return (
    <svg className={`chevron${open ? ' open' : ''}`} width="12" height="12" viewBox="0 0 12 12">
      <polyline points="3,4 6,7.5 9,4" fill="none" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round"></polyline>
    </svg>
  );
}

function PanelSection({ title, icon, defaultOpen = true, children }) {
  const [open, setOpen] = useSt(defaultOpen);
  return (
    <div className="panel-section">
      <button className="section-hd" onClick={() => setOpen(!open)}>
        <span className="section-ico">{icon}</span>
        <span className="section-tl">{title}</span>
        <Chevron open={open} />
      </button>
      <div className={`section-body${open ? ' open' : ''}`}>
        <div className="section-inner">{children}</div>
      </div>
    </div>
  );
}

function RoleBar({ label, value, max, color }) {
  const pct = max > 0 ? Math.min(100, (value / max) * 100) : 0;
  return (
    <div className="role-bar">
      <div className="role-bar-header">
        <span className="role-label">{label}</span>
        <span className="role-value">{value.toLocaleString()}</span>
      </div>
      <div className="role-track">
        <div className="role-fill" style={{ width: `${pct}%`, background: color }}></div>
      </div>
    </div>
  );
}

function DonutChart({ data, colors, size = 56 }) {
  const total = data.reduce((s, v) => s + v, 0);
  if (total === 0) return null;
  const r = (size / 2) - 6;
  const circ = 2 * Math.PI * r;
  let offset = 0;
  return (
    <svg width={size} height={size} style={{ transform: 'rotate(-90deg)', flexShrink: 0 }}>
      {data.map((v, i) => {
        const len = (v / total) * circ;
        const dash = `${len} ${circ - len}`;
        const o = -offset;
        offset += len;
        return <circle key={i} cx={size / 2} cy={size / 2} r={r} fill="none" stroke={colors[i]} strokeWidth="5" strokeDasharray={dash} strokeDashoffset={o} strokeLinecap="round"></circle>;
      })}
    </svg>
  );
}

function LeftPanel({ collapsed, onToggle, colony, health, energy }) {
  const total = colony.queen + colony.nurses + colony.foragers + colony.guards;

  if (collapsed) {
    return (
      <div className="side-panel collapsed-panel left-panel">
        <button className="panel-expand-btn" onClick={onToggle} title="Expandir">
          <svg width="14" height="14" viewBox="0 0 14 14"><polyline points="5,2 10,7 5,12" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round"></polyline></svg>
        </button>
      </div>
    );
  }

  const popIcon = <svg width="13" height="13" viewBox="0 0 14 14" fill="none"><circle cx="5" cy="4" r="2.5" stroke="currentColor" strokeWidth="1.1"></circle><circle cx="10" cy="5" r="1.8" stroke="currentColor" strokeWidth="1.1"></circle><path d="M.5 12.5c0-2.5 2-4.5 4.5-4.5s4.5 2 4.5 4.5" stroke="currentColor" strokeWidth="1.1"></path></svg>;
  const healthIcon = <svg width="13" height="13" viewBox="0 0 14 14" fill="none"><path d="M7 3v8M3 7h8" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round"></path><rect x="1" y="1" width="12" height="12" rx="3" stroke="currentColor" strokeWidth="1.1"></rect></svg>;
  const energyIcon = <svg width="13" height="13" viewBox="0 0 14 14" fill="none"><path d="M8.5 1L4 8h3.5L6 13l5-7H7.5z" stroke="currentColor" strokeWidth="1.1" strokeLinejoin="round"></path></svg>;

  return (
    <aside className="side-panel left-panel">
      <div className="panel-header">
        <svg width="14" height="14" viewBox="0 0 14 14" fill="none"><rect x="1.5" y="3" width="11" height="8" rx="1.5" stroke="currentColor" strokeWidth="1.1"></rect><path d="M4 3V2a3 3 0 0 1 6 0v1" stroke="currentColor" strokeWidth="1.1"></path><circle cx="7" cy="7.5" r="1" fill="currentColor"></circle></svg>
        <span className="panel-title">Inspector de colonia</span>
        <button className="panel-collapse-btn" onClick={onToggle} title="Colapsar">
          <svg width="14" height="14" viewBox="0 0 14 14"><polyline points="9,2 4,7 9,12" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round"></polyline></svg>
        </button>
      </div>
      <div className="panel-scroll">
        <PanelSection title="Población" icon={popIcon}>
          <div className="pop-total">
            <span className="pop-total-label">Total</span>
            <span className="pop-total-value">{total.toLocaleString()}</span>
          </div>
          <RoleBar label="Reina" value={colony.queen} max={1} color="#E8A020" />
          <RoleBar label="Nodrizas" value={colony.nurses} max={total} color="#E8A020" />
          <RoleBar label="Recolectoras" value={colony.foragers} max={total} color="#F0B840" />
          <RoleBar label="Guardianas" value={colony.guards} max={total} color="#D4912A" />
        </PanelSection>

        <PanelSection title="Salud (SIR)" icon={healthIcon}>
          <div className="health-row">
            <DonutChart data={[health.susceptible, health.infected, health.recovered]} colors={['#4CAF72', '#E05252', '#4A90D9']} />
            <div className="health-legend">
              <div className="legend-item"><span className="legend-dot" style={{ background: '#4CAF72' }}></span>Sanos<span className="legend-val">{health.susceptible}%</span></div>
              <div className="legend-item"><span className="legend-dot" style={{ background: '#E05252' }}></span>Infectados<span className="legend-val">{health.infected}%</span></div>
              <div className="legend-item"><span className="legend-dot" style={{ background: '#4A90D9' }}></span>Recuperados<span className="legend-val">{health.recovered}%</span></div>
            </div>
          </div>
        </PanelSection>

        <PanelSection title="Energía" icon={energyIcon}>
          <div className="energy-bars">
            <div className="energy-item">
              <div className="energy-header">
                <span className="energy-label">Reserva de miel</span>
                <span className="energy-val">{energy.honey.toFixed(1)}%</span>
              </div>
              <div className="energy-track"><div className="energy-fill" style={{ width: `${energy.honey}%`, background: '#E8A020' }}></div></div>
            </div>
            <div className="energy-item">
              <div className="energy-header">
                <span className="energy-label">Reserva de polen</span>
                <span className="energy-val">{energy.pollen.toFixed(1)}%</span>
              </div>
              <div className="energy-track"><div className="energy-fill" style={{ width: `${energy.pollen}%`, background: '#F0B840' }}></div></div>
            </div>
          </div>
        </PanelSection>
      </div>
    </aside>
  );
}

Object.assign(window, { LeftPanel, PanelSection, Chevron });
