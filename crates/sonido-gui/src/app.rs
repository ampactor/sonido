//! Main application state and UI layout.

use crate::atomic_param_bridge::AtomicParamBridge;
use crate::audio_bridge::{AudioBridge, EffectOrder, MeteringData};
use crate::chain_manager::{ChainCommand, ChainManager};
use crate::chain_view::ChainView;
use crate::file_player::FilePlayer;
use crate::preset_manager::PresetManager;
use crate::theme::Theme;
use crate::widgets::{Knob, LevelMeter};
use crossbeam_channel::{Receiver, Sender};
use egui::{
    Align, CentralPanel, Color32, Context, Frame, Layout, Margin, Rect, TopBottomPanel, UiBuilder,
    pos2, vec2,
};
use sonido_gui_core::effects_ui;
use sonido_gui_core::{ParamBridge, SlotIndex};
use sonido_registry::EffectRegistry;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(not(target_arch = "wasm32"))]
use std::time::{Duration, Instant};
#[cfg(target_arch = "wasm32")]
use web_time::Instant;

/// Default effect chain order — matches the slot indices used by `AtomicParamBridge`.
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

/// Main application state.
pub struct SonidoApp {
    // Audio
    audio_bridge: AudioBridge,
    /// Live cpal streams -- dropped to stop audio.
    _audio_streams: Vec<cpal::Stream>,
    /// Whether we've re-called play() after a user gesture (wasm autoplay policy).
    #[cfg(target_arch = "wasm32")]
    audio_resumed: bool,
    metering: MeteringData,

    /// Registry-driven parameter bridge (GUI ↔ audio thread).
    bridge: Arc<AtomicParamBridge>,

    /// Effect registry for creating new effects.
    registry: Arc<EffectRegistry>,

    // UI
    theme: Theme,
    chain_view: ChainView,
    file_player: FilePlayer,
    preset_manager: PresetManager,

    /// Cached effect panel: (slot, effect_id, panel).
    /// Avoids reconstructing the panel widget every frame.
    cached_panel: Option<(
        sonido_gui_core::SlotIndex,
        String,
        Box<dyn effects_ui::EffectPanel>,
    )>,

    // Status
    sample_rate: f32,
    buffer_size: usize,
    cpu_usage: f32,
    audio_error: Option<String>,

    /// When set, the app runs in single-effect mode (no chain view).
    single_effect: bool,

    // Preset dialog (native only — no filesystem on wasm)
    #[cfg(not(target_arch = "wasm32"))]
    show_save_dialog: bool,
    #[cfg(not(target_arch = "wasm32"))]
    new_preset_name: String,
    #[cfg(not(target_arch = "wasm32"))]
    new_preset_description: String,
}

impl SonidoApp {
    /// Create a new application instance.
    ///
    /// If `effect` is `Some("name")`, launches in single-effect mode with a
    /// simplified UI showing only that effect (no chain view, no presets).
    pub fn new(cc: &eframe::CreationContext<'_>, effect: Option<&str>) -> Self {
        let registry = Arc::new(EffectRegistry::new());

        let single_effect = effect.is_some();
        let chain: &[&str] = if let Some(name) = effect {
            // Validate that the effect exists in the registry
            assert!(
                registry.create(name, 48000.0).is_some(),
                "Unknown effect: {name}. Available: {:?}",
                registry
                    .all_effects()
                    .iter()
                    .map(|e| e.id)
                    .collect::<Vec<_>>()
            );
            // Leak into 'static — lives for the process lifetime (app runs once)
            let leaked: &'static str = Box::leak(name.to_owned().into_boxed_str());
            Box::leak(vec![leaked].into_boxed_slice())
        } else {
            DEFAULT_CHAIN
        };
        let bridge = Arc::new(AtomicParamBridge::new(&registry, chain, 48000.0));

        if !single_effect {
            // Effects that start bypassed (full chain mode only)
            bridge.set_default_bypass(SlotIndex(3), true); // gate
            bridge.set_default_bypass(SlotIndex(4), true); // eq
            bridge.set_default_bypass(SlotIndex(5), true); // wah
            bridge.set_default_bypass(SlotIndex(7), true); // flanger
            bridge.set_default_bypass(SlotIndex(8), true); // phaser
            bridge.set_default_bypass(SlotIndex(9), true); // tremolo
        }

        let audio_bridge = AudioBridge::new();
        let transport_tx = audio_bridge.transport_sender();

        let mut chain_view = ChainView::new();
        if single_effect {
            chain_view.set_selected(SlotIndex(0));
        }

        let mut app = Self {
            audio_bridge,
            _audio_streams: Vec::new(),
            #[cfg(target_arch = "wasm32")]
            audio_resumed: false,
            metering: MeteringData::default(),
            bridge,
            registry,
            theme: Theme::default(),
            chain_view,
            file_player: FilePlayer::new(transport_tx),
            preset_manager: PresetManager::new(),
            cached_panel: None,
            sample_rate: 48000.0,
            buffer_size: 512,
            cpu_usage: 0.0,
            audio_error: None,
            single_effect,
            #[cfg(not(target_arch = "wasm32"))]
            show_save_dialog: false,
            #[cfg(not(target_arch = "wasm32"))]
            new_preset_name: String::new(),
            #[cfg(not(target_arch = "wasm32"))]
            new_preset_description: "User".to_string(),
        };

        // Apply theme
        app.theme.apply(&cc.egui_ctx);

        // Load initial preset — select first preset which applies it to bridge
        if !app.preset_manager.presets().is_empty() {
            app.preset_manager.select(0, &*app.bridge);
        }

        // Start audio
        if let Err(e) = app.start_audio() {
            app.audio_error = Some(e);
        }

        app
    }

    /// Build cpal streams and start audio processing.
    ///
    /// Streams are stored in `_audio_streams` and stay alive until dropped.
    /// Works identically on native and wasm -- cpal handles threading internally.
    fn start_audio(&mut self) -> Result<(), String> {
        // Query actual device sample rate for GUI display and effect init
        {
            use cpal::traits::{DeviceTrait, HostTrait};
            if let Some(device) = cpal::default_host().default_output_device()
                && let Ok(config) = device.default_output_config()
            {
                self.sample_rate = config.sample_rate() as f32;
            }
        }

        let bridge = Arc::clone(&self.bridge);
        let registry = Arc::clone(&self.registry);
        let input_gain = self.audio_bridge.input_gain();
        let master_volume = self.audio_bridge.master_volume();
        let running = self.audio_bridge.running();
        let metering_tx = self.audio_bridge.metering_sender();
        let effect_order = self.chain_view.effect_order().clone();
        let command_rx = self.audio_bridge.command_receiver();
        let transport_rx = self.audio_bridge.transport_receiver();

        running.store(true, Ordering::SeqCst);

        let streams = build_audio_streams(
            bridge,
            &registry,
            input_gain,
            master_volume,
            running,
            metering_tx,
            effect_order,
            command_rx,
            transport_rx,
            self.sample_rate,
            self.buffer_size,
        )?;

        self._audio_streams = streams;
        Ok(())
    }

    /// Stop audio by dropping stream handles.
    fn stop_audio(&mut self) {
        self.audio_bridge.running().store(false, Ordering::SeqCst);
        self._audio_streams.clear();
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
                self.preset_manager.select(idx, &*self.bridge);
            }

            ui.add_space(8.0);
            self.file_player.render_source_toggle(ui);

            // Save button (native only — no filesystem on wasm)
            #[cfg(not(target_arch = "wasm32"))]
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

                let mut retry = false;
                if let Some(ref error) = self.audio_error {
                    ui.label(
                        egui::RichText::new(error)
                            .color(Color32::from_rgb(220, 100, 100))
                            .small(),
                    );
                    retry = ui.small_button("Retry").clicked();
                }
                if retry {
                    self.stop_audio();
                    match self.start_audio() {
                        Ok(()) => self.audio_error = None,
                        Err(e) => self.audio_error = Some(e),
                    }
                }
            });
        });
    }

    /// Render the I/O section with meters and gain controls.
    fn render_io_section(&mut self, ui: &mut egui::Ui) {
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
                let input_gain = self.audio_bridge.input_gain();
                let mut gain_val = input_gain.get();
                if ui
                    .add(
                        Knob::new(&mut gain_val, -20.0, 20.0, "GAIN")
                            .default(0.0)
                            .format_db()
                            .diameter(50.0),
                    )
                    .changed()
                {
                    input_gain.set(gain_val);
                    self.preset_manager.mark_modified();
                }
            });
        });
    }

    /// Render the output section.
    fn render_output_section(&mut self, ui: &mut egui::Ui) {
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
                let master_vol_param = self.audio_bridge.master_volume();
                let mut master_val = master_vol_param.get();
                if ui
                    .add(
                        Knob::new(&mut master_val, -40.0, 6.0, "MASTER")
                            .default(0.0)
                            .format_db()
                            .diameter(50.0),
                    )
                    .changed()
                {
                    master_vol_param.set(master_val);
                    self.preset_manager.mark_modified();
                }
            });
        });
    }

    /// Render the effect panel for the selected slot.
    ///
    /// The panel widget is cached in `self.cached_panel` and only reconstructed
    /// when the selected slot or effect type changes.
    fn render_effect_panel(&mut self, ui: &mut egui::Ui, slot: sonido_gui_core::SlotIndex) {
        let effect_id = self.bridge.effect_id(slot);
        let panel_name = self
            .registry
            .descriptor(effect_id)
            .map(|d| d.name)
            .unwrap_or("Unknown");

        // Populate cache if the slot or effect type changed
        let cache_hit = self
            .cached_panel
            .as_ref()
            .is_some_and(|(s, id, _)| *s == slot && id == effect_id);
        if !cache_hit {
            self.cached_panel =
                effects_ui::create_panel(effect_id).map(|p| (slot, effect_id.to_owned(), p));
        }

        let panel_frame = Frame::new()
            .fill(Color32::from_rgb(40, 40, 48))
            .corner_radius(8.0)
            .inner_margin(Margin::same(16));

        panel_frame.show(ui, |ui| {
            ui.set_min_height(160.0);

            ui.vertical(|ui| {
                // Panel title
                ui.heading(egui::RichText::new(panel_name).color(Color32::from_rgb(100, 180, 255)));
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(12.0);

                // Effect-specific controls from cache
                if let Some((_, _, ref mut panel)) = self.cached_panel {
                    let bridge: &dyn ParamBridge = &*self.bridge;
                    panel.ui(ui, bridge, slot);
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

    /// Render save preset dialog (native only — no filesystem on wasm).
    #[cfg(not(target_arch = "wasm32"))]
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
                            &*self.bridge,
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
            self.file_player.set_position(data.playback_position_secs);
            self.metering = data;
        }

        // Handle pending add/remove from chain view
        if let Some(id) = self.chain_view.take_pending_add()
            && let Some(effect) = self.registry.create(id, self.sample_rate)
        {
            self.audio_bridge
                .send_command(ChainCommand::Add { id, effect });
        }
        if let Some(slot) = self.chain_view.take_pending_remove() {
            if self.chain_view.selected() == Some(slot) {
                self.chain_view.clear_selection();
                self.cached_panel = None;
            }
            self.audio_bridge
                .send_command(ChainCommand::Remove { slot });
        }

        // Resume audio on first user gesture (wasm autoplay policy).
        // Browsers suspend AudioContext until a trusted user interaction.
        // Re-calling play() from within the user-activation window resumes it.
        #[cfg(target_arch = "wasm32")]
        if !self.audio_resumed && ctx.input(|i| i.pointer.any_pressed() || !i.events.is_empty()) {
            use cpal::traits::StreamTrait;
            for stream in &self._audio_streams {
                let _ = stream.play();
            }
            self.audio_resumed = true;
        }

        // Request continuous repaint for metering
        #[cfg(target_arch = "wasm32")]
        ctx.request_repaint_after(std::time::Duration::from_millis(33)); // 30fps
        #[cfg(not(target_arch = "wasm32"))]
        ctx.request_repaint_after(Duration::from_millis(16)); // ~60fps cap

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

        // File player bar (above status bar when file input active)
        if self.file_player.use_file_input() {
            TopBottomPanel::bottom("file_player").show(ctx, |ui| {
                ui.add_space(2.0);
                self.file_player.ui(ui);
                ui.add_space(2.0);
            });
        }

        // Main content
        CentralPanel::default().show(ctx, |ui| {
            #[cfg(target_arch = "wasm32")]
            if !self.audio_resumed {
                log::debug!(
                    "wasm layout: available={:.0}x{:.0} ppp={:.2}",
                    ui.available_width(),
                    ui.available_height(),
                    ctx.pixels_per_point()
                );
            }

            ui.add_space(8.0);

            // Main layout: INPUT (100px) | 16px gap | CENTER (flex) | 16px gap | OUTPUT (100px)
            // Manual rect splitting avoids the vertical_centered-inside-horizontal width bug.
            let avail = ui.available_rect_before_wrap();
            let io_width = 100.0;
            let gap = 16.0;
            let center_width = (avail.width() - 2.0 * io_width - 2.0 * gap).max(200.0);

            let input_rect = Rect::from_min_size(avail.min, vec2(io_width, avail.height()));
            let center_rect = Rect::from_min_size(
                pos2(avail.min.x + io_width + gap, avail.min.y),
                vec2(center_width, avail.height()),
            );
            let output_rect = Rect::from_min_size(
                pos2(
                    avail.min.x + io_width + gap + center_width + gap,
                    avail.min.y,
                ),
                vec2(io_width, avail.height()),
            );

            // Input column
            {
                let mut child = ui.new_child(
                    UiBuilder::new()
                        .id_salt("input_col")
                        .max_rect(input_rect)
                        .layout(Layout::top_down(Align::Center)),
                );
                self.render_io_section(&mut child);
            }

            // Center column (chain strip + effect panel)
            {
                let mut child = ui.new_child(
                    UiBuilder::new()
                        .id_salt("center_col")
                        .max_rect(center_rect)
                        .layout(Layout::top_down(Align::LEFT)),
                );

                if self.single_effect {
                    // Single-effect mode: show only the effect panel, no chain strip
                    self.render_effect_panel(&mut child, SlotIndex(0));
                } else {
                    // Full chain mode: chain strip + selected effect panel
                    child.group(|ui| {
                        ui.vertical_centered(|ui| {
                            ui.label(
                                egui::RichText::new("EFFECT CHAIN")
                                    .small()
                                    .color(Color32::from_rgb(150, 150, 160)),
                            );
                            ui.add_space(4.0);
                            self.chain_view.ui(ui, &*self.bridge, &self.registry);
                        });
                    });

                    child.add_space(16.0);

                    if let Some(slot) = self.chain_view.selected() {
                        self.render_effect_panel(&mut child, slot);
                    }
                }
            }

            // Output column
            {
                let mut child = ui.new_child(
                    UiBuilder::new()
                        .id_salt("output_col")
                        .max_rect(output_rect)
                        .layout(Layout::top_down(Align::Center)),
                );
                self.render_output_section(&mut child);
            }

            // Advance parent cursor past all three columns
            ui.advance_cursor_after_rect(Rect::from_min_max(
                avail.min,
                pos2(
                    avail.min.x + io_width + gap + center_width + gap + io_width,
                    avail.max.y,
                ),
            ));
        });

        // Dialogs (save dialog is native-only)
        #[cfg(not(target_arch = "wasm32"))]
        self.render_save_dialog(ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.stop_audio();
    }
}

/// File playback state owned by [`AudioProcessor`].
struct FilePlayback {
    left: Vec<f32>,
    right: Vec<f32>,
    position: usize,
    file_sample_rate: f32,
    playing: bool,
    looping: bool,
    file_mode: bool,
}

impl FilePlayback {
    fn new() -> Self {
        Self {
            left: Vec::new(),
            right: Vec::new(),
            position: 0,
            file_sample_rate: 48000.0,
            playing: false,
            looping: false,
            file_mode: false,
        }
    }

    /// Read the next stereo frame from the file buffer, advancing position.
    fn next_frame(&mut self) -> (f32, f32) {
        if self.left.is_empty() || !self.playing {
            return (0.0, 0.0);
        }
        if self.position >= self.left.len() {
            if self.looping {
                self.position = 0;
            } else {
                self.playing = false;
                self.position = 0;
                return (0.0, 0.0);
            }
        }
        let l = self.left[self.position];
        let r = self.right[self.position];
        self.position += 1;
        (l, r)
    }

    /// Current playback position in seconds.
    fn position_secs(&self) -> f32 {
        if self.file_sample_rate > 0.0 {
            self.position as f32 / self.file_sample_rate
        } else {
            0.0
        }
    }
}

/// All state needed by the audio output callback.
///
/// Constructed inside [`build_audio_streams`] and moved into the cpal output
/// closure. Encapsulates effect chain processing, file playback, parameter
/// sync, gain staging, and metering -- everything the callback previously
/// captured as loose variables.
struct AudioProcessor {
    chain: ChainManager,
    bridge: Arc<AtomicParamBridge>,
    effect_order: EffectOrder,
    /// Cached copy of the effect order; only refreshed when `effect_order` is dirty.
    cached_order: Vec<usize>,
    input_gain: Arc<crate::audio_bridge::AtomicParam>,
    master_volume: Arc<crate::audio_bridge::AtomicParam>,
    command_rx: Receiver<ChainCommand>,
    transport_rx: Receiver<crate::file_player::TransportCommand>,
    metering_tx: Sender<MeteringData>,
    /// Receiver for mic input samples from the input stream.
    input_rx: Receiver<f32>,
    file_pb: FilePlayback,
    out_ch: usize,
    in_ch: usize,
    buffer_time_secs: f64,
}

impl AudioProcessor {
    /// Process one output buffer: drain commands, sync params, run effects,
    /// apply gain, write interleaved output, send metering.
    fn process_buffer(&mut self, data: &mut [f32]) {
        use crate::file_player::TransportCommand;

        let process_start = Instant::now();

        // Drain transport commands
        while let Ok(cmd) = self.transport_rx.try_recv() {
            match cmd {
                TransportCommand::LoadFile {
                    left,
                    right,
                    sample_rate: sr,
                } => {
                    self.file_pb.left = left;
                    self.file_pb.right = right;
                    self.file_pb.file_sample_rate = sr;
                    self.file_pb.position = 0;
                    self.file_pb.playing = false;
                }
                TransportCommand::UnloadFile => {
                    self.file_pb.left.clear();
                    self.file_pb.right.clear();
                    self.file_pb.position = 0;
                    self.file_pb.playing = false;
                }
                TransportCommand::Play => self.file_pb.playing = true,
                TransportCommand::Pause => self.file_pb.playing = false,
                TransportCommand::Stop => {
                    self.file_pb.playing = false;
                    self.file_pb.position = 0;
                }
                TransportCommand::Seek(secs) => {
                    self.file_pb.position = (secs * self.file_pb.file_sample_rate) as usize;
                    if self.file_pb.position >= self.file_pb.left.len() {
                        self.file_pb.position = self.file_pb.left.len().saturating_sub(1);
                    }
                }
                TransportCommand::SetLoop(v) => self.file_pb.looping = v,
                TransportCommand::SetFileMode(v) => self.file_pb.file_mode = v,
            }
        }

        // Drain dynamic chain commands (transactional add/remove)
        while let Ok(cmd) = self.command_rx.try_recv() {
            match cmd {
                ChainCommand::Add { id, effect } => {
                    let count = effect.effect_param_count();
                    let descriptors: Vec<_> = (0..count)
                        .filter_map(|i| effect.effect_param_info(i))
                        .collect();
                    self.chain.add_transactional(
                        id,
                        effect,
                        &self.bridge,
                        &self.effect_order,
                        descriptors,
                    );
                }
                ChainCommand::Remove { slot } => {
                    self.chain
                        .remove_transactional(slot, &self.bridge, &self.effect_order);
                }
            }
        }

        // Global gain levels
        let ig = sonido_core::db_to_linear(self.input_gain.get());
        let mv = sonido_core::db_to_linear(self.master_volume.get());

        // Sync bridge -> effect parameters and bypass states
        self.bridge.sync_to_chain(&mut self.chain);

        // Sync effect order from GUI (only when changed)
        if self.effect_order.is_dirty() {
            self.cached_order.clone_from(&self.effect_order.get());
            self.chain.reorder(self.cached_order.clone());
            self.effect_order.clear_dirty();
        }

        let mut input_peak = 0.0_f32;
        let mut input_rms_sum = 0.0_f32;
        let mut output_peak = 0.0_f32;
        let mut output_rms_sum = 0.0_f32;

        let frames = data.len() / self.out_ch;
        let use_file = self.file_pb.file_mode && !self.file_pb.left.is_empty();

        for i in 0..frames {
            let (in_l, in_r) = if use_file {
                // Drain mic input to keep the ring buffer from overflowing
                for _ in 0..self.in_ch {
                    let _ = self.input_rx.try_recv();
                }
                self.file_pb.next_frame()
            } else {
                // Deinterleave mic input (mono: duplicate, stereo: take L/R)
                if self.in_ch >= 2 {
                    let l = self.input_rx.try_recv().unwrap_or(0.0);
                    let r = self.input_rx.try_recv().unwrap_or(0.0);
                    for _ in 2..self.in_ch {
                        let _ = self.input_rx.try_recv();
                    }
                    (l, r)
                } else {
                    let s = self.input_rx.try_recv().unwrap_or(0.0);
                    (s, s)
                }
            };

            let mut l = in_l * ig;
            let mut r = in_r * ig;

            let mono_in = (l + r) * 0.5;
            input_peak = input_peak.max(mono_in.abs());
            input_rms_sum += mono_in * mono_in;

            // Process through effect chain (order + bypass handled by ChainManager)
            (l, r) = self.chain.process_stereo(l, r);

            // Apply master volume
            l *= mv;
            r *= mv;

            let mono_out = (l + r) * 0.5;
            output_peak = output_peak.max(mono_out.abs());
            output_rms_sum += mono_out * mono_out;

            // Interleave output
            let idx = i * self.out_ch;
            match self.out_ch {
                1 => data[idx] = (l + r) * 0.5,
                2 => {
                    data[idx] = l;
                    data[idx + 1] = r;
                }
                _ => {
                    data[idx] = l;
                    data[idx + 1] = r;
                    for c in 2..self.out_ch {
                        data[idx + c] = 0.0;
                    }
                }
            }
        }

        // CPU usage measurement
        let elapsed = process_start.elapsed().as_secs_f64();
        let cpu_pct = (elapsed / self.buffer_time_secs * 100.0) as f32;

        // Send metering data (non-blocking)
        let count = frames.max(1) as f32;
        let _ = self.metering_tx.try_send(MeteringData {
            input_peak,
            input_rms: (input_rms_sum / count).sqrt(),
            output_peak,
            output_rms: (output_rms_sum / count).sqrt(),
            gain_reduction: 0.0,
            cpu_usage: cpu_pct,
            playback_position_secs: self.file_pb.position_secs(),
        });
    }
}

/// Build and start cpal audio streams.
///
/// Creates an output stream (always) and an input stream (if a mic is available).
/// Returns the stream handles -- caller must keep them alive for audio to continue.
/// Input is optional so the app works without mic permission (e.g., wasm, headless).
#[allow(clippy::too_many_arguments)]
fn build_audio_streams(
    bridge: Arc<AtomicParamBridge>,
    registry: &EffectRegistry,
    input_gain: Arc<crate::audio_bridge::AtomicParam>,
    master_volume: Arc<crate::audio_bridge::AtomicParam>,
    running: Arc<AtomicBool>,
    metering_tx: Sender<MeteringData>,
    effect_order: EffectOrder,
    command_rx: Receiver<ChainCommand>,
    transport_rx: Receiver<crate::file_player::TransportCommand>,
    sample_rate: f32,
    buffer_size: usize,
) -> Result<Vec<cpal::Stream>, String> {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

    let host = cpal::default_host();
    let output_device = host
        .default_output_device()
        .ok_or("No output device available")?;

    // Input device is optional (mic permission may be denied on wasm)
    let input_device = host.default_input_device();

    // Use device's actual sample rate; fall back to passed-in value on error
    let (output_channels, sample_rate) = match output_device.default_output_config() {
        Ok(config) => (config.channels(), config.sample_rate() as f32),
        Err(_) => (2, sample_rate),
    };

    let output_config = cpal::StreamConfig {
        channels: output_channels,
        sample_rate: sample_rate as u32,
        buffer_size: cpal::BufferSize::Fixed(buffer_size as u32),
    };

    // Create effect chain from shared registry
    let chain = ChainManager::new(registry, DEFAULT_CHAIN, sample_rate);

    // Stereo audio buffer (interleaved L, R pairs)
    let (tx, rx) = crossbeam_channel::bounded::<f32>(16384);

    let mut streams: Vec<cpal::Stream> = Vec::with_capacity(2);

    // Input stream (if mic available)
    let in_ch = if let Some(ref input_dev) = input_device {
        let input_channels = input_dev
            .default_input_config()
            .map(|c| c.channels())
            .unwrap_or(1);

        let input_config = cpal::StreamConfig {
            channels: input_channels,
            sample_rate: sample_rate as u32,
            buffer_size: cpal::BufferSize::Fixed(buffer_size as u32),
        };

        // Pre-fill with silence
        for _ in 0..(1024 * input_channels as usize) {
            let _ = tx.try_send(0.0);
        }

        let running_input = Arc::clone(&running);
        let input_stream = input_dev
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

        input_stream
            .play()
            .map_err(|e| format!("Failed to play input stream: {}", e))?;
        streams.push(input_stream);

        input_channels as usize
    } else {
        log::warn!("No input device available -- mic input disabled");
        // Pre-fill silence so output callback doesn't block
        for _ in 0..2048 {
            let _ = tx.try_send(0.0);
        }
        1 // default: mono input channel count for deinterleave logic
    };

    let running_output = Arc::clone(&running);
    let out_ch = output_channels as usize;
    let buffer_time_secs = buffer_size as f64 / sample_rate as f64;

    let mut processor = AudioProcessor {
        chain,
        bridge,
        effect_order,
        cached_order: Vec::new(),
        input_gain,
        master_volume,
        command_rx,
        transport_rx,
        metering_tx,
        input_rx: rx,
        file_pb: FilePlayback::new(),
        out_ch,
        in_ch,
        buffer_time_secs,
    };

    // Output stream -- delegates to AudioProcessor
    let output_stream = output_device
        .build_output_stream(
            &output_config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                if !running_output.load(Ordering::Relaxed) {
                    data.fill(0.0);
                    return;
                }
                processor.process_buffer(data);
            },
            |err| log::error!("Output stream error: {}", err),
            None,
        )
        .map_err(|e| format!("Failed to build output stream: {}", e))?;

    output_stream
        .play()
        .map_err(|e| format!("Failed to play output stream: {}", e))?;
    streams.push(output_stream);

    Ok(streams)
}
