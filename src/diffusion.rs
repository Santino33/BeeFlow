use rayon::prelude::*;

use crate::grid::{PheromoneKind, SpatialGrid, GRID_H, GRID_W, TOTAL_CELLS};

/// Tasa de decaimiento por tick: concentration *= (1 - DECAY_RATE).
/// Fuente: simulation_spec.md §Feromonas diffusion.decay_rate
pub const DECAY_RATE: f32 = 0.05;

/// Kernel Gaussiano 1D separable: [1,2,1]/4
const KERNEL: [f32; 3] = [0.25, 0.5, 0.25];

/// Sistema de difusión de feromonas.
///
/// Implementa un kernel Gaussiano 3×3 separable (paso H + paso V) con:
/// - Absorción en bordes (sin rebote ni toroide)
/// - Decaimiento exponencial 5 %/tick
/// - Paralelización por filas con rayon
pub struct DiffusionSystem {
    temp: Vec<f32>, // buffer intermedio entre el paso H y el paso V
}

impl DiffusionSystem {
    pub fn new() -> Self {
        Self { temp: vec![0.0; TOTAL_CELLS] }
    }

    /// Difunde y decae los 3 canales de feromonas, luego hace swap del double-buffer.
    pub fn step(&mut self, grid: &mut SpatialGrid) {
        for kind in [PheromoneKind::Alarm, PheromoneKind::Task, PheromoneKind::Attraction] {
            let (read, write, obstacles) = grid.channel_bufs_mut(kind);
            h_pass(read, &mut self.temp, obstacles);
            v_pass(&self.temp, write, obstacles);
        }
        grid.swap_pheromone_buffers();
    }
}

impl Default for DiffusionSystem {
    fn default() -> Self {
        Self::new()
    }
}

/// Convolución horizontal con kernel [0.25, 0.5, 0.25].
/// Normaliza por el peso total de vecinos válidos (absorción en bordes).
fn h_pass(src: &[f32], dst: &mut [f32], obstacles: &[bool]) {
    dst.par_chunks_mut(GRID_W).enumerate().for_each(|(y, row_out)| {
        for x in 0..GRID_W {
            let idx = y * GRID_W + x;
            if obstacles[idx] {
                row_out[x] = 0.0;
                continue;
            }
            let mut sum = 0.0f32;
            let mut w_total = 0.0f32;
            for (i, dx) in (-1i32..=1).enumerate() {
                let nx = x as i32 + dx;
                if nx < 0 || nx >= GRID_W as i32 {
                    continue;
                }
                let nidx = y * GRID_W + nx as usize;
                if obstacles[nidx] {
                    continue;
                }
                sum += src[nidx] * KERNEL[i];
                w_total += KERNEL[i];
            }
            row_out[x] = if w_total > 0.0 { sum / w_total } else { 0.0 };
        }
    });
}

/// Convolución vertical con kernel [0.25, 0.5, 0.25] + decaimiento.
/// Normaliza por el peso total de vecinos válidos (absorción en bordes).
fn v_pass(src: &[f32], dst: &mut [f32], obstacles: &[bool]) {
    dst.par_chunks_mut(GRID_W).enumerate().for_each(|(y, row_out)| {
        for x in 0..GRID_W {
            let idx = y * GRID_W + x;
            if obstacles[idx] {
                row_out[x] = 0.0;
                continue;
            }
            let mut sum = 0.0f32;
            let mut w_total = 0.0f32;
            for (i, dy) in (-1i32..=1).enumerate() {
                let ny = y as i32 + dy;
                if ny < 0 || ny >= GRID_H as i32 {
                    continue;
                }
                let nidx = ny as usize * GRID_W + x;
                if obstacles[nidx] {
                    continue;
                }
                sum += src[nidx] * KERNEL[i];
                w_total += KERNEL[i];
            }
            let val = if w_total > 0.0 { sum / w_total } else { 0.0 };
            row_out[x] = (val * (1.0 - DECAY_RATE)).min(1.0);
        }
    });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::{SpatialGrid, GRID_H, GRID_W};

    fn spatial_entropy(grid: &SpatialGrid, kind: PheromoneKind) -> f32 {
        let slice = grid.pheromone_read_slice(kind);
        let total: f32 = slice.iter().sum();
        if total < 1e-10 {
            return 0.0;
        }
        slice
            .iter()
            .filter(|&&v| v > 1e-10)
            .map(|&v| {
                let p = v / total;
                -p * p.ln()
            })
            .sum()
    }

    #[test]
    fn pheromone_spreads_to_neighbors() {
        let mut grid = SpatialGrid::new();
        let mut sys = DiffusionSystem::new();
        grid.add_pheromone(50, 50, PheromoneKind::Alarm, 1.0);
        grid.swap_pheromone_buffers(); // mueve emisión al read_buf
        sys.step(&mut grid);
        assert!(grid.pheromone(49, 50, PheromoneKind::Alarm) > 0.0);
        assert!(grid.pheromone(51, 50, PheromoneKind::Alarm) > 0.0);
        assert!(grid.pheromone(50, 49, PheromoneKind::Alarm) > 0.0);
        assert!(grid.pheromone(50, 51, PheromoneKind::Alarm) > 0.0);
    }

    #[test]
    fn pheromone_decays_without_source() {
        let mut grid = SpatialGrid::new();
        let mut sys = DiffusionSystem::new();
        grid.add_pheromone(50, 50, PheromoneKind::Task, 1.0);
        grid.swap_pheromone_buffers();
        for _ in 0..200 {
            sys.step(&mut grid);
        }
        let max_conc = grid
            .pheromone_read_slice(PheromoneKind::Task)
            .iter()
            .cloned()
            .fold(0.0f32, f32::max);
        assert!(
            max_conc < 0.001,
            "concentración máxima {max_conc} debería ser < 0.001 tras 200 pasos"
        );
    }

    #[test]
    fn border_cells_stay_zero() {
        let mut grid = SpatialGrid::new();
        let mut sys = DiffusionSystem::new();
        grid.add_pheromone(10, 10, PheromoneKind::Attraction, 1.0);
        grid.swap_pheromone_buffers();
        for _ in 0..50 {
            sys.step(&mut grid);
        }
        for y in 0..GRID_H {
            for x in 0..GRID_W {
                if SpatialGrid::is_border_xy(x, y) {
                    let v = grid.pheromone(x, y, PheromoneKind::Attraction);
                    assert_eq!(v, 0.0, "celda de borde ({x},{y}) = {v}, debe ser 0");
                }
            }
        }
    }

    #[test]
    fn entropy_decreases_after_source_removed() {
        // La entropía espacial normalizada crece mientras el pheromone se difunde a
        // nuevas celdas. Una vez que la concentración decae lo suficiente (<1e-10),
        // spatial_entropy devuelve 0. Verificamos que tras 300 ticks (decaimiento casi
        // total) la entropía es menor que en el pico de difusión (~paso 50).
        let mut grid = SpatialGrid::new();
        let mut sys = DiffusionSystem::new();
        grid.add_pheromone(50, 50, PheromoneKind::Alarm, 1.0);
        grid.swap_pheromone_buffers();

        // Pasos 1-50: blob en expansión → entropía en su máximo
        for _ in 0..50 {
            sys.step(&mut grid);
        }
        let peak_entropy = spatial_entropy(&grid, PheromoneKind::Alarm);

        // Pasos 51-300: decaimiento domina → concentración → 0 → entropía → 0
        for _ in 0..250 {
            sys.step(&mut grid);
        }
        let final_entropy = spatial_entropy(&grid, PheromoneKind::Alarm);
        assert!(
            final_entropy < peak_entropy,
            "entropía en paso 300 ({final_entropy}) debe ser < pico en paso 50 ({peak_entropy})"
        );
    }
}
