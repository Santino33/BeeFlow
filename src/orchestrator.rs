use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};
use tracing::{info, warn};

use crate::components::{
    AgeComponent, EnergyComponent, HealthComponent, PheromoneSensitivity, PositionComponent,
    Role, RoleComponent, SirState,
};
use crate::config::RunConfig;
use crate::diffusion::DiffusionSystem;
use crate::grid::{SpatialGrid, BORDER, GRID_W};
use crate::metrics::{MetricsExporter, MetricsSnapshot};
use crate::rng::RngSystem;
use crate::systems::{run_age_system, run_movement_system};

/// Motor de simulación. Controla el ciclo maestro de tick.
pub struct Orchestrator {
    tick: u64,
    config: RunConfig,
    pub rng: RngSystem,
    exporter: MetricsExporter,
    pub grid: SpatialGrid,         // M1
    diffusion: DiffusionSystem,    // M2
    pub world: hecs::World,        // M3 — entidades ECS
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

        let mut grid = SpatialGrid::new();
        let mut world = hecs::World::new();
        spawn_initial_population(&mut world, &mut grid, &rng, config.initial_population);

        Self {
            tick: 0,
            config,
            rng,
            exporter,
            grid,
            diffusion: DiffusionSystem::new(),
            world,
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
        let mut grid = SpatialGrid::new();
        let mut world = hecs::World::new();
        spawn_initial_population(&mut world, &mut grid, &rng, config.initial_population);
        Self {
            tick: 0,
            config,
            rng,
            exporter,
            grid,
            diffusion: DiffusionSystem::new(),
            world,
            metrics_dir,
        }
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
        self.diffusion.step(&mut self.grid);

        // 4. Regenerar recursos en celdas
        //    (M1/M10: no-op)

        // 5. Ejecutar sistemas ECS en orden canónico
        //    Orden: Movement → Energy → Foraging → Trophallaxis → Disease → Mortality → Role → Brood → Predator
        {
            let mut tick_rng = self.rng.tick_rng(self.tick);
            run_movement_system(&mut self.world, &mut self.grid, &mut tick_rng);
            run_age_system(&mut self.world);
        }

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

/// Pobla el mundo con `count` abejas en posiciones interiores aleatorias.
fn spawn_initial_population(
    world: &mut hecs::World,
    grid: &mut SpatialGrid,
    rng_system: &RngSystem,
    count: u32,
) {
    use rand::Rng;
    let mut rng = rng_system.global_rng();
    let lo = BORDER;
    let hi = GRID_W - BORDER;
    for _ in 0..count {
        let x = rng.gen_range(lo..hi) as u16;
        let y = rng.gen_range(lo..hi) as u16;
        let idx = SpatialGrid::idx(x as usize, y as usize);
        grid.occupancy[idx] = grid.occupancy[idx].saturating_add(1);
        world.spawn((
            PositionComponent(x, y),
            RoleComponent(Role::Nurse),
            EnergyComponent(0.8),
            HealthComponent(SirState::Susceptible),
            AgeComponent(0),
            PheromoneSensitivity([1.0, 1.0, 1.0]),
        ));
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

        // Con difusión activa (M2), el tick ya no es vacío. En debug (sin optimizaciones)
        // permitimos hasta 10 ms. La validación de rendimiento real está en el benchmark
        // de release `cargo bench --bench diffusion` (objetivo: < 5 ms en 8 cores).
        assert!(
            avg_ns < 10_000_000,
            "tick_once promedio: {} ns (límite: 10_000_000 ns)",
            avg_ns
        );
    }
}
