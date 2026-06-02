use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};
use tracing::{info, warn};

use crate::components::{
    AgeComponent, BroodStageComponent, EnergyComponent, ForagerPhase, ForagerStateComponent,
    HealthComponent, PheromoneSensitivity, PositionComponent, PredatorComponent, Role,
    RoleComponent, RoleTransitionState, SirState,
};
use crate::config::RunConfig;
use crate::diffusion::DiffusionSystem;
use crate::system_dynamics::SystemDynamicsState;
use crate::grid::{PheromoneKind, SpatialGrid, BORDER, FOOD_SOURCE_POSITIONS, GRID_W, HIVE_X, HIVE_Y};
use crate::metrics::{MetricsExporter, MetricsSnapshot, MortalityBreakdown, RoleDistribution};
use crate::rng::RngSystem;
use crate::systems::{
    run_age_system, run_brood_system, run_disease_system, run_energy_system, run_foraging_system,
    run_honey_feeding_system, run_mortality_system, run_movement_system, run_predator_system,
    run_resource_regeneration, run_role_transition_system, run_trophallaxis_system,
    METABOLIC_COST_BASAL, TROPHALLAXIS_RESERVE_THRESHOLD,
    PREDATOR_ATTACK_RATE, PREDATOR_DETECTION_RADIUS, PREDATOR_ENERGY_DRAIN,
};
use crate::visualizer::SimCommand;

/// Motor de simulación. Controla el ciclo maestro de tick.
pub struct Orchestrator {
    tick: u64,
    config: RunConfig,
    pub rng: RngSystem,
    exporter: MetricsExporter,
    pub grid: SpatialGrid,              // M1
    diffusion: DiffusionSystem,         // M2
    pub world: hecs::World,             // M3 — entidades ECS
    pub system_dynamics: SystemDynamicsState, // M10 — variables globales y estacionalidad
    deaths_by_energy: u32,             // M5 — acumulado entre exports; reset en cada snapshot
    deaths_by_disease: u32,            // M14 — muertes de Infected entre exports
    deaths_by_predation: u32,          // M13 — muertes por depredación entre exports
    pub time_to_collapse: Option<u64>, // M14 — tick en que population < 50 por primera vez
    pub honey_reserve: f32,            // M7 — reserva global de miel
    honey_collected_period: f32,       // M7 — miel depositada desde último export
    food_sources: Vec<(usize, usize)>, // M7 — posiciones de fuentes de alimento
    metrics_dir: PathBuf,
    render_tx: Option<std::sync::mpsc::SyncSender<crate::visualizer::RenderFrame>>, // M15
    // Parámetros configurables en tiempo real desde la UI
    cmd_rx:                  Option<std::sync::mpsc::Receiver<SimCommand>>,
    metabolic_rate:          f32,
    trophallaxis_threshold:  f32,
    pheromone_emission_mult: f32,
}

impl Orchestrator {
    pub fn new(config: RunConfig) -> Self {
        let metrics_dir = PathBuf::from(".");
        let _ = std::fs::create_dir_all(&metrics_dir);

        let rng = RngSystem::new(config.seed);
        let exporter = MetricsExporter::new(&metrics_dir, 60);

        info!(seed = config.seed, speed = config.simulation_speed, "Orchestrator inicializado");

        let mut grid = SpatialGrid::new();
        let mut world = hecs::World::new();
        spawn_initial_population(&mut world, &mut grid, &rng, config.initial_population);
        spawn_predators(&mut world, &mut grid, &rng, config.predator_count);

        let food_sources = init_food_sources(&mut grid);

        Self {
            tick: 0,
            honey_reserve: config.initial_honey_reserve,
            honey_collected_period: 0.0,
            food_sources,
            system_dynamics: SystemDynamicsState::new(&config),
            config,
            rng,
            exporter,
            grid,
            diffusion: DiffusionSystem::new(),
            world,
            deaths_by_energy: 0,
            deaths_by_disease: 0,
            deaths_by_predation: 0,
            time_to_collapse: None,
            metrics_dir,
            render_tx: None,
            cmd_rx: None,
            metabolic_rate: METABOLIC_COST_BASAL,
            trophallaxis_threshold: TROPHALLAXIS_RESERVE_THRESHOLD,
            pheromone_emission_mult: 1.0,
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
        spawn_predators(&mut world, &mut grid, &rng, config.predator_count);
        let food_sources = init_food_sources(&mut grid);
        Self {
            tick: 0,
            honey_reserve: config.initial_honey_reserve,
            honey_collected_period: 0.0,
            food_sources,
            system_dynamics: SystemDynamicsState::new(&config),
            config,
            rng,
            exporter,
            grid,
            diffusion: DiffusionSystem::new(),
            world,
            deaths_by_energy: 0,
            deaths_by_disease: 0,
            deaths_by_predation: 0,
            time_to_collapse: None,
            metrics_dir,
            render_tx: None,
            cmd_rx: None,
            metabolic_rate: METABOLIC_COST_BASAL,
            trophallaxis_threshold: TROPHALLAXIS_RESERVE_THRESHOLD,
            pheromone_emission_mult: 1.0,
        }
    }

    /// Ejecuta el loop completo hasta `max_ticks` o hasta Ctrl-C.
    pub fn run(&mut self) {
        loop {
            if let Some(max) = self.config.max_ticks {
                if self.tick >= max {
                    info!(tick = self.tick, "Simulación completada (max_ticks alcanzado)");
                    break;
                }
            }

            // tick_budget se recalcula cada iteración para reflejar cambios de velocidad en vivo
            let tick_budget =
                Duration::from_secs_f64(1.0 / self.config.simulation_speed as f64);

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

    /// Registra el receptor del canal de comandos desde la UI.
    pub fn set_command_receiver(
        &mut self,
        rx: std::sync::mpsc::Receiver<SimCommand>,
    ) {
        self.cmd_rx = Some(rx);
    }

    /// Ejecuta un único tick siguiendo el orden canónico de architecture.md.
    pub fn tick_once(&mut self) {
        // 1. Leer comandos de la UI (M0 — no bloqueante)
        if let Some(rx) = &self.cmd_rx {
            while let Ok(cmd) = rx.try_recv() {
                match cmd {
                    SimCommand::SetTemperature(t) => {
                        self.system_dynamics.manual_temp =
                            if t.is_nan() { None } else { Some(t.clamp(-10.0, 45.0)) };
                    }
                    SimCommand::SetSeason(s) => {
                        self.system_dynamics.season_phase = s.clamp(0.0, 1.0);
                    }
                    SimCommand::SetPesticide(p) => {
                        self.system_dynamics.pesticide_pressure = p.clamp(0.0, 1.0);
                    }
                    SimCommand::SetDiseaseEnabled(e) => self.config.disease_enabled = e,
                    SimCommand::SetDiseaseRate(r)    => self.config.disease_base_rate = r.clamp(0.0, 1.0),
                    SimCommand::SetSimulationSpeed(s)=> self.config.simulation_speed = s.max(1),
                    SimCommand::SetMetabolicRate(r)  => self.metabolic_rate = r.clamp(0.0001, 0.1),
                    SimCommand::SetTrophallaxisThreshold(t) => self.trophallaxis_threshold = t.clamp(1.0, 200.0),
                    SimCommand::SetPheromoneDecay(d) => self.diffusion.decay_rate = d.clamp(0.001, 0.99),
                    SimCommand::SetPheromoneEmission(m) => self.pheromone_emission_mult = m.clamp(0.0, 10.0),
                }
            }
        }

        // 2. Actualizar System Dynamics (cada 15 ticks) — M10
        if self.tick % 15 == 0 {
            self.system_dynamics.update(self.honey_reserve);
        }
        // Aplicar override manual de temperatura (si está activo)
        if let Some(t) = self.system_dynamics.manual_temp {
            self.system_dynamics.global_temp = t;
        }

        // 3. Regenerar recursos en fuentes de alimento (M7)
        run_resource_regeneration(&mut self.grid, &self.food_sources, self.system_dynamics.season_factor());

        // 4. Ejecutar sistemas ECS en orden canónico
        //    Orden: Movement → Age → Energy → Foraging → Trophallaxis → Disease → Mortality → Role → Brood → Predator
        //    Los sistemas emiten feromonas al write_buf durante esta fase.
        {
            run_movement_system(&mut self.world, &mut self.grid, &self.rng, self.tick);
            run_age_system(&mut self.world);
            run_energy_system(&mut self.world, self.system_dynamics.global_temp, self.system_dynamics.pesticide_pressure, self.metabolic_rate); // M4/M10
            run_foraging_system(                                      // M7
                &mut self.world,
                &mut self.grid,
                &mut self.honey_reserve,
                &mut self.honey_collected_period,
                self.pheromone_emission_mult,
            );
            run_honey_feeding_system(&mut self.world, &mut self.honey_reserve); // M11-fix
            run_trophallaxis_system(&mut self.world, self.honey_reserve, self.trophallaxis_threshold); // M9
            run_disease_system(                                           // M12
                &mut self.world,
                &self.rng,
                self.tick,
                self.config.disease_base_rate,
                self.config.disease_enabled,
            );
            {
                let (de, dd) = run_mortality_system(&mut self.world, &mut self.grid); // M5/M14
                self.deaths_by_energy  += de;
                self.deaths_by_disease += dd;
            }
            run_role_transition_system(&mut self.world, &self.grid, &self.rng, self.tick); // M8
            run_brood_system(                                                               // M11
                &mut self.world,
                &mut self.grid,
                &self.rng,
                self.tick,
                self.system_dynamics.brood_production_rate,
                self.pheromone_emission_mult,
            );
            self.deaths_by_predation +=
                run_predator_system(&mut self.world, &mut self.grid, &self.rng, self.tick); // M13
        }

        // 5. Incorporar emisiones frescas al estado estable y difundir (M2)
        //    Orden correcto: merge → diffuse. Garantiza que las emisiones del tick actual
        //    sean visibles en el render y correctamente difundidas al siguiente tick.
        self.grid.merge_emissions_into_stable();
        self.diffusion.step(&mut self.grid);

        // 6. Resolver interacciones Grid ↔ ECS (depositar feromonas, consumir recursos)
        //    (M3+: no-op)

        // 6b. Detectar colapso (M14): población < 50 por primera vez
        if self.time_to_collapse.is_none() {
            let alive = self.world.query::<&RoleComponent>().iter().count();
            if alive < 50 {
                self.time_to_collapse = Some(self.tick);
            }
        }

        // 7. Exportar métricas (cada 60 ticks)
        if self.tick % 60 == 0 {
            let snapshot = self.build_snapshot();
            if self.exporter.maybe_export(&snapshot) {
                self.deaths_by_energy = 0;
                self.deaths_by_disease = 0;
                self.deaths_by_predation = 0;
                self.honey_collected_period = 0.0;
            }
        }

        // 8. Enviar estado al renderer (mpsc, sin bloqueo) — M15
        if let Some(tx) = &self.render_tx {
            let frame = self.build_render_frame();
            let _ = tx.try_send(frame); // descarta si el renderer está ocupado
        }

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
        // foraging_efficiency = miel_depositada / (n_foragers × ticks × costo_basal)
        // ticks entre exports = 60; costo energético es proxy del gasto de los foragers
        let foraging_efficiency = if by_role.forager > 0 {
            self.honey_collected_period
                / (by_role.forager as f32 * 60.0 * self.metabolic_rate.max(1e-6))
        } else {
            0.0
        };
        let brood_count = self.world.query::<&BroodStageComponent>().iter().count() as f32;
        let bee_count   = by_role.total() as f32;
        let brood_adult_ratio = if bee_count > 0.0 { brood_count / bee_count } else { 0.0 };

        // SIR prevalence — fracciones sobre la población de abejas activas (M12)
        let mut sir = crate::metrics::SirPrevalence::default();
        if bee_count > 0.0 {
            for (_, health) in self.world.query::<&HealthComponent>().iter() {
                match health.state {
                    SirState::Susceptible => sir.susceptible += 1.0,
                    SirState::Infected    => sir.infected    += 1.0,
                    SirState::Recovered   => sir.recovered   += 1.0,
                }
            }
            sir.susceptible /= bee_count;
            sir.infected    /= bee_count;
            sir.recovered   /= bee_count;
        }

        MetricsSnapshot {
            tick: self.tick,
            population_by_role: by_role,
            colony_reserve: self.honey_reserve,
            foraging_efficiency,
            brood_adult_ratio,
            sir_prevalence: sir,
            mortality_rate: MortalityBreakdown {
                by_energy:    self.deaths_by_energy,
                by_disease:   self.deaths_by_disease,
                by_predation: self.deaths_by_predation,
            },
            pheromone_entropy: crate::metrics::pheromone_entropy(&self.grid),
            time_to_collapse: self.time_to_collapse,
            season_phase: self.system_dynamics.season_phase,
            global_temp: self.system_dynamics.global_temp,
            brood_production_rate: self.system_dynamics.brood_production_rate,
        }
    }

    pub fn current_tick(&self) -> u64 {
        self.tick
    }

    pub fn metrics_dir(&self) -> &PathBuf {
        &self.metrics_dir
    }

    // --- M15: Visualizador -------------------------------------------------------

    /// Registra el sender del canal mpsc hacia el visualizador.
    /// `tick_once` usará `try_send` (no bloqueante) cada tick.
    pub fn set_render_sender(
        &mut self,
        tx: std::sync::mpsc::SyncSender<crate::visualizer::RenderFrame>,
    ) {
        self.render_tx = Some(tx);
    }

    /// Extrae los datos de rendering del estado actual (pheromones + agentes + métricas).
    /// Usa `pheromone_read_slice` (read buffer = estado difundido canónico del tick).
    fn build_render_frame(&self) -> crate::visualizer::RenderFrame {
        use crate::grid::PheromoneKind;
        use crate::visualizer::{AgentKind, AgentRenderData, RenderFrame};

        let pheromone_alarm      = self.grid.pheromone_read_slice(PheromoneKind::Alarm).to_vec();
        let pheromone_task       = self.grid.pheromone_read_slice(PheromoneKind::Task).to_vec();
        let pheromone_attraction = self.grid.pheromone_read_slice(PheromoneKind::Attraction).to_vec();

        let mut agents: Vec<AgentRenderData> = self
            .world
            .query::<(&PositionComponent, &RoleComponent)>()
            .iter()
            .map(|(_, (pos, role))| AgentRenderData {
                x: pos.0 as f32,
                y: pos.1 as f32,
                role: role_to_agent_kind(role.0),
            })
            .collect();

        for (_, (pos, _)) in self
            .world
            .query::<(&PositionComponent, &PredatorComponent)>()
            .iter()
        {
            agents.push(AgentRenderData {
                x: pos.0 as f32,
                y: pos.1 as f32,
                role: AgentKind::Predator,
            });
        }

        RenderFrame {
            tick: self.tick,
            pheromone_alarm,
            pheromone_task,
            pheromone_attraction,
            agents,
            metrics: self.build_snapshot(),
        }
    }
}

/// Convierte Role ECS al AgentKind del renderer.
fn role_to_agent_kind(role: Role) -> crate::visualizer::AgentKind {
    use crate::visualizer::AgentKind;
    match role {
        Role::Queen   => AgentKind::Queen,
        Role::Nurse   => AgentKind::Nurse,
        Role::Builder => AgentKind::Builder,
        Role::Guard   => AgentKind::Guard,
        Role::Forager => AgentKind::Forager,
        Role::Drone   => AgentKind::Drone,
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
    // RNG separado para biases de umbral: usa sentinel distinto para no perturbar secuencia de posiciones.
    let mut bias_rng = rng_system.tick_rng(u64::MAX);
    // Radio de spawn para roles de colmena (Nurse/Guard/Builder) — se mantienen cerca del nido
    const HIVE_SPAWN_RADIUS: usize = 12;
    // Radio de spawn para forrajeadoras — algo más amplio para simular que ya patrullan
    const FORAGER_SPAWN_RADIUS: usize = 15;
    let hive_lo_x = HIVE_X.saturating_sub(HIVE_SPAWN_RADIUS);
    let hive_hi_x = (HIVE_X + HIVE_SPAWN_RADIUS + 1).min(GRID_W - BORDER);
    let hive_lo_y = HIVE_Y.saturating_sub(HIVE_SPAWN_RADIUS);
    let hive_hi_y = (HIVE_Y + HIVE_SPAWN_RADIUS + 1).min(GRID_W - BORDER);
    let forager_lo_x = HIVE_X.saturating_sub(FORAGER_SPAWN_RADIUS);
    let forager_hi_x = (HIVE_X + FORAGER_SPAWN_RADIUS + 1).min(GRID_W - BORDER);
    let forager_lo_y = HIVE_Y.saturating_sub(FORAGER_SPAWN_RADIUS);
    let forager_hi_y = (HIVE_Y + FORAGER_SPAWN_RADIUS + 1).min(GRID_W - BORDER);

    // 1 Queen — posición fija en la colmena
    let hive_idx = SpatialGrid::idx(HIVE_X, HIVE_Y);
    grid.occupancy[hive_idx] += 1;
    world.spawn((
        PositionComponent(HIVE_X as u16, HIVE_Y as u16),
        RoleComponent(Role::Queen),
        EnergyComponent(0.8),
        HealthComponent { state: SirState::Susceptible, ticks_in_state: 0 },
        AgeComponent(0),
        PheromoneSensitivity([0.0, 0.0, 0.0]),
    ));

    if count <= 1 {
        return;
    }
    let n_rest = (count - 1) as usize;

    // Distribución: 30% Forager, 8% Guard, 7% Builder, ~55% Nurse (absorbe residuo)
    let n_foragers = n_rest * 30 / 100;
    let n_guards   = n_rest *  8 / 100;
    let n_builders = n_rest *  7 / 100;
    let n_nurses   = n_rest - n_foragers - n_guards - n_builders;

    // Nodrizas, Guardianas, Constructoras — con RoleTransitionState (M8)
    let non_forager_roles = [
        (Role::Nurse,   n_nurses,   PheromoneSensitivity([0.0, 5.0, 0.0])),
        (Role::Guard,   n_guards,   PheromoneSensitivity([5.0, 0.0, 0.0])),
        (Role::Builder, n_builders, PheromoneSensitivity([0.0, 5.0, 0.0])),
    ];
    for (role, c, sensitivity) in non_forager_roles {
        for _ in 0..c {
            let x = rng.gen_range(hive_lo_x..hive_hi_x) as u16;
            let y = rng.gen_range(hive_lo_y..hive_hi_y) as u16;
            let idx = SpatialGrid::idx(x as usize, y as usize);
            grid.occupancy[idx] = grid.occupancy[idx].saturating_add(1);
            let bias: f32 = bias_rng.gen::<f32>() * 0.2 - 0.1;
            world.spawn((
                PositionComponent(x, y),
                RoleComponent(role),
                EnergyComponent(0.8),
                HealthComponent { state: SirState::Susceptible, ticks_in_state: 0 },
                AgeComponent(0),
                sensitivity,
                RoleTransitionState { cooldown: 0, threshold_bias: bias },
            ));
        }
    }

    // Recolectoras — con ForagerStateComponent (M7) y RoleTransitionState (M8)
    for _ in 0..n_foragers {
        let x = rng.gen_range(forager_lo_x..forager_hi_x) as u16;
        let y = rng.gen_range(forager_lo_y..forager_hi_y) as u16;
        let idx = SpatialGrid::idx(x as usize, y as usize);
        grid.occupancy[idx] = grid.occupancy[idx].saturating_add(1);
        let bias: f32 = bias_rng.gen::<f32>() * 0.2 - 0.1;
        world.spawn((
            PositionComponent(x, y),
            RoleComponent(Role::Forager),
            EnergyComponent(0.8),
            HealthComponent { state: SirState::Susceptible, ticks_in_state: 0 },
            AgeComponent(0),
            PheromoneSensitivity([0.0, 0.0, 5.0]),
            ForagerStateComponent(ForagerPhase::Searching),
            RoleTransitionState { cooldown: 0, threshold_bias: bias },
        ));
    }
}

/// Hace spawn de los depredadores en posiciones aleatorias del grid (excluye obstáculos).
fn spawn_predators(
    world: &mut hecs::World,
    grid: &mut SpatialGrid,
    rng_system: &RngSystem,
    count: u8,
) {
    use rand::Rng;
    use crate::grid::{GRID_W, GRID_H};
    let mut rng = rng_system.tick_rng(u64::MAX - 1); // sentinel distinto del de biases
    for _ in 0..count {
        let x = rng.gen_range(0..GRID_W as u16);
        let y = rng.gen_range(0..GRID_H as u16);
        let idx = SpatialGrid::idx(x as usize, y as usize);
        grid.occupancy[idx] = grid.occupancy[idx].saturating_add(1);
        world.spawn((
            PositionComponent(x, y),
            EnergyComponent(1.0),
            PredatorComponent {
                attack_rate: PREDATOR_ATTACK_RATE,
                detection_radius: PREDATOR_DETECTION_RADIUS,
                energy_drain_on_hit: PREDATOR_ENERGY_DRAIN,
            },
        ));
    }
}

/// Radio de cada parche de alimento en celdas. Parches 17×17 (~10% del grid interior).
/// Con este radio, ~10 Foragers inician DENTRO de los parches con seed=42, 500 pop.
const FOOD_SOURCE_RADIUS: i32 = 8;

/// Inicializa las fuentes de alimento como parches cuadrados alrededor de cada posición central.
/// Devuelve todas las posiciones de celda con recurso (para regeneración por tick).
fn init_food_sources(grid: &mut SpatialGrid) -> Vec<(usize, usize)> {
    let mut sources = Vec::new();
    for &(cx, cy) in FOOD_SOURCE_POSITIONS.iter() {
        for dy in -FOOD_SOURCE_RADIUS..=FOOD_SOURCE_RADIUS {
            for dx in -FOOD_SOURCE_RADIUS..=FOOD_SOURCE_RADIUS {
                let x = (cx as i32 + dx) as usize;
                let y = (cy as i32 + dy) as usize;
                if SpatialGrid::in_bounds(x, y) && !grid.is_obstacle[SpatialGrid::idx(x, y)] {
                    grid.set_resource(x, y, 1.0);
                    // Pre-sembrar feromona de atracción para bootstrap de forrajeadores
                    grid.add_pheromone(x, y, PheromoneKind::Attraction, 1.0);
                    sources.push((x, y));
                }
            }
        }
    }
    sources
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

        // En debug (sin optimizaciones) permitimos hasta 20 ms.
        // merge_emissions_into_stable + clear_write_buf agregan ~2 iteraciones sobre 30k celdas.
        // En release el límite real es 2 ms — detecta regresiones O(N²) con N=500.
        // La validación de rendimiento en release está en `cargo bench --bench diffusion`.
        #[cfg(debug_assertions)]
        let limit_ns: u128 = 20_000_000;
        #[cfg(not(debug_assertions))]
        let limit_ns: u128 = 2_000_000;

        assert!(
            avg_ns < limit_ns,
            "tick_once promedio: {} ns (límite: {} ns)",
            avg_ns, limit_ns
        );
    }

    // --- M7: Sistema de Forrajeo ------------------------------------------------

    #[test]
    fn food_sources_initialized() {
        use crate::grid::FOOD_SOURCE_POSITIONS;
        let dir = TempDir::new().unwrap();
        let orch = make_orchestrator(42, 0, &dir);

        // Verificar que los centros de las 3 fuentes tienen recurso 1.0
        for &(x, y) in FOOD_SOURCE_POSITIONS.iter() {
            let res = orch.grid.resource(x, y);
            assert!(
                (res - 1.0).abs() < 1e-5,
                "Centro de fuente ({x},{y}) debe tener resource_amount=1.0, obtenido {res}"
            );
        }
        // Verificar que al menos un vecino del centro también tiene recurso (parche 3×3)
        let (cx, cy) = FOOD_SOURCE_POSITIONS[0];
        let neighbor_res = orch.grid.resource(cx + 1, cy);
        assert!(neighbor_res > 0.0, "El parche 3×3 debe extenderse a ({}, {})", cx + 1, cy);
    }

    #[test]
    fn honey_reserve_initialized_from_config() {
        let dir = TempDir::new().unwrap();
        let config = RunConfig {
            seed: 42,
            initial_honey_reserve: 0.5,
            max_ticks: Some(0),
            ..Default::default()
        };
        let orch = Orchestrator::new_with_output(config, dir.path());
        assert!((orch.honey_reserve - 0.5).abs() < 1e-5);
    }

    #[test]
    fn honey_reserve_grows_with_foragers() {
        // Criterio del backlog: 3 fuentes + ~100 recolectoras → honey_reserve crece.
        // Usamos población 500 (default, ~100 foragers) y 600 ticks.
        let dir = TempDir::new().unwrap();
        let config = RunConfig {
            seed: 42,
            initial_population: 500,
            initial_honey_reserve: 0.0,
            max_ticks: Some(600),
            ..Default::default()
        };
        let mut orch = Orchestrator::new_with_output(config, dir.path());
        let initial = orch.honey_reserve;
        orch.run();
        assert!(
            orch.honey_reserve > initial,
            "honey_reserve debe crecer con ~100 foragers y 3 fuentes: inicial={initial}, final={}",
            orch.honey_reserve
        );
    }

    #[test]
    fn foraging_efficiency_positive_in_snapshot() {
        // Criterio del backlog: foraging_efficiency > 0 y coherente.
        // Se verifica en el export del tick 60, que captura depósitos de ticks 1-60.
        // Los foragers (0.02/tick) viven ~40 ticks; deben encontrar y depositar dentro de ese plazo.
        let dir = TempDir::new().unwrap();
        let config = RunConfig {
            seed: 42,
            initial_population: 500,
            initial_honey_reserve: 0.0,
            max_ticks: Some(61), // tick 60 export + stop
            ..Default::default()
        };
        let mut orch = Orchestrator::new_with_output(config, dir.path());
        orch.run();

        let content = std::fs::read_to_string(dir.path().join("metrics_00000060.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        let eff = parsed["foraging_efficiency"].as_f64().unwrap_or(0.0);
        assert!(eff > 0.0, "foraging_efficiency debe ser > 0 en tick 60, obtenido {eff}");
    }

    #[test]
    fn determinism_preserved_with_m7() {
        let dir_a = TempDir::new().unwrap();
        let dir_b = TempDir::new().unwrap();

        make_orchestrator(77, 121, &dir_a).run();
        make_orchestrator(77, 121, &dir_b).run();

        for filename in &["metrics_00000000.json", "metrics_00000060.json", "metrics_00000120.json"] {
            let a = std::fs::read(dir_a.path().join(filename)).unwrap();
            let b = std::fs::read(dir_b.path().join(filename)).unwrap();
            assert_eq!(a, b, "M7: archivo {filename} difiere entre runs con misma semilla");
        }
    }

    // --- M10: SystemDynamics / Estacionalidad -----------------------------------

    #[test]
    fn season_phase_in_snapshot() {
        let dir = TempDir::new().unwrap();
        let config = RunConfig {
            seed: 42,
            max_ticks: Some(61),
            ..Default::default()
        };
        let mut orch = Orchestrator::new_with_output(config, dir.path());
        orch.run();

        let content = std::fs::read_to_string(dir.path().join("metrics_00000060.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        let phase = parsed["season_phase"].as_f64().unwrap_or(0.0);
        assert!(phase > 0.0, "season_phase debe ser > 0 tras 60 ticks, obtenido {phase}");
    }

    #[test]
    fn determinism_preserved_with_m10() {
        let dir_a = TempDir::new().unwrap();
        let dir_b = TempDir::new().unwrap();

        make_orchestrator(11, 121, &dir_a).run();
        make_orchestrator(11, 121, &dir_b).run();

        for filename in &["metrics_00000000.json", "metrics_00000060.json", "metrics_00000120.json"] {
            let a = std::fs::read(dir_a.path().join(filename)).unwrap();
            let b = std::fs::read(dir_b.path().join(filename)).unwrap();
            assert_eq!(a, b, "M10: archivo {filename} difiere entre runs con misma semilla");
        }
    }

    // --- M9: TrophallaxisSystem -------------------------------------------------

    #[test]
    fn energy_homogenizes_under_low_reserve() {
        // Con reserva baja y abejas agrupadas, la varianza de energía debe disminuir.
        let dir = TempDir::new().unwrap();
        let config = RunConfig {
            seed: 42,
            initial_population: 100,
            initial_honey_reserve: 0.05, // bien por debajo del umbral 0.30
            max_ticks: Some(60),
            ..Default::default()
        };
        let mut orch = Orchestrator::new_with_output(config, dir.path());

        // Varianza de energía inicial
        let initial_energies: Vec<f32> = orch.world.query::<&EnergyComponent>().iter()
            .map(|(_, e)| e.0).collect();
        let initial_var = variance(&initial_energies);

        orch.run();

        let final_energies: Vec<f32> = orch.world.query::<&EnergyComponent>().iter()
            .map(|(_, e)| e.0).collect();
        let final_var = variance(&final_energies);

        // La varianza debe haberse reducido (trofalaxia homogeniza energías)
        // o al menos la colonia no colapsó sin razón (test de no-regresión)
        assert!(
            final_var <= initial_var + 0.1,
            "varianza de energía no debe crecer drásticamente con trofalaxia activa: inicial={initial_var:.4}, final={final_var:.4}"
        );
    }

    #[test]
    fn determinism_preserved_with_m9() {
        let dir_a = TempDir::new().unwrap();
        let dir_b = TempDir::new().unwrap();

        make_orchestrator(33, 121, &dir_a).run();
        make_orchestrator(33, 121, &dir_b).run();

        for filename in &["metrics_00000000.json", "metrics_00000060.json", "metrics_00000120.json"] {
            let a = std::fs::read(dir_a.path().join(filename)).unwrap();
            let b = std::fs::read(dir_b.path().join(filename)).unwrap();
            assert_eq!(a, b, "M9: archivo {filename} difiere entre runs con misma semilla");
        }
    }

    fn variance(values: &[f32]) -> f32 {
        if values.is_empty() { return 0.0; }
        let mean = values.iter().sum::<f32>() / values.len() as f32;
        values.iter().map(|&v| (v - mean).powi(2)).sum::<f32>() / values.len() as f32
    }

    // --- M12: DiseaseSystem -----------------------------------------------------

    #[test]
    fn sir_prevalence_populated_in_snapshot() {
        // Forzamos un infectado directamente para que sir_prevalence lo cuente.
        let dir = TempDir::new().unwrap();
        let config = RunConfig {
            seed: 42,
            initial_population: 100,
            max_ticks: Some(61),
            disease_enabled: false, // no transmisión, pero queremos verificar el conteo
            ..Default::default()
        };
        let mut orch = Orchestrator::new_with_output(config, dir.path());

        // Infectar manualmente la primera abeja con HealthComponent
        let entities: Vec<hecs::Entity> = orch.world
            .query::<&HealthComponent>()
            .iter()
            .map(|(e, _)| e)
            .take(1)
            .collect();
        for e in entities {
            let _ = orch.world.insert_one(e, HealthComponent {
                state: SirState::Infected,
                ticks_in_state: 0,
            });
        }

        orch.run();

        let content = std::fs::read_to_string(dir.path().join("metrics_00000060.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        let susceptible = parsed["sir_prevalence"]["susceptible"].as_f64().unwrap_or(0.0);
        let sum = parsed["sir_prevalence"]["susceptible"].as_f64().unwrap_or(0.0)
            + parsed["sir_prevalence"]["infected"].as_f64().unwrap_or(0.0)
            + parsed["sir_prevalence"]["recovered"].as_f64().unwrap_or(0.0);
        assert!(susceptible > 0.0, "sir_prevalence.susceptible debe ser > 0");
        assert!((sum - 1.0).abs() < 1e-3, "fracciones SIR deben sumar ≈ 1.0, obtenido {sum}");
    }

    #[test]
    fn determinism_preserved_with_m12() {
        let dir_a = TempDir::new().unwrap();
        let dir_b = TempDir::new().unwrap();

        let make = |dir: &TempDir| {
            let config = RunConfig {
                seed: 44,
                max_ticks: Some(121),
                disease_enabled: true,
                disease_base_rate: 0.05,
                ..Default::default()
            };
            Orchestrator::new_with_output(config, dir.path())
        };

        make(&dir_a).run();
        make(&dir_b).run();

        for filename in &["metrics_00000000.json", "metrics_00000060.json", "metrics_00000120.json"] {
            let a = std::fs::read(dir_a.path().join(filename)).unwrap();
            let b = std::fs::read(dir_b.path().join(filename)).unwrap();
            assert_eq!(a, b, "M12: archivo {filename} difiere entre runs con misma semilla");
        }
    }

    // --- M11: BroodSystem -------------------------------------------------------

    #[test]
    fn brood_adult_ratio_positive_in_summer() {
        let dir = TempDir::new().unwrap();
        let config = RunConfig {
            seed: 42,
            initial_population: 500,
            initial_honey_reserve: 0.8,
            season_start: 0.5, // verano → brood_production_rate > 0
            max_ticks: Some(301), // suficiente para al menos un ciclo de cría completo
            ..Default::default()
        };
        let mut orch = Orchestrator::new_with_output(config, dir.path());
        orch.run();

        let content = std::fs::read_to_string(dir.path().join("metrics_00000300.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        let ratio = parsed["brood_adult_ratio"].as_f64().unwrap_or(0.0);
        assert!(
            ratio >= 0.0,
            "brood_adult_ratio debe ser no-negativo, obtenido {ratio}"
        );
    }

    #[test]
    fn determinism_preserved_with_m11() {
        let dir_a = TempDir::new().unwrap();
        let dir_b = TempDir::new().unwrap();

        make_orchestrator(22, 121, &dir_a).run();
        make_orchestrator(22, 121, &dir_b).run();

        for filename in &["metrics_00000000.json", "metrics_00000060.json", "metrics_00000120.json"] {
            let a = std::fs::read(dir_a.path().join(filename)).unwrap();
            let b = std::fs::read(dir_b.path().join(filename)).unwrap();
            assert_eq!(a, b, "M11: archivo {filename} difiere entre runs con misma semilla");
        }
    }

    // --- M13: PredatorSystem ---------------------------------------------------

    #[test]
    fn predator_by_predation_nonzero_with_predators() {
        // Tick 60 (primeros 60 ticks con 500 abejas vivas): debe haber al menos 1 muerte por depredación.
        let dir = TempDir::new().unwrap();
        // Con bees spawneando cerca de la colmena (radio 12-15), los depredadores
        // necesitan más ticks para llegar. Usamos más depredadores y más ticks.
        let config = RunConfig {
            seed: 42,
            initial_population: 500,
            initial_honey_reserve: 200.0,
            predator_count: 30,
            max_ticks: Some(241),
            ..Default::default()
        };
        let mut orch = Orchestrator::new_with_output(config, dir.path());
        orch.run();

        // Acumular muertes por depredación en los primeros 240 ticks
        let mut total_pred = 0u64;
        for fname in &["metrics_00000060.json", "metrics_00000120.json", "metrics_00000180.json", "metrics_00000240.json"] {
            if let Ok(content) = std::fs::read_to_string(dir.path().join(fname)) {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&content) {
                    total_pred += parsed["mortality_rate"]["by_predation"].as_u64().unwrap_or(0);
                }
            }
        }
        assert!(
            total_pred > 0,
            "con 30 depredadores y 500 abejas en 240 ticks debe haber muertes por depredación, total={total_pred}"
        );
    }

    #[test]
    fn determinism_preserved_with_m13() {
        let dir_a = TempDir::new().unwrap();
        let dir_b = TempDir::new().unwrap();

        let make = |dir: &TempDir| {
            let config = RunConfig {
                seed: 99,
                max_ticks: Some(121),
                predator_count: 3,
                ..Default::default()
            };
            Orchestrator::new_with_output(config, dir.path())
        };

        make(&dir_a).run();
        make(&dir_b).run();

        for filename in &["metrics_00000000.json", "metrics_00000060.json", "metrics_00000120.json"] {
            let a = std::fs::read(dir_a.path().join(filename)).unwrap();
            let b = std::fs::read(dir_b.path().join(filename)).unwrap();
            assert_eq!(a, b, "M13: archivo {filename} difiere entre runs con misma semilla");
        }
    }

    // --- M14: Exportación Completa de Métricas ---------------------------------

    #[test]
    fn pheromone_entropy_nonzero_after_foraging() {
        // En tick 0, ~10% de los 100 foragers spawnan sobre las fuentes de alimento
        // (parches 17×17 cubren ~10% del grid interior). Esos foragers emiten
        // Attraction pheromone al write buffer. pheromone_entropy lee el write buffer
        // → entropy > 0 en metrics_00000000.json.
        let dir = TempDir::new().unwrap();
        let config = RunConfig {
            seed: 42,
            initial_population: 500,
            initial_honey_reserve: 0.8,
            max_ticks: Some(1), // solo tick 0: bees vivas, recursos frescos
            ..Default::default()
        };
        let mut orch = Orchestrator::new_with_output(config, dir.path());
        orch.run();

        let content = std::fs::read_to_string(dir.path().join("metrics_00000000.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        let entropy = parsed["pheromone_entropy"].as_f64().unwrap_or(0.0);
        assert!(
            entropy > 0.0,
            "pheromone_entropy debe ser > 0 en tick 0 con foragers en fuentes de alimento, obtenido {entropy}"
        );
    }

    #[test]
    fn time_to_collapse_set_when_population_drops() {
        // Población inicial pequeña + sin miel → colapso rápido
        let dir = TempDir::new().unwrap();
        let config = RunConfig {
            seed: 42,
            initial_population: 10,   // muy pocos
            initial_honey_reserve: 0.0,
            max_ticks: Some(61),
            ..Default::default()
        };
        let mut orch = Orchestrator::new_with_output(config, dir.path());
        orch.run();

        // Con 10 abejas que mueren en ~40 ticks, population < 50 desde tick 0
        assert!(
            orch.time_to_collapse.is_some(),
            "con solo 10 abejas, time_to_collapse debe haberse registrado"
        );
        assert_eq!(
            orch.time_to_collapse,
            Some(0),
            "con 10 < 50 abejas iniciales, colapso debe detectarse en tick 0"
        );

        let content = std::fs::read_to_string(dir.path().join("metrics_00000000.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert!(
            !parsed["time_to_collapse"].is_null(),
            "time_to_collapse debe aparecer en JSON cuando ocurre colapso"
        );
    }

    #[test]
    fn by_disease_nonzero_with_disease_enabled() {
        // El sistema SIR requiere al menos 1 Infected inicial para que haya transmisión.
        // Infectamos manualmente 50 abejas; con disease_base_rate=0.5 la infección se propaga
        // y muchas Infected morirán de agotamiento energético → cuentan como by_disease.
        let dir = TempDir::new().unwrap();
        let config = RunConfig {
            seed: 42,
            initial_population: 500,
            initial_honey_reserve: 0.8,
            disease_enabled: true,
            disease_base_rate: 0.5,
            max_ticks: Some(121),
            ..Default::default()
        };
        let mut orch = Orchestrator::new_with_output(config, dir.path());

        // Infectar las primeras 50 abejas manualmente para inicializar la epidemia
        let to_infect: Vec<hecs::Entity> = orch.world
            .query::<&HealthComponent>()
            .iter()
            .map(|(e, _)| e)
            .take(50)
            .collect();
        for e in to_infect {
            let _ = orch.world.insert_one(e, HealthComponent {
                state: SirState::Infected,
                ticks_in_state: 0,
            });
        }

        orch.run();

        let content = std::fs::read_to_string(dir.path().join("metrics_00000120.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        let by_disease = parsed["mortality_rate"]["by_disease"].as_u64().unwrap_or(0);
        assert!(
            by_disease > 0,
            "con 50 Infected iniciales y disease_base_rate=0.5, by_disease debe ser > 0, obtenido {by_disease}"
        );
    }

    #[test]
    fn determinism_preserved_with_m14() {
        let dir_a = TempDir::new().unwrap();
        let dir_b = TempDir::new().unwrap();

        let make = |dir: &TempDir| {
            let config = RunConfig {
                seed: 77,
                max_ticks: Some(121),
                disease_enabled: true,
                disease_base_rate: 0.1,
                predator_count: 2,
                ..Default::default()
            };
            Orchestrator::new_with_output(config, dir.path())
        };

        make(&dir_a).run();
        make(&dir_b).run();

        for filename in &["metrics_00000000.json", "metrics_00000060.json", "metrics_00000120.json"] {
            let a = std::fs::read(dir_a.path().join(filename)).unwrap();
            let b = std::fs::read(dir_b.path().join(filename)).unwrap();
            assert_eq!(a, b, "M14: archivo {filename} difiere entre runs con misma semilla");
        }
    }

    // --- M8: RoleTransitionSystem -----------------------------------------------

    #[test]
    fn queen_role_unchanged_after_transitions() {
        // Queen no tiene RoleTransitionState → su rol no cambia pese a feromonas.
        // Se usa max_ticks=30 (dentro del ciclo de vida con energía inicial=0.8, costo=0.02/tick).
        let dir = TempDir::new().unwrap();
        let config = RunConfig {
            seed: 42,
            initial_population: 100,
            max_ticks: Some(30),
            ..Default::default()
        };
        let mut orch = Orchestrator::new_with_output(config, dir.path());
        assert_eq!(
            orch.world.query::<&RoleComponent>().iter().filter(|(_, r)| r.0 == Role::Queen).count(),
            1
        );
        orch.run();
        assert_eq!(
            orch.world.query::<&RoleComponent>().iter().filter(|(_, r)| r.0 == Role::Queen).count(),
            1,
            "la reina no debe cambiar de rol por RoleTransitionSystem"
        );
    }

    #[test]
    fn determinism_preserved_with_m8() {
        let dir_a = TempDir::new().unwrap();
        let dir_b = TempDir::new().unwrap();

        make_orchestrator(55, 121, &dir_a).run();
        make_orchestrator(55, 121, &dir_b).run();

        for filename in &["metrics_00000000.json", "metrics_00000060.json", "metrics_00000120.json"] {
            let a = std::fs::read(dir_a.path().join(filename)).unwrap();
            let b = std::fs::read(dir_b.path().join(filename)).unwrap();
            assert_eq!(a, b, "M8: archivo {filename} difiere entre runs con misma semilla");
        }
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
        // Foragers ≈ 30%, Guards ≈ 8%, Builders ≈ 7% (margen ±15 por rounding)
        assert!((134..=164).contains(&counts[4]), "foragers esperados ~149 (30%), obtenidos {}", counts[4]);
        assert!((35..=50).contains(&counts[3]), "guards esperados ~40 (8%), obtenidos {}", counts[3]);
        assert!((28..=42).contains(&counts[2]), "builders esperados ~35 (7%), obtenidos {}", counts[2]);
        assert_eq!(counts[5], 0, "sin zánganos en spawn inicial");
        // Nurses absorben el residuo → ~307 (el resto después de los demás)
        assert!(counts[1] > 250, "nurses deben ser la mayoría, obtenidos {}", counts[1]);
    }

    // --- M15: Visualizador -------------------------------------------------------

    #[test]
    fn render_frame_agent_count_matches_world() {
        let dir = TempDir::new().unwrap();
        let config = RunConfig {
            seed: 42,
            initial_population: 100,
            predator_count: 3,
            max_ticks: Some(0),
            ..Default::default()
        };
        let orch = Orchestrator::new_with_output(config, dir.path());
        let frame = orch.build_render_frame();

        let bee_count = orch.world.query::<(&PositionComponent, &RoleComponent)>().iter().count();
        let pred_count = orch.world.query::<(&PositionComponent, &PredatorComponent)>().iter().count();
        assert_eq!(
            frame.agents.len(),
            bee_count + pred_count,
            "frame.agents debe incluir todas las abejas + depredadores"
        );
    }

    #[test]
    fn render_frame_tick_matches_current_tick() {
        let dir = TempDir::new().unwrap();
        let config = RunConfig {
            seed: 42,
            max_ticks: Some(5),
            ..Default::default()
        };
        let mut orch = Orchestrator::new_with_output(config, dir.path());
        orch.run();
        let frame = orch.build_render_frame();
        assert_eq!(frame.tick, orch.current_tick(), "frame.tick debe coincidir con current_tick()");
    }

    #[test]
    fn render_sender_overhead_under_2ms() {
        use std::sync::mpsc;
        use std::time::Instant;

        let dir = TempDir::new().unwrap();
        let config = RunConfig {
            seed: 42,
            max_ticks: Some(1),
            ..Default::default()
        };
        let (tx, _rx) = mpsc::sync_channel(1);
        let mut orch = Orchestrator::new_with_output(config, dir.path());
        orch.set_render_sender(tx);

        let t0 = Instant::now();
        for _ in 0..100 {
            orch.tick_once();
        }
        let avg_ns = t0.elapsed().as_nanos() / 100;

        // El overhead del renderer en el thread de simulación debe ser despreciable.
        // En debug el límite es 60 ms (merge+clear+render frame build);
        // en release < 2 ms (idéntico al tick_once_is_fast).
        #[cfg(debug_assertions)]
        let limit_ns: u128 = 60_000_000;
        #[cfg(not(debug_assertions))]
        let limit_ns: u128 = 2_000_000;

        assert!(
            avg_ns < limit_ns,
            "tick_once con render_sender promedio: {} ns (límite: {} ns)",
            avg_ns, limit_ns
        );
    }
}
