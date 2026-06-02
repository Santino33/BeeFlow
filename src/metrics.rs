use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::warn;

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
    pub fn maybe_export(&self, snapshot: &MetricsSnapshot) {
        if snapshot.tick % self.export_every != 0 {
            return;
        }
        let filename = format!("metrics_{:08}.json", snapshot.tick);
        let path = self.output_dir.join(&filename);

        match serde_json::to_string_pretty(snapshot) {
            Ok(json) => {
                if let Err(e) = fs::write(&path, &json) {
                    warn!("No se pudo escribir {}: {}", filename, e);
                }
            }
            Err(e) => warn!("Error serializando métricas en tick {}: {}", snapshot.tick, e),
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
            exporter.maybe_export(&snapshot);
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
        exporter.maybe_export(&snapshot);

        let content = fs::read_to_string(dir.path().join("metrics_00000001.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed["tick"], 1);
        assert!((parsed["colony_reserve"].as_f64().unwrap() - 0.75).abs() < 1e-5);
    }
}
