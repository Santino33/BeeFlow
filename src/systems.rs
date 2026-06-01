use rand::Rng;

use crate::components::{
    AgeComponent, EnergyComponent, ForagerPhase, ForagerStateComponent, PheromoneSensitivity,
    PositionComponent, Role, RoleComponent,
};
use crate::grid::{PheromoneKind, SpatialGrid, HIVE_X, HIVE_Y};
use crate::rng::RngSystem;

/// Costo metabólico basal por tick (simulation_spec.md §Energía).
pub const METABOLIC_COST_BASAL: f32 = 0.02;

/// Máxima carga de néctar que puede llevar un Forager (simulation_spec.md §Forrajeo).
pub const FORAGER_CARRY_MAX: f32 = 0.5;

/// Radio Manhattan al que un Forager deposita su carga en la colmena.
pub const HIVE_DEPOSIT_RADIUS: u32 = 3;

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
        // Reina no se mueve
        if role.map(|r| r.0 == Role::Queen).unwrap_or(false) {
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

/// Aplica el costo metabólico a todas las abejas, modulado por temperatura y rol.
/// Drone: 0.025/tick (simulation_spec.md §Roles). Resto: METABOLIC_COST_BASAL.
/// La energía se clampea a 0.0; la muerte la gestiona MortalitySystem (M5).
pub fn run_energy_system(world: &mut hecs::World, global_temp: f32) {
    let temp_factor = 1.0 + 0.01 * (global_temp - 20.0);
    for (_, (energy, role)) in world.query_mut::<(&mut EnergyComponent, Option<&RoleComponent>)>() {
        let base_cost = match role.map(|r| r.0) {
            Some(Role::Drone) => 0.025,
            _                 => METABOLIC_COST_BASAL,
        };
        energy.0 = (energy.0 - base_cost * temp_factor).max(0.0);
    }
}

/// Elimina entidades con energía ≤ 0. Decrementa ocupación del grid.
/// Devuelve el número de muertes (causa: energía) para las métricas.
pub fn run_mortality_system(world: &mut hecs::World, grid: &mut SpatialGrid) -> u32 {
    let dead: Vec<(hecs::Entity, PositionComponent)> = world
        .query::<(&EnergyComponent, &PositionComponent)>()
        .iter()
        .filter_map(|(e, (energy, pos))| {
            if energy.0 <= 0.0 { Some((e, *pos)) } else { None }
        })
        .collect();

    let count = dead.len() as u32;
    for (entity, pos) in dead {
        let idx = SpatialGrid::idx(pos.0 as usize, pos.1 as usize);
        debug_assert!(grid.occupancy[idx] > 0, "occupancy underflow en muerte ({},{})", pos.0, pos.1);
        grid.occupancy[idx] -= 1;
        let _ = world.despawn(entity);
    }
    count
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
    for (_, (pos, role, energy, state)) in world.query_mut::<(
        &PositionComponent,
        &RoleComponent,
        &mut EnergyComponent,
        &mut ForagerStateComponent,
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
                    // Ganancia personal (spec §Energía: resource × 2.0, clampeada a 1.0)
                    let gain = (res * 2.0).min(1.0 - energy.0);
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
            HealthComponent(SirState::Susceptible),
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
        assert_eq!(health.0, SirState::Susceptible);
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
            run_energy_system(&mut world, 20.0);
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

        run_energy_system(&mut world, 20.0);

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

        run_energy_system(&mut world, 20.0);

        for (_, energy) in world.query::<&EnergyComponent>().iter() {
            assert!(energy.0 >= 0.0, "energy no debe ser negativa");
        }
    }

    #[test]
    fn energy_reaches_zero_at_tick_50() {
        let mut world = hecs::World::new();
        world.spawn((EnergyComponent(1.0), AgeComponent(0)));

        for _ in 0..50 {
            run_energy_system(&mut world, 20.0);
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

        run_energy_system(&mut world, 30.0); // factor = 1 + 0.01*10 = 1.1 → cost = 0.022

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

        run_energy_system(&mut world, 20.0);

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
}
