//! WAV file player for testing effects without a microphone.
//!
//! [`FilePlayer`] manages file loading, transport controls, and UI rendering.
//! Audio data is sent to the audio thread via [`TransportCommand`]s through
//! a crossbeam channel. Playback position flows back through `MeteringData`.

use crossbeam_channel::Sender;
use egui::{Color32, Ui};
use sonido_io::{read_wav_info, read_wav_stereo};
use std::path::PathBuf;

/// Commands sent from GUI thread to audio thread for file playback.
pub enum TransportCommand {
    /// Load a stereo file into the audio thread's playback buffer.
    LoadFile {
        /// Left channel samples.
        left: Vec<f32>,
        /// Right channel samples.
        right: Vec<f32>,
        /// File sample rate in Hz.
        sample_rate: f32,
    },
    /// Remove the loaded file.
    UnloadFile,
    /// Start or resume playback.
    Play,
    /// Pause playback (retains position).
    Pause,
    /// Stop playback and reset position to zero.
    Stop,
    /// Seek to a position in seconds.
    Seek(f32),
    /// Enable or disable loop mode.
    SetLoop(bool),
    /// Switch between file input (`true`) and mic input (`false`).
    SetFileMode(bool),
}

/// GUI-side file player state and controls.
///
/// Renders transport buttons, a position scrubber, and file info.
/// Communicates with the audio thread exclusively through commands.
pub struct FilePlayer {
    transport_tx: Sender<TransportCommand>,
    file_name: String,
    file_path: Option<PathBuf>,
    duration_secs: f32,
    position_secs: f32,
    is_playing: bool,
    is_looping: bool,
    use_file_input: bool,
    sample_rate: f32,
}

impl FilePlayer {
    /// Create a new file player with the given transport command sender.
    pub fn new(transport_tx: Sender<TransportCommand>) -> Self {
        Self {
            transport_tx,
            file_name: String::new(),
            file_path: None,
            duration_secs: 0.0,
            position_secs: 0.0,
            is_playing: false,
            is_looping: false,
            use_file_input: false,
            sample_rate: 48000.0,
        }
    }

    /// Update playback position from metering data (called each frame).
    pub fn set_position(&mut self, position_secs: f32) {
        self.position_secs = position_secs;
        // Detect playback stop (position reset by audio thread)
        if self.is_playing && position_secs <= 0.0 && self.duration_secs > 0.0 && !self.is_looping {
            self.is_playing = false;
        }
    }

    /// Whether file input mode is active.
    pub fn use_file_input(&self) -> bool {
        self.use_file_input
    }

    /// Load a WAV file from disk.
    fn load_file(&mut self, path: PathBuf) {
        // Read metadata first for duration
        let info = match read_wav_info(&path) {
            Ok(info) => info,
            Err(e) => {
                log::error!("Failed to read WAV info: {e}");
                return;
            }
        };

        // Load stereo samples
        let (samples, _spec) = match read_wav_stereo(&path) {
            Ok(s) => s,
            Err(e) => {
                log::error!("Failed to load WAV: {e}");
                return;
            }
        };

        self.file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        self.duration_secs = info.duration_secs as f32;
        self.sample_rate = info.sample_rate as f32;
        self.position_secs = 0.0;
        self.is_playing = false;
        self.file_path = Some(path);

        let _ = self.transport_tx.send(TransportCommand::LoadFile {
            left: samples.left,
            right: samples.right,
            sample_rate: self.sample_rate,
        });
    }

    /// Render the input source toggle (Mic / File) for the header bar.
    pub fn render_source_toggle(&mut self, ui: &mut Ui) {
        let label = if self.use_file_input { "File" } else { "Mic" };
        if ui
            .selectable_label(
                self.use_file_input,
                egui::RichText::new(label).small().strong(),
            )
            .clicked()
        {
            self.use_file_input = !self.use_file_input;
            let _ = self
                .transport_tx
                .send(TransportCommand::SetFileMode(self.use_file_input));
        }
    }

    /// Render the file player panel (bottom bar).
    pub fn ui(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            // Browse button
            if ui.button("Open").clicked()
                && let Some(path) = rfd::FileDialog::new()
                    .add_filter("WAV", &["wav"])
                    .pick_file()
            {
                self.load_file(path);
            }

            // File name display
            if self.file_name.is_empty() {
                ui.label(
                    egui::RichText::new("No file loaded")
                        .color(Color32::from_rgb(120, 120, 130))
                        .italics(),
                );
            } else {
                ui.label(
                    egui::RichText::new(&self.file_name).color(Color32::from_rgb(180, 180, 190)),
                );
            }

            ui.add_space(12.0);

            // Transport controls (only if file loaded)
            if self.file_path.is_some() {
                // Play / Pause
                let play_label = if self.is_playing { "||" } else { ">" };
                if ui.button(play_label).clicked() {
                    if self.is_playing {
                        self.is_playing = false;
                        let _ = self.transport_tx.send(TransportCommand::Pause);
                    } else {
                        self.is_playing = true;
                        let _ = self.transport_tx.send(TransportCommand::Play);
                    }
                }

                // Stop
                if ui.button("[]").clicked() {
                    self.is_playing = false;
                    self.position_secs = 0.0;
                    let _ = self.transport_tx.send(TransportCommand::Stop);
                }

                // Loop toggle
                let loop_color = if self.is_looping {
                    Color32::from_rgb(100, 200, 100)
                } else {
                    Color32::from_rgb(120, 120, 130)
                };
                if ui
                    .button(egui::RichText::new("L").color(loop_color))
                    .clicked()
                {
                    self.is_looping = !self.is_looping;
                    let _ = self
                        .transport_tx
                        .send(TransportCommand::SetLoop(self.is_looping));
                }

                ui.add_space(8.0);

                // Position scrubber
                let mut pos = self.position_secs;
                let slider = egui::Slider::new(&mut pos, 0.0..=self.duration_secs)
                    .show_value(false)
                    .trailing_fill(true);
                let response = ui.add_sized([200.0, 18.0], slider);
                if response.changed() {
                    let _ = self.transport_tx.send(TransportCommand::Seek(pos));
                    self.position_secs = pos;
                }

                // Time display
                ui.label(
                    egui::RichText::new(format!(
                        "{} / {}",
                        format_time(self.position_secs),
                        format_time(self.duration_secs),
                    ))
                    .color(Color32::from_rgb(160, 160, 170))
                    .small(),
                );
            }
        });

        // Handle drag-and-drop
        ui.ctx().input(|i| {
            if let Some(dropped) = i.raw.dropped_files.first()
                && let Some(path) = &dropped.path
                && path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("wav"))
            {
                self.file_path = Some(path.clone());
            }
        });

        // Deferred drag-drop load: check if file_path changed without a loaded file_name match
        if let Some(ref path) = self.file_path {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            if name != self.file_name && !name.is_empty() {
                let path = path.clone();
                self.load_file(path);
            }
        }
    }
}

/// Format seconds as `M:SS`.
fn format_time(secs: f32) -> String {
    let total = secs.max(0.0) as u32;
    let m = total / 60;
    let s = total % 60;
    format!("{m}:{s:02}")
}
