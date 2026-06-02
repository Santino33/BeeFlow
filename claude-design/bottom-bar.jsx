function Sparkline({ data, color, width = 110, height = 28 }) {
  if (!data || data.length < 2) return null;
  const min = Math.min(...data);
  const max = Math.max(...data);
  const range = max - min || 1;
  const pad = height * 0.1;

  const pts = data.map((v, i) => {
    const x = (i / (data.length - 1)) * width;
    const y = height - pad - ((v - min) / range) * (height - pad * 2);
    return `${x.toFixed(1)},${y.toFixed(1)}`;
  }).join(' ');

  const areaPts = `0,${height} ${pts} ${width},${height}`;

  return (
    <svg width={width} height={height} style={{ display: 'block', flexShrink: 0 }}>
      <polygon points={areaPts} fill={color} opacity="0.1"></polygon>
      <polyline points={pts} fill="none" stroke={color} strokeWidth="1.4" strokeLinejoin="round" strokeLinecap="round"></polyline>
    </svg>
  );
}

function MetricCard({ label, value, unit, trend, history, color }) {
  const isUp = trend >= 0;
  const arrow = isUp ? '↑' : '↓';
  const trendColor = isUp ? '#4CAF72' : '#E05252';
  const displayVal = typeof value === 'number'
    ? (value >= 1000 ? value.toLocaleString(undefined, { maximumFractionDigits: 0 })
      : value < 10 ? value.toFixed(1) : Math.round(value).toLocaleString())
    : value;

  return (
    <div className="metric-card">
      <div className="metric-top">
        <span className="metric-value">{displayVal}{unit || ''}</span>
        <span className="metric-trend" style={{ color: trendColor }}>{arrow} {Math.abs(trend).toFixed(1)}%</span>
      </div>
      <div className="metric-bottom">
        <span className="metric-label">{label}</span>
        <Sparkline data={history} color={color || '#E8A020'} />
      </div>
    </div>
  );
}

function BottomBar({ metrics }) {
  return (
    <div className="bottom-bar">
      <MetricCard label="Población total" value={metrics.population.value} trend={metrics.population.trend} history={metrics.population.history} color="#E8A020" />
      <MetricCard label="Mortalidad/tick" value={metrics.mortality.value} trend={metrics.mortality.trend} history={metrics.mortality.history} color="#E05252" />
      <MetricCard label="Reserva miel" value={metrics.honey.value} unit="%" trend={metrics.honey.trend} history={metrics.honey.history} color="#F0B840" />
      <MetricCard label="Tasa forrajeo" value={metrics.foraging.value} unit="%" trend={metrics.foraging.trend} history={metrics.foraging.history} color="#4CAF72" />
      <MetricCard label="Infectados" value={metrics.infected.value} trend={metrics.infected.trend} history={metrics.infected.history} color="#4A90D9" />
    </div>
  );
}

Object.assign(window, { BottomBar });
