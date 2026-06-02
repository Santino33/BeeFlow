const { useRef, useEffect, useState: useGridState, useCallback } = React;

const GRID_SIZE = 40;

function generateGridData(tick) {
  const grid = [];
  const hx = 20, hy = 20, hr = 3.8;
  const resources = [
    { x: 6, y: 7, r: 3 }, { x: 34, y: 10, r: 2.5 }, { x: 8, y: 33, r: 2.8 },
    { x: 32, y: 34, r: 3 }, { x: 15, y: 4, r: 2.2 }, { x: 28, y: 5, r: 2 }
  ];
  const obstacles = [
    { x: 25, y: 10, w: 3, h: 2 }, { x: 10, y: 18, w: 2, h: 3 },
    { x: 31, y: 26, w: 3, h: 2 }, { x: 16, y: 28, w: 2, h: 2 }
  ];
  const t = tick * 0.03;

  for (let y = 0; y < GRID_SIZE; y++) {
    const row = [];
    for (let x = 0; x < GRID_SIZE; x++) {
      let pheromone = 0, resource = 0, agents = 0, obstacle = false, isHive = false;

      const dh = Math.sqrt((x - hx) ** 2 + (y - hy) ** 2);
      if (dh < hr) {
        isHive = true;
        pheromone = Math.max(0.5, 1 - dh / hr);
        agents = 2 + Math.floor(Math.abs(Math.sin(t + x * 3 + y * 7)) * 4);
      }

      for (const res of resources) {
        const dx = res.x - hx, dy = res.y - hy;
        const len = Math.sqrt(dx * dx + dy * dy);
        if (len === 0) continue;
        const nx = dx / len, ny = dy / len;
        const px = x - hx, py = y - hy;
        const proj = px * nx + py * ny;
        if (proj > 0 && proj < len) {
          const perp = Math.abs(px * ny - py * nx);
          if (perp < 1.6) {
            const intensity = (1 - perp / 1.6) * 0.4;
            const noise = Math.sin(t * 0.8 + x * 0.7 + y * 0.5) * 0.06;
            pheromone = Math.max(pheromone, Math.max(0, intensity + noise));
            const agentChance = 0.06 + Math.sin(t * 1.5 + proj * 0.4) * 0.04;
            if (Math.abs(Math.sin(t * 2 + proj * 0.8 + perp * 5)) < agentChance * 3) agents = 1;
          }
        }
        const dr = Math.sqrt((x - res.x) ** 2 + (y - res.y) ** 2);
        if (dr < res.r) {
          resource = Math.max(resource, (1 - dr / res.r) * (0.5 + Math.sin(t * 0.3 + res.x) * 0.2));
        }
      }

      for (const obs of obstacles) {
        if (x >= obs.x && x < obs.x + obs.w && y >= obs.y && y < obs.y + obs.h) {
          obstacle = true; pheromone = 0; agents = 0; resource = 0;
        }
      }
      row.push({ pheromone, resource, agents, obstacle, isHive });
    }
    grid.push(row);
  }
  return grid;
}

function renderGrid(canvas, grid, zoom) {
  const ctx = canvas.getContext('2d');
  const w = canvas.width, h = canvas.height;
  ctx.clearRect(0, 0, w, h);

  const cellW = (w / GRID_SIZE) * zoom;
  const cellH = (h / GRID_SIZE) * zoom;
  const offX = (w - cellW * GRID_SIZE) / 2;
  const offY = (h - cellH * GRID_SIZE) / 2;

  for (let y = 0; y < GRID_SIZE; y++) {
    for (let x = 0; x < GRID_SIZE; x++) {
      const cell = grid[y][x];
      const cx = offX + x * cellW, cy = offY + y * cellH;
      const cw = cellW - 0.5, ch = cellH - 0.5;

      ctx.fillStyle = '#161916';
      ctx.fillRect(cx, cy, cw, ch);

      if (cell.obstacle) {
        ctx.fillStyle = '#1E211E';
        ctx.fillRect(cx, cy, cw, ch);
        continue;
      }

      if (cell.isHive) {
        ctx.fillStyle = `rgba(232,160,32,${0.25 + cell.pheromone * 0.45})`;
        ctx.fillRect(cx, cy, cw, ch);
      } else if (cell.pheromone > 0.02) {
        ctx.fillStyle = `rgba(232,160,32,${cell.pheromone * 0.55})`;
        ctx.fillRect(cx, cy, cw, ch);
      }

      if (cell.resource > 0.05) {
        ctx.fillStyle = `rgba(76,175,114,${cell.resource * 0.5})`;
        ctx.fillRect(cx, cy, cw, ch);
      }

      if (cell.agents > 0) {
        const dotR = Math.min(cw * 0.25, 3.5);
        ctx.fillStyle = cell.isHive ? 'rgba(255,235,180,0.9)' : 'rgba(255,255,255,0.85)';
        ctx.beginPath();
        ctx.arc(cx + cw / 2, cy + ch / 2, dotR, 0, Math.PI * 2);
        ctx.fill();
        if (cell.agents > 2) {
          ctx.beginPath();
          ctx.arc(cx + cw * 0.3, cy + ch * 0.35, dotR * 0.6, 0, Math.PI * 2);
          ctx.fill();
        }
      }
    }
  }
}

function CellTooltip({ x, y, data }) {
  return (
    <div className="cell-tooltip" style={{ left: x + 14, top: y - 10 }}>
      <div className="tooltip-title">Celda [{data.gridX}, {data.gridY}]</div>
      <div className="tooltip-row"><span>Feromona</span><span className="tooltip-val">{data.pheromone.toFixed(3)}</span></div>
      <div className="tooltip-row"><span>Recursos</span><span className="tooltip-val">{data.resource.toFixed(3)}</span></div>
      <div className="tooltip-row"><span>Agentes</span><span className="tooltip-val">{data.agents}</span></div>
      <div className="tooltip-row"><span>Tipo</span><span className="tooltip-val">{data.isHive ? 'Colmena' : data.obstacle ? 'Obstáculo' : 'Terreno'}</span></div>
    </div>
  );
}

function GridLegend() {
  return (
    <div className="grid-legend">
      <div className="legend-row"><span className="legend-swatch" style={{ background: '#E8A020' }}></span>Feromonas</div>
      <div className="legend-row"><span className="legend-swatch" style={{ background: '#4CAF72' }}></span>Recursos</div>
      <div className="legend-row"><span className="legend-swatch" style={{ background: '#fff', width: 6, height: 6, borderRadius: '50%', margin: '0 2px' }}></span>Agentes</div>
      <div className="legend-row"><span className="legend-swatch" style={{ background: '#1E211E', border: '1px solid rgba(255,255,255,0.08)' }}></span>Obstáculos</div>
      <div className="legend-row"><span className="legend-swatch" style={{ background: 'rgba(232,160,32,0.55)' }}></span>Colmena</div>
    </div>
  );
}

function SimGrid({ gridData }) {
  const containerRef = useRef(null);
  const canvasRef = useRef(null);
  const [tooltip, setTooltip] = useGridState(null);
  const [zoom, setZoom] = useGridState(1);

  useEffect(() => {
    const container = containerRef.current;
    const canvas = canvasRef.current;
    if (!container || !canvas) return;
    const ro = new ResizeObserver(() => {
      const s = Math.min(container.clientWidth - 24, container.clientHeight - 24);
      canvas.width = s;
      canvas.height = s;
      renderGrid(canvas, gridData, zoom);
    });
    ro.observe(container);
    return () => ro.disconnect();
  }, []);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (canvas && canvas.width > 0) renderGrid(canvas, gridData, zoom);
  }, [gridData, zoom]);

  const handleMove = useCallback((e) => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const rect = canvas.getBoundingClientRect();
    const mx = e.clientX - rect.left, my = e.clientY - rect.top;
    const cellW = (canvas.width / GRID_SIZE) * zoom;
    const offX = (canvas.width - cellW * GRID_SIZE) / 2;
    const offY = (canvas.height - cellW * GRID_SIZE) / 2;
    const gx = Math.floor((mx - offX) / cellW);
    const gy = Math.floor((my - offY) / cellW);
    if (gx >= 0 && gx < GRID_SIZE && gy >= 0 && gy < GRID_SIZE) {
      const cell = gridData[gy][gx];
      setTooltip({ x: e.clientX, y: e.clientY, data: { ...cell, gridX: gx, gridY: gy } });
    } else {
      setTooltip(null);
    }
  }, [gridData, zoom]);

  return (
    <div ref={containerRef} className="grid-container">
      <canvas ref={canvasRef} className="grid-canvas" onMouseMove={handleMove} onMouseLeave={() => setTooltip(null)}></canvas>
      {tooltip && <CellTooltip {...tooltip} />}
      <GridLegend />
      <div className="zoom-controls">
        <button className="zoom-btn" onClick={() => setZoom(z => Math.min(2, z + 0.15))}>+</button>
        <button className="zoom-btn" onClick={() => setZoom(z => Math.max(0.6, z - 0.15))}>−</button>
      </div>
    </div>
  );
}

Object.assign(window, { SimGrid, generateGridData });
