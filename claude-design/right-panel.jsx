function ParamSlider({ label, value, min, max, step, unit, onChange }) {
  return (
    <div className="slider-row">
      <div className="slider-header">
        <span className="slider-label">{label}</span>
        <span className="slider-val">{typeof value === 'number' ? (step < 1 ? value.toFixed(2) : value) : value}{unit || ''}</span>
      </div>
      <input type="range" min={min} max={max} step={step || 1} value={value} onChange={e => onChange(parseFloat(e.target.value))} />
    </div>
  );
}

function SeasonSelector({ value, onChange }) {
  const seasons = [
    { id: 'spring', label: 'Prim.', color: '#4CAF72' },
    { id: 'summer', label: 'Ver.', color: '#E8A020' },
    { id: 'autumn', label: 'Otoño', color: '#D4712A' },
    { id: 'winter', label: 'Inv.', color: '#4A90D9' }
  ];
  return (
    <div className="slider-row">
      <span className="slider-label" style={{ marginBottom: 4, display: 'block' }}>Fase estacional</span>
      <div className="season-selector">
        {seasons.map(s => (
          <button key={s.id}
            className={`season-btn${value === s.id ? ' active' : ''}`}
            style={value === s.id ? { borderBottom: `2px solid ${s.color}` } : {}}
            onClick={() => onChange(s.id)}>
            {s.label}
          </button>
        ))}
      </div>
    </div>
  );
}

function ParamToggle({ label, value, onChange }) {
  return (
    <div className="toggle-row">
      <span className="toggle-label">{label}</span>
      <div className={`toggle-switch${value ? ' on' : ''}`} onClick={() => onChange(!value)}>
        <div className="toggle-track"></div>
        <div className="toggle-thumb"></div>
      </div>
    </div>
  );
}

function RightPanel({ collapsed, onToggle, params, onParamChange }) {
  const set = (key) => (val) => onParamChange(key, val);

  if (collapsed) {
    return (
      <div className="side-panel collapsed-panel right-panel">
        <button className="panel-expand-btn" onClick={onToggle} title="Expandir">
          <svg width="14" height="14" viewBox="0 0 14 14"><polyline points="9,2 4,7 9,12" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round"></polyline></svg>
        </button>
      </div>
    );
  }

  const envIcon = <svg width="13" height="13" viewBox="0 0 14 14" fill="none"><circle cx="7" cy="7" r="5" stroke="currentColor" strokeWidth="1.1"></circle><path d="M2 7h10M7 2c-2 2-2 8 0 10M7 2c2 2 2 8 0 10" stroke="currentColor" strokeWidth="1.1"></path></svg>;
  const colonyIcon = <svg width="13" height="13" viewBox="0 0 14 14" fill="none"><path d="M7 1L1 5v4l6 4 6-4V5z" stroke="currentColor" strokeWidth="1.1" strokeLinejoin="round"></path><path d="M7 5v4" stroke="currentColor" strokeWidth="1.1"></path></svg>;
  const pherIcon = <svg width="13" height="13" viewBox="0 0 14 14" fill="none"><circle cx="7" cy="7" r="2" stroke="currentColor" strokeWidth="1.1"></circle><circle cx="7" cy="7" r="4.5" stroke="currentColor" strokeWidth="0.8" strokeDasharray="2 2"></circle><circle cx="7" cy="7" r="6.5" stroke="currentColor" strokeWidth="0.6" strokeDasharray="1.5 2.5"></circle></svg>;
  const diseaseIcon = <svg width="13" height="13" viewBox="0 0 14 14" fill="none"><path d="M7 1v3m0 6v3M1 7h3m6 0h3M2.8 2.8l2.1 2.1m4.2 4.2 2.1 2.1M11.2 2.8 9.1 4.9M4.9 9.1l-2.1 2.1" stroke="currentColor" strokeWidth="1.1" strokeLinecap="round"></path><circle cx="7" cy="7" r="1.5" fill="currentColor"></circle></svg>;

  return (
    <aside className="side-panel right-panel">
      <div className="panel-header">
        <svg width="14" height="14" viewBox="0 0 14 14" fill="none"><path d="M5.5 1.5h3v2l1.5 1-1 1.5h-4l-1-1.5 1.5-1z" stroke="currentColor" strokeWidth="1.1" strokeLinejoin="round"></path><path d="M4 6v6.5h6V6" stroke="currentColor" strokeWidth="1.1"></path><path d="M6 9h2" stroke="currentColor" strokeWidth="1.1" strokeLinecap="round"></path></svg>
        <span className="panel-title">Configuración</span>
        <button className="panel-collapse-btn" onClick={onToggle} title="Colapsar">
          <svg width="14" height="14" viewBox="0 0 14 14"><polyline points="5,2 10,7 5,12" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round"></polyline></svg>
        </button>
      </div>
      <div className="panel-scroll">
        <PanelSection title="Entorno" icon={envIcon}>
          <ParamSlider label="Temperatura global" value={params.temperature} min={-10} max={45} step={1} unit="°C" onChange={set('temperature')} />
          <SeasonSelector value={params.season} onChange={set('season')} />
          <ParamSlider label="Presión de pesticidas" value={params.pesticide} min={0} max={100} step={1} unit="%" onChange={set('pesticide')} />
        </PanelSection>

        <PanelSection title="Colonia" icon={colonyIcon}>
          <ParamSlider label="Población inicial" value={params.population} min={500} max={10000} step={100} onChange={set('population')} />
          <ParamSlider label="Tasa metabólica basal" value={params.metabolicRate} min={0.01} max={0.05} step={0.005} onChange={set('metabolicRate')} />
          <ParamSlider label="Umbral de trofalaxia" value={params.trophallaxis} min={10} max={50} step={1} unit="%" onChange={set('trophallaxis')} />
        </PanelSection>

        <PanelSection title="Feromonas" icon={pherIcon}>
          <ParamSlider label="Decay rate" value={params.decayRate} min={0.01} max={0.15} step={0.01} onChange={set('decayRate')} />
          <ParamSlider label="Intensidad de emisión" value={params.emissionIntensity} min={0.1} max={1.0} step={0.05} onChange={set('emissionIntensity')} />
        </PanelSection>

        <PanelSection title="Enfermedad" icon={diseaseIcon}>
          <ParamToggle label="Activar modelo SIR" value={params.sirEnabled} onChange={set('sirEnabled')} />
          {params.sirEnabled && (
            <ParamSlider label="Tasa base de infección" value={params.infectionRate} min={0.01} max={0.20} step={0.01} onChange={set('infectionRate')} />
          )}
        </PanelSection>

        <div style={{ padding: '12px' }}>
          <button className="apply-btn">Aplicar cambios</button>
        </div>
      </div>
    </aside>
  );
}

Object.assign(window, { RightPanel });
