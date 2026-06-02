use crate::metrics::MetricsSnapshot;

// ---------------------------------------------------------------------------
// Tipos públicos (siempre compilados — Orchestrator los usa sin feature flag)
// ---------------------------------------------------------------------------

/// Clasificación de un agente para el renderer.
pub enum AgentKind {
    Queen,
    Nurse,
    Builder,
    Guard,
    Forager,
    Drone,
    Predator,
}

/// Datos mínimos de un agente para renderizar.
pub struct AgentRenderData {
    pub x: f32,
    pub y: f32,
    pub role: AgentKind,
}

/// Comandos enviados desde la UI al Orchestrator para modificar parámetros en tiempo real.
pub enum SimCommand {
    /// Override manual de temperatura en °C. `f32::NAN` = volver al ciclo estacional.
    SetTemperature(f32),
    /// Salto directo de `season_phase` [0.0, 1.0).
    SetSeason(f32),
    /// Presión de pesticidas [0.0, 1.0].
    SetPesticide(f32),
    SetDiseaseEnabled(bool),
    SetDiseaseRate(f32),
    /// Velocidad de simulación en ticks/seg.
    SetSimulationSpeed(u32),
    /// Costo metabólico basal por tick [0.001, 0.05].
    SetMetabolicRate(f32),
    /// Umbral de reserva para activar trofalaxia [10, 50].
    SetTrophallaxisThreshold(f32),
    /// Tasa de decaimiento de feromonas por tick [0.01, 0.15].
    SetPheromoneDecay(f32),
    /// Multiplicador de intensidad de emisión de feromonas [0.1, 2.0].
    SetPheromoneEmission(f32),
}

/// Frame completo enviado del thread de simulación al renderer por mpsc.
pub struct RenderFrame {
    pub tick: u64,
    pub pheromone_alarm:      Vec<f32>,
    pub pheromone_task:       Vec<f32>,
    pub pheromone_attraction: Vec<f32>,
    pub agents:  Vec<AgentRenderData>,
    pub metrics: MetricsSnapshot,
}

// ---------------------------------------------------------------------------
// VisualizerApp — solo compilado con `--features visualizer`
// ---------------------------------------------------------------------------

#[cfg(feature = "visualizer")]
use eframe::egui;
#[cfg(feature = "visualizer")]
use std::collections::VecDeque;
#[cfg(feature = "visualizer")]
use std::sync::mpsc::{Receiver, Sender};
#[cfg(feature = "visualizer")]
use std::time::Instant;

// ── Paleta de colores (dark theme de claude-design/) ────────────────────────
#[cfg(feature = "visualizer")] const BG_APP:    egui::Color32 = egui::Color32::from_rgb(13,  15,  14);
#[cfg(feature = "visualizer")] const SURFACE_1: egui::Color32 = egui::Color32::from_rgb(20,  22,  20);
#[cfg(feature = "visualizer")] const SURFACE_2: egui::Color32 = egui::Color32::from_rgb(28,  31,  28);
#[cfg(feature = "visualizer")] const SURFACE_3: egui::Color32 = egui::Color32::from_rgb(36,  40,  36);
#[cfg(feature = "visualizer")] const AMBER:     egui::Color32 = egui::Color32::from_rgb(232, 160, 32);
#[cfg(feature = "visualizer")] const GREEN_M:   egui::Color32 = egui::Color32::from_rgb(76,  175, 114);
#[cfg(feature = "visualizer")] const RED_M:     egui::Color32 = egui::Color32::from_rgb(224, 82,  82);
#[cfg(feature = "visualizer")] const BLUE_M:    egui::Color32 = egui::Color32::from_rgb(74,  144, 217);
#[cfg(feature = "visualizer")] const TEXT_PRI:  egui::Color32 = egui::Color32::from_rgb(222, 222, 222);
#[cfg(feature = "visualizer")] const TEXT_SEC:  egui::Color32 = egui::Color32::from_rgb(140, 140, 140);

/// Cache local de valores de los controles del panel derecho.
/// Se sincroniza con el Orchestrator via `SimCommand`.
#[cfg(feature = "visualizer")]
struct UiParams {
    temperature:            f32,   // °C
    temp_manual:            bool,  // false = Auto (sigue estación)
    pesticide:              f32,   // [0.0, 1.0]
    disease_enabled:        bool,
    disease_rate:           f32,   // [0.01, 0.20]
    metabolic_rate:         f32,   // [0.001, 0.05]
    trophallaxis_threshold: f32,   // [10.0, 50.0]
    pheromone_decay:        f32,   // [0.01, 0.15]
    pheromone_emission:     f32,   // [0.1, 2.0]
    speed:                  u32,   // ticks/seg
}

#[cfg(feature = "visualizer")]
impl Default for UiParams {
    fn default() -> Self {
        Self {
            temperature:            20.0,
            temp_manual:            false,
            pesticide:              0.0,
            disease_enabled:        false,
            disease_rate:           0.05,
            metabolic_rate:         0.001,
            trophallaxis_threshold: 20.0,
            pheromone_decay:        0.05,
            pheromone_emission:     1.0,
            speed:                  30,
        }
    }
}

#[cfg(feature = "visualizer")]
#[derive(PartialEq, Clone, Copy)]
enum PheromoneChannel { Alarm, Task, Attraction }

#[cfg(feature = "visualizer")]
impl PheromoneChannel {
    fn label(self) -> &'static str {
        match self {
            Self::Alarm      => "Alarma",
            Self::Task       => "Tarea",
            Self::Attraction => "Atracción",
        }
    }
    fn color(self) -> egui::Color32 {
        match self { Self::Alarm => RED_M, Self::Task => GREEN_M, Self::Attraction => AMBER }
    }
}

#[cfg(feature = "visualizer")]
pub struct VisualizerApp {
    rx:                Receiver<RenderFrame>,
    current:           Option<RenderFrame>,
    prev:              Option<RenderFrame>,
    frame_received_at: Instant,
    tick_duration:     std::time::Duration,
    selected_channel:  PheromoneChannel,
    texture:           Option<egui::TextureHandle>,
    metrics_history:   VecDeque<MetricsSnapshot>,
    cmd_tx:            Option<Sender<SimCommand>>,
    ui_params:         UiParams,
}

#[cfg(feature = "visualizer")]
impl VisualizerApp {
    pub fn new(
        rx: Receiver<RenderFrame>,
        tick_duration: std::time::Duration,
        cmd_tx: Option<Sender<SimCommand>>,
    ) -> Self {
        Self {
            rx,
            current:           None,
            prev:              None,
            frame_received_at: Instant::now(),
            tick_duration,
            selected_channel:  PheromoneChannel::Attraction,
            texture:           None,
            metrics_history:   VecDeque::with_capacity(60),
            cmd_tx,
            ui_params:         UiParams::default(),
        }
    }

    fn send_cmd(&self, cmd: SimCommand) {
        if let Some(tx) = &self.cmd_tx {
            let _ = tx.send(cmd);
        }
    }

    fn drain_channel(&mut self) {
        while let Ok(frame) = self.rx.try_recv() {
            self.prev = self.current.take();
            self.metrics_history.push_back(frame.metrics.clone());
            if self.metrics_history.len() > 60 {
                self.metrics_history.pop_front();
            }
            self.current = Some(frame);
            self.frame_received_at = Instant::now();
        }
    }

    fn apply_theme(ctx: &egui::Context) {
        let mut v = egui::Visuals::dark();
        v.panel_fill       = BG_APP;
        v.window_fill      = SURFACE_2;
        v.extreme_bg_color = BG_APP;
        v.faint_bg_color   = SURFACE_1;
        v.widgets.noninteractive.bg_fill   = SURFACE_2;
        v.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, TEXT_PRI);
        v.widgets.inactive.bg_fill         = SURFACE_3;
        v.widgets.inactive.fg_stroke       = egui::Stroke::new(1.0, TEXT_SEC);
        v.widgets.hovered.bg_fill          = SURFACE_3;
        v.widgets.hovered.fg_stroke        = egui::Stroke::new(1.0, TEXT_PRI);
        v.widgets.active.bg_fill           = AMBER;
        v.widgets.active.fg_stroke         = egui::Stroke::new(1.0, BG_APP);
        v.selection.bg_fill                = egui::Color32::from_rgb(70, 50, 10);
        v.override_text_color              = Some(TEXT_PRI);
        ctx.set_visuals(v);
    }

    fn update_texture(&mut self, ctx: &egui::Context) {
        let pixels: Vec<egui::Color32> = {
            let Some(frame) = &self.current else { return };
            let slice = match self.selected_channel {
                PheromoneChannel::Alarm      => &frame.pheromone_alarm,
                PheromoneChannel::Task       => &frame.pheromone_task,
                PheromoneChannel::Attraction => &frame.pheromone_attraction,
            };
            let ch = self.selected_channel;
            let max = slice.iter().cloned().fold(0.0_f32, f32::max).max(1e-6);
            slice.iter().map(|&v| {
                let t = (v / max).clamp(0.0, 1.0);
                match ch {
                    PheromoneChannel::Attraction => egui::Color32::from_rgb(
                        (t * t * 232.0) as u8, (t * 100.0) as u8, (t * 8.0) as u8,
                    ),
                    PheromoneChannel::Alarm => egui::Color32::from_rgb(
                        (t * t * 224.0) as u8, (t * 30.0) as u8, 0,
                    ),
                    PheromoneChannel::Task => egui::Color32::from_rgb(
                        (t * 30.0) as u8, (t * t * 175.0) as u8, 30,
                    ),
                }
            }).collect()
        };
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

    // ── Topbar ───────────────────────────────────────────────────────────────
    fn draw_topbar(&mut self, ui: &mut egui::Ui) {
        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
            // Ícono de abeja (círculo ámbar con "B")
            let (bee_r, _) = ui.allocate_exact_size(egui::vec2(26.0, 26.0), egui::Sense::hover());
            let p = ui.painter_at(bee_r);
            p.circle_filled(bee_r.center(), 12.0, AMBER);
            p.text(bee_r.center(), egui::Align2::CENTER_CENTER, "B",
                egui::FontId::proportional(13.0), BG_APP);

            ui.add_space(6.0);
            ui.label(egui::RichText::new("BeeFlow").size(15.0).color(TEXT_PRI).strong());

            ui.add_space(12.0);
            ui.add(egui::Separator::default().vertical());
            ui.add_space(8.0);

            // Controles de reproducción (layout visual, sin lógica en MVP)
            ui.add(egui::Button::new(
                egui::RichText::new("▶").size(13.0).color(AMBER)
            ).fill(SURFACE_3).min_size(egui::vec2(28.0, 26.0)));
            ui.add_space(2.0);
            ui.add(egui::Button::new(
                egui::RichText::new("■").size(11.0).color(TEXT_SEC)
            ).fill(SURFACE_3).min_size(egui::vec2(28.0, 26.0)));

            ui.add_space(8.0);
            ui.add(egui::Separator::default().vertical());
            ui.add_space(6.0);

            // Selector de velocidad (funcional)
            for (label, speed) in [("×1", 1u32), ("×5", 5), ("×30", 30), ("×100", 100)] {
                let active = self.ui_params.speed == speed;
                let (fill, text_col) = if active {
                    (egui::Color32::from_rgb(60, 44, 8), AMBER)
                } else {
                    (SURFACE_3, TEXT_SEC)
                };
                if ui.add(egui::Button::new(
                    egui::RichText::new(label).size(11.0).color(text_col)
                ).fill(fill).min_size(egui::vec2(34.0, 22.0))).clicked() {
                    self.ui_params.speed = speed;
                    self.send_cmd(SimCommand::SetSimulationSpeed(speed));
                }
                ui.add_space(2.0);
            }

            ui.add_space(12.0);
            ui.add(egui::Separator::default().vertical());
            ui.add_space(8.0);

            // Tick y tiempo
            match &self.current {
                Some(frame) => {
                    let tick = frame.tick;
                    ui.label(egui::RichText::new("Tick").size(10.0).color(TEXT_SEC));
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new(format!("{:>7}", tick))
                        .size(13.0).color(TEXT_PRI).monospace().strong());
                    ui.add_space(10.0);
                    ui.label(egui::RichText::new(format_tick_time(tick))
                        .size(12.0).color(TEXT_SEC).monospace());
                }
                None => {
                    ui.label(egui::RichText::new("Esperando…").size(12.0).color(TEXT_SEC));
                }
            }

            // Badge de estado (derecha)
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(12.0);
                let (lbl, col) = if self.current.is_some() { ("RUNNING", GREEN_M) }
                                 else { ("WAITING", TEXT_SEC) };
                ui.label(egui::RichText::new(lbl).size(11.0).color(col).strong());
                ui.add_space(4.0);
                let (dot, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
                ui.painter_at(dot).circle_filled(dot.center(), 4.0, col);
            });
        });
    }

    // ── Panel izquierdo ──────────────────────────────────────────────────────
    fn draw_left_panel(&self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            let Some(frame) = &self.current else {
                ui.add_space(12.0);
                ui.label(egui::RichText::new("Sin datos…").size(12.0).color(TEXT_SEC));
                return;
            };
            let m = &frame.metrics;
            let pop = &m.population_by_role;
            let total = pop.total();

            // ── Población ──────────────────────────────────────────────────
            egui::CollapsingHeader::new(
                egui::RichText::new("POBLACIÓN").size(10.0).color(TEXT_SEC)
            ).default_open(true).show(ui, |ui| {
                ui.label(egui::RichText::new(total.to_string())
                    .size(28.0).color(AMBER).monospace().strong());
                ui.add_space(6.0);
                let max_n = total.max(1);
                draw_role_bar(ui, "Reina",         pop.queen,   1,     AMBER);
                draw_role_bar(ui, "Nodrizas",      pop.nurse,   max_n, AMBER);
                draw_role_bar(ui, "Forrajeras",    pop.forager, max_n,
                    egui::Color32::from_rgb(255, 180, 60));
                draw_role_bar(ui, "Guardianas",    pop.guard,   max_n,
                    egui::Color32::from_rgb(200, 120, 20));
                draw_role_bar(ui, "Constructoras", pop.builder, max_n,
                    egui::Color32::from_rgb(60, 120, 255));
                draw_role_bar(ui, "Zánganos",      pop.drone,   max_n, TEXT_SEC);
            });

            ui.add_space(8.0);

            // ── Salud SIR ─────────────────────────────────────────────────
            egui::CollapsingHeader::new(
                egui::RichText::new("SALUD (SIR)").size(10.0).color(TEXT_SEC)
            ).default_open(true).show(ui, |ui| {
                let s   = m.sir_prevalence.susceptible;
                let inf = m.sir_prevalence.infected;
                let r   = m.sir_prevalence.recovered;

                // Donut centrado
                ui.vertical_centered(|ui| {
                    let (donut_r, _) = ui.allocate_exact_size(
                        egui::vec2(120.0, 120.0), egui::Sense::hover()
                    );
                    let painter = ui.painter_at(donut_r);
                    let center  = donut_r.center();
                    let sum     = (s + inf + r).max(1e-6);
                    let start   = -std::f32::consts::FRAC_PI_2;
                    let a_s     = s   / sum * std::f32::consts::TAU;
                    let a_i     = inf / sum * std::f32::consts::TAU;
                    let a_r     = r   / sum * std::f32::consts::TAU;
                    draw_donut_arc(&painter, center, 32.0, 50.0, start,               start + a_s,             GREEN_M);
                    draw_donut_arc(&painter, center, 32.0, 50.0, start + a_s,         start + a_s + a_i,       RED_M);
                    draw_donut_arc(&painter, center, 32.0, 50.0, start + a_s + a_i,   start + a_s + a_i + a_r, BLUE_M);
                    // Si no hay datos, mostrar anillo gris
                    if sum < 0.01 {
                        draw_donut_arc(&painter, center, 32.0, 50.0,
                            start, start + std::f32::consts::TAU, SURFACE_3);
                    }
                    painter.text(center, egui::Align2::CENTER_CENTER,
                        format!("{:.0}%\nI", inf * 100.0),
                        egui::FontId::monospace(11.0), RED_M);
                });

                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    color_chip(ui, GREEN_M, &format!("S {:.0}%", s   * 100.0));
                    ui.add_space(6.0);
                    color_chip(ui, RED_M,   &format!("I {:.0}%", inf * 100.0));
                    ui.add_space(6.0);
                    color_chip(ui, BLUE_M,  &format!("R {:.0}%", r   * 100.0));
                });
            });

            ui.add_space(8.0);

            // ── Energía ───────────────────────────────────────────────────
            egui::CollapsingHeader::new(
                egui::RichText::new("ENERGÍA").size(10.0).color(TEXT_SEC)
            ).default_open(true).show(ui, |ui| {
                draw_reserve_bar(ui, "Reserva miel",  m.colony_reserve.clamp(0.0, 1.0),      AMBER);
                draw_reserve_bar(ui, "Ef. forrajeo",  m.foraging_efficiency.clamp(0.0, 1.0), GREEN_M);
            });

            ui.add_space(8.0);

            // ── Mortalidad ─────────────────────────────────────────────────
            egui::CollapsingHeader::new(
                egui::RichText::new("MORTALIDAD").size(10.0).color(TEXT_SEC)
            ).default_open(false).show(ui, |ui| {
                let mort = &m.mortality_rate;
                stat_row(ui, "Energía",     &mort.by_energy.to_string());
                stat_row(ui, "Enfermedad",  &mort.by_disease.to_string());
                stat_row(ui, "Depredación", &mort.by_predation.to_string());
            });

            // Alerta de colapso
            if let Some(t) = m.time_to_collapse {
                ui.add_space(10.0);
                let w = ui.available_width();
                let (warn, _) = ui.allocate_exact_size(egui::vec2(w, 28.0), egui::Sense::hover());
                ui.painter_at(warn).rect_filled(warn, 4.0, egui::Color32::from_rgb(80, 20, 20));
                ui.painter_at(warn).text(
                    warn.center(), egui::Align2::CENTER_CENTER,
                    format!("⚠ COLAPSO en tick {}", t),
                    egui::FontId::proportional(11.0), RED_M,
                );
            }
        });
    }

    // ── Panel derecho ─────────────────────────────────────────────────────────
    fn draw_right_panel(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {

            // ── Canal de feromonas ────────────────────────────────────────
            ui.label(egui::RichText::new("CANAL DE FEROMONAS").size(10.0).color(TEXT_SEC));
            ui.add_space(6.0);
            let w = ui.available_width();
            for ch in [PheromoneChannel::Alarm, PheromoneChannel::Task, PheromoneChannel::Attraction] {
                let selected = self.selected_channel == ch;
                let fill = if selected {
                    egui::Color32::from_rgba_premultiplied(
                        ch.color().r() / 5, ch.color().g() / 5, ch.color().b() / 5, 255,
                    )
                } else { SURFACE_3 };
                let text_color = if selected { ch.color() } else { TEXT_SEC };
                if ui.add(
                    egui::Button::new(egui::RichText::new(ch.label()).size(12.0).color(text_color))
                        .fill(fill).min_size(egui::vec2(w, 28.0))
                ).clicked() {
                    self.selected_channel = ch;
                }
                ui.add_space(3.0);
            }

            ui.add_space(10.0);
            ui.add(egui::Separator::default());
            ui.add_space(8.0);

            // ── Entorno ───────────────────────────────────────────────────
            egui::CollapsingHeader::new(
                egui::RichText::new("ENTORNO").size(10.0).color(TEXT_SEC)
            ).default_open(true).show(ui, |ui| {
                // Temperatura: checkbox Manual + slider
                let changed = ui.checkbox(
                    &mut self.ui_params.temp_manual,
                    egui::RichText::new("Temp. manual").size(11.0).color(TEXT_PRI)
                ).changed();
                if changed {
                    let cmd = if self.ui_params.temp_manual {
                        SimCommand::SetTemperature(self.ui_params.temperature)
                    } else {
                        SimCommand::SetTemperature(f32::NAN)
                    };
                    self.send_cmd(cmd);
                }
                ui.add_enabled_ui(self.ui_params.temp_manual, |ui| {
                    if ui.add(
                        egui::Slider::new(&mut self.ui_params.temperature, -10.0f32..=45.0)
                            .suffix(" °C")
                            .fixed_decimals(1)
                    ).changed() {
                        self.send_cmd(SimCommand::SetTemperature(self.ui_params.temperature));
                    }
                });
                ui.add_space(4.0);

                // Estación: 4 botones de salto
                ui.label(egui::RichText::new("Estación").size(10.0).color(TEXT_SEC));
                ui.horizontal(|ui| {
                    for (label, phase) in [("Inv", 0.0f32), ("Prim", 0.25), ("Ver", 0.5), ("Otoño", 0.75)] {
                        if ui.add(
                            egui::Button::new(egui::RichText::new(label).size(10.0).color(TEXT_PRI))
                                .fill(SURFACE_3).min_size(egui::vec2(44.0, 20.0))
                        ).clicked() {
                            self.send_cmd(SimCommand::SetSeason(phase));
                        }
                    }
                });
                ui.add_space(4.0);

                // Pesticida
                ui.label(egui::RichText::new("Pesticida").size(10.0).color(TEXT_SEC));
                if ui.add(
                    egui::Slider::new(&mut self.ui_params.pesticide, 0.0f32..=1.0)
                        .custom_formatter(|v, _| format!("{:.0}%", v * 100.0))
                ).changed() {
                    self.send_cmd(SimCommand::SetPesticide(self.ui_params.pesticide));
                }
            });

            ui.add_space(6.0);

            // ── Enfermedad ────────────────────────────────────────────────
            egui::CollapsingHeader::new(
                egui::RichText::new("ENFERMEDAD").size(10.0).color(TEXT_SEC)
            ).default_open(true).show(ui, |ui| {
                if ui.checkbox(
                    &mut self.ui_params.disease_enabled,
                    egui::RichText::new("SIR activo").size(11.0).color(TEXT_PRI)
                ).changed() {
                    self.send_cmd(SimCommand::SetDiseaseEnabled(self.ui_params.disease_enabled));
                }
                if self.ui_params.disease_enabled {
                    ui.label(egui::RichText::new("Tasa infección").size(10.0).color(TEXT_SEC));
                    if ui.add(
                        egui::Slider::new(&mut self.ui_params.disease_rate, 0.01f32..=0.20)
                            .fixed_decimals(3)
                    ).changed() {
                        self.send_cmd(SimCommand::SetDiseaseRate(self.ui_params.disease_rate));
                    }
                }
            });

            ui.add_space(6.0);

            // ── Colonia (avanzado) ─────────────────────────────────────────
            egui::CollapsingHeader::new(
                egui::RichText::new("COLONIA (avanzado)").size(10.0).color(TEXT_SEC)
            ).default_open(false).show(ui, |ui| {
                ui.label(egui::RichText::new("Tasa metabólica").size(10.0).color(TEXT_SEC));
                if ui.add(
                    egui::Slider::new(&mut self.ui_params.metabolic_rate, 0.001f32..=0.05)
                        .fixed_decimals(4)
                ).changed() {
                    self.send_cmd(SimCommand::SetMetabolicRate(self.ui_params.metabolic_rate));
                }
                ui.add_space(4.0);

                ui.label(egui::RichText::new("Umbral trofalaxia").size(10.0).color(TEXT_SEC));
                if ui.add(
                    egui::Slider::new(&mut self.ui_params.trophallaxis_threshold, 10.0f32..=50.0)
                        .fixed_decimals(1)
                ).changed() {
                    self.send_cmd(SimCommand::SetTrophallaxisThreshold(self.ui_params.trophallaxis_threshold));
                }
            });

            ui.add_space(6.0);

            // ── Feromonas ─────────────────────────────────────────────────
            egui::CollapsingHeader::new(
                egui::RichText::new("FEROMONAS").size(10.0).color(TEXT_SEC)
            ).default_open(false).show(ui, |ui| {
                ui.label(egui::RichText::new("Decaimiento").size(10.0).color(TEXT_SEC));
                if ui.add(
                    egui::Slider::new(&mut self.ui_params.pheromone_decay, 0.01f32..=0.15)
                        .fixed_decimals(3)
                ).changed() {
                    self.send_cmd(SimCommand::SetPheromoneDecay(self.ui_params.pheromone_decay));
                }
                ui.add_space(4.0);

                ui.label(egui::RichText::new("Intensidad emisión").size(10.0).color(TEXT_SEC));
                if ui.add(
                    egui::Slider::new(&mut self.ui_params.pheromone_emission, 0.1f32..=2.0)
                        .suffix("×")
                        .fixed_decimals(2)
                ).changed() {
                    self.send_cmd(SimCommand::SetPheromoneEmission(self.ui_params.pheromone_emission));
                }
            });

            // Stats de solo lectura al fondo
            if let Some(frame) = &self.current {
                let m = &frame.metrics;
                ui.add_space(10.0);
                ui.add(egui::Separator::default());
                ui.add_space(6.0);
                ui.label(egui::RichText::new("ESTADO ACTUAL").size(10.0).color(TEXT_SEC));
                ui.add_space(4.0);
                stat_row(ui, "Temp. efectiva", &format!("{:.1} °C", m.global_temp));
                stat_row(ui, "Estación",        &format!("{:.3}",    m.season_phase));
                stat_row(ui, "Tasa cría",       &format!("{:.4}",    m.brood_production_rate));
                stat_row(ui, "Entropía fen.",   &format!("{:.3}",    m.pheromone_entropy));
            }
        });
    }

    // ── Bottom bar (sparklines) ──────────────────────────────────────────────
    fn draw_bottom_bar(&self, ui: &mut egui::Ui) {
        let h = &self.metrics_history;

        let pop_s:  Vec<f32> = h.iter().map(|m| m.population_by_role.total() as f32).collect();
        let mort_s: Vec<f32> = h.iter().map(|m| {
            (m.mortality_rate.by_energy + m.mortality_rate.by_disease + m.mortality_rate.by_predation) as f32
        }).collect();
        let miel_s: Vec<f32> = h.iter().map(|m| m.colony_reserve * 100.0).collect();
        let forr_s: Vec<f32> = h.iter().map(|m| m.foraging_efficiency * 100.0).collect();
        let inf_s:  Vec<f32> = h.iter().map(|m| m.sir_prevalence.infected * 100.0).collect();

        let cw = ((ui.available_width() - 20.0) / 5.0).max(60.0);
        let ch = (ui.available_height() - 8.0).max(30.0);

        ui.horizontal(|ui| {
            draw_metric_card(ui, "Población",
                &pop_s.last().map_or("–".into(),  |&v| format!("{:.0}", v)),
                compute_trend(&pop_s),  &pop_s,  AMBER,   egui::vec2(cw, ch));
            ui.add_space(4.0);
            draw_metric_card(ui, "Muertes/tick",
                &mort_s.last().map_or("–".into(), |&v| format!("{:.0}", v)),
                compute_trend(&mort_s), &mort_s, RED_M,   egui::vec2(cw, ch));
            ui.add_space(4.0);
            draw_metric_card(ui, "Reserva miel",
                &miel_s.last().map_or("–".into(), |&v| format!("{:.1}%", v)),
                compute_trend(&miel_s), &miel_s, AMBER,   egui::vec2(cw, ch));
            ui.add_space(4.0);
            draw_metric_card(ui, "Forrajeo",
                &forr_s.last().map_or("–".into(), |&v| format!("{:.1}%", v)),
                compute_trend(&forr_s), &forr_s, GREEN_M, egui::vec2(cw, ch));
            ui.add_space(4.0);
            draw_metric_card(ui, "Infectados",
                &inf_s.last().map_or("–".into(),  |&v| format!("{:.1}%", v)),
                compute_trend(&inf_s),  &inf_s,  BLUE_M,  egui::vec2(cw, ch));
        });
    }

    // ── Grid central ─────────────────────────────────────────────────────────
    fn draw_grid_panel(&self, ui: &mut egui::Ui) {
        let available = ui.available_size();
        let side = available.x.min(available.y);
        let (rect, _) = ui.allocate_exact_size(egui::vec2(side, side), egui::Sense::hover());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, BG_APP);

        if let Some(tex) = &self.texture {
            painter.image(
                tex.id(), rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        } else {
            painter.text(rect.center(), egui::Align2::CENTER_CENTER,
                "Esperando simulación…", egui::FontId::proportional(14.0), TEXT_SEC);
        }

        if let Some(frame) = &self.current {
            let alpha = (self.frame_received_at.elapsed().as_secs_f32()
                / self.tick_duration.as_secs_f32()).clamp(0.0, 1.0);

            for (i, agent) in frame.agents.iter().enumerate() {
                let (ax, ay) = if let Some(prev) = &self.prev {
                    if let Some(pa) = prev.agents.get(i) {
                        (pa.x + (agent.x - pa.x) * alpha, pa.y + (agent.y - pa.y) * alpha)
                    } else { (agent.x, agent.y) }
                } else { (agent.x, agent.y) };

                painter.circle_filled(
                    egui::pos2(
                        rect.min.x + ax / 100.0 * rect.width(),
                        rect.min.y + ay / 100.0 * rect.height(),
                    ),
                    2.0, agent_color(&agent.role),
                );
            }
        }
    }
}

// ── eframe::App ─────────────────────────────────────────────────────────────
#[cfg(feature = "visualizer")]
impl eframe::App for VisualizerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_channel();
        Self::apply_theme(ctx);
        self.update_texture(ctx);

        egui::TopBottomPanel::top("topbar")
            .exact_height(44.0)
            .show(ctx, |ui| { self.draw_topbar(ui); });

        egui::TopBottomPanel::bottom("bottom_bar")
            .exact_height(80.0)
            .show(ctx, |ui| {
                ui.add_space(4.0);
                self.draw_bottom_bar(ui);
            });

        egui::SidePanel::left("left_panel")
            .resizable(false)
            .min_width(240.0)
            .max_width(240.0)
            .show(ctx, |ui| {
                ui.add_space(6.0);
                self.draw_left_panel(ui);
            });

        egui::SidePanel::right("right_panel")
            .resizable(false)
            .min_width(260.0)
            .max_width(260.0)
            .show(ctx, |ui| {
                ui.add_space(6.0);
                self.draw_right_panel(ui);
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            self.draw_grid_panel(ui);
        });

        ctx.request_repaint();
    }
}

// ── Helpers (solo con feature) ───────────────────────────────────────────────
#[cfg(feature = "visualizer")]
fn agent_color(kind: &AgentKind) -> egui::Color32 {
    match kind {
        AgentKind::Queen    => egui::Color32::from_rgb(255, 220, 0),
        AgentKind::Nurse    => egui::Color32::from_rgb(50,  200, 50),
        AgentKind::Builder  => egui::Color32::from_rgb(60,  120, 255),
        AgentKind::Guard    => egui::Color32::from_rgb(220, 50,  50),
        AgentKind::Forager  => egui::Color32::from_rgb(255, 140, 0),
        AgentKind::Drone    => egui::Color32::from_rgb(160, 160, 160),
        AgentKind::Predator => egui::Color32::from_rgb(220, 0,   220),
    }
}

#[cfg(feature = "visualizer")]
fn format_tick_time(tick: u64) -> String {
    let total_s = (tick as f64 * 0.5) as u64;
    format!("{:02}:{:02}:{:02}", total_s / 3600, (total_s % 3600) / 60, total_s % 60)
}

/// Barra horizontal de rol con etiqueta + contador + fill proporcional.
#[cfg(feature = "visualizer")]
fn draw_role_bar(ui: &mut egui::Ui, label: &str, count: u32, max: u32, color: egui::Color32) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(label).size(10.0).color(TEXT_SEC));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(count.to_string()).size(10.0).color(TEXT_PRI).monospace());
        });
    });
    let (bar, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 5.0), egui::Sense::hover());
    let p = ui.painter_at(bar);
    p.rect_filled(bar, 2.0, SURFACE_3);
    if max > 0 && count > 0 {
        let fw = bar.width() * (count as f32 / max as f32).min(1.0);
        p.rect_filled(egui::Rect::from_min_size(bar.min, egui::vec2(fw, bar.height())), 2.0, color);
    }
    ui.add_space(5.0);
}

/// Barra de reserva con porcentaje (0.0–1.0).
#[cfg(feature = "visualizer")]
fn draw_reserve_bar(ui: &mut egui::Ui, label: &str, fraction: f32, color: egui::Color32) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(label).size(10.0).color(TEXT_SEC));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(format!("{:.0}%", fraction * 100.0))
                .size(10.0).color(TEXT_PRI).monospace());
        });
    });
    let (bar, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 7.0), egui::Sense::hover());
    let p = ui.painter_at(bar);
    p.rect_filled(bar, 3.0, SURFACE_3);
    if fraction > 0.0 {
        let fw = bar.width() * fraction.clamp(0.0, 1.0);
        p.rect_filled(egui::Rect::from_min_size(bar.min, egui::vec2(fw, bar.height())), 3.0, color);
    }
    ui.add_space(6.0);
}

/// Arco de donut (aproximado con quads convexos).
#[cfg(feature = "visualizer")]
fn draw_donut_arc(
    painter: &egui::Painter,
    center: egui::Pos2,
    r_inner: f32, r_outer: f32,
    start_a: f32, end_a: f32,
    color: egui::Color32,
) {
    if (end_a - start_a).abs() < 1e-4 { return; }
    let steps = 36usize;
    for i in 0..steps {
        let a0 = start_a + (end_a - start_a) * i       as f32 / steps as f32;
        let a1 = start_a + (end_a - start_a) * (i + 1) as f32 / steps as f32;
        let pts = vec![
            center + egui::Vec2::new(a0.cos() * r_inner, a0.sin() * r_inner),
            center + egui::Vec2::new(a0.cos() * r_outer, a0.sin() * r_outer),
            center + egui::Vec2::new(a1.cos() * r_outer, a1.sin() * r_outer),
            center + egui::Vec2::new(a1.cos() * r_inner, a1.sin() * r_inner),
        ];
        painter.add(egui::Shape::convex_polygon(pts, color, egui::Stroke::NONE));
    }
}

/// Pequeño punto de color + etiqueta para leyendas.
#[cfg(feature = "visualizer")]
fn color_chip(ui: &mut egui::Ui, color: egui::Color32, label: &str) {
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
        ui.painter_at(r).circle_filled(r.center(), 4.0, color);
        ui.label(egui::RichText::new(label).size(10.0).color(TEXT_SEC));
    });
}

/// Fila de estadística: etiqueta izquierda, valor derecha.
#[cfg(feature = "visualizer")]
fn stat_row(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(label).size(11.0).color(TEXT_SEC));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(value).size(11.0).color(TEXT_PRI).monospace());
        });
    });
    ui.add_space(2.0);
}

/// Tendencia como % de cambio entre últimos 5 vs 6-15 puntos.
#[cfg(feature = "visualizer")]
fn compute_trend(series: &[f32]) -> f32 {
    if series.len() < 10 { return 0.0; }
    let recent = series.iter().rev().take(5).sum::<f32>() / 5.0;
    let older  = series.iter().rev().skip(5).take(10).sum::<f32>() / 10.0;
    if older.abs() < 1e-6 { return 0.0; }
    (recent - older) / older * 100.0
}

/// Sparkline como polilínea sobre el painter del área dada.
#[cfg(feature = "visualizer")]
fn draw_sparkline(painter: &egui::Painter, rect: egui::Rect, data: &[f32], color: egui::Color32) {
    if data.len() < 2 { return; }
    let min = data.iter().cloned().fold(f32::INFINITY,     f32::min);
    let max = data.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let range = (max - min).max(1e-6);
    let n = data.len();
    let points: Vec<egui::Pos2> = data.iter().enumerate().map(|(i, &v)| {
        egui::pos2(
            rect.left() + i as f32 / (n - 1) as f32 * rect.width(),
            rect.bottom() - (v - min) / range * rect.height(),
        )
    }).collect();
    painter.add(egui::Shape::line(points, egui::Stroke::new(1.5, color)));
}

/// Tarjeta de métrica: valor grande + tendencia + etiqueta + sparkline.
#[cfg(feature = "visualizer")]
fn draw_metric_card(
    ui:     &mut egui::Ui,
    label:  &str,
    value:  &str,
    trend:  f32,
    series: &[f32],
    color:  egui::Color32,
    size:   egui::Vec2,
) {
    let (card, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let p = ui.painter_at(card);
    p.rect_filled(card, 6.0, SURFACE_2);

    // Valor
    p.text(egui::pos2(card.left() + 10.0, card.top() + 8.0),
        egui::Align2::LEFT_TOP, value, egui::FontId::monospace(15.0), TEXT_PRI);

    // Tendencia
    let (tsym, tcol) = if trend > 1.5 { ("↑", RED_M) }
                       else if trend < -1.5 { ("↓", GREEN_M) }
                       else { ("→", TEXT_SEC) };
    p.text(egui::pos2(card.right() - 8.0, card.top() + 8.0),
        egui::Align2::RIGHT_TOP, tsym, egui::FontId::proportional(12.0), tcol);

    // Etiqueta
    p.text(egui::pos2(card.left() + 10.0, card.top() + 28.0),
        egui::Align2::LEFT_TOP, label, egui::FontId::proportional(9.0), TEXT_SEC);

    // Sparkline
    let spark = egui::Rect::from_min_max(
        egui::pos2(card.left() + 6.0,  card.top() + 42.0),
        egui::pos2(card.right() - 6.0, card.bottom() - 4.0),
    );
    if spark.height() > 4.0 {
        draw_sparkline(&p, spark, series, color);
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
        tx.try_send(dummy_frame(0)).expect("primer send debe tener éxito");
        let result = tx.try_send(dummy_frame(1));
        assert!(result.is_err(), "try_send con canal lleno debe retornar Err");
    }
}
