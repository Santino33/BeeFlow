use beeflow::config::parse_args;
use beeflow::orchestrator::Orchestrator;
use tracing::info;

fn main() {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_level(true)
        .init();

    let config = parse_args();
    let no_gui = std::env::args().any(|a| a == "--no-gui");

    info!(
        seed = config.seed,
        max_ticks = ?config.max_ticks,
        speed = config.simulation_speed,
        no_gui,
        "BeeFlow iniciando"
    );

    if no_gui {
        let mut orchestrator = Orchestrator::new(config);
        orchestrator.run();
        info!(tick = orchestrator.current_tick(), "BeeFlow finalizado");
        return;
    }

    #[cfg(feature = "visualizer")]
    {
        use eframe::egui;
        use std::sync::mpsc;
        use std::time::Duration;

        let tick_duration = Duration::from_secs_f64(1.0 / config.simulation_speed as f64);
        let (tx, rx) = mpsc::sync_channel(1);

        let mut orchestrator = Orchestrator::new(config);
        orchestrator.set_render_sender(tx);

        std::thread::spawn(move || {
            orchestrator.run();
        });

        let native_options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_title("BeeFlow — Visualizador")
                .with_inner_size([1200.0, 700.0]),
            ..Default::default()
        };

        eframe::run_native(
            "BeeFlow",
            native_options,
            Box::new(move |_cc| {
                Ok(Box::new(beeflow::visualizer::VisualizerApp::new(rx, tick_duration))
                    as Box<dyn eframe::App>)
            }),
        )
        .expect("Error al iniciar ventana egui");
    }

    #[cfg(not(feature = "visualizer"))]
    {
        // Feature visualizer no activa — correr en modo headless
        let mut orchestrator = Orchestrator::new(config);
        orchestrator.run();
        info!(tick = orchestrator.current_tick(), "BeeFlow finalizado");
    }
}
