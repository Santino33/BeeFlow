/// Parámetros de configuración de un run. Coincide con simulation_spec.md §RunConfig.
/// `max_ticks` es una extensión práctica para tests y benchmarks (no está en la spec).
#[derive(Debug, Clone)]
pub struct RunConfig {
    pub seed: u64,
    pub initial_population: u32,
    pub initial_honey_reserve: f32,
    pub disease_enabled: bool,
    pub disease_base_rate: f32,
    pub pesticide_pressure: f32,
    pub predator_count: u8,
    pub season_start: f32,
    /// Ticks/segundo lógicos (controla la velocidad del loop).
    pub simulation_speed: u32,
    /// None = loop infinito hasta Ctrl-C.
    pub max_ticks: Option<u64>,
}

impl Default for RunConfig {
    fn default() -> Self {
        Self {
            seed: 42,
            initial_population: 500,
            initial_honey_reserve: 200.0,
            disease_enabled: false,
            disease_base_rate: 0.05,
            pesticide_pressure: 0.0,
            predator_count: 0,
            season_start: 0.5,
            simulation_speed: 30,
            max_ticks: None,
        }
    }
}

/// Parsea --seed N y --max-ticks N desde los argumentos de línea de comandos.
/// Ignora argumentos desconocidos silenciosamente.
pub fn parse_args() -> RunConfig {
    let mut config = RunConfig::default();
    let args: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--seed" => {
                if let Some(val) = args.get(i + 1) {
                    if let Ok(n) = val.parse::<u64>() {
                        config.seed = n;
                    }
                    i += 1;
                }
            }
            "--max-ticks" => {
                if let Some(val) = args.get(i + 1) {
                    if let Ok(n) = val.parse::<u64>() {
                        config.max_ticks = Some(n);
                    }
                    i += 1;
                }
            }
            "--speed" => {
                if let Some(val) = args.get(i + 1) {
                    if let Ok(n) = val.parse::<u32>() {
                        config.simulation_speed = n.max(1);
                    }
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    config
}
