use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::warn;

use crate::grid::{PheromoneKind, SpatialGrid};

/// Distribución de población por rol. Coincide con simulation_spec.md §Roles.
#[derive(Debug, Default, Clone, Serialize)]
pub struct RoleDistribution {
    pub queen: u32,
    pub nurse: u32,
    pub builder: u32,
    pub guard: u32,
    pub forager: u32,
    pub drone: u32,
}

impl RoleDistribution {
    pub fn total(&self) -> u32 {
        self.queen + self.nurse + self.builder + self.guard + self.forager + self.drone
    }
}

/// Desglose de mortalidad por causa.
#[derive(Debug, Default, Clone, Serialize)]
pub struct MortalityBreakdown {
    pub by_energy: u32,
    pub by_disease: u32,
    pub by_predation: u32,
}

/// Prevalencia epidemiológica (fracción de la población en cada estado SIR).
#[derive(Debug, Default, Clone, Serialize)]
pub struct SirPrevalence {
    pub susceptible: f32,
    pub infected: f32,
    pub recovered: f32,
}

/// Snapshot completo de métricas de emergencia. Coincide con simulation_spec.md §Métricas.
/// En M0 todos los campos son 0/Default; los módulos posteriores los rellenan.
#[derive(Debug, Default, Clone, Serialize)]
pub struct MetricsSnapshot {
    pub tick: u64,
    pub population_by_role: RoleDistribution,
    pub colony_reserve: f32,
    pub mortality_rate: MortalityBreakdown,
    pub pheromone_entropy: f32,
    pub foraging_efficiency: f32,
    pub brood_adult_ratio: f32,
    pub sir_prevalence: SirPrevalence,
    pub time_to_collapse: Option<u64>,
    pub season_phase: f32,
    pub global_temp: f32,
    pub brood_production_rate: f32,
}

/// Exporta métricas a disco en formato JSON cada `export_every` ticks.
pub struct MetricsExporter {
    output_dir: PathBuf,
    export_every: u64,
}

impl MetricsExporter {
    pub fn new(output_dir: impl AsRef<Path>, export_every: u64) -> Self {
        Self {
            output_dir: output_dir.as_ref().to_owned(),
            export_every,
        }
    }

    /// Escribe `metrics_{tick:08}.json` si `tick % export_every == 0`.
    /// Retorna `true` si la exportación tuvo éxito (o no era turno de exportar),
    /// `false` si hubo un error de I/O o serialización. Los acumuladores del
    /// Orchestrator solo deben resetearse cuando esta función retorna `true`.
    pub fn maybe_export(&self, snapshot: &MetricsSnapshot) -> bool {
        if snapshot.tick % self.export_every != 0 {
            return true;
        }
        let filename = format!("metrics_{:08}.json", snapshot.tick);
        let path = self.output_dir.join(&filename);

        match serde_json::to_string_pretty(snapshot) {
            Ok(json) => match fs::write(&path, &json) {
                Ok(_) => true,
                Err(e) => {
                    warn!("No se pudo escribir {}: {}", filename, e);
                    false
                }
            },
            Err(e) => {
                warn!("Error serializando métricas en tick {}: {}", snapshot.tick, e);
                false
            }
        }
    }
}

/// Entropía de Shannon espacial sobre los 3 canales de feromona. simulation_spec.md §Métricas.
///
/// `H = -Σ p(x,y) log p(x,y)` donde p es la concentración normalizada por canal.
/// Promedia sobre los canales con suma > 0; retorna 0.0 si no hay feromona alguna.
/// Lee del read_buf (estado estable post-difusión): contiene el estado difundido del tick actual
/// incluyendo las emisiones del tick una vez incorporadas por `merge_emissions_into_stable`.
pub fn pheromone_entropy(grid: &SpatialGrid) -> f32 {
    let channels = [PheromoneKind::Alarm, PheromoneKind::Task, PheromoneKind::Attraction];
    let mut total_h = 0.0_f32;
    let mut active = 0u32;

    for kind in channels {
        // Leer del read_buf: estado estable actualizado con emisiones + difusión de este tick
        let slice = grid.pheromone_read_slice(kind);
        let sum: f32 = slice.iter().sum();
        if sum < 1e-9 {
            continue;
        }
        active += 1;
        let h: f32 = slice
            .iter()
            .filter(|&&v| v > 1e-9)
            .map(|&v| {
                let p = v / sum;
                -p * p.ln()
            })
            .sum();
        total_h += h;
    }

    if active == 0 { 0.0 } else { total_h / active as f32 }
}

impl MetricsExporter {
    /// Exporta snapshot en formato Parquet (para análisis masivo con pandas/polars).
    /// Solo disponible con `--features parquet-export`.
    /// La implementación completa requiere las crates `parquet` y `arrow-array` (ver Cargo.toml).
    #[cfg(feature = "parquet-export")]
    pub fn maybe_export_parquet(&self, snapshot: &MetricsSnapshot) -> bool {
        if snapshot.tick % self.export_every != 0 {
            return true;
        }
        // Pendiente: construir RecordBatch con campos aplanados y escribir con SerializedFileWriter.
        // Por ahora se escribe un placeholder vacío para verificar que el feature flag compila.
        let filename = format!("metrics_{:08}.parquet", snapshot.tick);
        let path = self.output_dir.join(&filename);
        match fs::write(&path, b"PAR1") {
            Ok(_) => true,
            Err(e) => {
                warn!("No se pudo crear {}: {}", filename, e);
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn exports_on_correct_ticks() {
        let dir = TempDir::new().unwrap();
        let exporter = MetricsExporter::new(dir.path(), 60);

        for tick in 0u64..=120 {
            let snapshot = MetricsSnapshot { tick, ..Default::default() };
            let _ = exporter.maybe_export(&snapshot);
        }

        assert!(dir.path().join("metrics_00000000.json").exists());
        assert!(dir.path().join("metrics_00000060.json").exists());
        assert!(dir.path().join("metrics_00000120.json").exists());
        assert!(!dir.path().join("metrics_00000001.json").exists());
        assert!(!dir.path().join("metrics_00000059.json").exists());
    }

    #[test]
    fn exported_json_is_valid() {
        let dir = TempDir::new().unwrap();
        let exporter = MetricsExporter::new(dir.path(), 1);
        let snapshot = MetricsSnapshot {
            tick: 1,
            colony_reserve: 0.75,
            ..Default::default()
        };
        assert!(exporter.maybe_export(&snapshot), "export de tick 1 debe tener éxito");

        let content = fs::read_to_string(dir.path().join("metrics_00000001.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed["tick"], 1);
        assert!((parsed["colony_reserve"].as_f64().unwrap() - 0.75).abs() < 1e-5);
    }

    // --- M14: pheromone_entropy --------------------------------------------

    #[test]
    fn entropy_zero_when_no_pheromone() {
        let grid = SpatialGrid::new();
        let h = pheromone_entropy(&grid);
        assert!(
            h.abs() < 1e-6,
            "grid sin feromona debe tener entropy=0.0, obtenido {h}"
        );
    }

    #[test]
    fn entropy_positive_after_emission() {
        let mut grid = SpatialGrid::new();
        // add_pheromone escribe al write buffer; pheromone_entropy lee del write buffer
        grid.add_pheromone(50, 50, crate::grid::PheromoneKind::Alarm, 0.5);
        grid.add_pheromone(51, 50, crate::grid::PheromoneKind::Alarm, 0.3);

        let h = pheromone_entropy(&grid);
        assert!(h > 0.0, "grid con feromona en 2 celdas debe tener entropy > 0, obtenido {h}");
    }

    #[test]
    fn entropy_higher_when_more_dispersed() {
        // H(uniforme) > H(concentrado); ambos en write buffer (no swap)
        let mut grid_conc = SpatialGrid::new();
        grid_conc.add_pheromone(50, 50, crate::grid::PheromoneKind::Task, 1.0);

        let mut grid_disp = SpatialGrid::new();
        for x in 10..90usize {
            grid_disp.add_pheromone(x, 50, crate::grid::PheromoneKind::Task, 0.1);
        }

        let h_conc = pheromone_entropy(&grid_conc);
        let h_disp = pheromone_entropy(&grid_disp);
        assert!(
            h_disp > h_conc,
            "feromona dispersa debe tener mayor entropía: conc={h_conc}, disp={h_disp}"
        );
    }

    #[test]
    fn export_latency_under_10ms() {
        use std::time::Instant;
        let dir = TempDir::new().unwrap();
        let exporter = MetricsExporter::new(dir.path(), 1);
        let snapshot = MetricsSnapshot {
            tick: 1,
            colony_reserve: 0.75,
            pheromone_entropy: 2.5,
            ..Default::default()
        };

        let t0 = Instant::now();
        let ok = exporter.maybe_export(&snapshot);
        let elapsed_ms = t0.elapsed().as_millis();

        assert!(ok, "export debe tener éxito");

        // En debug (sin optimizaciones) permitimos hasta 100 ms.
        // En release el límite real es 10 ms según simulation_spec.md §Métricas.
        #[cfg(debug_assertions)]
        let limit_ms: u128 = 100;
        #[cfg(not(debug_assertions))]
        let limit_ms: u128 = 10;

        assert!(
            elapsed_ms < limit_ms,
            "latencia de export debe ser < {} ms, obtenida {} ms",
            limit_ms, elapsed_ms
        );
    }
}
