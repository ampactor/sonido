//! Main application state and UI layout.

use crate::audio_bridge::{AudioBridge, EffectOrder, MeteringData, SharedParams};
use crate::chain_manager::ChainManager;
use crate::chain_view::ChainView;
use crate::effects_ui::{
    ChorusPanel, CompressorPanel, DelayPanel, DistortionPanel, EffectType, FilterPanel,
    FlangerPanel, GatePanel, MultiVibratoPanel, ParametricEqPanel, PhaserPanel, PreampPanel,
    ReverbPanel, TapePanel, TremoloPanel, WahPanel,
};
use crate::preset_manager::PresetManager;
use crate::theme::Theme;
use crate::widgets::{Knob, LevelMeter};
use crossbeam_channel::Sender;
use egui::{CentralPanel, Color32, Context, Frame, Margin, TopBottomPanel};
use sonido_registry::EffectRegistry;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Instant;

/// Default effect chain order — matches the slot indices used by SharedParams.
const DEFAULT_CHAIN: &[&str] = &[
    "preamp",       // 0
    "distortion",   // 1
    "compressor",   // 2
    "gate",         // 3
    "eq",           // 4
    "wah",          // 5
    "chorus",       // 6
    "flanger",      // 7
    "phaser",       // 8
    "tremolo",      // 9
    "delay",        // 10
    "filter",       // 11
    "multivibrato", // 12
    "tape",         // 13
    "reverb",       // 14
];

/// Audio processing thread state.
struct AudioThread {
    handle: Option<thread::JoinHandle<()>>,
    running: Arc<AtomicBool>,
}

/// Main application state.
pub struct SonidoApp {
    // Audio
    audio_bridge: AudioBridge,
    audio_thread: Option<AudioThread>,
    metering: MeteringData,

    // UI
    theme: Theme,
    chain_view: ChainView,
    preset_manager: PresetManager,

    // Effect panels
    preamp_panel: PreampPanel,
    distortion_panel: DistortionPanel,
    compressor_panel: CompressorPanel,
    gate_panel: GatePanel,
    eq_panel: ParametricEqPanel,
    wah_panel: WahPanel,
    chorus_panel: ChorusPanel,
    flanger_panel: FlangerPanel,
    phaser_panel: PhaserPanel,
    tremolo_panel: TremoloPanel,
    delay_panel: DelayPanel,
    filter_panel: FilterPanel,
    multivibrato_panel: MultiVibratoPanel,
    tape_panel: TapePanel,
    reverb_panel: ReverbPanel,

    // Status
    sample_rate: f32,
    buffer_size: usize,
    cpu_usage: f32,
    audio_error: Option<String>,

    // Preset dialog
    show_save_dialog: bool,
    new_preset_name: String,
    new_preset_description: String,
}

impl SonidoApp {
    /// Create a new application instance.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut app = Self {
            audio_bridge: AudioBridge::new(),
            audio_thread: None,
            metering: MeteringData::default(),
            theme: Theme::default(),
            chain_view: ChainView::new(),
            preset_manager: PresetManager::new(),
            preamp_panel: PreampPanel::new(),
            distortion_panel: DistortionPanel::new(),
            compressor_panel: CompressorPanel::new(),
            gate_panel: GatePanel::new(),
            eq_panel: ParametricEqPanel::new(),
            wah_panel: WahPanel::new(),
            chorus_panel: ChorusPanel::new(),
            flanger_panel: FlangerPanel::new(),
            phaser_panel: PhaserPanel::new(),
            tremolo_panel: TremoloPanel::new(),
            delay_panel: DelayPanel::new(),
            filter_panel: FilterPanel::new(),
            multivibrato_panel: MultiVibratoPanel::new(),
            tape_panel: TapePanel::new(),
            reverb_panel: ReverbPanel::new(),
            sample_rate: 48000.0,
            buffer_size: 512,
            cpu_usage: 0.0,
            audio_error: None,
            show_save_dialog: false,
            new_preset_name: String::new(),
            new_preset_description: "User".to_string(),
        };

        // Apply theme
        app.theme.apply(&cc.egui_ctx);

        // Load initial preset - select first preset which applies it to params
        if !app.preset_manager.presets().is_empty() {
            app.preset_manager.select(0, &app.audio_bridge.params);
        }

        // Start audio
        if let Err(e) = app.start_audio() {
            app.audio_error = Some(e);
        }

        app
    }

    /// Start the audio processing thread.
    fn start_audio(&mut self) -> Result<(), String> {
        let params = self.audio_bridge.params();
        let running = self.audio_bridge.running();
        let metering_tx = self.audio_bridge.metering_sender();
        let effect_order = self.chain_view.effect_order().clone();

        running.store(true, Ordering::SeqCst);

        let sample_rate = self.sample_rate;
        let buffer_size = self.buffer_size;

        let handle = thread::spawn(move || {
            if let Err(e) = run_audio_thread(
                params,
                running.clone(),
                metering_tx,
                effect_order,
                sample_rate,
                buffer_size,
            ) {
                log::error!("Audio thread error: {}", e);
            }
            running.store(false, Ordering::SeqCst);
        });

        self.audio_thread = Some(AudioThread {
            handle: Some(handle),
            running: self.audio_bridge.running(),
        });

        Ok(())
    }

    /// Stop the audio processing thread.
    fn stop_audio(&mut self) {
        if let Some(ref audio) = self.audio_thread {
            audio.running.store(false, Ordering::SeqCst);
        }
        if let Some(mut audio) = self.audio_thread.take()
            && let Some(handle) = audio.handle.take()
        {
            let _ = handle.join();
        }
    }

    /// Render the header/toolbar.
    fn render_header(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading(
                egui::RichText::new("SONIDO")
                    .color(Color32::from_rgb(100, 180, 255))
                    .strong(),
            );

            ui.add_space(20.0);

            // Preset selector
            let current_name = self
                .preset_manager
                .current()
                .map(|p| p.preset.name.as_str())
                .unwrap_or("Init");
            let display_name = if self.preset_manager.is_modified() {
                format!("{}*", current_name)
            } else {
                current_name.to_string()
            };

            // Collect preset names to avoid borrow issues
            let preset_names: Vec<(usize, String)> = self
                .preset_manager
                .presets()
                .iter()
                .enumerate()
                .map(|(i, p)| (i, p.preset.name.clone()))
                .collect();
            let current_idx = self.preset_manager.current_preset();

            let mut selected_preset = None;
            egui::ComboBox::from_id_salt("preset_selector")
                .selected_text(&display_name)
                .width(150.0)
                .show_ui(ui, |ui| {
                    for (i, name) in &preset_names {
                        if ui.selectable_label(*i == current_idx, name).clicked() {
                            selected_preset = Some(*i);
                        }
                    }
                });

            // Apply preset selection after borrow ends
            if let Some(idx) = selected_preset {
                self.preset_manager.select(idx, &self.audio_bridge.params);
            }

            if ui.button("Save").clicked() {
                self.show_save_dialog = true;
                self.new_preset_name = self
                    .preset_manager
                    .current()
                    .map(|p| p.preset.name.clone())
                    .unwrap_or_default();
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Audio status indicator
                let status_color = if self.audio_bridge.is_running() {
                    Color32::from_rgb(80, 200, 80)
                } else {
                    Color32::from_rgb(200, 80, 80)
                };
                ui.label(egui::RichText::new("●").color(status_color).size(12.0));

                if let Some(ref error) = self.audio_error {
                    ui.label(
                        egui::RichText::new(error)
                            .color(Color32::from_rgb(220, 100, 100))
                            .small(),
                    );
                }
            });
        });
    }

    /// Render the I/O section with meters and gain controls.
    fn render_io_section(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            // Input section
            ui.group(|ui| {
                ui.set_min_width(80.0);
                ui.vertical_centered(|ui| {
                    ui.label(
                        egui::RichText::new("INPUT")
                            .small()
                            .color(Color32::from_rgb(150, 150, 160)),
                    );

                    ui.add_space(4.0);

                    // Input meter
                    ui.add(
                        LevelMeter::new(self.metering.input_peak, self.metering.input_rms)
                            .size(24.0, 100.0),
                    );

                    ui.add_space(8.0);

                    // Input gain knob
                    let mut input_gain = self.audio_bridge.params.input_gain.get();
                    if ui
                        .add(
                            Knob::new(&mut input_gain, -20.0, 20.0, "GAIN")
                                .default(0.0)
                                .format_db()
                                .diameter(50.0),
                        )
                        .changed()
                    {
                        self.audio_bridge.params.input_gain.set(input_gain);
                        self.preset_manager.mark_modified();
                    }
                });
            });
        });
    }

    /// Render the output section.
    fn render_output_section(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            ui.group(|ui| {
                ui.set_min_width(80.0);
                ui.vertical_centered(|ui| {
                    ui.label(
                        egui::RichText::new("OUTPUT")
                            .small()
                            .color(Color32::from_rgb(150, 150, 160)),
                    );

                    ui.add_space(4.0);

                    // Output meter
                    ui.add(
                        LevelMeter::new(self.metering.output_peak, self.metering.output_rms)
                            .size(24.0, 100.0),
                    );

                    ui.add_space(8.0);

                    // Master volume knob
                    let mut master_vol = self.audio_bridge.params.master_volume.get();
                    if ui
                        .add(
                            Knob::new(&mut master_vol, -40.0, 6.0, "MASTER")
                                .default(0.0)
                                .format_db()
                                .diameter(50.0),
                        )
                        .changed()
                    {
                        self.audio_bridge.params.master_volume.set(master_vol);
                        self.preset_manager.mark_modified();
                    }
                });
            });
        });
    }

    /// Render the effect panel for the selected effect.
    fn render_effect_panel(&mut self, ui: &mut egui::Ui, effect_type: EffectType) {
        let panel_frame = Frame::new()
            .fill(Color32::from_rgb(40, 40, 48))
            .corner_radius(8.0)
            .inner_margin(Margin::same(16));

        panel_frame.show(ui, |ui| {
            ui.set_min_height(160.0);

            ui.vertical(|ui| {
                // Panel title
                ui.heading(
                    egui::RichText::new(effect_type.name()).color(Color32::from_rgb(100, 180, 255)),
                );
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(12.0);

                // Effect-specific controls
                match effect_type {
                    EffectType::Preamp => self.preamp_panel.ui(ui, &self.audio_bridge.params),
                    EffectType::Distortion => {
                        self.distortion_panel.ui(ui, &self.audio_bridge.params)
                    }
                    EffectType::Compressor => {
                        self.compressor_panel.ui(ui, &self.audio_bridge.params)
                    }
                    EffectType::Gate => self.gate_panel.ui(ui, &self.audio_bridge.params),
                    EffectType::ParametricEq => self.eq_panel.ui(ui, &self.audio_bridge.params),
                    EffectType::Wah => self.wah_panel.ui(ui, &self.audio_bridge.params),
                    EffectType::Chorus => self.chorus_panel.ui(ui, &self.audio_bridge.params),
                    EffectType::Flanger => self.flanger_panel.ui(ui, &self.audio_bridge.params),
                    EffectType::Phaser => self.phaser_panel.ui(ui, &self.audio_bridge.params),
                    EffectType::Tremolo => self.tremolo_panel.ui(ui, &self.audio_bridge.params),
                    EffectType::Delay => self.delay_panel.ui(ui, &self.audio_bridge.params),
                    EffectType::Filter => self.filter_panel.ui(ui, &self.audio_bridge.params),
                    EffectType::MultiVibrato => {
                        self.multivibrato_panel.ui(ui, &self.audio_bridge.params)
                    }
                    EffectType::Tape => self.tape_panel.ui(ui, &self.audio_bridge.params),
                    EffectType::Reverb => self.reverb_panel.ui(ui, &self.audio_bridge.params),
                }
            });
        });
    }

    /// Render the status bar.
    fn render_status_bar(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(format!("{:.0} Hz", self.sample_rate));
            ui.separator();
            ui.label(format!("{} samples", self.buffer_size));
            ui.separator();
            let latency_ms = self.buffer_size as f32 / self.sample_rate * 1000.0;
            ui.label(format!("{:.1} ms", latency_ms));
            ui.separator();
            ui.label(format!("CPU: {:.1}%", self.cpu_usage));
        });
    }

    /// Render save preset dialog.
    fn render_save_dialog(&mut self, ctx: &Context) {
        if !self.show_save_dialog {
            return;
        }

        egui::Window::new("Save Preset")
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Name:");
                    ui.text_edit_singleline(&mut self.new_preset_name);
                });

                ui.horizontal(|ui| {
                    ui.label("Description:");
                    ui.text_edit_singleline(&mut self.new_preset_description);
                });

                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        self.show_save_dialog = false;
                    }

                    if ui.button("Save").clicked() && !self.new_preset_name.is_empty() {
                        let description = if self.new_preset_description.is_empty() {
                            None
                        } else {
                            Some(self.new_preset_description.as_str())
                        };
                        if let Err(e) = self.preset_manager.save_as(
                            &self.new_preset_name,
                            description,
                            &self.audio_bridge.params,
                        ) {
                            log::error!("Failed to save preset: {}", e);
                        }
                        self.show_save_dialog = false;
                    }
                });
            });
    }
}

impl eframe::App for SonidoApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        // Update metering data
        if let Some(data) = self.audio_bridge.receive_metering() {
            self.cpu_usage = data.cpu_usage;
            self.metering = data;
        }

        // Request continuous repaint for metering
        ctx.request_repaint();

        // Header
        TopBottomPanel::top("header").show(ctx, |ui| {
            ui.add_space(4.0);
            self.render_header(ui);
            ui.add_space(4.0);
        });

        // Status bar
        TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.add_space(2.0);
            self.render_status_bar(ui);
            ui.add_space(2.0);
        });

        // Main content
        CentralPanel::default().show(ctx, |ui| {
            ui.add_space(8.0);

            // Main horizontal layout: Input | Chain + Effect | Output
            ui.horizontal(|ui| {
                // Input section
                self.render_io_section(ui);

                ui.add_space(16.0);

                // Center section (chain + effect panel)
                ui.vertical(|ui| {
                    // Effect chain strip
                    ui.group(|ui| {
                        ui.set_min_width(500.0);
                        ui.vertical_centered(|ui| {
                            ui.label(
                                egui::RichText::new("EFFECT CHAIN")
                                    .small()
                                    .color(Color32::from_rgb(150, 150, 160)),
                            );
                            ui.add_space(4.0);
                            self.chain_view.ui(ui, &self.audio_bridge.params);
                        });
                    });

                    ui.add_space(16.0);

                    // Selected effect panel
                    if let Some(selected) = self.chain_view.selected() {
                        self.render_effect_panel(ui, selected);
                    }
                });

                ui.add_space(16.0);

                // Output section
                self.render_output_section(ui);
            });
        });

        // Dialogs
        self.render_save_dialog(ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.stop_audio();
    }
}

/// Sync SharedParams values to ChainManager effects via `set_param()`.
///
/// SharedParams stores some values in internal representation (0-1 for fractions)
/// while `set_param()` expects user-facing values (0-100 for percent). This function
/// handles the conversion. Temporary — will be eliminated when SharedParams is replaced
/// by `AtomicParamBridge` in Phase 3.
#[allow(clippy::too_many_lines)]
fn sync_shared_params_to_chain(params: &SharedParams, chain: &mut ChainManager) {
    // Helper: fraction (0-1) stored in SharedParams → percent (0-100) for set_param
    let pct = |v: f32| v * 100.0;

    // Slot 0: Preamp — gain (dB), output, headroom all in dB (direct)
    if let Some(slot) = chain.slot_mut(0) {
        slot.effect.effect_set_param(0, params.preamp_gain.get());
    }

    // Slot 1: Distortion — drive/tone/level in dB/Hz, waveshape as enum index
    if let Some(slot) = chain.slot_mut(1) {
        slot.effect.effect_set_param(0, params.dist_drive.get());
        slot.effect.effect_set_param(1, params.dist_tone.get());
        slot.effect.effect_set_param(2, params.dist_level.get());
        slot.effect
            .effect_set_param(3, params.dist_waveshape.load(Ordering::Relaxed) as f32);
    }

    // Slot 2: Compressor — threshold/makeup in dB, ratio direct, attack/release in ms
    if let Some(slot) = chain.slot_mut(2) {
        slot.effect.effect_set_param(0, params.comp_threshold.get());
        slot.effect.effect_set_param(1, params.comp_ratio.get());
        slot.effect.effect_set_param(2, params.comp_attack.get());
        slot.effect.effect_set_param(3, params.comp_release.get());
        slot.effect.effect_set_param(4, params.comp_makeup.get());
    }

    // Slot 3: Gate — threshold in dB, attack/release/hold in ms
    if let Some(slot) = chain.slot_mut(3) {
        slot.effect.effect_set_param(0, params.gate_threshold.get());
        slot.effect.effect_set_param(1, params.gate_attack.get());
        slot.effect.effect_set_param(2, params.gate_release.get());
        slot.effect.effect_set_param(3, params.gate_hold.get());
    }

    // Slot 4: EQ — freq in Hz, gain in dB, Q direct
    if let Some(slot) = chain.slot_mut(4) {
        slot.effect.effect_set_param(0, params.eq_low_freq.get());
        slot.effect.effect_set_param(1, params.eq_low_gain.get());
        slot.effect.effect_set_param(2, params.eq_low_q.get());
        slot.effect.effect_set_param(3, params.eq_mid_freq.get());
        slot.effect.effect_set_param(4, params.eq_mid_gain.get());
        slot.effect.effect_set_param(5, params.eq_mid_q.get());
        slot.effect.effect_set_param(6, params.eq_high_freq.get());
        slot.effect.effect_set_param(7, params.eq_high_gain.get());
        slot.effect.effect_set_param(8, params.eq_high_q.get());
    }

    // Slot 5: Wah — freq in Hz, resonance direct, sensitivity 0-1→0-100, mode as enum
    if let Some(slot) = chain.slot_mut(5) {
        slot.effect.effect_set_param(0, params.wah_frequency.get());
        slot.effect.effect_set_param(1, params.wah_resonance.get());
        slot.effect
            .effect_set_param(2, pct(params.wah_sensitivity.get()));
        slot.effect
            .effect_set_param(3, params.wah_mode.load(Ordering::Relaxed) as f32);
    }

    // Slot 6: Chorus — rate in Hz, depth/mix 0-1→0-100
    if let Some(slot) = chain.slot_mut(6) {
        slot.effect.effect_set_param(0, params.chorus_rate.get());
        slot.effect
            .effect_set_param(1, pct(params.chorus_depth.get()));
        slot.effect
            .effect_set_param(2, pct(params.chorus_mix.get()));
    }

    // Slot 7: Flanger — rate in Hz, depth/feedback/mix 0-1→0-100
    if let Some(slot) = chain.slot_mut(7) {
        slot.effect.effect_set_param(0, params.flanger_rate.get());
        slot.effect
            .effect_set_param(1, pct(params.flanger_depth.get()));
        slot.effect
            .effect_set_param(2, pct(params.flanger_feedback.get()));
        slot.effect
            .effect_set_param(3, pct(params.flanger_mix.get()));
    }

    // Slot 8: Phaser — rate in Hz, depth/feedback/mix 0-1→0-100, stages as int
    if let Some(slot) = chain.slot_mut(8) {
        slot.effect.effect_set_param(0, params.phaser_rate.get());
        slot.effect
            .effect_set_param(1, pct(params.phaser_depth.get()));
        slot.effect
            .effect_set_param(2, params.phaser_stages.load(Ordering::Relaxed) as f32);
        slot.effect
            .effect_set_param(3, pct(params.phaser_feedback.get()));
        slot.effect
            .effect_set_param(4, pct(params.phaser_mix.get()));
    }

    // Slot 9: Tremolo — rate in Hz, depth 0-1→0-100, waveform as enum
    if let Some(slot) = chain.slot_mut(9) {
        slot.effect.effect_set_param(0, params.tremolo_rate.get());
        slot.effect
            .effect_set_param(1, pct(params.tremolo_depth.get()));
        slot.effect
            .effect_set_param(2, params.tremolo_waveform.load(Ordering::Relaxed) as f32);
    }

    // Slot 10: Delay — time in ms, feedback/mix 0-1→0-100
    if let Some(slot) = chain.slot_mut(10) {
        slot.effect.effect_set_param(0, params.delay_time.get());
        slot.effect
            .effect_set_param(1, pct(params.delay_feedback.get()));
        slot.effect.effect_set_param(2, pct(params.delay_mix.get()));
    }

    // Slot 11: Filter — cutoff in Hz, resonance (Q) direct
    if let Some(slot) = chain.slot_mut(11) {
        slot.effect.effect_set_param(0, params.filter_cutoff.get());
        slot.effect
            .effect_set_param(1, params.filter_resonance.get());
    }

    // Slot 12: MultiVibrato — depth 0-1→0-100
    if let Some(slot) = chain.slot_mut(12) {
        slot.effect
            .effect_set_param(0, pct(params.vibrato_depth.get()));
    }

    // Slot 13: Tape Saturation — drive in dB, saturation 0-1→0-100
    if let Some(slot) = chain.slot_mut(13) {
        slot.effect.effect_set_param(0, params.tape_drive.get());
        slot.effect
            .effect_set_param(1, pct(params.tape_saturation.get()));
    }

    // Slot 14: Reverb — room_size/decay/damping/mix 0-1→0-100, predelay in ms, type as enum
    if let Some(slot) = chain.slot_mut(14) {
        slot.effect
            .effect_set_param(0, pct(params.reverb_room_size.get()));
        slot.effect
            .effect_set_param(1, pct(params.reverb_decay.get()));
        slot.effect
            .effect_set_param(2, pct(params.reverb_damping.get()));
        slot.effect
            .effect_set_param(3, params.reverb_predelay.get());
        slot.effect
            .effect_set_param(4, pct(params.reverb_mix.get()));
        // Params 5 (stereo width) and 6 (reverb type) not in SharedParams — use defaults
    }

    // Sync bypass states
    chain.set_bypassed(0, params.bypass.preamp.load(Ordering::Relaxed));
    chain.set_bypassed(1, params.bypass.distortion.load(Ordering::Relaxed));
    chain.set_bypassed(2, params.bypass.compressor.load(Ordering::Relaxed));
    chain.set_bypassed(3, params.bypass.gate.load(Ordering::Relaxed));
    chain.set_bypassed(4, params.bypass.eq.load(Ordering::Relaxed));
    chain.set_bypassed(5, params.bypass.wah.load(Ordering::Relaxed));
    chain.set_bypassed(6, params.bypass.chorus.load(Ordering::Relaxed));
    chain.set_bypassed(7, params.bypass.flanger.load(Ordering::Relaxed));
    chain.set_bypassed(8, params.bypass.phaser.load(Ordering::Relaxed));
    chain.set_bypassed(9, params.bypass.tremolo.load(Ordering::Relaxed));
    chain.set_bypassed(10, params.bypass.delay.load(Ordering::Relaxed));
    chain.set_bypassed(11, params.bypass.filter.load(Ordering::Relaxed));
    chain.set_bypassed(12, params.bypass.multivibrato.load(Ordering::Relaxed));
    chain.set_bypassed(13, params.bypass.tape.load(Ordering::Relaxed));
    chain.set_bypassed(14, params.bypass.reverb.load(Ordering::Relaxed));
}

/// Run the audio processing thread.
///
/// Processes audio in stereo through the effect chain, respecting the user-defined
/// effect order. Uses [`ChainManager`] for registry-driven effect creation and
/// dispatch. Measures CPU usage and reports metering data to the GUI.
#[allow(clippy::too_many_arguments)]
fn run_audio_thread(
    params: Arc<SharedParams>,
    running: Arc<AtomicBool>,
    metering_tx: Sender<MeteringData>,
    effect_order: EffectOrder,
    sample_rate: f32,
    buffer_size: usize,
) -> Result<(), String> {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

    let host = cpal::default_host();
    let input_device = host
        .default_input_device()
        .ok_or("No input device available")?;
    let output_device = host
        .default_output_device()
        .ok_or("No output device available")?;

    // Use stereo config; fall back to device default channel count
    let output_channels = output_device
        .default_output_config()
        .map(|c| c.channels())
        .unwrap_or(2);
    let input_channels = input_device
        .default_input_config()
        .map(|c| c.channels())
        .unwrap_or(1);

    let output_config = cpal::StreamConfig {
        channels: output_channels,
        sample_rate: cpal::SampleRate(sample_rate as u32),
        buffer_size: cpal::BufferSize::Fixed(buffer_size as u32),
    };
    let input_config = cpal::StreamConfig {
        channels: input_channels,
        sample_rate: cpal::SampleRate(sample_rate as u32),
        buffer_size: cpal::BufferSize::Fixed(buffer_size as u32),
    };

    // Create effect chain from registry
    let registry = EffectRegistry::new();
    let mut chain = ChainManager::new(&registry, DEFAULT_CHAIN, sample_rate);

    // Stereo audio buffer (interleaved L, R pairs)
    let (tx, rx) = crossbeam_channel::bounded::<f32>(16384);

    // Pre-fill with silence (stereo frames)
    for _ in 0..(1024 * input_channels as usize) {
        let _ = tx.try_send(0.0);
    }

    let running_input = Arc::clone(&running);

    // Input stream - forward all samples (mono or stereo)
    let input_stream = input_device
        .build_input_stream(
            &input_config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                if !running_input.load(Ordering::Relaxed) {
                    return;
                }
                for &sample in data {
                    let _ = tx.try_send(sample);
                }
            },
            |err| log::error!("Input stream error: {}", err),
            None,
        )
        .map_err(|e| format!("Failed to build input stream: {}", e))?;

    let params_output = Arc::clone(&params);
    let running_output = Arc::clone(&running);
    let in_ch = input_channels as usize;
    let out_ch = output_channels as usize;
    let buffer_time_secs = buffer_size as f64 / sample_rate as f64;

    // Output stream - process and output in stereo
    let output_stream = output_device
        .build_output_stream(
            &output_config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                if !running_output.load(Ordering::Relaxed) {
                    data.fill(0.0);
                    return;
                }

                let process_start = Instant::now();

                // Global gain levels
                let input_gain_db = params_output.input_gain.get();
                let master_vol_db = params_output.master_volume.get();
                let input_gain = 10.0_f32.powf(input_gain_db / 20.0);
                let master_vol = 10.0_f32.powf(master_vol_db / 20.0);

                // Sync SharedParams → effect parameters and bypass states
                sync_shared_params_to_chain(&params_output, &mut chain);

                // Sync effect order from GUI
                let order = effect_order.get();
                chain.reorder(order);

                let mut input_peak = 0.0_f32;
                let mut input_rms_sum = 0.0_f32;
                let mut output_peak = 0.0_f32;
                let mut output_rms_sum = 0.0_f32;

                let frames = data.len() / out_ch;

                for i in 0..frames {
                    // Deinterleave input (mono: duplicate, stereo: take L/R)
                    let (in_l, in_r) = if in_ch >= 2 {
                        let l = rx.try_recv().unwrap_or(0.0);
                        let r = rx.try_recv().unwrap_or(0.0);
                        for _ in 2..in_ch {
                            let _ = rx.try_recv();
                        }
                        (l, r)
                    } else {
                        let s = rx.try_recv().unwrap_or(0.0);
                        (s, s)
                    };

                    let mut l = in_l * input_gain;
                    let mut r = in_r * input_gain;

                    let mono_in = (l + r) * 0.5;
                    input_peak = input_peak.max(mono_in.abs());
                    input_rms_sum += mono_in * mono_in;

                    // Process through effect chain (order + bypass handled by ChainManager)
                    (l, r) = chain.process_stereo(l, r);

                    // Apply master volume
                    l *= master_vol;
                    r *= master_vol;

                    let mono_out = (l + r) * 0.5;
                    output_peak = output_peak.max(mono_out.abs());
                    output_rms_sum += mono_out * mono_out;

                    // Interleave output
                    let idx = i * out_ch;
                    match out_ch {
                        1 => data[idx] = (l + r) * 0.5,
                        2 => {
                            data[idx] = l;
                            data[idx + 1] = r;
                        }
                        _ => {
                            data[idx] = l;
                            data[idx + 1] = r;
                            for c in 2..out_ch {
                                data[idx + c] = 0.0;
                            }
                        }
                    }
                }

                // CPU usage measurement
                let elapsed = process_start.elapsed().as_secs_f64();
                let cpu_pct = (elapsed / buffer_time_secs * 100.0) as f32;

                // Send metering data (non-blocking)
                let count = frames.max(1) as f32;
                let _ = metering_tx.try_send(MeteringData {
                    input_peak,
                    input_rms: (input_rms_sum / count).sqrt(),
                    output_peak,
                    output_rms: (output_rms_sum / count).sqrt(),
                    // TODO: gain reduction requires metering trait on dyn EffectWithParams
                    gain_reduction: 0.0,
                    cpu_usage: cpu_pct,
                });
            },
            |err| log::error!("Output stream error: {}", err),
            None,
        )
        .map_err(|e| format!("Failed to build output stream: {}", e))?;

    input_stream
        .play()
        .map_err(|e| format!("Failed to play input stream: {}", e))?;
    output_stream
        .play()
        .map_err(|e| format!("Failed to play output stream: {}", e))?;

    // Keep thread alive while running
    while running.load(Ordering::Relaxed) {
        thread::sleep(std::time::Duration::from_millis(100));
    }

    Ok(())
}
