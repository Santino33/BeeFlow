const { useState } = React;

function BeeIcon() {
  return (
    <svg width="20" height="20" viewBox="0 0 24 24" fill="none">
      <ellipse cx="12" cy="13" rx="5" ry="4" fill="#E8A020"/>
      <rect x="7.5" y="11.5" width="9" height="1" rx=".5" fill="#0D0F0E"/>
      <rect x="8.5" y="14" width="7" height="1" rx=".5" fill="#0D0F0E"/>
      <path d="M9 9Q7 5 5 7" stroke="rgba(255,255,255,0.18)" strokeWidth="1.3" fill="none"/>
      <path d="M15 9Q17 5 19 7" stroke="rgba(255,255,255,0.18)" strokeWidth="1.3" fill="none"/>
    </svg>
  );
}

function StatusBadge({ status }) {
  const m = {
    RUNNING: { bg: 'rgba(76,175,114,0.12)', c: '#4CAF72' },
    PAUSED:  { bg: 'rgba(232,160,32,0.15)', c: '#E8A020' },
    STOPPED: { bg: 'rgba(224,82,82,0.12)',   c: '#E05252' }
  };
  const s = m[status] || m.STOPPED;
  return (
    <span className="status-badge" style={{ background: s.bg, color: s.c }}>
      <span className="status-dot" style={{
        background: s.c,
        animation: status === 'RUNNING' ? 'pulse 1.5s ease infinite' : 'none'
      }}></span>
      {status}
    </span>
  );
}

function Topbar({ status, speed, tick, onPlay, onPause, onStop, onSetSpeed }) {
  const speeds = [1, 5, 30, 100];
  const totalSec = Math.floor(tick * 0.5);
  const h = String(Math.floor(totalSec / 3600)).padStart(2, '0');
  const mn = String(Math.floor((totalSec % 3600) / 60)).padStart(2, '0');
  const sc = String(totalSec % 60).padStart(2, '0');

  return (
    <header className="topbar">
      <div className="topbar-section topbar-left">
        <BeeIcon />
        <span className="app-title">BeeFlow</span>
        <span className="app-ver">v0.4.2</span>
      </div>
      <div className="topbar-section topbar-center">
        <div className="sim-controls">
          <button className="ctrl-btn" onClick={status === 'RUNNING' ? onPause : onPlay} title={status === 'RUNNING' ? 'Pausar' : 'Iniciar'}>
            {status === 'RUNNING'
              ? <svg width="12" height="12" viewBox="0 0 12 12"><rect x="1.5" y="1" width="3" height="10" rx=".8" fill="currentColor"></rect><rect x="7.5" y="1" width="3" height="10" rx=".8" fill="currentColor"></rect></svg>
              : <svg width="12" height="12" viewBox="0 0 12 12"><polygon points="2.5,0.5 11,6 2.5,11.5" fill="currentColor"></polygon></svg>}
          </button>
          <button className="ctrl-btn" onClick={onStop} title="Detener">
            <svg width="12" height="12" viewBox="0 0 12 12"><rect x="1.5" y="1.5" width="9" height="9" rx="1.2" fill="currentColor"></rect></svg>
          </button>
        </div>
        <div className="speed-group">
          {speeds.map(s => (
            <button key={s} className={`speed-btn${speed === s ? ' active' : ''}`} onClick={() => onSetSpeed(s)}>×{s}</button>
          ))}
        </div>
        <div className="topbar-divider"></div>
        <div className="tick-info">
          <span className="tick-lbl">Tick</span>
          <span className="tick-val">{tick.toLocaleString()}</span>
        </div>
        <div className="tick-info">
          <span className="tick-lbl">Tiempo</span>
          <span className="tick-val">{h}:{mn}:{sc}</span>
        </div>
        <StatusBadge status={status} />
      </div>
      <div className="topbar-section topbar-right">
        <button className="icon-btn" title="Exportar datos">
          <svg width="15" height="15" viewBox="0 0 16 16" fill="none"><path d="M8 2v8m-3-3 3 3 3-3M3 12v1.5h10V12" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round"></path></svg>
        </button>
        <button className="icon-btn" title="Configuración">
          <svg width="15" height="15" viewBox="0 0 16 16" fill="none"><circle cx="8" cy="8" r="2.2" stroke="currentColor" strokeWidth="1.2"></circle><path d="M8 2v2m0 8v2M2 8h2m8 0h2M3.8 3.8l1.4 1.4m5.6 5.6 1.4 1.4M3.8 12.2l1.4-1.4m5.6-5.6 1.4-1.4" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round"></path></svg>
        </button>
        <button className="icon-btn" title="Ayuda">
          <svg width="15" height="15" viewBox="0 0 16 16" fill="none"><circle cx="8" cy="8" r="5.5" stroke="currentColor" strokeWidth="1.2"></circle><path d="M6.2 6.5a1.8 1.8 0 0 1 2.7 1.5c0 .8-.5 1.2-.9 1.5m0 1.8h.01" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round"></path></svg>
        </button>
      </div>
    </header>
  );
}

Object.assign(window, { Topbar });
