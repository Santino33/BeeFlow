use arrayvec::ArrayVec;

// ---------------------------------------------------------------------------
// Constantes del mundo (simulation_spec.md §Grid y Mundo)
// ---------------------------------------------------------------------------

pub const GRID_W: usize = 100;
pub const GRID_H: usize = 100;
pub const CHUNK_SIZE: usize = 16;
pub const CHUNKS_X: usize = 7; // ceil(100/16)
pub const CHUNKS_Y: usize = 7;
pub const BORDER: usize = 5;
pub const TOTAL_CELLS: usize = GRID_W * GRID_H;
pub const PHEROMONE_CHANNELS: usize = 3;

/// Tipos de feromona. Los índices coinciden con simulation_spec.md §Feromonas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PheromoneKind {
    Alarm = 0,
    Task = 1,
    Attraction = 2,
}

impl PheromoneKind {
    #[inline]
    pub fn index(self) -> usize {
        self as usize
    }
}

// ---------------------------------------------------------------------------
// Chunk ranges precalculados
// (x_start, x_end_exclusive, y_start, y_end_exclusive)
// ---------------------------------------------------------------------------

pub type ChunkRange = (usize, usize, usize, usize);

fn build_chunk_ranges() -> Vec<ChunkRange> {
    let mut ranges = Vec::with_capacity(CHUNKS_X * CHUNKS_Y);
    for cy in 0..CHUNKS_Y {
        for cx in 0..CHUNKS_X {
            let x0 = cx * CHUNK_SIZE;
            let y0 = cy * CHUNK_SIZE;
            let x1 = (x0 + CHUNK_SIZE).min(GRID_W);
            let y1 = (y0 + CHUNK_SIZE).min(GRID_H);
            if x0 < GRID_W && y0 < GRID_H {
                ranges.push((x0, x1, y0, y1));
            }
        }
    }
    ranges
}

// ---------------------------------------------------------------------------
// SpatialGrid — layout SOA para máxima localidad de caché
// ---------------------------------------------------------------------------

/// Grid espacial 100×100 en formato SOA.
///
/// Feromonas usan double-buffering:
///   - `read_buf` apunta al buffer de lectura actual.
///   - Los escritores (agentes, difusión) siempre escriben en el buffer opuesto.
///   - `swap_pheromone_buffers()` intercambia los roles tras cada tick de difusión.
pub struct SpatialGrid {
    // phero[canal][buf][celda]: double-buffer × 3 canales
    phero: [[Vec<f32>; 2]; PHEROMONE_CHANNELS],
    pub read_buf: usize,

    pub resource_amount: Vec<f32>, // [0.0, 1.0]
    pub occupancy: Vec<u64>,       // bitset de presencia
    pub local_temp: Vec<f32>,      // °C, default 20.0
    pub is_obstacle: Vec<bool>,

    chunk_ranges: Vec<ChunkRange>,
}

impl SpatialGrid {
    /// Crea un grid limpio. Las celdas de borde quedan marcadas como `is_obstacle`.
    pub fn new() -> Self {
        let mut grid = Self {
            phero: [
                [vec![0.0; TOTAL_CELLS], vec![0.0; TOTAL_CELLS]],
                [vec![0.0; TOTAL_CELLS], vec![0.0; TOTAL_CELLS]],
                [vec![0.0; TOTAL_CELLS], vec![0.0; TOTAL_CELLS]],
            ],
            read_buf: 0,
            resource_amount: vec![0.0; TOTAL_CELLS],
            occupancy: vec![0u64; TOTAL_CELLS],
            local_temp: vec![20.0; TOTAL_CELLS],
            is_obstacle: vec![false; TOTAL_CELLS],
            chunk_ranges: build_chunk_ranges(),
        };

        // Marcar celdas de borde
        for y in 0..GRID_H {
            for x in 0..GRID_W {
                if Self::is_border_xy(x, y) {
                    grid.is_obstacle[Self::idx(x, y)] = true;
                }
            }
        }
        grid
    }

    // -----------------------------------------------------------------------
    // Indexación
    // -----------------------------------------------------------------------

    /// Índice plano: `y * GRID_W + x`. Sin bounds check en release.
    #[inline(always)]
    pub fn idx(x: usize, y: usize) -> usize {
        y * GRID_W + x
    }

    #[inline(always)]
    pub fn in_bounds(x: usize, y: usize) -> bool {
        x < GRID_W && y < GRID_H
    }

    #[inline(always)]
    pub fn is_border_xy(x: usize, y: usize) -> bool {
        x < BORDER || x >= GRID_W - BORDER || y < BORDER || y >= GRID_H - BORDER
    }

    #[inline]
    pub fn is_border(&self, x: usize, y: usize) -> bool {
        Self::is_border_xy(x, y)
    }

    // -----------------------------------------------------------------------
    // Feromonas (double-buffer)
    // -----------------------------------------------------------------------

    /// Lee la concentración de feromona del buffer activo de lectura.
    #[inline]
    pub fn pheromone(&self, x: usize, y: usize, kind: PheromoneKind) -> f32 {
        debug_assert!(Self::in_bounds(x, y));
        self.phero[kind.index()][self.read_buf][Self::idx(x, y)]
    }

    /// Añade `amount` de feromona al buffer de escritura (opuesto al de lectura).
    /// - No-op si la celda es borde/obstacle.
    /// - Clampea a 1.0 (simulation_spec.md: `max_concentration: 1.0`).
    #[inline]
    pub fn add_pheromone(&mut self, x: usize, y: usize, kind: PheromoneKind, amount: f32) {
        if !Self::in_bounds(x, y) || self.is_obstacle[Self::idx(x, y)] {
            return;
        }
        let write_buf = 1 - self.read_buf;
        let idx = Self::idx(x, y);
        let v = &mut self.phero[kind.index()][write_buf][idx];
        *v = (*v + amount).min(1.0);
    }

    /// Intercambia los buffers de lectura y escritura. Llamado por M2 tras difusión.
    #[inline]
    pub fn swap_pheromone_buffers(&mut self) {
        self.read_buf ^= 1;
    }

    /// Acceso de solo lectura al buffer activo para un canal (usado por M2 para difusión).
    #[inline]
    pub fn pheromone_read_slice(&self, kind: PheromoneKind) -> &[f32] {
        &self.phero[kind.index()][self.read_buf]
    }

    /// Acceso mutable al buffer de escritura para un canal (usado por M2 para difusión).
    #[inline]
    pub fn pheromone_write_slice_mut(&mut self, kind: PheromoneKind) -> &mut [f32] {
        let write_buf = 1 - self.read_buf;
        &mut self.phero[kind.index()][write_buf]
    }

    /// Devuelve (read_slice, write_slice, is_obstacle) para el canal dado.
    ///
    /// # Safety
    /// `read_buf != write_buf`, por lo que `phero[ch][rb]`, `phero[ch][wb]` e
    /// `is_obstacle` son asignaciones de heap distintas sin aliasing.
    pub fn channel_bufs_mut(&mut self, kind: PheromoneKind) -> (&[f32], &mut [f32], &[bool]) {
        let ch = kind.index();
        let rb = self.read_buf;
        let wb = 1 - rb;
        let read_ptr = self.phero[ch][rb].as_ptr();
        let read_len = self.phero[ch][rb].len();
        let obs_ptr = self.is_obstacle.as_ptr();
        let obs_len = self.is_obstacle.len();
        // SAFETY: Las tres Vecs tienen asignaciones de heap independientes.
        let read = unsafe { std::slice::from_raw_parts(read_ptr, read_len) };
        let obstacles = unsafe { std::slice::from_raw_parts(obs_ptr, obs_len) };
        let write = &mut self.phero[ch][wb];
        (read, write, obstacles)
    }

    // -----------------------------------------------------------------------
    // Recursos y temperatura
    // -----------------------------------------------------------------------

    #[inline]
    pub fn resource(&self, x: usize, y: usize) -> f32 {
        debug_assert!(Self::in_bounds(x, y));
        self.resource_amount[Self::idx(x, y)]
    }

    /// Establece `resource_amount`, clampeado a [0.0, 1.0].
    #[inline]
    pub fn set_resource(&mut self, x: usize, y: usize, v: f32) {
        debug_assert!(Self::in_bounds(x, y));
        self.resource_amount[Self::idx(x, y)] = v.clamp(0.0, 1.0);
    }

    #[inline]
    pub fn temp(&self, x: usize, y: usize) -> f32 {
        debug_assert!(Self::in_bounds(x, y));
        self.local_temp[Self::idx(x, y)]
    }

    #[inline]
    pub fn set_temp(&mut self, x: usize, y: usize, v: f32) {
        debug_assert!(Self::in_bounds(x, y));
        self.local_temp[Self::idx(x, y)] = v;
    }

    // -----------------------------------------------------------------------
    // Ocupación
    // -----------------------------------------------------------------------

    #[inline]
    pub fn is_occupied(&self, x: usize, y: usize) -> bool {
        debug_assert!(Self::in_bounds(x, y));
        self.occupancy[Self::idx(x, y)] != 0
    }

    #[inline]
    pub fn set_occupancy(&mut self, x: usize, y: usize, bits: u64) {
        debug_assert!(Self::in_bounds(x, y));
        self.occupancy[Self::idx(x, y)] = bits;
    }

    // -----------------------------------------------------------------------
    // Vecindad (sin heap allocation via ArrayVec)
    // -----------------------------------------------------------------------

    /// Vecinos 4-conectados (N/S/E/W). Excluye celdas fuera de bounds.
    pub fn neighbors_4(&self, x: usize, y: usize) -> ArrayVec<(usize, usize), 4> {
        let mut out = ArrayVec::new();
        if y > 0 { out.push((x, y - 1)); }
        if y + 1 < GRID_H { out.push((x, y + 1)); }
        if x > 0 { out.push((x - 1, y)); }
        if x + 1 < GRID_W { out.push((x + 1, y)); }
        out
    }

    /// Vecinos 8-conectados. Excluye celdas fuera de bounds.
    pub fn neighbors_8(&self, x: usize, y: usize) -> ArrayVec<(usize, usize), 8> {
        let mut out = ArrayVec::new();
        let x_min = x.saturating_sub(1);
        let x_max = (x + 1).min(GRID_W - 1);
        let y_min = y.saturating_sub(1);
        let y_max = (y + 1).min(GRID_H - 1);
        for ny in y_min..=y_max {
            for nx in x_min..=x_max {
                if nx != x || ny != y {
                    out.push((nx, ny));
                }
            }
        }
        out
    }

    // -----------------------------------------------------------------------
    // Iteración por chunks (para rayon en M2/M3)
    // -----------------------------------------------------------------------

    /// Rangos de chunk precalculados: `(x_start, x_end, y_start, y_end)`.
    /// Cada rango es un bloque 16×16 (o menor en los bordes del grid).
    pub fn chunk_ranges(&self) -> &[ChunkRange] {
        &self.chunk_ranges
    }
}

impl Default for SpatialGrid {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn new_marks_border_as_obstacle() {
        let g = SpatialGrid::new();
        // Borde izquierdo
        for y in 0..GRID_H {
            for x in 0..BORDER {
                assert!(g.is_obstacle[SpatialGrid::idx(x, y)], "({x},{y}) debe ser obstacle");
            }
        }
        // Borde derecho
        for y in 0..GRID_H {
            for x in (GRID_W - BORDER)..GRID_W {
                assert!(g.is_obstacle[SpatialGrid::idx(x, y)], "({x},{y}) debe ser obstacle");
            }
        }
        // Borde superior e inferior
        for x in 0..GRID_W {
            for y in 0..BORDER {
                assert!(g.is_obstacle[SpatialGrid::idx(x, y)], "({x},{y}) debe ser obstacle");
            }
            for y in (GRID_H - BORDER)..GRID_H {
                assert!(g.is_obstacle[SpatialGrid::idx(x, y)], "({x},{y}) debe ser obstacle");
            }
        }
    }

    #[test]
    fn interior_cells_not_obstacle() {
        let g = SpatialGrid::new();
        assert!(!g.is_obstacle[SpatialGrid::idx(50, 50)]);
        assert!(!g.is_obstacle[SpatialGrid::idx(BORDER, BORDER)]);
        assert!(!g.is_obstacle[SpatialGrid::idx(GRID_W - BORDER - 1, GRID_H - BORDER - 1)]);
    }

    #[test]
    fn pheromone_add_and_read() {
        let mut g = SpatialGrid::new();
        // Escribimos en el write buffer
        g.add_pheromone(50, 50, PheromoneKind::Alarm, 0.5);
        // Antes del swap, el read_buf no refleja el cambio
        assert_eq!(g.pheromone(50, 50, PheromoneKind::Alarm), 0.0);
        // Tras el swap, ahora podemos leer
        g.swap_pheromone_buffers();
        assert!((g.pheromone(50, 50, PheromoneKind::Alarm) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn pheromone_clamps_at_1() {
        let mut g = SpatialGrid::new();
        g.add_pheromone(50, 50, PheromoneKind::Task, 0.8);
        g.add_pheromone(50, 50, PheromoneKind::Task, 0.8); // total sería 1.6
        g.swap_pheromone_buffers();
        assert!((g.pheromone(50, 50, PheromoneKind::Task) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn pheromone_border_noop() {
        let mut g = SpatialGrid::new();
        g.add_pheromone(0, 0, PheromoneKind::Attraction, 0.5);
        g.swap_pheromone_buffers();
        assert_eq!(g.pheromone(0, 0, PheromoneKind::Attraction), 0.0);
    }

    #[test]
    fn swap_alternates_buffers() {
        let mut g = SpatialGrid::new();
        let original = g.read_buf;
        g.swap_pheromone_buffers();
        assert_ne!(g.read_buf, original);
        g.swap_pheromone_buffers();
        assert_eq!(g.read_buf, original);
    }

    #[test]
    fn neighbors_4_interior() {
        let g = SpatialGrid::new();
        let n = g.neighbors_4(50, 50);
        assert_eq!(n.len(), 4);
    }

    #[test]
    fn neighbors_4_corner_0_0() {
        let g = SpatialGrid::new();
        // (0,0) tiene solo vecinos (1,0) y (0,1) en bounds
        let n = g.neighbors_4(0, 0);
        assert_eq!(n.len(), 2);
    }

    #[test]
    fn neighbors_4_valid_border_cell() {
        let g = SpatialGrid::new();
        // (5,5) es la primera celda interior; tiene 4 vecinos en bounds
        let n = g.neighbors_4(BORDER, BORDER);
        assert_eq!(n.len(), 4);
    }

    #[test]
    fn neighbors_8_interior() {
        let g = SpatialGrid::new();
        assert_eq!(g.neighbors_8(50, 50).len(), 8);
    }

    #[test]
    fn neighbors_8_corner() {
        let g = SpatialGrid::new();
        // Esquina (0,0): solo 3 vecinos
        assert_eq!(g.neighbors_8(0, 0).len(), 3);
        // Borde superior (50, 0): 5 vecinos
        assert_eq!(g.neighbors_8(50, 0).len(), 5);
    }

    #[test]
    fn resource_clamps() {
        let mut g = SpatialGrid::new();
        g.set_resource(50, 50, 1.5);
        assert!((g.resource(50, 50) - 1.0).abs() < 1e-6);
        g.set_resource(50, 50, -0.1);
        assert!((g.resource(50, 50) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn in_bounds_false_outside() {
        assert!(!SpatialGrid::in_bounds(100, 0));
        assert!(!SpatialGrid::in_bounds(0, 100));
        assert!(!SpatialGrid::in_bounds(100, 100));
        assert!(SpatialGrid::in_bounds(99, 99));
    }

    #[test]
    fn chunk_ranges_cover_all_cells() {
        let g = SpatialGrid::new();
        let mut covered: HashSet<(usize, usize)> = HashSet::new();
        for &(x0, x1, y0, y1) in g.chunk_ranges() {
            for y in y0..y1 {
                for x in x0..x1 {
                    covered.insert((x, y));
                }
            }
        }
        assert_eq!(covered.len(), TOTAL_CELLS, "Los chunk ranges no cubren todas las {TOTAL_CELLS} celdas");
    }

    #[test]
    fn default_temp_is_20() {
        let g = SpatialGrid::new();
        assert!((g.temp(50, 50) - 20.0).abs() < 1e-6);
    }

    #[test]
    fn pheromone_read_write_slices() {
        let mut g = SpatialGrid::new();
        let write_buf = 1 - g.read_buf;
        g.pheromone_write_slice_mut(PheromoneKind::Alarm)[SpatialGrid::idx(10, 10)] = 0.75;
        assert!((g.phero[0][write_buf][SpatialGrid::idx(10, 10)] - 0.75).abs() < 1e-6);
        g.swap_pheromone_buffers();
        let slice = g.pheromone_read_slice(PheromoneKind::Alarm);
        assert!((slice[SpatialGrid::idx(10, 10)] - 0.75).abs() < 1e-6);
    }
}
