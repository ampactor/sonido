//! Main application state and UI layout.

use crate::audio_bridge::{AudioBridge, MeteringData, SharedParams};
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
use sonido_core::Effect;
use sonido_effects::{
    Chorus, CleanPreamp, Compressor, Delay, Distortion, Flanger, Gate, LowPassFilter, MultiVibrato,
    ParametricEq, Phaser, Reverb, TapeSaturation, Tremolo, TremoloWaveform, Wah, WaveShape,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

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

        running.store(true, Ordering::SeqCst);

        let sample_rate = self.sample_rate;

        let handle = thread::spawn(move || {
            if let Err(e) = run_audio_thread(params, running.clone(), metering_tx, sample_rate) {
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

/// Run the audio processing thread.
fn run_audio_thread(
    params: Arc<SharedParams>,
    running: Arc<AtomicBool>,
    metering_tx: Sender<MeteringData>,
    sample_rate: f32,
) -> Result<(), String> {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

    let host = cpal::default_host();
    let input_device = host
        .default_input_device()
        .ok_or("No input device available")?;
    let output_device = host
        .default_output_device()
        .ok_or("No output device available")?;

    let config = cpal::StreamConfig {
        channels: 1,
        sample_rate: cpal::SampleRate(sample_rate as u32),
        buffer_size: cpal::BufferSize::Fixed(512),
    };

    // Create effects
    let mut preamp = CleanPreamp::new(sample_rate);
    let mut distortion = Distortion::new(sample_rate);
    let mut compressor = Compressor::new(sample_rate);
    let mut gate = Gate::new(sample_rate);
    let mut eq = ParametricEq::new(sample_rate);
    let mut wah = Wah::new(sample_rate);
    let mut chorus = Chorus::new(sample_rate);
    let mut flanger = Flanger::new(sample_rate);
    let mut phaser = Phaser::new(sample_rate);
    let mut tremolo = Tremolo::new(sample_rate);
    let mut delay = Delay::new(sample_rate);
    let mut filter = LowPassFilter::new(sample_rate);
    let mut vibrato = MultiVibrato::new(sample_rate);
    let mut tape = TapeSaturation::new(sample_rate);
    let mut reverb = Reverb::new(sample_rate);

    // Audio buffer for communication between input and output
    // Use larger buffer to absorb timing variations between streams
    let (tx, rx) = crossbeam_channel::bounded::<f32>(8192);

    // Pre-fill buffer with silence to prevent initial underruns
    for _ in 0..1024 {
        let _ = tx.try_send(0.0);
    }

    let running_input = Arc::clone(&running);

    // Input stream - just forward samples
    let input_stream = input_device
        .build_input_stream(
            &config,
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

    // Output stream - process and output
    let output_stream = output_device
        .build_output_stream(
            &config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                if !running_output.load(Ordering::Relaxed) {
                    data.fill(0.0);
                    return;
                }

                // Update effect parameters from shared params
                let input_gain_db = params_output.input_gain.get();
                let master_vol_db = params_output.master_volume.get();
                let input_gain = 10.0_f32.powf(input_gain_db / 20.0);
                let master_vol = 10.0_f32.powf(master_vol_db / 20.0);

                // Update effect parameters
                preamp.set_gain_db(params_output.preamp_gain.get());

                distortion.set_drive_db(params_output.dist_drive.get());
                distortion.set_tone_hz(params_output.dist_tone.get());
                distortion.set_level_db(params_output.dist_level.get());
                let ws = params_output.dist_waveshape.load(Ordering::Relaxed);
                distortion.set_waveshape(match ws {
                    0 => WaveShape::SoftClip,
                    1 => WaveShape::HardClip,
                    2 => WaveShape::Foldback,
                    _ => WaveShape::Asymmetric,
                });

                compressor.set_threshold_db(params_output.comp_threshold.get());
                compressor.set_ratio(params_output.comp_ratio.get());
                compressor.set_attack_ms(params_output.comp_attack.get());
                compressor.set_release_ms(params_output.comp_release.get());
                compressor.set_makeup_gain_db(params_output.comp_makeup.get());

                gate.set_threshold_db(params_output.gate_threshold.get());
                gate.set_attack_ms(params_output.gate_attack.get());
                gate.set_release_ms(params_output.gate_release.get());
                gate.set_hold_ms(params_output.gate_hold.get());

                eq.set_low_freq(params_output.eq_low_freq.get());
                eq.set_low_gain(params_output.eq_low_gain.get());
                eq.set_low_q(params_output.eq_low_q.get());
                eq.set_mid_freq(params_output.eq_mid_freq.get());
                eq.set_mid_gain(params_output.eq_mid_gain.get());
                eq.set_mid_q(params_output.eq_mid_q.get());
                eq.set_high_freq(params_output.eq_high_freq.get());
                eq.set_high_gain(params_output.eq_high_gain.get());
                eq.set_high_q(params_output.eq_high_q.get());

                wah.set_frequency(params_output.wah_frequency.get());
                wah.set_resonance(params_output.wah_resonance.get());
                wah.set_sensitivity(params_output.wah_sensitivity.get());
                wah.set_mode_index(params_output.wah_mode.load(Ordering::Relaxed) as usize);

                chorus.set_rate(params_output.chorus_rate.get());
                chorus.set_depth(params_output.chorus_depth.get());
                chorus.set_mix(params_output.chorus_mix.get());

                flanger.set_rate(params_output.flanger_rate.get());
                flanger.set_depth(params_output.flanger_depth.get());
                flanger.set_feedback(params_output.flanger_feedback.get());
                flanger.set_mix(params_output.flanger_mix.get());

                phaser.set_rate(params_output.phaser_rate.get());
                phaser.set_depth(params_output.phaser_depth.get());
                phaser.set_feedback(params_output.phaser_feedback.get());
                phaser.set_mix(params_output.phaser_mix.get());
                phaser.set_stages(params_output.phaser_stages.load(Ordering::Relaxed) as usize);

                tremolo.set_rate(params_output.tremolo_rate.get());
                tremolo.set_depth(params_output.tremolo_depth.get());
                let trem_wave = params_output.tremolo_waveform.load(Ordering::Relaxed);
                tremolo.set_waveform(match trem_wave {
                    0 => TremoloWaveform::Sine,
                    1 => TremoloWaveform::Triangle,
                    2 => TremoloWaveform::Square,
                    _ => TremoloWaveform::SampleHold,
                });

                delay.set_delay_time_ms(params_output.delay_time.get());
                delay.set_feedback(params_output.delay_feedback.get());
                delay.set_mix(params_output.delay_mix.get());

                filter.set_cutoff_hz(params_output.filter_cutoff.get());
                filter.set_q(params_output.filter_resonance.get());

                vibrato.set_depth(params_output.vibrato_depth.get());

                // Convert tape drive from dB to linear
                let tape_drive_linear = 10.0_f32.powf(params_output.tape_drive.get() / 20.0);
                tape.set_drive(tape_drive_linear);
                tape.set_saturation(params_output.tape_saturation.get());

                reverb.set_room_size(params_output.reverb_room_size.get());
                reverb.set_decay(params_output.reverb_decay.get());
                reverb.set_damping(params_output.reverb_damping.get());
                reverb.set_predelay_ms(params_output.reverb_predelay.get());
                reverb.set_mix(params_output.reverb_mix.get());

                // Cache bypass states to avoid per-sample atomic loads
                let bypass_preamp = params_output.bypass.preamp.load(Ordering::Relaxed);
                let bypass_distortion = params_output.bypass.distortion.load(Ordering::Relaxed);
                let bypass_compressor = params_output.bypass.compressor.load(Ordering::Relaxed);
                let bypass_gate = params_output.bypass.gate.load(Ordering::Relaxed);
                let bypass_eq = params_output.bypass.eq.load(Ordering::Relaxed);
                let bypass_wah = params_output.bypass.wah.load(Ordering::Relaxed);
                let bypass_chorus = params_output.bypass.chorus.load(Ordering::Relaxed);
                let bypass_flanger = params_output.bypass.flanger.load(Ordering::Relaxed);
                let bypass_phaser = params_output.bypass.phaser.load(Ordering::Relaxed);
                let bypass_tremolo = params_output.bypass.tremolo.load(Ordering::Relaxed);
                let bypass_delay = params_output.bypass.delay.load(Ordering::Relaxed);
                let bypass_filter = params_output.bypass.filter.load(Ordering::Relaxed);
                let bypass_vibrato = params_output.bypass.multivibrato.load(Ordering::Relaxed);
                let bypass_tape = params_output.bypass.tape.load(Ordering::Relaxed);
                let bypass_reverb = params_output.bypass.reverb.load(Ordering::Relaxed);

                let mut input_peak = 0.0_f32;
                let mut input_rms_sum = 0.0_f32;
                let mut output_peak = 0.0_f32;
                let mut output_rms_sum = 0.0_f32;

                for sample in data.iter_mut() {
                    // Get input sample
                    let input = rx.try_recv().unwrap_or(0.0) * input_gain;
                    input_peak = input_peak.max(input.abs());
                    input_rms_sum += input * input;

                    // Process through effect chain (using cached bypass states)
                    let mut out = input;

                    if !bypass_preamp {
                        out = preamp.process(out);
                    }
                    if !bypass_distortion {
                        out = distortion.process(out);
                    }
                    if !bypass_compressor {
                        out = compressor.process(out);
                    }
                    if !bypass_gate {
                        out = gate.process(out);
                    }
                    if !bypass_eq {
                        out = eq.process(out);
                    }
                    if !bypass_wah {
                        out = wah.process(out);
                    }
                    if !bypass_chorus {
                        out = chorus.process(out);
                    }
                    if !bypass_flanger {
                        out = flanger.process(out);
                    }
                    if !bypass_phaser {
                        out = phaser.process(out);
                    }
                    if !bypass_tremolo {
                        out = tremolo.process(out);
                    }
                    if !bypass_delay {
                        out = delay.process(out);
                    }
                    if !bypass_filter {
                        out = filter.process(out);
                    }
                    if !bypass_vibrato {
                        out = vibrato.process(out);
                    }
                    if !bypass_tape {
                        out = tape.process(out);
                    }
                    if !bypass_reverb {
                        out = reverb.process(out);
                    }

                    // Apply master volume
                    out *= master_vol;

                    output_peak = output_peak.max(out.abs());
                    output_rms_sum += out * out;

                    *sample = out;
                }

                // Send metering data (non-blocking)
                let count = data.len().max(1) as f32;
                let _ = metering_tx.try_send(MeteringData {
                    input_peak,
                    input_rms: (input_rms_sum / count).sqrt(),
                    output_peak,
                    output_rms: (output_rms_sum / count).sqrt(),
                    gain_reduction: compressor.gain_reduction_db(),
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
