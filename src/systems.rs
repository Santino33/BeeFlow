use rand::Rng;

use crate::components::{
    AgeComponent, BroodStage, BroodStageComponent, EnergyComponent, ForagerPhase,
    ForagerStateComponent, HealthComponent, PheromoneSensitivity, PositionComponent,
    PredatorComponent, Role, RoleComponent, RoleTransitionState, SirState,
};
use crate::grid::{PheromoneKind, SpatialGrid, HIVE_X, HIVE_Y};
use crate::rng::RngSystem;

/// Costo metabólico basal por tick (simulation_spec.md §Energía).
pub const METABOLIC_COST_BASAL: f32 = 0.008;

/// Máxima carga de néctar que puede llevar un Forager (simulation_spec.md §Forrajeo).
pub const FORAGER_CARRY_MAX: f32 = 0.5;

/// Radio Manhattan al que un Forager deposita su carga en la colmena.
pub const HIVE_DEPOSIT_RADIUS: u32 = 8;

/// Tasa de emisión de feromona de atracción al recolectar (simulation_spec.md §Feromonas).
pub const FORAGER_PHEROMONE_EMISSION: f32 = 0.15;

/// Mueve cada abeja a uno de sus 8 vecinos no-obstacle.
/// - Queen: no se mueve (simulation_spec.md §Roles).
/// - Forager Returning: mover greedy hacia colmena (minimiza distancia Manhattan).
/// - Forager Searching / Nurse / Builder / Guard: selección ponderada por feromona del rol.
/// - Drone: paseo aleatorio puro.
/// RNG por (entity_id, tick) para determinismo estable post-despawn.
pub fn run_movement_system(
    world: &mut hecs::World,
    grid: &mut SpatialGrid,
    rng_system: &RngSystem,
    tick: u64,
) {
    for (entity, (pos, role, sens, forager_state)) in world.query_mut::<(
        &mut PositionComponent,
        Option<&RoleComponent>,
        Option<&PheromoneSensitivity>,
        Option<&ForagerStateComponent>,
    )>() {
        // Reina no se mueve; entidades sin rol (cría pasiva) tampoco.
        if role.map_or(true, |r| r.0 == Role::Queen) {
            continue;
        }

        let x = pos.0 as usize;
        let y = pos.1 as usize;

        let valid: arrayvec::ArrayVec<(usize, usize), 8> = grid
            .neighbors_8(x, y)
            .into_iter()
            .filter(|&(nx, ny)| !grid.is_obstacle[SpatialGrid::idx(nx, ny)])
            .collect();

        if valid.is_empty() {
            continue;
        }

        let mut agent_rng = rng_system.agent_rng_for_tick(entity.id() as u64, tick);

        // Forager Returning: mover greedy hacia colmena
        let is_returning = matches!(
            forager_state.map(|s| s.0),
            Some(ForagerPhase::Returning { .. })
        );

        let chosen_idx = if is_returning {
            let hive_x = HIVE_X as i32;
            let hive_y = HIVE_Y as i32;
            let cur_dist = manhattan(x as i32, y as i32, hive_x, hive_y);
            // Preferir vecinos que acercan a la colmena
            let closer: arrayvec::ArrayVec<usize, 8> = valid
                .iter()
                .enumerate()
                .filter(|(_, &(nx, ny))| manhattan(nx as i32, ny as i32, hive_x, hive_y) < cur_dist)
                .map(|(i, _)| i)
                .collect();
            if closer.is_empty() {
                agent_rng.gen_range(0..valid.len())
            } else {
                closer[agent_rng.gen_range(0..closer.len())]
            }
        } else {
            // Canal de feromona según rol
            let pheromone_kind = role.and_then(|r| match r.0 {
                Role::Forager             => Some(PheromoneKind::Attraction),
                Role::Nurse | Role::Builder => Some(PheromoneKind::Task),
                Role::Guard               => Some(PheromoneKind::Alarm),
                _                         => None,
            });

            if let (Some(kind), Some(sensitivity)) = (pheromone_kind, sens) {
                let channel_sens = sensitivity.0[kind as usize];
                if channel_sens > 0.0 {
                    let weights: arrayvec::ArrayVec<f32, 8> = valid
                        .iter()
                        .map(|&(nx, ny)| 1.0_f32 + grid.pheromone(nx, ny, kind) * channel_sens)
                        .collect();
                    let total: f32 = weights.iter().sum();
                    let mut pick = agent_rng.gen::<f32>() * total;
                    weights
                        .iter()
                        .position(|&w| {
                            pick -= w;
                            pick <= 0.0
                        })
                        .unwrap_or(valid.len() - 1)
                } else {
                    agent_rng.gen_range(0..valid.len())
                }
            } else {
                agent_rng.gen_range(0..valid.len())
            }
        };

        let (nx, ny) = valid[chosen_idx];
        let oi = SpatialGrid::idx(x, y);
        let ni = SpatialGrid::idx(nx, ny);
        debug_assert!(grid.occupancy[oi] > 0, "occupancy underflow en ({x},{y})");
        grid.occupancy[oi] -= 1;
        grid.occupancy[ni] = grid.occupancy[ni].saturating_add(1);

        pos.0 = nx as u16;
        pos.1 = ny as u16;
    }
}

#[inline(always)]
fn manhattan(ax: i32, ay: i32, bx: i32, by: i32) -> i32 {
    (ax - bx).abs() + (ay - by).abs()
}

/// Incrementa la edad de todas las abejas en 1 tick.
pub fn run_age_system(world: &mut hecs::World) {
    for (_, age) in world.query_mut::<&mut AgeComponent>() {
        age.0 += 1;
    }
}

/// Aplica el costo metabólico a todas las abejas, modulado por temperatura, rol y enfermedad.
/// Drone: 0.025/tick (simulation_spec.md §Roles). Resto: METABOLIC_COST_BASAL.
/// Foragers reciben drain adicional de `0.001 × pesticide_pressure` por pesticidas (M10).
/// Infected: ×DISEASE_ENERGY_MULTIPLIER sobre el costo total (M12).
/// La energía se clampea a 0.0; la muerte la gestiona MortalitySystem (M5).
pub fn run_energy_system(world: &mut hecs::World, global_temp: f32, pesticide_pressure: f32) {
    let temp_factor = 1.0 + 0.01 * (global_temp - 20.0);
    for (_, (energy, role, health)) in world.query_mut::<(
        &mut EnergyComponent,
        Option<&RoleComponent>,
        Option<&HealthComponent>,
    )>() {
        let base_cost = match role.map(|r| r.0) {
            Some(Role::Drone) => 0.025,
            _                 => METABOLIC_COST_BASAL,
        };
        let extra_cost = if role.map(|r| r.0) == Some(Role::Forager) {
            0.001 * pesticide_pressure
        } else {
            0.0
        };
        let disease_factor = if health.map(|h| h.state == SirState::Infected).unwrap_or(false) {
            DISEASE_ENERGY_MULTIPLIER
        } else {
            1.0
        };
        energy.0 = (energy.0 - (base_cost + extra_cost) * temp_factor * disease_factor).max(0.0);
    }
}

/// Elimina entidades con energía ≤ 0. Decrementa ocupación del grid.
/// Devuelve `(deaths_by_energy, deaths_by_disease)`: Infected que mueren se cuentan como disease.
pub fn run_mortality_system(world: &mut hecs::World, grid: &mut SpatialGrid) -> (u32, u32) {
    let dead: Vec<(hecs::Entity, PositionComponent, bool)> = world
        .query::<(&EnergyComponent, &PositionComponent, Option<&HealthComponent>)>()
        .iter()
        .filter_map(|(e, (energy, pos, health))| {
            if energy.0 <= 0.0 {
                let is_disease = health.map(|h| h.state == SirState::Infected).unwrap_or(false);
                Some((e, *pos, is_disease))
            } else {
                None
            }
        })
        .collect();

    let mut by_energy = 0u32;
    let mut by_disease = 0u32;
    for (entity, pos, is_disease) in dead {
        let idx = SpatialGrid::idx(pos.0 as usize, pos.1 as usize);
        debug_assert!(grid.occupancy[idx] > 0, "occupancy underflow en muerte ({},{})", pos.0, pos.1);
        grid.occupancy[idx] -= 1;
        let _ = world.despawn(entity);
        if is_disease { by_disease += 1; } else { by_energy += 1; }
    }
    (by_energy, by_disease)
}

/// Ciclo completo de forrajeo (M7). simulation_spec.md §Forrajeo y §Energía.
///
/// - Forager Searching en celda con recurso:
///   1. Gana energía personal: resource × 2.0, clampeada (spec §Energía).
///   2. Llena carry con el recurso consumido (para entregar a la colmena).
///   3. Emite feromona Attraction.
///   4. Pasa a estado Returning.
/// - Forager Returning cerca de colmena: deposita carry en honey_reserve, pasa a Searching.
pub fn run_foraging_system(
    world: &mut hecs::World,
    grid: &mut SpatialGrid,
    honey_reserve: &mut f32,
    honey_collected: &mut f32,
) {
    for (_, (pos, role, energy, state, health)) in world.query_mut::<(
        &PositionComponent,
        &RoleComponent,
        &mut EnergyComponent,
        &mut ForagerStateComponent,
        Option<&HealthComponent>,
    )>() {
        if role.0 != Role::Forager {
            continue;
        }

        let x = pos.0 as usize;
        let y = pos.1 as usize;
        let idx = SpatialGrid::idx(x, y);

        match state.0 {
            ForagerPhase::Searching => {
                let res = grid.resource_amount[idx];
                if res > 0.0 {
                    // Infected Forager gana menos energía (simulation_spec.md §Disease).
                    let efficiency = if health.map(|h| h.state == SirState::Infected).unwrap_or(false) {
                        DISEASE_FORAGING_EFFICIENCY
                    } else {
                        1.0
                    };
                    // Ganancia personal (spec §Energía: resource × 2.0 × efficiency, clampeada).
                    let gain = (res * 2.0 * efficiency).min(1.0 - energy.0);
                    energy.0 += gain;
                    let personal_consumed = gain / 2.0;
                    // Carry: recurso adicional para la colmena
                    let carry = (res - personal_consumed).min(FORAGER_CARRY_MAX);
                    grid.resource_amount[idx] = (res - personal_consumed - carry).max(0.0);
                    grid.add_pheromone(x, y, PheromoneKind::Attraction, FORAGER_PHEROMONE_EMISSION);
                    state.0 = ForagerPhase::Returning { carry };
                }
            }
            ForagerPhase::Returning { carry } => {
                let dist = manhattan(x as i32, y as i32, HIVE_X as i32, HIVE_Y as i32) as u32;
                if dist <= HIVE_DEPOSIT_RADIUS {
                    *honey_reserve += carry;
                    *honey_collected += carry;
                    state.0 = ForagerPhase::Searching;
                }
            }
        }
    }
}

/// Regenera recursos en las fuentes de alimento a 0.001/tick × season_factor.
/// simulation_spec.md §Recursos.
pub fn run_resource_regeneration(
    grid: &mut SpatialGrid,
    food_sources: &[(usize, usize)],
    season_factor: f32,
) {
    for &(x, y) in food_sources {
        let idx = SpatialGrid::idx(x, y);
        grid.resource_amount[idx] = (grid.resource_amount[idx] + 0.001 * season_factor).min(1.0);
    }
}

/// Reserva de colonia por debajo de la cual se activa la trofalaxia (simulation_spec.md §Energy).
pub const TROPHALLAXIS_RESERVE_THRESHOLD: f32 = 0.70;

/// Energía transferida por tick entre un par adyacente durante trofalaxia.
pub const TROPHALLAXIS_TRANSFER_RATE: f32 = 0.05;

/// Redistribuye energía entre abejas adyacentes cuando la reserva de colonia está baja.
///
/// Activado solo cuando `honey_reserve < TROPHALLAXIS_RESERVE_THRESHOLD`.
/// Por cada par en celdas adyacentes (Chebyshev ≤ 1): transfiere `TROPHALLAXIS_TRANSFER_RATE`
/// de la abeja con más energía a la de menos. simulation_spec.md §Energy.
pub fn run_trophallaxis_system(world: &mut hecs::World, honey_reserve: f32) {
    use std::collections::HashMap;

    if honey_reserve >= TROPHALLAXIS_RESERVE_THRESHOLD {
        return;
    }

    // Paso 1: snapshot de posiciones y energías
    let agents: Vec<(hecs::Entity, u16, u16, f32)> = world
        .query::<(&PositionComponent, &EnergyComponent)>()
        .iter()
        .map(|(e, (pos, en))| (e, pos.0, pos.1, en.0))
        .collect();

    // Paso 2: calcular deltas para todos los pares adyacentes
    let mut deltas: HashMap<hecs::Entity, f32> = HashMap::new();

    for i in 0..agents.len() {
        for j in (i + 1)..agents.len() {
            let (e_i, x_i, y_i, en_i) = agents[i];
            let (e_j, x_j, y_j, en_j) = agents[j];

            let dx = (x_i as i32 - x_j as i32).unsigned_abs();
            let dy = (y_i as i32 - y_j as i32).unsigned_abs();
            if dx > 1 || dy > 1 {
                continue; // Chebyshev > 1 → no adyacentes
            }
            if (en_i - en_j).abs() < 1e-6 {
                continue; // misma energía → sin transferencia
            }

            let (donor, receiver, en_d, en_r) = if en_i >= en_j {
                (e_i, e_j, en_i, en_j)
            } else {
                (e_j, e_i, en_j, en_i)
            };

            let transfer = TROPHALLAXIS_TRANSFER_RATE
                .min(en_d - en_r) // no superar la diferencia
                .min(en_d);       // donor no cae a negativo

            *deltas.entry(donor).or_insert(0.0) -= transfer;
            *deltas.entry(receiver).or_insert(0.0) += transfer;
        }
    }

    // Paso 3: aplicar deltas
    for (entity, (energy,)) in world.query_mut::<(&mut EnergyComponent,)>() {
        if let Some(&delta) = deltas.get(&entity) {
            energy.0 = (energy.0 + delta).clamp(0.0, 1.0);
        }
    }
}

/// Umbral base del modelo Fixed-Threshold (Seeley 1995). simulation_spec.md §RoleTransition.
pub const ROLE_TRANSITION_THRESHOLD_BASE: f32 = 0.5;

/// Ticks mínimos entre transiciones de rol por agente.
pub const ROLE_TRANSITION_COOLDOWN: u32 = 30;

fn gaussian(x: f32, mean: f32, sigma: f32) -> f32 {
    (-(x - mean).powi(2) / (2.0 * sigma.powi(2))).exp()
}

/// Factor de edad por rol candidato (simulation_spec.md §RoleTransition).
fn age_factor(role: Role, age: u32) -> f32 {
    let a = age as f32;
    match role {
        Role::Nurse   => (1.0 - a / 400.0).max(0.0),
        Role::Builder => gaussian(a, 250.0, 100.0),
        Role::Guard   => gaussian(a, 350.0, 100.0),
        Role::Forager => (a / 400.0).min(1.0),
        _             => 0.0,
    }
}

/// Factor de salud para la probabilidad de transición.
fn health_factor(state: SirState) -> f32 {
    match state {
        SirState::Susceptible => 1.0,
        SirState::Infected    => 0.5,
        SirState::Recovered   => 0.9,
    }
}

/// Feromona que impulsa la transición hacia un rol candidato.
fn candidate_pheromone(role: Role) -> PheromoneKind {
    match role {
        Role::Nurse | Role::Builder => PheromoneKind::Task,
        Role::Guard                 => PheromoneKind::Alarm,
        Role::Forager               => PheromoneKind::Attraction,
        _                           => PheromoneKind::Task,
    }
}

/// Evalúa el Fixed-Threshold Model y cambia roles según estímulos de feromona.
///
/// Fórmula: `P(→T) = s²/(s²+θ²)` donde `s = pheromone × age_factor × health_factor`.
/// Solo abejas con `RoleTransitionState` son evaluadas (Queen/Drone no tienen ese componente).
/// Al transicionar DESDE/HACIA Forager, gestiona `ForagerStateComponent` de forma coherente.
pub fn run_role_transition_system(
    world: &mut hecs::World,
    grid: &SpatialGrid,
    rng_system: &RngSystem,
    tick: u64,
) {
    const CANDIDATES: [Role; 4] = [Role::Nurse, Role::Builder, Role::Guard, Role::Forager];

    // Fase 1: evaluar transiciones y decrementar cooldowns.
    // transition_data = (entity, new_role, from_forager, to_forager, threshold_bias)
    let transition_data: Vec<(hecs::Entity, Role, bool, bool, f32)> = {
        let mut out = Vec::new();
        for (entity, (pos, role, age, health, ts)) in world.query_mut::<(
            &PositionComponent,
            &RoleComponent,
            &AgeComponent,
            &HealthComponent,
            &mut RoleTransitionState,
        )>() {
            if ts.cooldown > 0 {
                ts.cooldown -= 1;
                continue;
            }
            let x = pos.0 as usize;
            let y = pos.1 as usize;
            let hf = health_factor(health.state);
            let theta = ROLE_TRANSITION_THRESHOLD_BASE * (1.0 + ts.threshold_bias);
            let bias = ts.threshold_bias;
            let mut rng = rng_system.agent_rng_for_tick(entity.id() as u64, tick);
            for &candidate in CANDIDATES.iter() {
                if candidate == role.0 {
                    continue;
                }
                let kind = candidate_pheromone(candidate);
                let stimulus = grid.pheromone(x, y, kind) * age_factor(candidate, age.0) * hf;
                let p = stimulus * stimulus / (stimulus * stimulus + theta * theta);
                let r: f32 = rng.gen();
                if r < p {
                    out.push((entity, candidate, role.0 == Role::Forager, candidate == Role::Forager, bias));
                    break;
                }
            }
        }
        out
    };

    // Fase 2: aplicar transiciones usando insert_one (reemplaza componentes existentes).
    for (entity, new_role, from_forager, to_forager, bias) in transition_data {
        if from_forager {
            let _ = world.remove_one::<ForagerStateComponent>(entity);
        }
        if to_forager {
            let _ = world.insert_one(entity, ForagerStateComponent(ForagerPhase::Searching));
        }
        let _ = world.insert_one(entity, RoleComponent(new_role));
        let _ = world.insert_one(entity, RoleTransitionState { cooldown: ROLE_TRANSITION_COOLDOWN, threshold_bias: bias });
        let new_sens = match new_role {
            Role::Nurse | Role::Builder => [0.0, 1.0, 0.0],
            Role::Guard                 => [1.0, 0.0, 0.0],
            Role::Forager               => [0.0, 0.0, 1.0],
            _                           => [0.0, 0.0, 0.0],
        };
        let _ = world.insert_one(entity, PheromoneSensitivity(new_sens));
    }
}

// ---------------------------------------------------------------------------
// M12: DiseaseSystem (SIR)
// ---------------------------------------------------------------------------

/// Duración del estado Infected antes de pasar a Recovered. simulation_spec.md §Disease.
pub const DISEASE_INFECTION_DURATION: u32 = 200;
/// Duración del estado Recovered antes de volver a Susceptible.
pub const DISEASE_RECOVERY_DURATION: u32 = 300;
/// Factor multiplicativo de costo metabólico para Infected (+20%).
pub const DISEASE_ENERGY_MULTIPLIER: f32 = 1.20;
/// Factor de eficiencia de forrajeo para Infected (-30%).
pub const DISEASE_FORAGING_EFFICIENCY: f32 = 0.70;
/// Susceptibilidad reducida para abejas jóvenes (age < 100).
pub const DISEASE_YOUNG_SUSCEPTIBILITY: f32 = 0.8;

/// Transmisión SIR y progresión de la enfermedad. simulation_spec.md §Sistema Epidemiológico.
///
/// Fórmula: `P(S→I) = base_rate × contact_density × susceptibility`
/// donde `contact_density` = nº de Infected en celda propia + 8 celdas adyacentes (Chebyshev ≤ 1).
/// No-op si `enabled=false`.
pub fn run_disease_system(
    world: &mut hecs::World,
    rng_system: &RngSystem,
    tick: u64,
    base_rate: f32,
    enabled: bool,
) {
    if !enabled {
        return;
    }

    use crate::grid::{GRID_H, GRID_W};

    // Fase 1 — Mapa de Infected por celda
    let mut infected_per_cell = vec![0u32; GRID_W * GRID_H];
    for (_, (pos, health)) in world
        .query::<(&PositionComponent, &HealthComponent)>()
        .iter()
    {
        if health.state == SirState::Infected {
            infected_per_cell[SpatialGrid::idx(pos.0 as usize, pos.1 as usize)] += 1;
        }
    }

    // Fase 2 — Densidad de contacto (Chebyshev ≤ 1) para cada celda
    let mut contact_density = vec![0u32; GRID_W * GRID_H];
    for y in 0..GRID_H {
        for x in 0..GRID_W {
            let mut sum = 0u32;
            for nx in x.saturating_sub(1)..=(x + 1).min(GRID_W - 1) {
                for ny in y.saturating_sub(1)..=(y + 1).min(GRID_H - 1) {
                    sum += infected_per_cell[SpatialGrid::idx(nx, ny)];
                }
            }
            contact_density[SpatialGrid::idx(x, y)] = sum;
        }
    }

    // Fase 3 — Evaluar transiciones (query inmutable → Vec)
    let mut to_infect:      Vec<hecs::Entity> = Vec::new();
    let mut to_recover:     Vec<hecs::Entity> = Vec::new();
    let mut to_susceptible: Vec<hecs::Entity> = Vec::new();

    for (entity, (pos, health, age)) in world
        .query::<(&PositionComponent, &HealthComponent, &AgeComponent)>()
        .iter()
    {
        match health.state {
            SirState::Susceptible => {
                let cd = contact_density[SpatialGrid::idx(pos.0 as usize, pos.1 as usize)] as f32;
                if cd == 0.0 {
                    continue;
                }
                let susceptibility = if age.0 < 100 { DISEASE_YOUNG_SUSCEPTIBILITY } else { 1.0 };
                let p = base_rate * cd * susceptibility;
                let mut rng = rng_system.agent_rng_for_tick(entity.id() as u64, tick);
                if rng.gen::<f32>() < p {
                    to_infect.push(entity);
                }
            }
            SirState::Infected => {
                if health.ticks_in_state >= DISEASE_INFECTION_DURATION {
                    to_recover.push(entity);
                }
            }
            SirState::Recovered => {
                if health.ticks_in_state >= DISEASE_RECOVERY_DURATION {
                    to_susceptible.push(entity);
                }
            }
        }
    }

    // Fase 3b — Aplicar transiciones
    for entity in to_infect {
        let _ = world.insert_one(entity, HealthComponent { state: SirState::Infected, ticks_in_state: 0 });
    }
    for entity in to_recover {
        let _ = world.insert_one(entity, HealthComponent { state: SirState::Recovered, ticks_in_state: 0 });
    }
    for entity in to_susceptible {
        let _ = world.insert_one(entity, HealthComponent { state: SirState::Susceptible, ticks_in_state: 0 });
    }

    // Fase 4 — Avanzar timers de Infected y Recovered
    for (_, health) in world.query_mut::<&mut HealthComponent>() {
        if health.state != SirState::Susceptible {
            health.ticks_in_state += 1;
        }
    }
}

// ---------------------------------------------------------------------------
// M11: BroodSystem
// ---------------------------------------------------------------------------

/// Umbral de edad (ticks desde oviposición) para cada transición de etapa.
pub const BROOD_EGG_END: u32   = 36;
pub const BROOD_LARVA_END: u32 = 36 + 144; // 180
pub const BROOD_PUPA_END: u32  = 36 + 144 + 120; // 300

/// Pérdida de salud virtual por tick cuando no hay Nurse adyacente. simulation_spec.md §Cría.
pub const BROOD_HEALTH_DRAIN: f32 = 0.01;

/// Feromona Task emitida por cada larva por tick.
pub const BROOD_TASK_EMISSION: f32 = 0.1;

/// Ciclo de cría completo: oviposición, transiciones de etapa, feromona, muerte y eclosión.
///
/// - Fase 1: Queen pone huevo con probabilidad `brood_production_rate` (si > 0).
/// - Fase 2: Snapshot de posiciones de Nurses para chequeo de adyacencia.
/// - Fase 3: Transiciones Egg→Larva→Pupa, drain de salud virtual sin Nurse, emisión de feromona Task.
/// - Fase 4: Despawn larvas muertas; eclosión de pupas completas como Bee Nurse.
pub fn run_brood_system(
    world: &mut hecs::World,
    grid: &mut SpatialGrid,
    rng_system: &RngSystem,
    tick: u64,
    brood_production_rate: f32,
) {
    // Fase 1 — Queen pone huevo
    if brood_production_rate > 0.0 {
        let queen_pos: Option<(u16, u16)> = world
            .query::<(&RoleComponent, &PositionComponent)>()
            .iter()
            .find_map(|(_, (r, p))| {
                if r.0 == Role::Queen { Some((p.0, p.1)) } else { None }
            });

        if let Some((qx, qy)) = queen_pos {
            let mut rng = rng_system.tick_rng(tick);
            if rng.gen::<f32>() < brood_production_rate {
                world.spawn((
                    PositionComponent(qx, qy),
                    AgeComponent(0),
                    BroodStageComponent(BroodStage::Egg),
                ));
            }
        }
    }

    // Fase 2 — Snapshot de posiciones de Nurses
    use std::collections::HashSet;
    let nurse_positions: HashSet<(u16, u16)> = world
        .query::<(&RoleComponent, &PositionComponent)>()
        .iter()
        .filter_map(|(_, (r, p))| {
            if r.0 == Role::Nurse { Some((p.0, p.1)) } else { None }
        })
        .collect();

    // Fase 3 — Transiciones, feromona, drain de salud
    for (_, (age, stage, pos)) in world.query_mut::<(
        &AgeComponent,
        &mut BroodStageComponent,
        &PositionComponent,
    )>() {
        let x = pos.0 as usize;
        let y = pos.1 as usize;
        let a = age.0;

        match stage.0 {
            BroodStage::Egg => {
                if a >= BROOD_EGG_END {
                    stage.0 = BroodStage::Larva { health_virtual: 1.0 };
                }
            }
            BroodStage::Larva { ref mut health_virtual } => {
                grid.add_pheromone(x, y, PheromoneKind::Task, BROOD_TASK_EMISSION);

                let has_nurse = chebyshev_adjacent(pos.0, pos.1, &nurse_positions);
                if !has_nurse {
                    *health_virtual -= BROOD_HEALTH_DRAIN;
                }

                if a >= BROOD_LARVA_END {
                    stage.0 = BroodStage::Pupa;
                }
            }
            BroodStage::Pupa => {}
        }
    }

    // Fase 4 — Recoger despawns y eclosiones
    let dead_larvae: Vec<hecs::Entity> = world
        .query::<&BroodStageComponent>()
        .iter()
        .filter_map(|(e, s)| {
            if matches!(s.0, BroodStage::Larva { health_virtual } if health_virtual <= 0.0) {
                Some(e)
            } else {
                None
            }
        })
        .collect();

    let ready_eclose: Vec<(hecs::Entity, u16, u16)> = world
        .query::<(&BroodStageComponent, &AgeComponent, &PositionComponent)>()
        .iter()
        .filter_map(|(e, (s, age, pos))| {
            if s.0 == BroodStage::Pupa && age.0 >= BROOD_PUPA_END {
                Some((e, pos.0, pos.1))
            } else {
                None
            }
        })
        .collect();

    for entity in dead_larvae {
        let _ = world.despawn(entity);
    }

    for (entity, ex, ey) in ready_eclose {
        let bias: f32 = rng_system
            .agent_rng_for_tick(entity.id() as u64, tick)
            .gen::<f32>()
            * 0.2
            - 0.1;
        let _ = world.despawn(entity);
        world.spawn((
            PositionComponent(ex, ey),
            RoleComponent(Role::Nurse),
            EnergyComponent(0.8),
            HealthComponent { state: SirState::Susceptible, ticks_in_state: 0 },
            AgeComponent(0),
            PheromoneSensitivity([0.0, 1.0, 0.0]),
            RoleTransitionState { cooldown: 0, threshold_bias: bias },
        ));
        let idx = SpatialGrid::idx(ex as usize, ey as usize);
        grid.occupancy[idx] = grid.occupancy[idx].saturating_add(1);
    }
}

#[inline(always)]
fn chebyshev_adjacent(x: u16, y: u16, set: &std::collections::HashSet<(u16, u16)>) -> bool {
    let xi = x as i32;
    let yi = y as i32;
    for dy in -1i32..=1 {
        for dx in -1i32..=1 {
            let nx = xi + dx;
            let ny = yi + dy;
            if nx >= 0 && ny >= 0 && set.contains(&(nx as u16, ny as u16)) {
                return true;
            }
        }
    }
    false
}

// ---------------------------------------------------------------------------
// M13: PredatorSystem
// ---------------------------------------------------------------------------

/// Probabilidad de ataque exitoso por tick. simulation_spec.md §Predator.
pub const PREDATOR_ATTACK_RATE: f32 = 0.3;
/// Radio de detección Chebyshev (área 5×5 centrada en el depredador).
pub const PREDATOR_DETECTION_RADIUS: u32 = 2;
/// Energía drenada a la abeja atacada por impacto.
pub const PREDATOR_ENERGY_DRAIN: f32 = 0.4;
/// Feromona Alarm emitida por Guard cuando detecta un depredador en su radio.
pub const PREDATOR_ALARM_EMISSION: f32 = 0.2;

/// Movimiento, ataques y respuesta de Guards frente a depredadores.
///
/// - Fase 1: cada depredador se mueve siguiendo el gradiente de feromona Attraction
///   (neighbors_8, Chebyshev ≤ 1) o hace paseo aleatorio si no hay gradiente.
/// - Fase 2: cada depredador ataca una abeja aleatoria dentro de Chebyshev ≤ 2
///   con probabilidad `attack_rate`. Si la abeja cae a energía 0, se mata en este tick.
/// - Fase 3: Guards en radio Chebyshev ≤ 2 de un depredador emiten feromona Alarm.
///
/// Retorna el número de abejas muertas por depredación en este tick.
pub fn run_predator_system(
    world: &mut hecs::World,
    grid: &mut SpatialGrid,
    rng_system: &RngSystem,
    tick: u64,
) -> u32 {
    // Fase 1 — Mover depredadores (collect → apply)
    let predator_moves: Vec<(hecs::Entity, u16, u16, u16, u16)> = {
        let mut moves = Vec::new();
        for (entity, (pos, _pred)) in world
            .query::<(&PositionComponent, &PredatorComponent)>()
            .iter()
        {
            let x = pos.0 as usize;
            let y = pos.1 as usize;

            let valid: arrayvec::ArrayVec<(usize, usize), 8> = grid
                .neighbors_8(x, y)
                .into_iter()
                .filter(|&(nx, ny)| !grid.is_obstacle[SpatialGrid::idx(nx, ny)])
                .collect();

            if valid.is_empty() {
                continue;
            }

            let mut rng = rng_system.agent_rng_for_tick(entity.id() as u64, tick);

            // Elige el vecino con mayor feromona Attraction; si todos < 0.05, paseo aleatorio
            let max_attr = valid
                .iter()
                .map(|&(nx, ny)| grid.pheromone(nx, ny, PheromoneKind::Attraction))
                .fold(0.0_f32, f32::max);

            let chosen = if max_attr > 0.05 {
                *valid
                    .iter()
                    .max_by(|&&(ax, ay), &&(bx, by)| {
                        grid.pheromone(ax, ay, PheromoneKind::Attraction)
                            .partial_cmp(&grid.pheromone(bx, by, PheromoneKind::Attraction))
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .unwrap()
            } else {
                valid[rng.gen_range(0..valid.len())]
            };

            moves.push((entity, pos.0, pos.1, chosen.0 as u16, chosen.1 as u16));
        }
        moves
    };

    for (entity, ox, oy, nx, ny) in predator_moves {
        let oi = SpatialGrid::idx(ox as usize, oy as usize);
        let ni = SpatialGrid::idx(nx as usize, ny as usize);
        debug_assert!(grid.occupancy[oi] > 0, "predator occupancy underflow en ({ox},{oy})");
        grid.occupancy[oi] -= 1;
        grid.occupancy[ni] = grid.occupancy[ni].saturating_add(1);
        let _ = world.insert_one(entity, PositionComponent(nx, ny));
    }

    // Snapshot de entidades y posiciones de depredadores para fases 2 y 3
    let predators: Vec<(hecs::Entity, usize, usize, f32, f32)> = world
        .query::<(&PositionComponent, &PredatorComponent)>()
        .iter()
        .map(|(e, (pos, pred))| {
            (e, pos.0 as usize, pos.1 as usize, pred.attack_rate, pred.energy_drain_on_hit)
        })
        .collect();

    if predators.is_empty() {
        return 0;
    }

    // Fase 2 — Ataques: snapshot de abejas → collect hits → apply
    let bee_snapshot: Vec<(hecs::Entity, u16, u16, f32)> = world
        .query::<(&PositionComponent, &EnergyComponent, &RoleComponent)>()
        .iter()
        .map(|(e, (pos, en, _))| (e, pos.0, pos.1, en.0))
        .collect();

    let mut energy_updates: Vec<(hecs::Entity, f32)> = Vec::new();
    let mut to_kill: Vec<(hecs::Entity, u16, u16)> = Vec::new();

    let radius = PREDATOR_DETECTION_RADIUS as i32;
    for (pred_entity, px, py, attack_rate, energy_drain) in &predators {
        let in_radius: Vec<(hecs::Entity, u16, u16, f32)> = bee_snapshot
            .iter()
            .filter(|&&(_, bx, by, _)| {
                (bx as i32 - *px as i32).abs() <= radius
                    && (by as i32 - *py as i32).abs() <= radius
            })
            .copied()
            .collect();

        if in_radius.is_empty() {
            continue;
        }

        // RNG con id del depredador + offset para distinguir de la fase de movimiento
        let mut rng = rng_system.agent_rng_for_tick(
            pred_entity.id() as u64 ^ 0x5A5A_5A5A_u64,
            tick,
        );

        if rng.gen::<f32>() < *attack_rate {
            let target_idx = rng.gen_range(0..in_radius.len());
            let (target_entity, tx, ty, old_energy) = in_radius[target_idx];
            let new_energy = (old_energy - energy_drain).max(0.0);
            if new_energy <= 0.0 {
                to_kill.push((target_entity, tx, ty));
            } else {
                energy_updates.push((target_entity, new_energy));
            }
        }
    }

    for (entity, new_energy) in energy_updates {
        let _ = world.insert_one(entity, EnergyComponent(new_energy));
    }

    let deaths = to_kill.len() as u32;
    for (entity, x, y) in to_kill {
        let idx = SpatialGrid::idx(x as usize, y as usize);
        if grid.occupancy[idx] > 0 {
            grid.occupancy[idx] -= 1;
        }
        let _ = world.despawn(entity);
    }

    // Fase 3 — Guards adyacentes a un depredador emiten feromona Alarm
    let guard_positions: Vec<(u16, u16)> = world
        .query::<(&PositionComponent, &RoleComponent)>()
        .iter()
        .filter_map(|(_, (pos, role))| {
            if role.0 == Role::Guard { Some((pos.0, pos.1)) } else { None }
        })
        .collect();

    for (gx, gy) in guard_positions {
        let near = predators.iter().any(|&(_, px, py, _, _)| {
            (gx as i32 - px as i32).abs() <= radius
                && (gy as i32 - py as i32).abs() <= radius
        });
        if near {
            grid.add_pheromone(
                gx as usize,
                gy as usize,
                PheromoneKind::Alarm,
                PREDATOR_ALARM_EMISSION,
            );
        }
    }

    deaths
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::*;
    use crate::grid::SpatialGrid;
    use crate::rng::RngSystem;

    fn make_bee(x: u16, y: u16) -> (
        PositionComponent,
        RoleComponent,
        EnergyComponent,
        HealthComponent,
        AgeComponent,
        PheromoneSensitivity,
    ) {
        (
            PositionComponent(x, y),
            RoleComponent(Role::Nurse),
            EnergyComponent(0.8),
            HealthComponent { state: SirState::Susceptible, ticks_in_state: 0 },
            AgeComponent(0),
            PheromoneSensitivity([1.0, 1.0, 1.0]),
        )
    }

    #[test]
    fn spawn_and_read_all_components() {
        let mut world = hecs::World::new();
        let entity = world.spawn(make_bee(50, 50));

        let mut q = world.query_one::<(
            &PositionComponent,
            &RoleComponent,
            &EnergyComponent,
            &HealthComponent,
            &AgeComponent,
            &PheromoneSensitivity,
        )>(entity)
        .unwrap();
        let (pos, role, energy, health, age, sens) = q.get().unwrap();

        assert_eq!(*pos, PositionComponent(50, 50));
        assert_eq!(role.0, Role::Nurse);
        assert!((energy.0 - 0.8).abs() < 1e-6);
        assert_eq!(health.state, SirState::Susceptible);
        assert_eq!(age.0, 0);
        assert!((sens.0[0] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn despawn_no_zombie() {
        let mut world = hecs::World::new();
        let entity = world.spawn(make_bee(50, 50));
        world.despawn(entity).unwrap();

        let count = world.query::<&PositionComponent>().iter().count();
        assert_eq!(count, 0, "no debe haber entidades tras despawn");
    }

    #[test]
    fn movement_updates_position() {
        let mut world = hecs::World::new();
        let mut grid = SpatialGrid::new();
        let rng_system = RngSystem::new(42);

        let entity = world.spawn(make_bee(50, 50));
        grid.occupancy[SpatialGrid::idx(50, 50)] = 1;

        run_movement_system(&mut world, &mut grid, &rng_system, 0);

        let mut q = world.query_one::<&PositionComponent>(entity).unwrap();
        let pos = q.get().unwrap();
        assert!(pos.0 != 50 || pos.1 != 50, "la abeja debe haberse movido desde (50,50)");
        assert!(SpatialGrid::in_bounds(pos.0 as usize, pos.1 as usize));
        assert!(!SpatialGrid::is_border_xy(pos.0 as usize, pos.1 as usize));
    }

    #[test]
    fn age_increments_per_tick() {
        let mut world = hecs::World::new();
        world.spawn(make_bee(50, 50));

        run_age_system(&mut world);
        run_age_system(&mut world);
        run_age_system(&mut world);

        for (_, age) in world.query::<&AgeComponent>().iter() {
            assert_eq!(age.0, 3);
        }
    }

    // --- M5: MortalitySystem ------------------------------------------------

    #[test]
    fn dead_bee_removed_from_world() {
        let mut world = hecs::World::new();
        let mut grid = SpatialGrid::new();
        let idx = SpatialGrid::idx(50, 50);
        grid.occupancy[idx] = 1;
        world.spawn((PositionComponent(50, 50), EnergyComponent(0.0), AgeComponent(0)));

        run_mortality_system(&mut world, &mut grid);

        assert_eq!(world.query::<&EnergyComponent>().iter().count(), 0);
    }

    #[test]
    fn alive_bee_stays() {
        let mut world = hecs::World::new();
        let mut grid = SpatialGrid::new();
        grid.occupancy[SpatialGrid::idx(50, 50)] = 1;
        world.spawn((PositionComponent(50, 50), EnergyComponent(0.01), AgeComponent(0)));

        run_mortality_system(&mut world, &mut grid);

        assert_eq!(world.query::<&EnergyComponent>().iter().count(), 1);
    }

    #[test]
    fn occupancy_decremented_on_death() {
        let mut world = hecs::World::new();
        let mut grid = SpatialGrid::new();
        let idx = SpatialGrid::idx(50, 50);
        grid.occupancy[idx] = 1;
        world.spawn((PositionComponent(50, 50), EnergyComponent(0.0), AgeComponent(0)));

        run_mortality_system(&mut world, &mut grid);

        assert_eq!(grid.occupancy[idx], 0);
    }

    #[test]
    fn all_dead_at_tick_50() {
        let mut world = hecs::World::new();
        let mut grid = SpatialGrid::new();
        for _ in 0..1_000 {
            let idx = SpatialGrid::idx(50, 50);
            grid.occupancy[idx] = grid.occupancy[idx].saturating_add(1);
            world.spawn((PositionComponent(50, 50), EnergyComponent(1.0), AgeComponent(0)));
        }

        for _ in 0..51 {
            run_energy_system(&mut world, 20.0, 0.0);
            run_mortality_system(&mut world, &mut grid);
        }

        assert_eq!(
            world.query::<&EnergyComponent>().iter().count(),
            0,
            "todas las abejas deben haber muerto en ~51 ticks (1.0 / 0.02 con f32)"
        );
    }

    // --- M4: EnergySystem ---------------------------------------------------

    #[test]
    fn energy_decreases_by_basal_cost() {
        let mut world = hecs::World::new();
        world.spawn(make_bee(50, 50)); // energy = 0.8

        run_energy_system(&mut world, 20.0, 0.0);

        for (_, energy) in world.query::<&EnergyComponent>().iter() {
            assert!(
                (energy.0 - 0.78).abs() < 1e-5,
                "energy esperada 0.78, obtenida {}",
                energy.0
            );
        }
    }

    #[test]
    fn energy_clamped_at_zero() {
        let mut world = hecs::World::new();
        world.spawn((EnergyComponent(0.01), AgeComponent(0)));

        run_energy_system(&mut world, 20.0, 0.0);

        for (_, energy) in world.query::<&EnergyComponent>().iter() {
            assert!(energy.0 >= 0.0, "energy no debe ser negativa");
        }
    }

    #[test]
    fn energy_reaches_zero_at_tick_50() {
        let mut world = hecs::World::new();
        world.spawn((EnergyComponent(1.0), AgeComponent(0)));

        for _ in 0..50 {
            run_energy_system(&mut world, 20.0, 0.0);
        }

        for (_, energy) in world.query::<&EnergyComponent>().iter() {
            assert!(
                energy.0 < 1e-5,
                "energy debe ser ~0 tras 50 ticks, obtenida {}",
                energy.0
            );
        }
    }

    #[test]
    fn temp_modulates_cost() {
        let mut world = hecs::World::new();
        world.spawn((EnergyComponent(1.0), AgeComponent(0)));

        run_energy_system(&mut world, 30.0, 0.0); // factor = 1 + 0.01*10 = 1.1 → cost = 0.022

        for (_, energy) in world.query::<&EnergyComponent>().iter() {
            let expected = 1.0 - 0.022_f32;
            assert!(
                (energy.0 - expected).abs() < 1e-5,
                "a 30°C energy esperada {}, obtenida {}",
                expected,
                energy.0
            );
        }
    }

    // --- H1: occupancy como contador estricto ----------------------------------

    #[test]
    fn occupancy_conservation_with_multiple_agents() {
        let mut world = hecs::World::new();
        let mut grid = SpatialGrid::new();
        let rng_system = RngSystem::new(42);

        for _ in 0..3 {
            grid.occupancy[SpatialGrid::idx(50, 50)] += 1;
            world.spawn(make_bee(50, 50));
        }

        run_movement_system(&mut world, &mut grid, &rng_system, 0);

        let total: u64 = grid.occupancy.iter().sum();
        assert_eq!(total, 3, "la suma de occupancy debe ser 3 tras el movimiento");
    }

    // --- M6: Roles diferenciados ------------------------------------------------

    #[test]
    fn queen_skips_movement() {
        let mut world = hecs::World::new();
        let mut grid = SpatialGrid::new();
        let rng_system = RngSystem::new(42);

        let idx = SpatialGrid::idx(50, 50);
        grid.occupancy[idx] = 1;
        let entity = world.spawn((
            PositionComponent(50, 50),
            RoleComponent(Role::Queen),
            EnergyComponent(0.8),
            AgeComponent(0),
            PheromoneSensitivity([0.0, 0.0, 0.0]),
        ));

        for t in 0..10 {
            run_movement_system(&mut world, &mut grid, &rng_system, t);
        }

        let mut q = world.query_one::<&PositionComponent>(entity).unwrap();
        let pos = q.get().unwrap();
        assert_eq!((pos.0, pos.1), (50, 50), "la reina no debe moverse");
    }

    #[test]
    fn drone_energy_cost_is_higher() {
        let mut world = hecs::World::new();
        world.spawn((EnergyComponent(1.0), RoleComponent(Role::Drone), AgeComponent(0)));

        run_energy_system(&mut world, 20.0, 0.0);

        for (_, energy) in world.query::<&EnergyComponent>().iter() {
            let expected = 1.0_f32 - 0.025;
            assert!(
                (energy.0 - expected).abs() < 1e-5,
                "Drone debe costar 0.025/tick, obtenido {}",
                energy.0
            );
        }
    }

    #[test]
    fn forager_biased_toward_attraction_pheromone() {
        let mut world = hecs::World::new();
        let mut grid = SpatialGrid::new();
        let rng_system = RngSystem::new(7);

        grid.add_pheromone(55, 50, PheromoneKind::Attraction, 1.0);
        grid.swap_pheromone_buffers();

        let idx = SpatialGrid::idx(50, 50);
        grid.occupancy[idx] = 1;
        let entity = world.spawn((
            PositionComponent(50, 50),
            RoleComponent(Role::Forager),
            EnergyComponent(0.8),
            AgeComponent(0),
            PheromoneSensitivity([0.0, 0.0, 1.0]),
            ForagerStateComponent(ForagerPhase::Searching),
        ));

        let mut visits_right = 0u32;
        let mut visits_total = 0u32;
        let n_ticks = 200u64;
        for t in 0..n_ticks {
            run_movement_system(&mut world, &mut grid, &rng_system, t);
            let mut q = world.query_one::<&PositionComponent>(entity).unwrap();
            let pos = q.get().unwrap();
            if pos.0 > 50 { visits_right += 1; }
            visits_total += 1;
        }
        let frac = visits_right as f32 / visits_total as f32;
        assert!(
            frac > 0.4,
            "Forager debe preferir el lado con feromona, fracción derecha: {:.2}",
            frac
        );
    }

    // --- H2: determinismo post-despawn -----------------------------------------

    #[test]
    fn movement_deterministic_after_despawn() {
        let rng_system = RngSystem::new(99);

        let run = || {
            let mut world = hecs::World::new();
            let mut grid = SpatialGrid::new();
            let mut entities = Vec::new();
            for i in 0..3u16 {
                grid.occupancy[SpatialGrid::idx(50, (50 + i) as usize)] += 1;
                entities.push(world.spawn(make_bee(50, 50 + i)));
            }
            let idx0 = SpatialGrid::idx(50, 50);
            debug_assert!(grid.occupancy[idx0] > 0);
            grid.occupancy[idx0] -= 1;
            world.despawn(entities[0]).unwrap();

            run_movement_system(&mut world, &mut grid, &rng_system, 1);

            let mut positions: Vec<(u32, u16, u16)> = world
                .query::<&PositionComponent>()
                .iter()
                .map(|(e, p)| (e.id(), p.0, p.1))
                .collect();
            positions.sort_by_key(|&(id, _, _)| id);
            positions
        };

        let a = run();
        let b = run();
        assert_eq!(a, b, "posiciones deben ser idénticas en dos runs con misma seed tras despawn");
    }

    // --- M7: ForagingSystem completo -------------------------------------------

    #[test]
    fn forager_personal_energy_increases_on_collect() {
        let mut world = hecs::World::new();
        let mut grid = SpatialGrid::new();
        let x = 30usize;
        let y = 30usize;
        grid.resource_amount[SpatialGrid::idx(x, y)] = 1.0;

        world.spawn((
            PositionComponent(x as u16, y as u16),
            RoleComponent(Role::Forager),
            EnergyComponent(0.5), // energía inicial baja para ver el gain
            AgeComponent(0),
            ForagerStateComponent(ForagerPhase::Searching),
        ));

        let mut honey = 0.0_f32;
        let mut collected = 0.0_f32;
        run_foraging_system(&mut world, &mut grid, &mut honey, &mut collected);

        for (_, energy) in world.query::<&EnergyComponent>().iter() {
            assert!(energy.0 > 0.5, "Forager debe haber ganado energía personal, obtenido {}", energy.0);
        }
    }

    #[test]
    fn forager_switches_to_returning_on_resource() {
        let mut world = hecs::World::new();
        let mut grid = SpatialGrid::new();
        let x = 30usize;
        let y = 30usize;
        grid.resource_amount[SpatialGrid::idx(x, y)] = 0.8;

        world.spawn((
            PositionComponent(x as u16, y as u16),
            RoleComponent(Role::Forager),
            EnergyComponent(0.8),
            AgeComponent(0),
            ForagerStateComponent(ForagerPhase::Searching),
        ));

        let mut honey = 0.0_f32;
        let mut collected = 0.0_f32;
        run_foraging_system(&mut world, &mut grid, &mut honey, &mut collected);

        // Recurso decrece
        assert!(grid.resource_amount[SpatialGrid::idx(x, y)] < 0.8, "recurso debe haber disminuido");

        // Estado cambió a Returning
        for (_, state) in world.query::<&ForagerStateComponent>().iter() {
            assert!(
                matches!(state.0, ForagerPhase::Returning { .. }),
                "Forager debe estar en estado Returning tras recolectar"
            );
        }
    }

    #[test]
    fn forager_emits_pheromone_on_collect() {
        let mut world = hecs::World::new();
        let mut grid = SpatialGrid::new();
        let x = 30usize;
        let y = 30usize;
        grid.resource_amount[SpatialGrid::idx(x, y)] = 1.0;

        world.spawn((
            PositionComponent(x as u16, y as u16),
            RoleComponent(Role::Forager),
            EnergyComponent(0.8),
            AgeComponent(0),
            ForagerStateComponent(ForagerPhase::Searching),
        ));

        let mut honey = 0.0_f32;
        let mut collected = 0.0_f32;
        run_foraging_system(&mut world, &mut grid, &mut honey, &mut collected);
        // La feromona queda en write_buf; visible tras swap
        grid.swap_pheromone_buffers();

        let phero = grid.pheromone(x, y, PheromoneKind::Attraction);
        assert!(phero > 0.0, "debe haber feromona Attraction en la celda de recolección (phero={})", phero);
    }

    #[test]
    fn forager_no_collect_when_no_resource() {
        let mut world = hecs::World::new();
        let mut grid = SpatialGrid::new();
        // Sin recurso (resource_amount = 0 por defecto)

        world.spawn((
            PositionComponent(30, 30),
            RoleComponent(Role::Forager),
            EnergyComponent(0.8),
            AgeComponent(0),
            ForagerStateComponent(ForagerPhase::Searching),
        ));

        let mut honey = 0.0_f32;
        let mut collected = 0.0_f32;
        run_foraging_system(&mut world, &mut grid, &mut honey, &mut collected);

        for (_, state) in world.query::<&ForagerStateComponent>().iter() {
            assert_eq!(state.0, ForagerPhase::Searching, "sin recurso el Forager sigue Searching");
        }
        assert_eq!(honey, 0.0);
    }

    #[test]
    fn forager_deposits_at_hive_increments_reserve() {
        let mut world = hecs::World::new();
        let mut grid = SpatialGrid::new();
        let carry = 0.4_f32;

        // Forager justo en la colmena con carga
        world.spawn((
            PositionComponent(HIVE_X as u16, HIVE_Y as u16),
            RoleComponent(Role::Forager),
            EnergyComponent(0.8),
            AgeComponent(0),
            ForagerStateComponent(ForagerPhase::Returning { carry }),
        ));

        let mut honey = 0.5_f32;
        let mut collected = 0.0_f32;
        run_foraging_system(&mut world, &mut grid, &mut honey, &mut collected);

        assert!((honey - (0.5 + carry)).abs() < 1e-5, "honey_reserve debe aumentar en carry={}", carry);
        assert!((collected - carry).abs() < 1e-5);

        for (_, state) in world.query::<&ForagerStateComponent>().iter() {
            assert_eq!(state.0, ForagerPhase::Searching, "tras depositar debe volver a Searching");
        }
    }

    #[test]
    fn forager_returning_far_from_hive_does_not_deposit() {
        let mut world = hecs::World::new();
        let mut grid = SpatialGrid::new();

        // Posición lejos de la colmena (dist > HIVE_DEPOSIT_RADIUS)
        let x = 20u16;
        let y = 20u16;
        world.spawn((
            PositionComponent(x, y),
            RoleComponent(Role::Forager),
            EnergyComponent(0.8),
            AgeComponent(0),
            ForagerStateComponent(ForagerPhase::Returning { carry: 0.3 }),
        ));

        let mut honey = 0.0_f32;
        let mut collected = 0.0_f32;
        run_foraging_system(&mut world, &mut grid, &mut honey, &mut collected);

        assert_eq!(honey, 0.0, "no debe depositar lejos de la colmena");
        for (_, state) in world.query::<&ForagerStateComponent>().iter() {
            assert!(matches!(state.0, ForagerPhase::Returning { .. }), "debe seguir Returning");
        }
    }

    #[test]
    fn resource_regenerates_each_tick() {
        let mut grid = SpatialGrid::new();
        let sources = vec![(20usize, 50usize)];
        grid.resource_amount[SpatialGrid::idx(20, 50)] = 0.5;

        run_resource_regeneration(&mut grid, &sources, 1.0);

        let res = grid.resource_amount[SpatialGrid::idx(20, 50)];
        assert!((res - 0.501).abs() < 1e-5, "recurso debe haber crecido en 0.001, obtenido {}", res);
    }

    #[test]
    fn resource_regeneration_clamped_at_1() {
        let mut grid = SpatialGrid::new();
        let sources = vec![(20usize, 50usize)];
        grid.resource_amount[SpatialGrid::idx(20, 50)] = 1.0;

        run_resource_regeneration(&mut grid, &sources, 1.0);

        let res = grid.resource_amount[SpatialGrid::idx(20, 50)];
        assert!((res - 1.0).abs() < 1e-5, "recurso no debe superar 1.0");
    }

    // --- M10: EnergySystem con pesticidas ---------------------------------------

    #[test]
    fn pesticide_drains_forager_extra() {
        let mut world = hecs::World::new();
        let forager = world.spawn((
            EnergyComponent(1.0),
            RoleComponent(Role::Forager),
            AgeComponent(0),
        ));
        let nurse = world.spawn((
            EnergyComponent(1.0),
            RoleComponent(Role::Nurse),
            AgeComponent(0),
        ));

        run_energy_system(&mut world, 20.0, 1.0); // pesticide_pressure = 1.0

        let mut qf = world.query_one::<&EnergyComponent>(forager).unwrap();
        let mut qn = world.query_one::<&EnergyComponent>(nurse).unwrap();
        let forager_energy = qf.get().unwrap().0;
        let nurse_energy   = qn.get().unwrap().0;

        // Forager pierde METABOLIC_COST_BASAL + 0.001 * 1.0 = 0.021
        assert!(
            (forager_energy - (1.0 - 0.021)).abs() < 1e-5,
            "Forager debe perder 0.021/tick con pesticida=1.0, obtenido {}",
            forager_energy
        );
        // Nurse no tiene extra drain
        assert!(
            (nurse_energy - (1.0 - 0.020)).abs() < 1e-5,
            "Nurse debe perder solo 0.020/tick, obtenido {}",
            nurse_energy
        );
    }

    // --- M9: TrophallaxisSystem -------------------------------------------------

    #[test]
    fn no_trophallaxis_above_threshold() {
        let mut world = hecs::World::new();
        world.spawn((PositionComponent(30, 30), EnergyComponent(0.8), AgeComponent(0)));
        world.spawn((PositionComponent(31, 30), EnergyComponent(0.3), AgeComponent(0)));

        run_trophallaxis_system(&mut world, 0.5); // reserve >= 0.30 → no-op

        let energies: Vec<f32> = world.query::<&EnergyComponent>().iter()
            .map(|(_, e)| e.0).collect();
        assert!(energies.iter().any(|&e| (e - 0.8).abs() < 1e-5), "energía alta sin cambio");
        assert!(energies.iter().any(|&e| (e - 0.3).abs() < 1e-5), "energía baja sin cambio");
    }

    #[test]
    fn transfer_from_high_to_low_energy() {
        let mut world = hecs::World::new();
        let e_high = world.spawn((PositionComponent(30, 30), EnergyComponent(0.8), AgeComponent(0)));
        let e_low  = world.spawn((PositionComponent(31, 30), EnergyComponent(0.3), AgeComponent(0)));

        run_trophallaxis_system(&mut world, 0.1); // reserve < 0.30

        let mut q_high = world.query_one::<&EnergyComponent>(e_high).unwrap();
        let mut q_low  = world.query_one::<&EnergyComponent>(e_low).unwrap();
        let high_after = q_high.get().unwrap().0;
        let low_after  = q_low.get().unwrap().0;

        assert!((high_after - 0.75).abs() < 1e-5, "donor debe perder 0.05, obtenido {high_after}");
        assert!((low_after  - 0.35).abs() < 1e-5, "receptor debe ganar 0.05, obtenido {low_after}");
    }

    #[test]
    fn same_cell_bees_exchange() {
        let mut world = hecs::World::new();
        let e_high = world.spawn((PositionComponent(30, 30), EnergyComponent(0.7), AgeComponent(0)));
        let e_low  = world.spawn((PositionComponent(30, 30), EnergyComponent(0.2), AgeComponent(0)));

        run_trophallaxis_system(&mut world, 0.1);

        let mut q_high = world.query_one::<&EnergyComponent>(e_high).unwrap();
        let mut q_low  = world.query_one::<&EnergyComponent>(e_low).unwrap();
        assert!(q_high.get().unwrap().0 < 0.7, "abeja en misma celda con más energía debe perder");
        assert!(q_low.get().unwrap().0  > 0.2, "abeja en misma celda con menos energía debe ganar");
    }

    #[test]
    fn no_transfer_when_far_apart() {
        let mut world = hecs::World::new();
        world.spawn((PositionComponent(10, 10), EnergyComponent(0.9), AgeComponent(0)));
        world.spawn((PositionComponent(30, 30), EnergyComponent(0.1), AgeComponent(0)));

        run_trophallaxis_system(&mut world, 0.1);

        let mut energies: Vec<f32> = world.query::<&EnergyComponent>().iter()
            .map(|(_, e)| e.0).collect();
        energies.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert!((energies[0] - 0.1).abs() < 1e-5, "abeja lejana no debe ganar energía");
        assert!((energies[1] - 0.9).abs() < 1e-5, "abeja lejana no debe perder energía");
    }

    #[test]
    fn donor_energy_not_negative() {
        let mut world = hecs::World::new();
        let e_donor = world.spawn((PositionComponent(30, 30), EnergyComponent(0.03), AgeComponent(0)));
        world.spawn((PositionComponent(31, 30), EnergyComponent(0.0), AgeComponent(0)));

        run_trophallaxis_system(&mut world, 0.1);

        let mut q = world.query_one::<&EnergyComponent>(e_donor).unwrap();
        let after = q.get().unwrap().0;
        assert!(after >= 0.0, "donor no debe tener energía negativa, obtenido {after}");
    }

    #[test]
    fn no_transfer_equal_energy() {
        let mut world = hecs::World::new();
        world.spawn((PositionComponent(30, 30), EnergyComponent(0.5), AgeComponent(0)));
        world.spawn((PositionComponent(31, 30), EnergyComponent(0.5), AgeComponent(0)));

        run_trophallaxis_system(&mut world, 0.1);

        for (_, en) in world.query::<&EnergyComponent>().iter() {
            assert!((en.0 - 0.5).abs() < 1e-5, "abejas con igual energía no deben cambiar");
        }
    }

    #[test]
    fn single_bee_no_change() {
        let mut world = hecs::World::new();
        let entity = world.spawn((PositionComponent(30, 30), EnergyComponent(0.6), AgeComponent(0)));

        run_trophallaxis_system(&mut world, 0.1);

        let mut q = world.query_one::<&EnergyComponent>(entity).unwrap();
        assert!((q.get().unwrap().0 - 0.6).abs() < 1e-5, "abeja sola no debe cambiar");
    }

    // --- M8: RoleTransitionSystem -----------------------------------------------

    fn make_transition_bee(x: u16, y: u16, role: Role, age: u32) -> (
        PositionComponent,
        RoleComponent,
        EnergyComponent,
        HealthComponent,
        AgeComponent,
        PheromoneSensitivity,
        RoleTransitionState,
    ) {
        let sens = match role {
            Role::Nurse | Role::Builder => PheromoneSensitivity([0.0, 1.0, 0.0]),
            Role::Guard                 => PheromoneSensitivity([1.0, 0.0, 0.0]),
            Role::Forager               => PheromoneSensitivity([0.0, 0.0, 1.0]),
            _                           => PheromoneSensitivity([0.0, 0.0, 0.0]),
        };
        (
            PositionComponent(x, y),
            RoleComponent(role),
            EnergyComponent(0.8),
            HealthComponent { state: SirState::Susceptible, ticks_in_state: 0 },
            AgeComponent(age),
            sens,
            RoleTransitionState { cooldown: 0, threshold_bias: 0.0 },
        )
    }

    #[test]
    fn queen_never_transitions() {
        // Queen no tiene RoleTransitionState → no es queryeada → nunca transiciona.
        let mut world = hecs::World::new();
        let grid = SpatialGrid::new();
        let rng = RngSystem::new(42);
        world.spawn((
            PositionComponent(50, 50),
            RoleComponent(Role::Queen),
            EnergyComponent(0.8),
            HealthComponent { state: SirState::Susceptible, ticks_in_state: 0 },
            AgeComponent(0),
            PheromoneSensitivity([0.0, 0.0, 0.0]),
        ));
        run_role_transition_system(&mut world, &grid, &rng, 0);
        for (_, role) in world.query::<&RoleComponent>().iter() {
            assert_eq!(role.0, Role::Queen);
        }
    }

    #[test]
    fn drone_never_transitions() {
        let mut world = hecs::World::new();
        let grid = SpatialGrid::new();
        let rng = RngSystem::new(42);
        world.spawn((
            PositionComponent(50, 50),
            RoleComponent(Role::Drone),
            EnergyComponent(0.8),
            HealthComponent { state: SirState::Susceptible, ticks_in_state: 0 },
            AgeComponent(0),
            PheromoneSensitivity([0.0, 0.0, 0.0]),
        ));
        run_role_transition_system(&mut world, &grid, &rng, 0);
        for (_, role) in world.query::<&RoleComponent>().iter() {
            assert_eq!(role.0, Role::Drone);
        }
    }

    #[test]
    fn no_transition_without_pheromone() {
        // Con stimulus = 0, P = 0 → sin transición.
        let mut world = hecs::World::new();
        let grid = SpatialGrid::new(); // pheromone = 0 en todo el grid
        let rng = RngSystem::new(42);
        world.spawn(make_transition_bee(50, 50, Role::Nurse, 200));
        run_role_transition_system(&mut world, &grid, &rng, 0);
        for (_, role) in world.query::<&RoleComponent>().iter() {
            assert_eq!(role.0, Role::Nurse, "sin feromona no debe haber transición");
        }
    }

    #[test]
    fn cooldown_prevents_transition() {
        let mut world = hecs::World::new();
        let mut grid = SpatialGrid::new();
        let rng = RngSystem::new(42);

        // Feromona alta para que P ≈ 1 si no hubiera cooldown
        grid.add_pheromone(50, 50, PheromoneKind::Attraction, 100.0);
        grid.swap_pheromone_buffers();

        world.spawn((
            PositionComponent(50, 50),
            RoleComponent(Role::Nurse),
            EnergyComponent(0.8),
            HealthComponent { state: SirState::Susceptible, ticks_in_state: 0 },
            AgeComponent(400), // age_factor(Forager,400)=1.0
            PheromoneSensitivity([0.0, 1.0, 0.0]),
            RoleTransitionState { cooldown: 15, threshold_bias: 0.0 },
        ));

        run_role_transition_system(&mut world, &grid, &rng, 0);

        // Rol no cambió
        for (_, role) in world.query::<&RoleComponent>().iter() {
            assert_eq!(role.0, Role::Nurse, "cooldown debe impedir la transición");
        }
        // Cooldown decrementó
        for (_, ts) in world.query::<&RoleTransitionState>().iter() {
            assert_eq!(ts.cooldown, 14, "cooldown debe decrementar en 1");
        }
    }

    #[test]
    fn transition_to_forager_adds_forager_state() {
        // Nurse con feromona Attraction muy alta y age=400 → casi certeza de transición a Forager.
        let mut world = hecs::World::new();
        let mut grid = SpatialGrid::new();
        let rng = RngSystem::new(42);

        grid.add_pheromone(30, 30, PheromoneKind::Attraction, 1000.0);
        grid.swap_pheromone_buffers();

        world.spawn(make_transition_bee(30, 30, Role::Nurse, 400));

        run_role_transition_system(&mut world, &grid, &rng, 0);

        let roles: Vec<Role> = world.query::<&RoleComponent>().iter().map(|(_, r)| r.0).collect();
        assert_eq!(roles, vec![Role::Forager], "debe haber transicionado a Forager");

        let has_state = world.query::<&ForagerStateComponent>().iter().count() == 1;
        assert!(has_state, "Forager recién transicionado debe tener ForagerStateComponent");
    }

    #[test]
    fn transition_from_forager_removes_forager_state() {
        // Forager con feromona Task alta y age=200 → transición a Nurse.
        let mut world = hecs::World::new();
        let mut grid = SpatialGrid::new();
        let rng = RngSystem::new(42);

        grid.add_pheromone(30, 30, PheromoneKind::Task, 1000.0);
        grid.swap_pheromone_buffers();

        world.spawn((
            PositionComponent(30, 30),
            RoleComponent(Role::Forager),
            EnergyComponent(0.8),
            HealthComponent { state: SirState::Susceptible, ticks_in_state: 0 },
            AgeComponent(200),
            PheromoneSensitivity([0.0, 0.0, 1.0]),
            ForagerStateComponent(ForagerPhase::Searching),
            RoleTransitionState { cooldown: 0, threshold_bias: 0.0 },
        ));

        run_role_transition_system(&mut world, &grid, &rng, 0);

        // Debe haber transicionado a Nurse
        for (_, role) in world.query::<&RoleComponent>().iter() {
            assert_eq!(role.0, Role::Nurse, "Forager debe haber transicionado a Nurse");
        }
        // Ya no debe tener ForagerStateComponent
        let still_has = world.query::<&ForagerStateComponent>().iter().count();
        assert_eq!(still_has, 0, "ForagerStateComponent debe haberse eliminado al dejar de ser Forager");
    }

    #[test]
    fn cooldown_set_after_transition() {
        let mut world = hecs::World::new();
        let mut grid = SpatialGrid::new();
        let rng = RngSystem::new(42);

        grid.add_pheromone(30, 30, PheromoneKind::Attraction, 1000.0);
        grid.swap_pheromone_buffers();

        world.spawn(make_transition_bee(30, 30, Role::Nurse, 400));

        run_role_transition_system(&mut world, &grid, &rng, 0);

        for (_, ts) in world.query::<&RoleTransitionState>().iter() {
            assert_eq!(ts.cooldown, ROLE_TRANSITION_COOLDOWN, "cooldown debe ser 30 tras transición");
        }
    }

    #[test]
    fn age_factor_nurse_zero_at_max_age() {
        assert!((age_factor(Role::Nurse, 400) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn age_factor_forager_one_at_max_age() {
        assert!((age_factor(Role::Forager, 400) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn age_factor_forager_zero_at_birth() {
        assert!((age_factor(Role::Forager, 0) - 0.0).abs() < 1e-6);
    }

    // --- M11: BroodSystem -------------------------------------------------------

    fn make_brood_world_with_queen() -> (hecs::World, SpatialGrid, RngSystem) {
        let mut world = hecs::World::new();
        let mut grid = SpatialGrid::new();
        let rng = RngSystem::new(42);
        let idx = SpatialGrid::idx(50, 50);
        grid.occupancy[idx] += 1;
        world.spawn((
            PositionComponent(50, 50),
            RoleComponent(Role::Queen),
            EnergyComponent(0.8),
            HealthComponent { state: SirState::Susceptible, ticks_in_state: 0 },
            AgeComponent(0),
            PheromoneSensitivity([0.0, 0.0, 0.0]),
        ));
        (world, grid, rng)
    }

    #[test]
    fn egg_spawned_when_rate_one() {
        let (mut world, mut grid, rng) = make_brood_world_with_queen();
        run_brood_system(&mut world, &mut grid, &rng, 0, 1.0);
        let count = world.query::<&BroodStageComponent>().iter().count();
        assert_eq!(count, 1, "con rate=1.0 debe spawnearse exactamente 1 huevo");
    }

    #[test]
    fn no_egg_spawned_when_rate_zero() {
        let (mut world, mut grid, rng) = make_brood_world_with_queen();
        run_brood_system(&mut world, &mut grid, &rng, 0, 0.0);
        let count = world.query::<&BroodStageComponent>().iter().count();
        assert_eq!(count, 0, "con rate=0.0 no debe spawnearse nada");
    }

    #[test]
    fn egg_transitions_to_larva_at_36() {
        let mut world = hecs::World::new();
        let mut grid = SpatialGrid::new();
        let rng = RngSystem::new(42);
        world.spawn((
            PositionComponent(50, 50),
            AgeComponent(BROOD_EGG_END),
            BroodStageComponent(BroodStage::Egg),
        ));
        run_brood_system(&mut world, &mut grid, &rng, 0, 0.0);
        for (_, s) in world.query::<&BroodStageComponent>().iter() {
            assert!(
                matches!(s.0, BroodStage::Larva { .. }),
                "egg con age=36 debe transicionar a Larva"
            );
        }
    }

    #[test]
    fn larva_transitions_to_pupa_at_180() {
        let mut world = hecs::World::new();
        let mut grid = SpatialGrid::new();
        let rng = RngSystem::new(42);
        world.spawn((
            PositionComponent(50, 50),
            AgeComponent(BROOD_LARVA_END),
            BroodStageComponent(BroodStage::Larva { health_virtual: 1.0 }),
        ));
        run_brood_system(&mut world, &mut grid, &rng, 0, 0.0);
        for (_, s) in world.query::<&BroodStageComponent>().iter() {
            assert_eq!(s.0, BroodStage::Pupa, "larva con age=180 debe transicionar a Pupa");
        }
    }

    #[test]
    fn pupa_ecloses_at_300() {
        let mut world = hecs::World::new();
        let mut grid = SpatialGrid::new();
        let rng = RngSystem::new(42);
        world.spawn((
            PositionComponent(50, 50),
            AgeComponent(BROOD_PUPA_END),
            BroodStageComponent(BroodStage::Pupa),
        ));
        run_brood_system(&mut world, &mut grid, &rng, 0, 0.0);
        let brood_left = world.query::<&BroodStageComponent>().iter().count();
        assert_eq!(brood_left, 0, "pupa completa debe despawnearse");
        let nurses = world.query::<&RoleComponent>().iter()
            .filter(|(_, r)| r.0 == Role::Nurse).count();
        assert_eq!(nurses, 1, "debe haber eclosionado 1 Nurse");
    }

    #[test]
    fn eclosed_nurse_has_correct_components() {
        let mut world = hecs::World::new();
        let mut grid = SpatialGrid::new();
        let rng = RngSystem::new(42);
        world.spawn((
            PositionComponent(40, 40),
            AgeComponent(BROOD_PUPA_END),
            BroodStageComponent(BroodStage::Pupa),
        ));
        run_brood_system(&mut world, &mut grid, &rng, 0, 0.0);
        for (_, (pos, role, energy, age)) in
            world.query::<(&PositionComponent, &RoleComponent, &EnergyComponent, &AgeComponent)>().iter()
        {
            assert_eq!(role.0, Role::Nurse);
            assert!((energy.0 - 0.8).abs() < 1e-5, "Energy debe ser 0.8, obtenido {}", energy.0);
            assert_eq!(age.0, 0, "Age debe ser 0 al eclosionar");
            assert_eq!((pos.0, pos.1), (40, 40), "posición debe coincidir con la pupa");
        }
    }

    #[test]
    fn larva_health_drains_without_nurse() {
        let mut world = hecs::World::new();
        let mut grid = SpatialGrid::new();
        let rng = RngSystem::new(42);
        world.spawn((
            PositionComponent(50, 50),
            AgeComponent(40), // age < BROOD_LARVA_END → no transiciona a Pupa
            BroodStageComponent(BroodStage::Larva { health_virtual: 1.0 }),
        ));
        run_brood_system(&mut world, &mut grid, &rng, 0, 0.0);
        for (_, s) in world.query::<&BroodStageComponent>().iter() {
            if let BroodStage::Larva { health_virtual } = s.0 {
                assert!(
                    (health_virtual - (1.0 - BROOD_HEALTH_DRAIN)).abs() < 1e-5,
                    "health_virtual debe haber caído en {BROOD_HEALTH_DRAIN}, obtenido {health_virtual}"
                );
            } else {
                panic!("debe seguir siendo Larva");
            }
        }
    }

    #[test]
    fn larva_survives_with_adjacent_nurse() {
        let mut world = hecs::World::new();
        let mut grid = SpatialGrid::new();
        let rng = RngSystem::new(42);
        // Larva en (50, 50)
        world.spawn((
            PositionComponent(50, 50),
            AgeComponent(40),
            BroodStageComponent(BroodStage::Larva { health_virtual: 1.0 }),
        ));
        // Nurse adyacente en (51, 50)
        world.spawn((
            PositionComponent(51, 50),
            RoleComponent(Role::Nurse),
            EnergyComponent(0.8),
            HealthComponent { state: SirState::Susceptible, ticks_in_state: 0 },
            AgeComponent(0),
            PheromoneSensitivity([0.0, 1.0, 0.0]),
        ));
        run_brood_system(&mut world, &mut grid, &rng, 0, 0.0);
        for (_, s) in world.query::<&BroodStageComponent>().iter() {
            if let BroodStage::Larva { health_virtual } = s.0 {
                assert!(
                    (health_virtual - 1.0).abs() < 1e-5,
                    "con Nurse adyacente health_virtual no debe caer, obtenido {health_virtual}"
                );
            }
        }
    }

    #[test]
    fn larva_dies_when_health_zero() {
        let mut world = hecs::World::new();
        let mut grid = SpatialGrid::new();
        let rng = RngSystem::new(42);
        world.spawn((
            PositionComponent(50, 50),
            AgeComponent(40),
            BroodStageComponent(BroodStage::Larva { health_virtual: 0.0 }),
        ));
        run_brood_system(&mut world, &mut grid, &rng, 0, 0.0);
        let count = world.query::<&BroodStageComponent>().iter().count();
        assert_eq!(count, 0, "larva con health_virtual=0 debe despawnearse");
    }

    #[test]
    fn larva_emits_task_pheromone() {
        let mut world = hecs::World::new();
        let mut grid = SpatialGrid::new();
        let rng = RngSystem::new(42);
        world.spawn((
            PositionComponent(50, 50),
            AgeComponent(40),
            BroodStageComponent(BroodStage::Larva { health_virtual: 1.0 }),
        ));
        run_brood_system(&mut world, &mut grid, &rng, 0, 0.0);
        grid.swap_pheromone_buffers();
        let phero = grid.pheromone(50, 50, PheromoneKind::Task);
        assert!(
            phero > 0.0,
            "larva debe emitir feromona Task en su celda, obtenido {phero}"
        );
    }

    #[test]
    fn occupancy_incremented_on_eclosion() {
        let mut world = hecs::World::new();
        let mut grid = SpatialGrid::new();
        let rng = RngSystem::new(42);
        world.spawn((
            PositionComponent(40, 40),
            AgeComponent(BROOD_PUPA_END),
            BroodStageComponent(BroodStage::Pupa),
        ));
        let before = grid.occupancy[SpatialGrid::idx(40, 40)];
        run_brood_system(&mut world, &mut grid, &rng, 0, 0.0);
        let after = grid.occupancy[SpatialGrid::idx(40, 40)];
        assert_eq!(after, before + 1, "eclosión debe incrementar occupancy en 1");
    }

    // --- M12: DiseaseSystem (SIR) -----------------------------------------------

    fn make_bee_at(x: u16, y: u16, state: SirState, age: u32) -> (
        PositionComponent,
        RoleComponent,
        EnergyComponent,
        HealthComponent,
        AgeComponent,
        PheromoneSensitivity,
    ) {
        (
            PositionComponent(x, y),
            RoleComponent(Role::Nurse),
            EnergyComponent(0.8),
            HealthComponent { state, ticks_in_state: 0 },
            AgeComponent(age),
            PheromoneSensitivity([0.0, 1.0, 0.0]),
        )
    }

    #[test]
    fn susceptible_infected_near_infected() {
        // base_rate=1.0, 1 Infected adyacente → P = 1.0 × 1 × 1.0 = 1.0 → siempre se infecta.
        let mut world = hecs::World::new();
        let rng = RngSystem::new(42);
        let s_entity = world.spawn(make_bee_at(50, 50, SirState::Susceptible, 200));
        world.spawn(make_bee_at(51, 50, SirState::Infected, 200));

        run_disease_system(&mut world, &rng, 0, 1.0, true);

        let mut q = world.query_one::<&HealthComponent>(s_entity).unwrap();
        let h = q.get().unwrap();
        assert_eq!(h.state, SirState::Infected, "Susceptible adyacente a Infected debe infectarse con base_rate=1.0");
    }

    #[test]
    fn susceptible_far_from_infected_stays_s() {
        let mut world = hecs::World::new();
        let rng = RngSystem::new(42);
        let s_entity = world.spawn(make_bee_at(10, 10, SirState::Susceptible, 200));
        world.spawn(make_bee_at(80, 80, SirState::Infected, 200));

        run_disease_system(&mut world, &rng, 0, 1.0, true);

        let mut q = world.query_one::<&HealthComponent>(s_entity).unwrap();
        let h = q.get().unwrap();
        assert_eq!(h.state, SirState::Susceptible, "Susceptible lejos de Infected debe mantenerse S");
    }

    #[test]
    fn disease_disabled_no_transmission() {
        let mut world = hecs::World::new();
        let rng = RngSystem::new(42);
        let s_entity = world.spawn(make_bee_at(50, 50, SirState::Susceptible, 200));
        world.spawn(make_bee_at(51, 50, SirState::Infected, 200));

        run_disease_system(&mut world, &rng, 0, 1.0, false); // desactivado

        let mut q = world.query_one::<&HealthComponent>(s_entity).unwrap();
        let h = q.get().unwrap();
        assert_eq!(h.state, SirState::Susceptible, "con disease_enabled=false no debe haber transmisión");
    }

    #[test]
    fn infected_recovers_after_200_ticks() {
        let mut world = hecs::World::new();
        let rng = RngSystem::new(42);
        let entity = world.spawn((
            PositionComponent(50, 50),
            RoleComponent(Role::Nurse),
            EnergyComponent(0.8),
            HealthComponent { state: SirState::Infected, ticks_in_state: DISEASE_INFECTION_DURATION },
            AgeComponent(200),
            PheromoneSensitivity([0.0, 1.0, 0.0]),
        ));

        run_disease_system(&mut world, &rng, 0, 0.0, true);

        let mut q = world.query_one::<&HealthComponent>(entity).unwrap();
        let h = q.get().unwrap();
        assert_eq!(h.state, SirState::Recovered, "Infected con timer=200 debe pasar a Recovered");
        // La Fase 4 (advance timers) corre en la misma llamada, por lo que el timer parte en 1.
        assert_eq!(h.ticks_in_state, 1, "timer debe ser 1 tras transición + avance en mismo tick");
    }

    #[test]
    fn recovered_returns_to_susceptible_at_300() {
        let mut world = hecs::World::new();
        let rng = RngSystem::new(42);
        let entity = world.spawn((
            PositionComponent(50, 50),
            RoleComponent(Role::Nurse),
            EnergyComponent(0.8),
            HealthComponent { state: SirState::Recovered, ticks_in_state: DISEASE_RECOVERY_DURATION },
            AgeComponent(200),
            PheromoneSensitivity([0.0, 1.0, 0.0]),
        ));

        run_disease_system(&mut world, &rng, 0, 0.0, true);

        let mut q = world.query_one::<&HealthComponent>(entity).unwrap();
        let h = q.get().unwrap();
        assert_eq!(h.state, SirState::Susceptible, "Recovered con timer=300 debe volver a Susceptible");
    }

    #[test]
    fn timer_advances_per_tick_for_infected() {
        let mut world = hecs::World::new();
        let rng = RngSystem::new(42);
        let entity = world.spawn((
            PositionComponent(50, 50),
            RoleComponent(Role::Nurse),
            EnergyComponent(0.8),
            HealthComponent { state: SirState::Infected, ticks_in_state: 5 },
            AgeComponent(200),
            PheromoneSensitivity([0.0, 1.0, 0.0]),
        ));

        run_disease_system(&mut world, &rng, 0, 0.0, true);

        let mut q = world.query_one::<&HealthComponent>(entity).unwrap();
        let h = q.get().unwrap();
        assert_eq!(h.ticks_in_state, 6, "timer debe avanzar 1 por tick para Infected");
    }

    #[test]
    fn susceptible_timer_does_not_advance() {
        let mut world = hecs::World::new();
        let rng = RngSystem::new(42);
        let entity = world.spawn((
            PositionComponent(50, 50),
            RoleComponent(Role::Nurse),
            EnergyComponent(0.8),
            HealthComponent { state: SirState::Susceptible, ticks_in_state: 0 },
            AgeComponent(200),
            PheromoneSensitivity([0.0, 1.0, 0.0]),
        ));

        run_disease_system(&mut world, &rng, 0, 0.0, true);

        let mut q = world.query_one::<&HealthComponent>(entity).unwrap();
        let h = q.get().unwrap();
        assert_eq!(h.ticks_in_state, 0, "timer no debe avanzar para Susceptible");
    }

    #[test]
    fn young_bee_lower_susceptibility() {
        // Con age < 100, susceptibility = 0.8 → con contact=1, base_rate=0.9: P = 0.9×1×0.8 = 0.72 < 1.
        // Con age >= 100, susceptibility = 1.0 → P = 0.9×1×1.0 = 0.9.
        // Verificamos que la proporción de infecciones es menor para abejas jóvenes.
        let rng = RngSystem::new(42);
        let mut young_infected = 0u32;
        let mut old_infected   = 0u32;
        let trials = 500u64;

        for t in 0..trials {
            let mut world = hecs::World::new();
            // Joven susceptible (age=50)
            let young = world.spawn((
                PositionComponent(50, 50),
                RoleComponent(Role::Nurse),
                EnergyComponent(0.8),
                HealthComponent { state: SirState::Susceptible, ticks_in_state: 0 },
                AgeComponent(50),
                PheromoneSensitivity([0.0, 1.0, 0.0]),
            ));
            // Adulto susceptible (age=300)
            let old = world.spawn((
                PositionComponent(52, 50),
                RoleComponent(Role::Nurse),
                EnergyComponent(0.8),
                HealthComponent { state: SirState::Susceptible, ticks_in_state: 0 },
                AgeComponent(300),
                PheromoneSensitivity([0.0, 1.0, 0.0]),
            ));
            // Infectado adyacente a ambos
            world.spawn((
                PositionComponent(51, 50),
                RoleComponent(Role::Nurse),
                EnergyComponent(0.8),
                HealthComponent { state: SirState::Infected, ticks_in_state: 0 },
                AgeComponent(200),
                PheromoneSensitivity([0.0, 1.0, 0.0]),
            ));
            run_disease_system(&mut world, &rng, t, 0.9, true);
            if world.query_one::<&HealthComponent>(young).unwrap().get().unwrap().state == SirState::Infected { young_infected += 1; }
            if world.query_one::<&HealthComponent>(old).unwrap().get().unwrap().state == SirState::Infected { old_infected += 1; }
        }

        assert!(
            young_infected < old_infected,
            "abejas jóvenes (age<100) deben infectarse menos que adultas: jóvenes={young_infected}, adultas={old_infected}"
        );
    }

    #[test]
    fn infected_energy_cost_higher() {
        let mut world = hecs::World::new();
        let infected = world.spawn((
            EnergyComponent(1.0),
            RoleComponent(Role::Nurse),
            AgeComponent(0),
            HealthComponent { state: SirState::Infected, ticks_in_state: 0 },
        ));
        let susceptible = world.spawn((
            EnergyComponent(1.0),
            RoleComponent(Role::Nurse),
            AgeComponent(0),
            HealthComponent { state: SirState::Susceptible, ticks_in_state: 0 },
        ));

        run_energy_system(&mut world, 20.0, 0.0);

        let ei = world.query_one::<&EnergyComponent>(infected).unwrap().get().unwrap().0;
        let es = world.query_one::<&EnergyComponent>(susceptible).unwrap().get().unwrap().0;

        // Infected: 1.0 - 0.02 × 1.20 = 1.0 - 0.024 = 0.976
        assert!((ei - (1.0 - 0.024_f32)).abs() < 1e-5, "Infected debe perder 0.024/tick, obtenido {ei}");
        // Susceptible: 1.0 - 0.02 × 1.0 = 0.98
        assert!((es - 0.98_f32).abs() < 1e-5, "Susceptible debe perder 0.020/tick, obtenido {es}");
        assert!(ei < es, "Infected debe perder más energía que Susceptible");
    }

    #[test]
    fn infected_forager_gains_less() {
        let mut world = hecs::World::new();
        let mut grid = SpatialGrid::new();
        let x = 30usize;
        let y = 30usize;
        grid.resource_amount[SpatialGrid::idx(x, y)] = 1.0;

        let infected = world.spawn((
            PositionComponent(x as u16, y as u16),
            RoleComponent(Role::Forager),
            EnergyComponent(0.0),
            AgeComponent(0),
            ForagerStateComponent(ForagerPhase::Searching),
            HealthComponent { state: SirState::Infected, ticks_in_state: 0 },
        ));

        let mut honey = 0.0_f32;
        let mut collected = 0.0_f32;
        run_foraging_system(&mut world, &mut grid, &mut honey, &mut collected);

        let gained = world.query_one::<&EnergyComponent>(infected).unwrap().get().unwrap().0;
        // Sin enfermedad: gain = min(1.0×2.0, 1.0) = 1.0 (energía completa)
        // Con efficiency=0.70: gain = min(1.0×2.0×0.70, 1.0) = min(1.4, 1.0) = 1.0
        // Ambos llegan a 1.0 (clampeado). Verificar con recurso menor para ver diferencia.
        assert!(gained > 0.0, "Infected Forager debe ganar algo de energía: {gained}");
        // Con recurso = 0.4 la diferencia sería visible; este test verifica que el sistema no crashea.
    }

    #[test]
    fn infected_forager_gains_less_than_susceptible() {
        // Recurso bajo (0.4) para que el clamp no oculte la diferencia.
        let rng = RngSystem::new(42);
        let x = 30usize;
        let y = 30usize;

        let run_forager = |state: SirState| -> f32 {
            let mut world = hecs::World::new();
            let mut grid = SpatialGrid::new();
            grid.resource_amount[SpatialGrid::idx(x, y)] = 0.4;
            let entity = world.spawn((
                PositionComponent(x as u16, y as u16),
                RoleComponent(Role::Forager),
                EnergyComponent(0.0),
                AgeComponent(0),
                ForagerStateComponent(ForagerPhase::Searching),
                HealthComponent { state, ticks_in_state: 0 },
            ));
            let mut honey = 0.0_f32;
            let mut collected = 0.0_f32;
            run_foraging_system(&mut world, &mut grid, &mut honey, &mut collected);
            let _ = rng; // suppress unused
            let energy = {
                let mut q = world.query_one::<&EnergyComponent>(entity).unwrap();
                q.get().unwrap().0
            };
            energy
        };

        let gain_s = run_forager(SirState::Susceptible);
        let gain_i = run_forager(SirState::Infected);

        assert!(
            gain_i < gain_s,
            "Infected Forager debe ganar menos energía: Susceptible={gain_s}, Infected={gain_i}"
        );
    }

    #[test]
    fn health_factor_not_regressed() {
        assert!((health_factor(SirState::Susceptible) - 1.0).abs() < 1e-6);
        assert!((health_factor(SirState::Infected)    - 0.5).abs() < 1e-6);
        assert!((health_factor(SirState::Recovered)   - 0.9).abs() < 1e-6);
    }

    #[test]
    fn returning_forager_moves_toward_hive() {
        let mut world = hecs::World::new();
        let mut grid = SpatialGrid::new();
        let rng_system = RngSystem::new(42);

        // Forager lejos de la colmena en estado Returning
        let start_x = 20u16;
        let start_y = 20u16;
        let idx = SpatialGrid::idx(start_x as usize, start_y as usize);
        grid.occupancy[idx] = 1;

        let entity = world.spawn((
            PositionComponent(start_x, start_y),
            RoleComponent(Role::Forager),
            EnergyComponent(0.8),
            AgeComponent(0),
            PheromoneSensitivity([0.0, 0.0, 1.0]),
            ForagerStateComponent(ForagerPhase::Returning { carry: 0.3 }),
        ));

        // Ejecutar varios ticks de movimiento
        for t in 0..20u64 {
            run_movement_system(&mut world, &mut grid, &rng_system, t);
        }

        let mut q = world.query_one::<&PositionComponent>(entity).unwrap();
        let pos = q.get().unwrap();

        let initial_dist = manhattan(start_x as i32, start_y as i32, HIVE_X as i32, HIVE_Y as i32);
        let final_dist = manhattan(pos.0 as i32, pos.1 as i32, HIVE_X as i32, HIVE_Y as i32);
        assert!(
            final_dist < initial_dist,
            "Forager Returning debe acercarse a la colmena: dist inicial={}, final={}",
            initial_dist, final_dist
        );
    }

    // --- M13: PredatorSystem ---------------------------------------------------

    fn make_predator(x: u16, y: u16) -> (PositionComponent, EnergyComponent, PredatorComponent) {
        (
            PositionComponent(x, y),
            EnergyComponent(1.0),
            PredatorComponent {
                attack_rate: PREDATOR_ATTACK_RATE,
                detection_radius: PREDATOR_DETECTION_RADIUS,
                energy_drain_on_hit: PREDATOR_ENERGY_DRAIN,
            },
        )
    }

    #[test]
    fn predator_attacks_bee_in_radius() {
        let mut world = hecs::World::new();
        let mut grid = SpatialGrid::new();
        let rng = RngSystem::new(42);

        // attack_rate=1.0 garantiza ataque en cada tick
        grid.occupancy[SpatialGrid::idx(50, 50)] += 1;
        world.spawn((
            PositionComponent(50, 50),
            EnergyComponent(1.0),
            PredatorComponent { attack_rate: 1.0, detection_radius: 2, energy_drain_on_hit: PREDATOR_ENERGY_DRAIN },
        ));

        grid.occupancy[SpatialGrid::idx(51, 50)] += 1;
        let bee = world.spawn((
            PositionComponent(51, 50),
            EnergyComponent(0.8),
            RoleComponent(Role::Nurse),
            AgeComponent(0),
        ));

        run_predator_system(&mut world, &mut grid, &rng, 0);

        // 0.8 - 0.4 = 0.4 > 0: abeja sobrevive con energía reducida
        let mut q = world.query_one::<&EnergyComponent>(bee).unwrap();
        let e = q.get().unwrap().0;
        assert!(
            (e - 0.4).abs() < 1e-5,
            "energía debe ser 0.4 tras ataque con drain=0.4, obtenida {e}"
        );
    }

    #[test]
    fn predator_no_attack_out_of_radius() {
        let mut world = hecs::World::new();
        let mut grid = SpatialGrid::new();
        let rng = RngSystem::new(42);

        grid.occupancy[SpatialGrid::idx(50, 50)] += 1;
        world.spawn(make_predator(50, 50));

        grid.occupancy[SpatialGrid::idx(60, 60)] += 1;
        let bee = world.spawn((
            PositionComponent(60, 60),
            EnergyComponent(0.8),
            RoleComponent(Role::Nurse),
            AgeComponent(0),
        ));

        run_predator_system(&mut world, &mut grid, &rng, 0);

        let mut q = world.query_one::<&EnergyComponent>(bee).unwrap();
        let e = q.get().unwrap().0;
        assert!(
            (e - 0.8).abs() < 1e-6,
            "abeja fuera de radio no debe ser atacada, energía={e}"
        );
    }

    #[test]
    fn predator_zero_attack_rate_no_drain() {
        let mut world = hecs::World::new();
        let mut grid = SpatialGrid::new();
        let rng = RngSystem::new(42);

        grid.occupancy[SpatialGrid::idx(50, 50)] += 1;
        world.spawn((
            PositionComponent(50, 50),
            EnergyComponent(1.0),
            PredatorComponent { attack_rate: 0.0, detection_radius: 2, energy_drain_on_hit: 0.4 },
        ));

        grid.occupancy[SpatialGrid::idx(51, 50)] += 1;
        let bee = world.spawn((
            PositionComponent(51, 50),
            EnergyComponent(0.8),
            RoleComponent(Role::Nurse),
            AgeComponent(0),
        ));

        run_predator_system(&mut world, &mut grid, &rng, 0);

        let mut q = world.query_one::<&EnergyComponent>(bee).unwrap();
        let e = q.get().unwrap().0;
        assert!(
            (e - 0.8).abs() < 1e-6,
            "con attack_rate=0.0 la abeja no debe perder energía, obtenida {e}"
        );
    }

    #[test]
    fn guard_emits_alarm_near_predator() {
        let mut world = hecs::World::new();
        let mut grid = SpatialGrid::new();
        let rng = RngSystem::new(42);

        grid.occupancy[SpatialGrid::idx(50, 50)] += 1;
        world.spawn(make_predator(50, 50));

        grid.occupancy[SpatialGrid::idx(51, 50)] += 1;
        world.spawn((
            PositionComponent(51, 50),
            RoleComponent(Role::Guard),
            EnergyComponent(0.8),
            AgeComponent(0),
        ));

        run_predator_system(&mut world, &mut grid, &rng, 0);
        // La feromona se escribió al write buffer; swap para leer con pheromone()
        grid.swap_pheromone_buffers();

        let alarm = grid.pheromone(51, 50, PheromoneKind::Alarm);
        assert!(alarm > 0.0, "Guard junto a depredador debe emitir alarma, obtenido {alarm}");
    }

    #[test]
    fn non_guard_no_alarm_emission() {
        let mut world = hecs::World::new();
        let mut grid = SpatialGrid::new();
        let rng = RngSystem::new(42);

        grid.occupancy[SpatialGrid::idx(50, 50)] += 1;
        world.spawn(make_predator(50, 50));

        grid.occupancy[SpatialGrid::idx(51, 50)] += 1;
        world.spawn((
            PositionComponent(51, 50),
            RoleComponent(Role::Nurse),
            EnergyComponent(0.8),
            AgeComponent(0),
        ));

        run_predator_system(&mut world, &mut grid, &rng, 0);

        let alarm = grid.pheromone(51, 50, PheromoneKind::Alarm);
        assert!(
            alarm < 1e-6,
            "Nurse no debe emitir alarma aunque haya depredador, obtenido {alarm}"
        );
    }

    #[test]
    fn predator_kills_bee_low_energy() {
        let mut world = hecs::World::new();
        let mut grid = SpatialGrid::new();
        let rng = RngSystem::new(42);

        // attack_rate=1.0 garantiza ataque; energy 0.3 < drain 0.4 → muerte inmediata
        grid.occupancy[SpatialGrid::idx(50, 50)] += 2; // predador + abeja en misma celda
        world.spawn((
            PositionComponent(50, 50),
            EnergyComponent(1.0),
            PredatorComponent { attack_rate: 1.0, detection_radius: 2, energy_drain_on_hit: 0.4 },
        ));

        let bee = world.spawn((
            PositionComponent(50, 50),
            EnergyComponent(0.3),
            RoleComponent(Role::Nurse),
            AgeComponent(0),
        ));

        let deaths = run_predator_system(&mut world, &mut grid, &rng, 0);

        assert_eq!(deaths, 1, "debe retornar 1 muerte por depredación");
        // hecs retorna Err(NoSuchEntity) para entidades despawneadas
        assert!(
            world.query_one::<&EnergyComponent>(bee).is_err(),
            "abeja con energy 0.3 atacada con drain 0.4 debe ser despawneada"
        );
    }

    #[test]
    fn predator_follows_attraction_pheromone() {
        let mut world = hecs::World::new();
        let mut grid = SpatialGrid::new();
        let rng = RngSystem::new(42);

        // add_pheromone escribe al write buffer; swap lo mueve al read buffer para que
        // pheromone() lo vea durante el movimiento del depredador.
        grid.add_pheromone(52, 50, PheromoneKind::Attraction, 1.0);
        grid.swap_pheromone_buffers();

        grid.occupancy[SpatialGrid::idx(51, 50)] += 1;
        world.spawn(make_predator(51, 50));

        run_predator_system(&mut world, &mut grid, &rng, 0);

        let pos = world
            .query::<(&PositionComponent, &PredatorComponent)>()
            .iter()
            .next()
            .map(|(_, (p, _))| (p.0, p.1))
            .unwrap();

        assert_eq!(
            pos,
            (52, 50),
            "depredador debe moverse hacia feromona Attraction máxima en (52,50)"
        );
    }

    #[test]
    fn predator_random_walk_no_pheromone() {
        let mut world = hecs::World::new();
        let mut grid = SpatialGrid::new();
        let rng = RngSystem::new(42);

        grid.occupancy[SpatialGrid::idx(50, 50)] += 1;
        world.spawn(make_predator(50, 50));

        run_predator_system(&mut world, &mut grid, &rng, 0);

        let pos = world
            .query::<(&PositionComponent, &PredatorComponent)>()
            .iter()
            .next()
            .map(|(_, (p, _))| (p.0, p.1))
            .unwrap();

        assert!(
            pos != (50, 50),
            "depredador sin feromonas debe moverse desde (50,50)"
        );
    }

    #[test]
    fn no_predators_no_deaths() {
        let mut world = hecs::World::new();
        let mut grid = SpatialGrid::new();
        let rng = RngSystem::new(42);

        grid.occupancy[SpatialGrid::idx(50, 50)] += 1;
        world.spawn((
            PositionComponent(50, 50),
            EnergyComponent(0.8),
            RoleComponent(Role::Nurse),
            AgeComponent(0),
        ));

        let deaths = run_predator_system(&mut world, &mut grid, &rng, 0);
        assert_eq!(deaths, 0, "sin depredadores no debe haber muertes");
    }

    // --- M14: MortalitySystem — desglose por causa --------------------------------

    #[test]
    fn mortality_infected_counts_as_disease() {
        let mut world = hecs::World::new();
        let mut grid = SpatialGrid::new();
        let idx = SpatialGrid::idx(50, 50);
        grid.occupancy[idx] = 1;
        world.spawn((
            PositionComponent(50, 50),
            EnergyComponent(0.0),
            AgeComponent(0),
            HealthComponent { state: SirState::Infected, ticks_in_state: 0 },
        ));

        let (by_energy, by_disease) = run_mortality_system(&mut world, &mut grid);
        assert_eq!(by_disease, 1, "Infected que muere debe contar como disease");
        assert_eq!(by_energy, 0);
    }

    #[test]
    fn mortality_susceptible_counts_as_energy() {
        let mut world = hecs::World::new();
        let mut grid = SpatialGrid::new();
        let idx = SpatialGrid::idx(50, 50);
        grid.occupancy[idx] = 1;
        world.spawn((
            PositionComponent(50, 50),
            EnergyComponent(0.0),
            AgeComponent(0),
            HealthComponent { state: SirState::Susceptible, ticks_in_state: 0 },
        ));

        let (by_energy, by_disease) = run_mortality_system(&mut world, &mut grid);
        assert_eq!(by_energy, 1, "Susceptible que muere debe contar como energy");
        assert_eq!(by_disease, 0);
    }

    #[test]
    fn mortality_mixed_causes() {
        let mut world = hecs::World::new();
        let mut grid = SpatialGrid::new();
        grid.occupancy[SpatialGrid::idx(50, 50)] = 3;
        // 2 infectadas + 1 susceptible, todas con energy=0
        for _ in 0..2 {
            world.spawn((
                PositionComponent(50, 50),
                EnergyComponent(0.0),
                AgeComponent(0),
                HealthComponent { state: SirState::Infected, ticks_in_state: 0 },
            ));
        }
        world.spawn((
            PositionComponent(50, 50),
            EnergyComponent(0.0),
            AgeComponent(0),
            HealthComponent { state: SirState::Susceptible, ticks_in_state: 0 },
        ));

        let (by_energy, by_disease) = run_mortality_system(&mut world, &mut grid);
        assert_eq!(by_disease, 2, "2 infectadas deben contar como disease");
        assert_eq!(by_energy, 1, "1 susceptible debe contar como energy");
    }
}
