use crate::metrics::MetricsSnapshot;

// ---------------------------------------------------------------------------
// Tipos públicos (siempre compilados — Orchestrator los usa sin feature flag)
// ---------------------------------------------------------------------------

/// Clasificación de un agente para el renderer. Sin refs al ECS.
pub enum AgentKind {
    Queen,
    Nurse,
    Builder,
    Guard,
    Forager,
    Drone,
    Predator,
}

/// Datos mínimos de un agente para renderizar. No contiene refs al ECS.
pub struct AgentRenderData {
    pub x: f32,
    pub y: f32,
    pub role: AgentKind,
}

/// Frame completo enviado del thread de simulación al renderer por mpsc.
/// Siempre compilado (sin feature gate) para que Orchestrator pueda usarlo.
pub struct RenderFrame {
    pub tick: u64,
    /// Concentraciones del canal Alarm, flat row-major: idx = y*100 + x.
    pub pheromone_alarm: Vec<f32>,
    /// Concentraciones del canal Task.
    pub pheromone_task: Vec<f32>,
    /// Concentraciones del canal Attraction.
    pub pheromone_attraction: Vec<f32>,
    pub agents: Vec<AgentRenderData>,
    pub metrics: MetricsSnapshot,
}

// ---------------------------------------------------------------------------
// VisualizerApp — solo compilado con `--features visualizer`
// ---------------------------------------------------------------------------

#[cfg(feature = "visualizer")]
use eframe::egui;
#[cfg(feature = "visualizer")]
use std::sync::mpsc::Receiver;
#[cfg(feature = "visualizer")]
use std::time::Instant;

#[cfg(feature = "visualizer")]
#[derive(PartialEq, Clone, Copy)]
enum PheromoneChannel {
    Alarm,
    Task,
    Attraction,
}

#[cfg(feature = "visualizer")]
impl PheromoneChannel {
    fn label(self) -> &'static str {
        match self {
            Self::Alarm      => "Alarma",
            Self::Task       => "Tarea",
            Self::Attraction => "Atracción",
        }
    }
}

#[cfg(feature = "visualizer")]
pub struct VisualizerApp {
    rx: Receiver<RenderFrame>,
    current: Option<RenderFrame>,
    prev: Option<RenderFrame>,
    frame_received_at: Instant,
    tick_duration: std::time::Duration,
    selected_channel: PheromoneChannel,
    texture: Option<egui::TextureHandle>,
}

#[cfg(feature = "visualizer")]
impl VisualizerApp {
    pub fn new(rx: Receiver<RenderFrame>, tick_duration: std::time::Duration) -> Self {
        Self {
            rx,
            current: None,
            prev: None,
            frame_received_at: Instant::now(),
            tick_duration,
            selected_channel: PheromoneChannel::Attraction,
            texture: None,
        }
    }

    fn drain_channel(&mut self) {
        while let Ok(frame) = self.rx.try_recv() {
            self.prev = self.current.take();
            self.current = Some(frame);
            self.frame_received_at = Instant::now();
        }
    }

    fn update_texture(&mut self, ctx: &egui::Context) {
        let Some(frame) = &self.current else { return };
        let slice = match self.selected_channel {
            PheromoneChannel::Alarm      => &frame.pheromone_alarm,
            PheromoneChannel::Task       => &frame.pheromone_task,
            PheromoneChannel::Attraction => &frame.pheromone_attraction,
        };
        let max = slice.iter().cloned().fold(0.0_f32, f32::max).max(1e-6);
        let pixels: Vec<egui::Color32> = slice
            .iter()
            .map(|&v| {
                let t = (v / max).clamp(0.0, 1.0);
                let r = (t * t * 255.0) as u8;
                let g = (t * 255.0) as u8;
                egui::Color32::from_rgb(r, g, 255)
            })
            .collect();
        let image = egui::ColorImage { size: [100, 100], pixels };
        self.texture = Some(ctx.load_texture(
            "pheromone_heatmap",
            image,
            egui::TextureOptions {
                magnification: egui::TextureFilter::Nearest,
                minification:  egui::TextureFilter::Nearest,
                ..Default::default()
            },
        ));
    }

    fn draw_grid_panel(&self, ui: &mut egui::Ui) {
        let available = ui.available_size();
        let side = available.x.min(available.y);
        let (rect, _) = ui.allocate_exact_size(
            egui::vec2(side, side),
            egui::Sense::hover(),
        );

        let painter = ui.painter_at(rect);

        // Heatmap de feromonas
        if let Some(tex) = &self.texture {
            painter.image(
                tex.id(),
                rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        } else {
            painter.rect_filled(rect, 0.0, egui::Color32::from_gray(20));
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "Esperando simulación...",
                egui::FontId::default(),
                egui::Color32::GRAY,
            );
        }

        // Puntos de agentes
        if let Some(frame) = &self.current {
            // Alpha de interpolación entre prev y current
            let alpha = (self.frame_received_at.elapsed().as_secs_f32()
                / self.tick_duration.as_secs_f32())
                .clamp(0.0, 1.0);

            for (i, agent) in frame.agents.iter().enumerate() {
                let (ax, ay) = if let Some(prev) = &self.prev {
                    if let Some(pa) = prev.agents.get(i) {
                        (pa.x + (agent.x - pa.x) * alpha, pa.y + (agent.y - pa.y) * alpha)
                    } else {
                        (agent.x, agent.y)
                    }
                } else {
                    (agent.x, agent.y)
                };

                let screen_pos = egui::pos2(
                    rect.min.x + ax / 100.0 * rect.width(),
                    rect.min.y + ay / 100.0 * rect.height(),
                );
                painter.circle_filled(screen_pos, 2.0, agent_color(&agent.role));
            }
        }
    }

    fn draw_metrics_panel(&self, ui: &mut egui::Ui) {
        ui.heading("BeeFlow");
        ui.separator();

        let Some(frame) = &self.current else {
            ui.label("Sin datos aún...");
            return;
        };
        let m = &frame.metrics;

        ui.label(format!("Tick: {}", frame.tick));
        ui.separator();

        ui.label("Población");
        let pop = &m.population_by_role;
        ui.label(format!("  Total: {}", pop.total()));
        ui.label(format!("  Reina: {}", pop.queen));
        ui.label(format!("  Nodrizas: {}", pop.nurse));
        ui.label(format!("  Constructoras: {}", pop.builder));
        ui.label(format!("  Guardianas: {}", pop.guard));
        ui.label(format!("  Recolectoras: {}", pop.forager));
        ui.label(format!("  Zánganos: {}", pop.drone));
        ui.separator();

        ui.label("Colonia");
        ui.label(format!("  Reserva: {:.3}", m.colony_reserve));
        ui.label(format!("  Ef. forrajeo: {:.3}", m.foraging_efficiency));
        ui.label(format!("  Cría/adultos: {:.3}", m.brood_adult_ratio));
        ui.label(format!("  Entropía fer.: {:.3}", m.pheromone_entropy));
        ui.separator();

        ui.label("Ambiente");
        ui.label(format!("  Temp: {:.1}°C", m.global_temp));
        ui.label(format!("  Estación: {:.3}", m.season_phase));
        ui.label(format!("  Tasa cría: {:.4}", m.brood_production_rate));
        ui.separator();

        ui.label("SIR");
        ui.label(format!("  S: {:.1}%", m.sir_prevalence.susceptible * 100.0));
        ui.label(format!("  I: {:.1}%", m.sir_prevalence.infected * 100.0));
        ui.label(format!("  R: {:.1}%", m.sir_prevalence.recovered * 100.0));
        ui.separator();

        ui.label("Muertes");
        ui.label(format!("  Energía: {}", m.mortality_rate.by_energy));
        ui.label(format!("  Enfermedad: {}", m.mortality_rate.by_disease));
        ui.label(format!("  Depredación: {}", m.mortality_rate.by_predation));

        if let Some(t) = m.time_to_collapse {
            ui.separator();
            ui.colored_label(
                egui::Color32::from_rgb(255, 80, 80),
                format!("COLAPSO en tick {}", t),
            );
        }
    }
}

#[cfg(feature = "visualizer")]
impl eframe::App for VisualizerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_channel();
        self.update_texture(ctx);

        egui::SidePanel::right("metrics_panel")
            .min_width(200.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.label("Feromona");
                    for ch in [
                        PheromoneChannel::Alarm,
                        PheromoneChannel::Task,
                        PheromoneChannel::Attraction,
                    ] {
                        if ui
                            .selectable_label(self.selected_channel == ch, ch.label())
                            .clicked()
                        {
                            self.selected_channel = ch;
                        }
                    }
                    ui.separator();
                    self.draw_metrics_panel(ui);
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            self.draw_grid_panel(ui);
        });

        ctx.request_repaint();
    }
}

// ---------------------------------------------------------------------------
// Helpers (siempre compilados — usados en tests aunque no haya feature)
// ---------------------------------------------------------------------------

#[cfg(feature = "visualizer")]
fn agent_color(kind: &AgentKind) -> egui::Color32 {
    match kind {
        AgentKind::Queen    => egui::Color32::from_rgb(255, 220, 0),   // amarillo
        AgentKind::Nurse    => egui::Color32::from_rgb(50, 200, 50),   // verde
        AgentKind::Builder  => egui::Color32::from_rgb(60, 120, 255),  // azul
        AgentKind::Guard    => egui::Color32::from_rgb(220, 50, 50),   // rojo
        AgentKind::Forager  => egui::Color32::from_rgb(255, 140, 0),   // naranja
        AgentKind::Drone    => egui::Color32::from_rgb(160, 160, 160), // gris
        AgentKind::Predator => egui::Color32::from_rgb(220, 0, 220),   // magenta
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::MetricsSnapshot;
    use std::sync::mpsc;

    fn dummy_frame(tick: u64) -> RenderFrame {
        RenderFrame {
            tick,
            pheromone_alarm:      vec![0.0; 10_000],
            pheromone_task:       vec![0.0; 10_000],
            pheromone_attraction: vec![0.0; 10_000],
            agents:               Vec::new(),
            metrics:              MetricsSnapshot { tick, ..Default::default() },
        }
    }

    #[test]
    fn render_frame_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<RenderFrame>();
    }

    #[test]
    fn try_send_no_block_when_full() {
        let (tx, _rx) = mpsc::sync_channel::<RenderFrame>(1);
        // Llenar el canal con 1 frame (capacidad máxima)
        tx.try_send(dummy_frame(0)).expect("primer send debe tener éxito");
        // El segundo try_send debe retornar Err(Full) sin bloquear
        let result = tx.try_send(dummy_frame(1));
        assert!(result.is_err(), "try_send con canal lleno debe retornar Err");
    }
}
