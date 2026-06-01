use beeflow::config::RunConfig;
use beeflow::orchestrator::Orchestrator;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use tempfile::TempDir;

fn bench_tick_once(c: &mut Criterion) {
    let dir = TempDir::new().unwrap();
    let config = RunConfig {
        max_ticks: None,
        ..Default::default()
    };
    let mut orch = Orchestrator::new_with_output(config, dir.path());

    c.bench_function("tick_once_empty", |b| {
        b.iter(|| {
            black_box(orch.tick_once());
        })
    });
}

criterion_group!(benches, bench_tick_once);
criterion_main!(benches);
