const { useState: useAppState, useEffect: useAppEffect, useCallback: useAppCb, useRef: useAppRef } = React;

function genHistory(base, variance, len = 60) {
  const d = [base];
  for (let i = 1; i < len; i++) d.push(Math.max(0, d[i - 1] + (Math.random() - 0.48) * variance));
  return d;
}

function clamp(v, lo, hi) { return Math.max(lo, Math.min(hi, v)); }

function App() {
  const [status, setStatus] = useAppState('PAUSED');
  const [speed, setSpeed] = useAppState(1);
  const [tick, setTick] = useAppState(247);
  const [leftOpen, setLeftOpen] = useAppState(true);
  const [rightOpen, setRightOpen] = useAppState(true);

  const [colony, setColony] = useAppState({ queen: 1, nurses: 1487, foragers: 2634, guards: 879 });
  const [health, setHealth] = useAppState({ susceptible: 87, infected: 8, recovered: 5 });
  const [energy, setEnergy] = useAppState({ honey: 72.4, pollen: 58.1 });

  const [params, setParams] = useAppState({
    temperature: 22, season: 'summer', pesticide: 15,
    population: 5000, metabolicRate: 0.03, trophallaxis: 30,
    decayRate: 0.08, emissionIntensity: 0.5,
    sirEnabled: true, infectionRate: 0.05
  });

  const [metrics, setMetrics] = useAppState(() => ({
    population: { value: 5001, trend: 2.3, history: genHistory(5000, 50) },
    mortality: { value: 3.2, trend: -0.8, history: genHistory(3, 0.5) },
    honey: { value: 72.4, trend: 1.5, history: genHistory(70, 3) },
    foraging: { value: 68, trend: 0.4, history: genHistory(65, 4) },
    infected: { value: 402, trend: -1.2, history: genHistory(400, 20) }
  }));

  const [gridData, setGridData] = useAppState(() => generateGridData(247));
  const tickRef = useAppRef(tick);
  tickRef.current = tick;

  useAppEffect(() => {
    if (status !== 'RUNNING') return;
    const id = setInterval(() => {
      setTick(t => {
        const next = t + speed;
        if (next % Math.max(1, Math.floor(4 / Math.min(speed, 4))) === 0) {
          setGridData(generateGridData(next));
        }
        setColony(c => ({
          queen: 1,
          nurses: clamp(c.nurses + Math.floor((Math.random() - 0.47) * 4), 800, 2500),
          foragers: clamp(c.foragers + Math.floor((Math.random() - 0.47) * 6), 1500, 4000),
          guards: clamp(c.guards + Math.floor((Math.random() - 0.47) * 3), 400, 1500)
        }));
        setEnergy(e => ({
          honey: clamp(e.honey + (Math.random() - 0.47) * 0.4, 20, 95),
          pollen: clamp(e.pollen + (Math.random() - 0.47) * 0.35, 15, 90)
        }));
        setMetrics(m => {
          const upd = (met, variance, lo, hi) => {
            const nv = clamp(met.value + (Math.random() - 0.48) * variance, lo, hi);
            const h = [...met.history.slice(1), nv];
            const recent = h.slice(-10);
            const older = h.slice(-20, -10);
            const rAvg = recent.reduce((a, b) => a + b, 0) / recent.length;
            const oAvg = older.reduce((a, b) => a + b, 0) / older.length;
            const trend = oAvg > 0 ? ((rAvg - oAvg) / oAvg) * 100 : 0;
            return { value: nv, trend: clamp(trend, -15, 15), history: h };
          };
          return {
            population: upd(m.population, 18, 3000, 8000),
            mortality: upd(m.mortality, 0.3, 0.5, 12),
            honey: upd(m.honey, 0.6, 20, 95),
            foraging: upd(m.foraging, 0.8, 30, 95),
            infected: upd(m.infected, 10, 50, 1500)
          };
        });
        return next;
      });
    }, 120);
    return () => clearInterval(id);
  }, [status, speed]);

  const handlePlay = useAppCb(() => setStatus('RUNNING'), []);
  const handlePause = useAppCb(() => setStatus('PAUSED'), []);
  const handleStop = useAppCb(() => { setStatus('STOPPED'); setTick(0); setGridData(generateGridData(0)); }, []);
  const handleParamChange = useAppCb((key, val) => {
    setParams(p => ({ ...p, [key]: val }));
  }, []);

  return (
    <div className="app-layout">
      <Topbar
        status={status} speed={speed} tick={tick}
        onPlay={handlePlay} onPause={handlePause} onStop={handleStop} onSetSpeed={setSpeed}
      />
      <div className="main-area">
        <LeftPanel
          collapsed={!leftOpen} onToggle={() => setLeftOpen(o => !o)}
          colony={colony} health={health} energy={energy}
        />
        <div className="grid-area">
          <SimGrid gridData={gridData} />
        </div>
        <RightPanel
          collapsed={!rightOpen} onToggle={() => setRightOpen(o => !o)}
          params={params} onParamChange={handleParamChange}
        />
      </div>
      <BottomBar metrics={metrics} />
    </div>
  );
}

ReactDOM.createRoot(document.getElementById('root')).render(<App />);
