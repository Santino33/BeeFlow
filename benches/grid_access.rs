use beeflow::grid::{PheromoneKind, SpatialGrid, GRID_H, GRID_W};
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rand::Rng;
use rand_xoshiro::rand_core::SeedableRng;
use rand_xoshiro::Xoshiro256StarStar;

fn bench_random_rw(c: &mut Criterion) {
    let mut g = SpatialGrid::new();
    let mut rng = Xoshiro256StarStar::seed_from_u64(42);

    // Pre-generar 10_000 coordenadas aleatorias para que el benchmark sea estable
    let coords: Vec<(usize, usize)> = (0..10_000)
        .map(|_| (rng.gen_range(0..GRID_W), rng.gen_range(0..GRID_H)))
        .collect();

    c.bench_function("grid_random_rw_10k", |b| {
        b.iter(|| {
            for &(x, y) in &coords {
                g.set_resource(x, y, black_box(0.5));
                black_box(g.resource(x, y));
            }
        })
    });
}

fn bench_full_sequential_scan(c: &mut Criterion) {
    let g = SpatialGrid::new();

    c.bench_function("grid_full_scan_resource", |b| {
        b.iter(|| {
            let mut sum = 0.0f32;
            for y in 0..GRID_H {
                for x in 0..GRID_W {
                    sum += black_box(g.resource(x, y));
                }
            }
            black_box(sum)
        })
    });
}

fn bench_pheromone_add_swap(c: &mut Criterion) {
    let mut g = SpatialGrid::new();

    c.bench_function("grid_pheromone_add_swap_100", |b| {
        b.iter(|| {
            for i in 0u32..100 {
                let x = (i % GRID_W as u32) as usize;
                let y = (i / GRID_W as u32) as usize;
                g.add_pheromone(x, y, PheromoneKind::Alarm, black_box(0.1));
            }
            g.swap_pheromone_buffers();
        })
    });
}

fn bench_neighbors_8(c: &mut Criterion) {
    let g = SpatialGrid::new();

    c.bench_function("grid_neighbors8_1000_calls", |b| {
        b.iter(|| {
            let mut total = 0usize;
            for y in (5..95).step_by(10) {
                for x in (5..95).step_by(10) {
                    total += black_box(g.neighbors_8(x, y)).len();
                }
            }
            black_box(total)
        })
    });
}

criterion_group!(
    benches,
    bench_random_rw,
    bench_full_sequential_scan,
    bench_pheromone_add_swap,
    bench_neighbors_8
);
criterion_main!(benches);
