use beeflow::diffusion::DiffusionSystem;
use beeflow::grid::{PheromoneKind, SpatialGrid};
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_diffusion_full(c: &mut Criterion) {
    let mut grid = SpatialGrid::new();
    let mut sys = DiffusionSystem::new();
    // Sembrar feromona para que la difusión no sea trivial
    grid.add_pheromone(50, 50, PheromoneKind::Alarm, 1.0);
    grid.add_pheromone(30, 40, PheromoneKind::Task, 0.8);
    grid.add_pheromone(70, 60, PheromoneKind::Attraction, 0.9);
    grid.swap_pheromone_buffers();

    c.bench_function("diffusion_full_grid_3ch", |b| {
        b.iter(|| sys.step(black_box(&mut grid)))
    });
}

criterion_group!(benches, bench_diffusion_full);
criterion_main!(benches);
