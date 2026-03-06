//! DSL console widget for graph topology input.
//!
//! Provides a text editor and "Build" button for entering graph DSL specs.
//! On successful build, sends a [`GraphCommand::ReplaceTopology`] to the
//! audio thread for atomic topology replacement.

use crate::atomic_param_bridge::AtomicParamBridge;
use crate::audio_bridge::AudioBridge;
use crate::chain_manager::GraphCommand;
use egui::{Color32, RichText, TextEdit};
use sonido_core::ParamDescriptor;
use sonido_core::graph::GraphEngine;
use sonido_graph_dsl::{build_graph, parse_graph_dsl, validate_spec};
use sonido_registry::EffectRegistry;
use std::sync::Arc;

/// State for the DSL console widget.
pub struct DslConsole {
    /// Current DSL text input.
    input: String,
    /// Last error message (parse, validate, or build).
    error: Option<String>,
    /// Status line after successful build.
    status: Option<String>,
    /// Sample rate for building effects.
    sample_rate: f32,
    /// Block size for graph construction.
    block_size: usize,
}

impl DslConsole {
    /// Create a new DSL console.
    pub fn new(sample_rate: f32, block_size: usize) -> Self {
        Self {
            input: String::new(),
            error: None,
            status: None,
            sample_rate,
            block_size,
        }
    }

    /// Update sample rate (e.g., after device change).
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
    }

    /// Update block size.
    pub fn set_block_size(&mut self, block_size: usize) {
        self.block_size = block_size;
    }

    /// Render the DSL console UI.
    ///
    /// Returns the slot index of a clicked effect name (for parameter panel
    /// selection), or `None` if no effect was clicked.
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        audio_bridge: &AudioBridge,
        bridge: &Arc<AtomicParamBridge>,
        registry: &EffectRegistry,
    ) {
        ui.vertical(|ui| {
            ui.label(
                RichText::new("DSL CONSOLE")
                    .small()
                    .color(Color32::from_rgb(150, 150, 160)),
            );
            ui.add_space(4.0);

            // Hint text
            ui.label(
                RichText::new("Enter a graph DSL expression (Ctrl+Enter to build)")
                    .small()
                    .color(Color32::from_rgb(120, 120, 130)),
            );
            ui.add_space(4.0);

            // Text editor
            let response = ui.add(
                TextEdit::multiline(&mut self.input)
                    .desired_rows(3)
                    .desired_width(ui.available_width())
                    .font(egui::TextStyle::Monospace)
                    .hint_text("distortion:drive=20 | reverb:mix=0.3"),
            );

            // Ctrl+Enter to build
            let ctrl_enter = response.has_focus()
                && ui.input(|i| i.key_pressed(egui::Key::Enter) && i.modifiers.ctrl);

            ui.add_space(4.0);

            ui.horizontal(|ui| {
                let build_clicked = ui.button("Build").clicked();

                if build_clicked || ctrl_enter {
                    self.try_build(audio_bridge, bridge, registry);
                }

                ui.add_space(8.0);

                // Status / error display
                if let Some(ref error) = self.error {
                    ui.label(
                        RichText::new(error)
                            .small()
                            .color(Color32::from_rgb(220, 100, 100)),
                    );
                } else if let Some(ref status) = self.status {
                    ui.label(
                        RichText::new(status)
                            .small()
                            .color(Color32::from_rgb(100, 200, 100)),
                    );
                }
            });
        });
    }

    /// Attempt to parse, validate, build, and send a topology replacement.
    fn try_build(
        &mut self,
        audio_bridge: &AudioBridge,
        bridge: &Arc<AtomicParamBridge>,
        registry: &EffectRegistry,
    ) {
        self.error = None;
        self.status = None;

        let input = self.input.trim();
        if input.is_empty() {
            self.error = Some("Empty input".to_string());
            return;
        }

        // 1. Parse
        let spec = match parse_graph_dsl(input) {
            Ok(s) => s,
            Err(e) => {
                self.error = Some(format!("Parse error: {e}"));
                return;
            }
        };

        // 2. Validate
        if let Err(e) = validate_spec(&spec) {
            self.error = Some(format!("Validation error: {e}"));
            return;
        }

        // 3. Build graph + manifest
        let (graph, manifest) = match build_graph(&spec, self.sample_rate, self.block_size) {
            Ok(result) => result,
            Err(e) => {
                self.error = Some(format!("Build error: {e}"));
                return;
            }
        };

        let effect_count = manifest.len();

        // 4. Create GraphEngine from DAG
        let engine = GraphEngine::new_dag(graph, manifest.clone());

        // 5. Collect effect IDs and descriptors for bridge rebuild
        let effect_ids: Vec<&'static str> = manifest.iter().map(|(_, id)| *id).collect();
        let slot_descriptors: Vec<Vec<ParamDescriptor>> = effect_ids
            .iter()
            .map(|&id| {
                registry
                    .create(id, self.sample_rate)
                    .map(|effect| {
                        (0..effect.effect_param_count())
                            .filter_map(|i| effect.effect_param_info(i))
                            .collect()
                    })
                    .unwrap_or_default()
            })
            .collect();

        // 6. Rebuild bridge on GUI thread (immediate, for knob display)
        bridge.rebuild_from_manifest(&effect_ids, &slot_descriptors);

        // 7. Send topology replacement to audio thread
        audio_bridge.send_command(GraphCommand::ReplaceTopology {
            engine: Box::new(engine),
            effect_ids,
            slot_descriptors,
        });

        let split_count = sonido_graph_dsl::count_nodes(&spec) - effect_count;
        self.status = Some(format!(
            "{} effect{}, {} split{}",
            effect_count,
            if effect_count == 1 { "" } else { "s" },
            split_count,
            if split_count == 1 { "" } else { "s" },
        ));
    }
}
