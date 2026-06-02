use std::f32::consts::PI;

use crate::config::RunConfig;

/// Variables de nivel global que modulan la dinámica de la colonia.
/// Actualizado cada 15 ticks desde `tick_once`. simulation_spec.md §SystemDynamics.
pub struct SystemDynamicsState {
    /// Fase del ciclo anual [0.0, 1.0); 0=invierno, 0.25=primavera, 0.5=verano, 0.75=otoño.
    /// Avanza 15/86400 por llamada a `update` (ciclo completo en ~86400 ticks).
    pub season_phase: f32,
    /// Temperatura global en °C, rango [-5.0, 45.0]. Ciclo sinusoidal con season_phase.
    pub global_temp: f32,
    /// Tasa de producción de cría [0.0, 0.1]. Máxima en verano con reservas llenas.
    pub brood_production_rate: f32,
    /// Reserva de polen [0.0, ∞). Placeholder para BroodSystem (M11).
    pub pollen_reserve: f32,
    /// Presión de pesticidas [0.0, 1.0]. Drain extra de energía para Foragers.
    pub pesticide_pressure: f32,
}

impl SystemDynamicsState {
    pub fn new(config: &RunConfig) -> Self {
        let season_phase = config.season_start;
        let global_temp = Self::temp_from_phase(season_phase);
        Self {
            season_phase,
            global_temp,
            brood_production_rate: 0.0,
            pollen_reserve: 1.0,
            pesticide_pressure: config.pesticide_pressure,
        }
    }

    /// Avanza 15 ticks de dinámica global. Debe llamarse cuando `tick % 15 == 0`.
    pub fn update(&mut self, honey_reserve: f32) {
        self.season_phase = (self.season_phase + 15.0 / 86400.0) % 1.0;
        self.global_temp = Self::temp_from_phase(self.season_phase);

        let season_factor = self.season_factor();
        let reserve_factor = (honey_reserve / 0.5).clamp(0.0, 1.0);
        self.brood_production_rate = 0.1 * season_factor * reserve_factor;
    }

    /// Factor estacional [0.0, 1.0]: sin(phase × π). Máximo (1.0) en verano (phase=0.5).
    pub fn season_factor(&self) -> f32 {
        (self.season_phase * PI).sin().max(0.0)
    }

    fn temp_from_phase(phase: f32) -> f32 {
        (20.0_f32 - 15.0 * (phase * 2.0 * PI).cos()).clamp(-5.0, 45.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_state(season_phase: f32) -> SystemDynamicsState {
        let config = RunConfig {
            season_start: season_phase,
            ..Default::default()
        };
        SystemDynamicsState::new(&config)
    }

    #[test]
    fn season_phase_advances_per_update() {
        let mut state = make_state(0.0);
        state.update(1.0);
        let expected = 15.0_f32 / 86400.0;
        assert!(
            (state.season_phase - expected).abs() < 1e-6,
            "season_phase debe avanzar {expected:.6} por update, obtenido {}",
            state.season_phase
        );
    }

    #[test]
    fn season_phase_wraps_at_1() {
        let mut state = make_state(0.0);
        let n_updates = 86400u32 / 15;
        for _ in 0..n_updates {
            state.update(1.0);
        }
        assert!(
            state.season_phase < 1.0,
            "season_phase debe estar en [0, 1), obtenido {}",
            state.season_phase
        );
        assert!(
            state.season_phase < 0.01,
            "después de un ciclo completo debe estar cerca de 0, obtenido {}",
            state.season_phase
        );
    }

    #[test]
    fn season_factor_zero_in_winter() {
        let state = make_state(0.0);
        assert!(
            state.season_factor() < 1e-5,
            "season_factor debe ser ≈0 en invierno (phase=0), obtenido {}",
            state.season_factor()
        );
    }

    #[test]
    fn season_factor_one_in_summer() {
        let state = make_state(0.5);
        assert!(
            (state.season_factor() - 1.0).abs() < 1e-5,
            "season_factor debe ser ≈1 en verano (phase=0.5), obtenido {}",
            state.season_factor()
        );
    }

    #[test]
    fn brood_rate_positive_in_summer() {
        let mut state = make_state(0.5);
        state.update(1.0); // honey_reserve = 1.0
        assert!(
            state.brood_production_rate > 0.09,
            "brood_rate debe ser ≈0.1 en verano con reservas llenas, obtenido {}",
            state.brood_production_rate
        );
    }

    #[test]
    fn temp_warmer_in_summer_than_winter() {
        let winter = make_state(0.0);
        let summer = make_state(0.5);
        assert!(
            summer.global_temp > winter.global_temp,
            "verano ({:.1}°C) debe ser más cálido que invierno ({:.1}°C)",
            summer.global_temp,
            winter.global_temp
        );
    }

    #[test]
    fn pesticide_from_config() {
        let config = RunConfig {
            pesticide_pressure: 0.7,
            ..Default::default()
        };
        let state = SystemDynamicsState::new(&config);
        assert!((state.pesticide_pressure - 0.7).abs() < 1e-6);
    }
}
