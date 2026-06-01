use rand::SeedableRng;
use rand_xoshiro::Xoshiro256StarStar;

/// Primo de Knuth para dispersar IDs de agente en el espacio de semillas.
const SEED_MULTIPLIER: u64 = 6364136223846793005;

/// Gestiona la aleatoriedad determinista del sistema.
///
/// Garantía: `agent_rng(id)` con el mismo `seed` siempre produce la misma
/// secuencia, independientemente del orden de llamada.
pub struct RngSystem {
    global_seed: u64,
}

impl RngSystem {
    pub fn new(seed: u64) -> Self {
        Self { global_seed: seed }
    }

    /// RNG global para uso del Orchestrator (no para agentes individuales).
    pub fn global_rng(&self) -> Xoshiro256StarStar {
        Xoshiro256StarStar::seed_from_u64(self.global_seed)
    }

    /// RNG determinista para un agente dado su ID.
    /// Usa XOR + multiplicación para dispersar uniformemente en el espacio de semillas.
    pub fn agent_rng(&self, agent_id: u64) -> Xoshiro256StarStar {
        let derived = self
            .global_seed
            .wrapping_add(agent_id.wrapping_mul(SEED_MULTIPLIER));
        Xoshiro256StarStar::seed_from_u64(derived)
    }

    /// RNG determinista para un tick dado. La semilla varía con cada tick,
    /// produciendo movimientos distintos cada vez sin mutar el estado global.
    pub fn tick_rng(&self, tick: u64) -> Xoshiro256StarStar {
        Xoshiro256StarStar::seed_from_u64(
            self.global_seed.wrapping_add(tick.wrapping_mul(SEED_MULTIPLIER)),
        )
    }

    pub fn global_seed(&self) -> u64 {
        self.global_seed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::RngCore;

    #[test]
    fn same_seed_same_sequence() {
        let sys = RngSystem::new(42);
        let mut a = sys.agent_rng(7);
        let mut b = sys.agent_rng(7);
        assert_eq!(a.next_u64(), b.next_u64());
        assert_eq!(a.next_u64(), b.next_u64());
    }

    #[test]
    fn different_agents_different_sequences() {
        let sys = RngSystem::new(42);
        let mut a = sys.agent_rng(1);
        let mut b = sys.agent_rng(2);
        // Con altísima probabilidad son distintos (colisión sería un bug)
        assert_ne!(a.next_u64(), b.next_u64());
    }

    #[test]
    fn different_seeds_different_sequences() {
        let sys_a = RngSystem::new(1);
        let sys_b = RngSystem::new(2);
        let mut a = sys_a.agent_rng(0);
        let mut b = sys_b.agent_rng(0);
        assert_ne!(a.next_u64(), b.next_u64());
    }

    #[test]
    fn tick_rng_differs_per_tick() {
        let sys = RngSystem::new(42);
        let mut a = sys.tick_rng(0);
        let mut b = sys.tick_rng(1);
        assert_ne!(a.next_u64(), b.next_u64());
    }
}
