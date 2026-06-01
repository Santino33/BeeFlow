use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};
use tracing::{info, warn};

use crate::config::RunConfig;
use crate::grid::SpatialGrid;
use crate::metrics::{MetricsExporter, MetricsSnapshot};
use crate::rng::RngSystem;

/// Motor de simulación. Controla el ciclo maestro de tick.
///
/// Estructura diseñada para recibir los módulos posteriores:
/// - M3: hecs World (agentes ECS)
/// - M10: SystemDynamicsState (variables globales)
pub struct Orchestrator {
    tick: u64,
    config: RunConfig,
    pub rng: RngSystem,
    exporter: MetricsExporter,
    pub grid: SpatialGrid, // M1
    metrics_dir: PathBuf,
}

impl Orchestrator {
    pub fn new(config: RunConfig) -> Self {
        let metrics_dir = PathBuf::from(".");
        // Crear directorio de métricas si no existe
        let _ = std::fs::create_dir_all(&metrics_dir);

        let rng = RngSystem::new(config.seed);
        let exporter = MetricsExporter::new(&metrics_dir, 60);

        info!(seed = config.seed, speed = config.simulation_speed, "Orchestrator inicializado");

        Self {
            tick: 0,
            config,
            rng,
            exporter,
            grid: SpatialGrid::new(),
            metrics_dir,
        }
    }

    /// Construye el Orchestrator apuntando los JSON a un directorio específico.
    /// Útil para tests que no quieren escribir en el directorio de trabajo.
    pub fn new_with_output(config: RunConfig, output_dir: impl Into<PathBuf>) -> Self {
        let metrics_dir: PathBuf = output_dir.into();
        let _ = std::fs::create_dir_all(&metrics_dir);
        let rng = RngSystem::new(config.seed);
        let exporter = MetricsExporter::new(&metrics_dir, 60);
        Self { tick: 0, config, rng, exporter, grid: SpatialGrid::new(), metrics_dir }
    }

    /// Ejecuta el loop completo hasta `max_ticks` o hasta Ctrl-C.
    pub fn run(&mut self) {
        let tick_budget =
            Duration::from_secs_f64(1.0 / self.config.simulation_speed as f64);

        loop {
            if let Some(max) = self.config.max_ticks {
                if self.tick >= max {
                    info!(tick = self.tick, "Simulación completada (max_ticks alcanzado)");
                    break;
                }
            }

            let t0 = Instant::now();
            self.tick_once();
            let elapsed = t0.elapsed();

            if elapsed > tick_budget {
                warn!(
                    tick = self.tick,
                    elapsed_us = elapsed.as_micros(),
                    budget_us = tick_budget.as_micros(),
                    "Tick superó el presupuesto de tiempo"
                );
            } else {
                thread::sleep(tick_budget - elapsed);
            }
        }
    }

    /// Ejecuta un único tick siguiendo el orden canónico de architecture.md.
    pub fn tick_once(&mut self) {
        // 1. Leer inputs externos
        //    (M0: no-op — placeholder para eventos de usuario/config en caliente)

        // 2. Actualizar System Dynamics (cada 15 ticks)
        //    (M10: no-op)
        if self.tick % 15 == 0 {
            // SystemDynamicsState::update(...)
        }

        // 3. Difundir feromonas en el Grid (double-buffer swap)
        //    (M2: no-op)

        // 4. Regenerar recursos en celdas
        //    (M1/M10: no-op)

        // 5. Ejecutar sistemas ECS en orden canónico (rayon)
        //    Orden: Movement → Energy → Foraging → Trophallaxis → Disease → Mortality → Role → Brood → Predator
        //    (M3+: no-op)

        // 6. Resolver interacciones Grid ↔ ECS (depositar feromonas, consumir recursos)
        //    (M3+: no-op)

        // 7. Exportar métricas (cada 60 ticks)
        if self.tick % 60 == 0 {
            let snapshot = self.build_snapshot();
            self.exporter.maybe_export(&snapshot);
        }

        // 8. Enviar estado al renderer (mpsc, sin bloqueo)
        //    (M15: no-op)

        // 9. Incrementar contador de tick
        self.tick += 1;
    }

    /// Construye el snapshot de métricas del estado actual.
    /// En M0 todos los valores son 0/Default.
    fn build_snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            tick: self.tick,
            ..Default::default()
        }
    }

    pub fn current_tick(&self) -> u64 {
        self.tick
    }

    pub fn metrics_dir(&self) -> &PathBuf {
        &self.metrics_dir
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_orchestrator(seed: u64, max_ticks: u64, dir: &TempDir) -> Orchestrator {
        let config = RunConfig {
            seed,
            max_ticks: Some(max_ticks),
            ..Default::default()
        };
        Orchestrator::new_with_output(config, dir.path())
    }

    #[test]
    fn tick_counter_advances() {
        let dir = TempDir::new().unwrap();
        let mut orch = make_orchestrator(42, 10, &dir);
        orch.run();
        assert_eq!(orch.current_tick(), 10);
    }

    #[test]
    fn same_seed_produces_identical_metrics() {
        let dir_a = TempDir::new().unwrap();
        let dir_b = TempDir::new().unwrap();

        make_orchestrator(42, 121, &dir_a).run();
        make_orchestrator(42, 121, &dir_b).run();

        for filename in &[
            "metrics_00000000.json",
            "metrics_00000060.json",
            "metrics_00000120.json",
        ] {
            let a = std::fs::read(dir_a.path().join(filename)).unwrap();
            let b = std::fs::read(dir_b.path().join(filename)).unwrap();
            assert_eq!(a, b, "Archivo {} difiere entre runs con misma semilla", filename);
        }
    }

    #[test]
    fn tick_once_is_fast() {
        let dir = TempDir::new().unwrap();
        let config = RunConfig { max_ticks: Some(1), ..Default::default() };
        let mut orch = Orchestrator::new_with_output(config, dir.path());

        let t0 = Instant::now();
        for _ in 0..1000 {
            orch.tick_once();
        }
        let avg_ns = t0.elapsed().as_nanos() / 1000;

        // El tick vacío debe ser << 1ms (1_000_000 ns). Usamos 100_000 ns como margen holgado.
        assert!(
            avg_ns < 100_000,
            "tick_once promedio: {} ns (límite: 100_000 ns)",
            avg_ns
        );
    }
}
