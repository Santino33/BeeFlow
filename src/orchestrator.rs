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
use crate::grid::{SpatialGrid, BORDER, GRID_W, HIVE_X, HIVE_Y};
use crate::metrics::{MetricsExporter, MetricsSnapshot, MortalityBreakdown, RoleDistribution};
use crate::rng::RngSystem;
use crate::systems::{
    run_age_system, run_energy_system, run_foraging_system, run_mortality_system,
    run_movement_system,
};

/// Motor de simulación. Controla el ciclo maestro de tick.
pub struct Orchestrator {
    tick: u64,
    config: RunConfig,
    pub rng: RngSystem,
    exporter: MetricsExporter,
    pub grid: SpatialGrid,         // M1
    diffusion: DiffusionSystem,    // M2
    pub world: hecs::World,        // M3 — entidades ECS
    pub global_temp: f32,          // M4 — °C; actualizado por M10 (SystemDynamics)
    deaths_by_energy: u32,         // M5 — acumulado entre exports; reset en cada snapshot
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
            global_temp: 20.0,
            deaths_by_energy: 0,
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
            global_temp: 20.0,
            deaths_by_energy: 0,
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
            run_movement_system(&mut self.world, &mut self.grid, &self.rng, self.tick);
            run_age_system(&mut self.world);
            run_energy_system(&mut self.world, self.global_temp);   // M4
            run_foraging_system(&mut self.world, &mut self.grid);   // M4
            self.deaths_by_energy +=
                run_mortality_system(&mut self.world, &mut self.grid); // M5
        }

        // 6. Resolver interacciones Grid ↔ ECS (depositar feromonas, consumir recursos)
        //    (M3+: no-op)

        // 7. Exportar métricas (cada 60 ticks)
        if self.tick % 60 == 0 {
            let snapshot = self.build_snapshot();
            self.exporter.maybe_export(&snapshot);
            self.deaths_by_energy = 0; // reset tras export
        }

        // 8. Enviar estado al renderer (mpsc, sin bloqueo)
        //    (M15: no-op)

        // 9. Incrementar contador de tick
        self.tick += 1;
    }

    fn build_snapshot(&self) -> MetricsSnapshot {
        let mut by_role = RoleDistribution::default();
        for (_, role) in self.world.query::<&RoleComponent>().iter() {
            match role.0 {
                Role::Queen   => by_role.queen   += 1,
                Role::Nurse   => by_role.nurse   += 1,
                Role::Builder => by_role.builder += 1,
                Role::Guard   => by_role.guard   += 1,
                Role::Forager => by_role.forager += 1,
                Role::Drone   => by_role.drone   += 1,
            }
        }
        MetricsSnapshot {
            tick: self.tick,
            population_by_role: by_role,
            mortality_rate: MortalityBreakdown {
                by_energy: self.deaths_by_energy,
                ..Default::default()
            },
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

/// Pobla el mundo con la distribución biológica de roles (simulation_spec.md §Roles).
/// 1 Queen en la colmena (HIVE_X, HIVE_Y); el resto distribuido por porcentajes.
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

    // 1 Queen — posición fija en la colmena
    let hive_idx = SpatialGrid::idx(HIVE_X, HIVE_Y);
    grid.occupancy[hive_idx] += 1;
    world.spawn((
        PositionComponent(HIVE_X as u16, HIVE_Y as u16),
        RoleComponent(Role::Queen),
        EnergyComponent(0.8),
        HealthComponent(SirState::Susceptible),
        AgeComponent(0),
        PheromoneSensitivity([0.0, 0.0, 0.0]),
    ));

    if count <= 1 {
        return;
    }
    let n_rest = (count - 1) as usize;

    // Distribución: 20% Forager, 10% Guard, 9% Builder, ~61% Nurse (absorbe residuo)
    let n_foragers = n_rest * 20 / 100;
    let n_guards   = n_rest * 10 / 100;
    let n_builders = n_rest *  9 / 100;
    let n_nurses   = n_rest - n_foragers - n_guards - n_builders;

    let roles = [
        (Role::Nurse,   n_nurses,   PheromoneSensitivity([0.0, 1.0, 0.0])),
        (Role::Forager, n_foragers, PheromoneSensitivity([0.0, 0.0, 1.0])),
        (Role::Guard,   n_guards,   PheromoneSensitivity([1.0, 0.0, 0.0])),
        (Role::Builder, n_builders, PheromoneSensitivity([0.0, 1.0, 0.0])),
    ];

    for (role, c, sensitivity) in roles {
        for _ in 0..c {
            let x = rng.gen_range(lo..hi) as u16;
            let y = rng.gen_range(lo..hi) as u16;
            let idx = SpatialGrid::idx(x as usize, y as usize);
            grid.occupancy[idx] = grid.occupancy[idx].saturating_add(1);
            world.spawn((
                PositionComponent(x, y),
                RoleComponent(role),
                EnergyComponent(0.8),
                HealthComponent(SirState::Susceptible),
                AgeComponent(0),
                sensitivity,
            ));
        }
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

        // En debug (sin optimizaciones) permitimos hasta 10 ms.
        // En release el límite real es 2 ms — detecta regresiones O(N²) con N=500.
        // La validación de rendimiento en release está en `cargo bench --bench diffusion`.
        #[cfg(debug_assertions)]
        let limit_ns: u128 = 10_000_000;
        #[cfg(not(debug_assertions))]
        let limit_ns: u128 = 2_000_000;

        assert!(
            avg_ns < limit_ns,
            "tick_once promedio: {} ns (límite: {} ns)",
            avg_ns, limit_ns
        );
    }

    // --- M6: Roles diferenciados ------------------------------------------------

    #[test]
    fn queen_at_hive_position() {
        use crate::grid::{HIVE_X, HIVE_Y};
        let dir = TempDir::new().unwrap();
        let orch = make_orchestrator(42, 0, &dir);

        let queens: Vec<_> = orch
            .world
            .query::<(&RoleComponent, &PositionComponent)>()
            .iter()
            .filter(|(_, (role, _))| role.0 == Role::Queen)
            .map(|(_, (_, pos))| (pos.0, pos.1))
            .collect();

        assert_eq!(queens.len(), 1, "debe existir exactamente 1 reina");
        assert_eq!(
            queens[0],
            (HIVE_X as u16, HIVE_Y as u16),
            "la reina debe estar en la colmena ({HIVE_X},{HIVE_Y})"
        );
    }

    #[test]
    fn initial_role_distribution() {
        let dir = TempDir::new().unwrap();
        let orch = make_orchestrator(42, 0, &dir);

        let mut counts = [0u32; 6]; // [queen, nurse, builder, guard, forager, drone]
        for (_, role) in orch.world.query::<&RoleComponent>().iter() {
            match role.0 {
                Role::Queen   => counts[0] += 1,
                Role::Nurse   => counts[1] += 1,
                Role::Builder => counts[2] += 1,
                Role::Guard   => counts[3] += 1,
                Role::Forager => counts[4] += 1,
                Role::Drone   => counts[5] += 1,
            }
        }
        let total: u32 = counts.iter().sum();
        assert_eq!(total, 500, "debe haber 500 abejas en total");
        assert_eq!(counts[0], 1, "exactamente 1 reina");
        // Foragers ≈ 20%, Guards ≈ 10%, Builders ≈ 9% (margen ±10 por rounding)
        assert!((90..=110).contains(&counts[4]), "foragers esperados ~99, obtenidos {}", counts[4]);
        assert!((44..=54).contains(&counts[3]), "guards esperados ~49, obtenidos {}", counts[3]);
        assert!((39..=50).contains(&counts[2]), "builders esperados ~44, obtenidos {}", counts[2]);
        assert_eq!(counts[5], 0, "sin zánganos en spawn inicial");
        // Nurses absorben el residuo → ~307 (el resto después de los demás)
        assert!(counts[1] > 250, "nurses deben ser la mayoría, obtenidos {}", counts[1]);
    }
}
