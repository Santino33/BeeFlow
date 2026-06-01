use rand::Rng;
use rand_xoshiro::Xoshiro256StarStar;

use crate::components::{AgeComponent, PositionComponent};
use crate::grid::SpatialGrid;

/// Paseo aleatorio: cada abeja se mueve a uno de sus 8 vecinos no-obstacle.
/// Usa `rng` para elegir dirección (derivado de tick en el Orchestrator).
pub fn run_movement_system(
    world: &mut hecs::World,
    grid: &mut SpatialGrid,
    rng: &mut Xoshiro256StarStar,
) {
    for (_, pos) in world.query_mut::<&mut PositionComponent>() {
        let x = pos.0 as usize;
        let y = pos.1 as usize;

        // neighbors_8 devuelve ArrayVec por valor; el borrow de grid se libera
        // antes de las escrituras a grid.occupancy que vienen después
        let valid: arrayvec::ArrayVec<(usize, usize), 8> = grid
            .neighbors_8(x, y)
            .into_iter()
            .filter(|&(nx, ny)| !grid.is_obstacle[SpatialGrid::idx(nx, ny)])
            .collect();

        if valid.is_empty() {
            continue;
        }

        let (nx, ny) = valid[rng.gen_range(0..valid.len())];

        let oi = SpatialGrid::idx(x, y);
        let ni = SpatialGrid::idx(nx, ny);
        grid.occupancy[oi] = grid.occupancy[oi].saturating_sub(1);
        grid.occupancy[ni] = grid.occupancy[ni].saturating_add(1);

        pos.0 = nx as u16;
        pos.1 = ny as u16;
    }
}

/// Incrementa la edad de todas las abejas en 1 tick.
pub fn run_age_system(world: &mut hecs::World) {
    for (_, age) in world.query_mut::<&mut AgeComponent>() {
        age.0 += 1;
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use rand_xoshiro::rand_core::SeedableRng;

    use super::*;
    use crate::components::*;
    use crate::grid::SpatialGrid;

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
        let mut rng = Xoshiro256StarStar::seed_from_u64(42);

        let entity = world.spawn(make_bee(50, 50));
        grid.occupancy[SpatialGrid::idx(50, 50)] = 1;

        run_movement_system(&mut world, &mut grid, &mut rng);

        let mut q = world.query_one::<&PositionComponent>(entity).unwrap();
        let pos = q.get().unwrap();
        // La abeja tiene 8 vecinos válidos desde (50,50); siempre se mueve
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
}
