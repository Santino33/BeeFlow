use beeflow::config::RunConfig;
use beeflow::orchestrator::Orchestrator;
use criterion::{criterion_group, criterion_main, Criterion};
use tempfile::TempDir;

fn bench_movement_5k(c: &mut Criterion) {
    let dir = TempDir::new().unwrap();
    let config = RunConfig {
        initial_population: 5_000,
        max_ticks: Some(1),
        ..Default::default()
    };
    let mut orch = Orchestrator::new_with_output(config, dir.path());

    c.bench_function("movement_5k_agents", |b| b.iter(|| orch.tick_once()));
}

criterion_group!(benches, bench_movement_5k);
criterion_main!(benches);
