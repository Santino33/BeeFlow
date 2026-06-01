use beeflow::config::parse_args;
use beeflow::orchestrator::Orchestrator;
use tracing::info;

fn main() {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_level(true)
        .init();

    let config = parse_args();

    info!(
        seed = config.seed,
        max_ticks = ?config.max_ticks,
        speed = config.simulation_speed,
        "BeeFlow iniciando"
    );

    let mut orchestrator = Orchestrator::new(config);
    orchestrator.run();

    info!(tick = orchestrator.current_tick(), "BeeFlow finalizado");
}
