/// Roles de una abeja. Queen y Drone nunca transicionan (simulation_spec.md).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Queen,
    Nurse,
    Builder,
    Guard,
    Forager,
    Drone,
}

/// Estado epidemiológico SIR (simulation_spec.md §Disease).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SirState {
    Susceptible,
    Infected,
    Recovered,
}

/// Posición actual en el grid (coordenadas de celda).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PositionComponent(pub u16, pub u16);

/// Rol activo de la abeja.
#[derive(Clone, Copy, Debug)]
pub struct RoleComponent(pub Role);

/// Energía actual [0.0, 1.0]. La abeja muere cuando llega a 0.
#[derive(Clone, Copy, Debug)]
pub struct EnergyComponent(pub f32);

/// Estado epidemiológico de la abeja.
#[derive(Clone, Copy, Debug)]
pub struct HealthComponent(pub SirState);

/// Edad en ticks desde la eclosión.
#[derive(Clone, Copy, Debug)]
pub struct AgeComponent(pub u32);

/// Umbral de respuesta a cada tipo de feromona: [alarm, task, attraction].
/// Valor 1.0 = sensibilidad base; modulado por rol en M6.
#[derive(Clone, Copy, Debug)]
pub struct PheromoneSensitivity(pub [f32; 3]);

/// Fase del ciclo de forrajeo. Solo Foragers tienen este componente.
/// simulation_spec.md §Forrajeo.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ForagerPhase {
    Searching,
    Returning { carry: f32 },
}

pub struct ForagerStateComponent(pub ForagerPhase);

/// Estado de transición de rol. Solo abejas transicionables (Nurse/Builder/Guard/Forager).
/// Queen y Drone nunca tienen este componente — la ausencia es la restricción estructural.
pub struct RoleTransitionState {
    pub cooldown: u32,        // ticks restantes hasta próxima transición
    pub threshold_bias: f32,  // factor ±10% del umbral base, fijo por agente desde seed
}
