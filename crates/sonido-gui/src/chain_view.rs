//! Effect chain visualization and reordering.

use crate::audio_bridge::{EffectOrder, SharedParams};
use crate::effects_ui::EffectType;
use egui::{Color32, Response, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use std::sync::Arc;
use std::sync::atomic::Ordering;

/// Chain view state for drag-and-drop.
pub struct ChainView {
    effect_order: EffectOrder,
    dragging: Option<usize>,
    drag_offset: f32,
    selected: Option<EffectType>,
}

impl ChainView {
    /// Create a new chain view.
    pub fn new() -> Self {
        Self {
            effect_order: EffectOrder::default(),
            dragging: None,
            drag_offset: 0.0,
            selected: Some(EffectType::Distortion), // Default selection
        }
    }

    /// Get the current effect order.
    pub fn effect_order(&self) -> &EffectOrder {
        &self.effect_order
    }

    /// Get the currently selected effect.
    pub fn selected(&self) -> Option<EffectType> {
        self.selected
    }

    /// Render the chain view.
    pub fn ui(&mut self, ui: &mut Ui, params: &Arc<SharedParams>) -> Option<EffectType> {
        let order = self.effect_order.get();
        let effect_width = 70.0;
        let spacing = 8.0;
        let arrow_width = 20.0;

        let total_width = order.len() as f32 * (effect_width + spacing + arrow_width) - arrow_width;

        ui.horizontal(|ui| {
            // Center the chain
            let available = ui.available_width();
            if available > total_width {
                ui.add_space((available - total_width) / 2.0);
            }

            for (pos, &effect_idx) in order.iter().enumerate() {
                let effect_type = EffectType::from_index(effect_idx);
                if effect_type.is_none() {
                    continue;
                }
                let effect_type = effect_type.unwrap();

                // Effect pedal button
                let is_selected = self.selected == Some(effect_type);
                let is_bypassed = self.is_effect_bypassed(effect_type, params);

                let response = self.effect_pedal(ui, effect_type, is_selected, is_bypassed, params);

                // Handle selection
                if response.clicked() {
                    self.selected = Some(effect_type);
                }

                // Handle drag start
                if response.drag_started() {
                    self.dragging = Some(pos);
                }

                // Handle drag
                if self.dragging == Some(pos) && response.dragged() {
                    let delta = response.drag_delta().x;
                    self.drag_offset += delta;

                    // Check if we should swap with adjacent effect
                    let swap_threshold = effect_width / 2.0 + spacing;
                    if self.drag_offset > swap_threshold && pos < order.len() - 1 {
                        self.effect_order.move_effect(pos, pos + 1);
                        self.dragging = Some(pos + 1);
                        self.drag_offset = 0.0;
                    } else if self.drag_offset < -swap_threshold && pos > 0 {
                        self.effect_order.move_effect(pos, pos - 1);
                        self.dragging = Some(pos - 1);
                        self.drag_offset = 0.0;
                    }
                }

                // Handle drag end
                if response.drag_stopped() {
                    self.dragging = None;
                    self.drag_offset = 0.0;
                }

                // Arrow between effects (except last)
                if pos < order.len() - 1 {
                    ui.add_space(spacing / 2.0);
                    self.draw_arrow(ui, arrow_width);
                    ui.add_space(spacing / 2.0);
                }
            }
        });

        self.selected
    }

    /// Draw a single effect pedal in the chain.
    fn effect_pedal(
        &self,
        ui: &mut Ui,
        effect_type: EffectType,
        is_selected: bool,
        is_bypassed: bool,
        params: &Arc<SharedParams>,
    ) -> Response {
        let size = vec2(70.0, 50.0);
        let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());

        if ui.is_rect_visible(rect) {
            let painter = ui.painter();

            // Pedal body color based on state
            let body_color = if is_selected {
                if is_bypassed {
                    Color32::from_rgb(50, 55, 60)
                } else {
                    Color32::from_rgb(55, 70, 65)
                }
            } else if is_bypassed {
                Color32::from_rgb(35, 35, 42)
            } else {
                Color32::from_rgb(45, 55, 52)
            };

            // Draw pedal body
            painter.rect_filled(rect, 6.0, body_color);

            // Border (highlighted if selected)
            let border_color = if is_selected {
                Color32::from_rgb(100, 180, 255)
            } else {
                Color32::from_rgb(60, 60, 70)
            };
            painter.rect_stroke(
                rect,
                6.0,
                Stroke::new(if is_selected { 2.0 } else { 1.0 }, border_color),
                StrokeKind::Inside,
            );

            // LED indicator
            let led_pos = pos2(rect.center().x, rect.top() + 12.0);
            let led_color = if !is_bypassed {
                Color32::from_rgb(100, 255, 100)
            } else {
                Color32::from_rgb(40, 50, 40)
            };
            painter.circle_filled(led_pos, 4.0, led_color);
            if !is_bypassed {
                // Glow effect
                painter.circle_filled(led_pos, 7.0, led_color.gamma_multiply(0.25));
            }

            // Effect name
            let text_color = if is_bypassed {
                Color32::from_rgb(100, 100, 110)
            } else {
                Color32::from_rgb(200, 200, 210)
            };
            painter.text(
                pos2(rect.center().x, rect.bottom() - 12.0),
                egui::Align2::CENTER_CENTER,
                effect_type.short_name(),
                egui::FontId::proportional(11.0),
                text_color,
            );

            // Drag indicator
            if self.dragging.is_some() && response.hovered() {
                painter.rect_stroke(
                    rect.expand(2.0),
                    8.0,
                    Stroke::new(2.0, Color32::from_rgb(100, 180, 255).gamma_multiply(0.5)),
                    StrokeKind::Outside,
                );
            }
        }

        // Double-click to toggle bypass
        if response.double_clicked() {
            self.toggle_effect_bypass(effect_type, params);
        }

        response
    }

    /// Draw an arrow between effects.
    fn draw_arrow(&self, ui: &mut Ui, width: f32) {
        let (rect, _response) = ui.allocate_exact_size(vec2(width, 50.0), Sense::hover());

        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            let center_y = rect.center().y;
            let arrow_color = Color32::from_rgb(80, 80, 90);

            // Line
            painter.line_segment(
                [
                    pos2(rect.left(), center_y),
                    pos2(rect.right() - 6.0, center_y),
                ],
                Stroke::new(2.0, arrow_color),
            );

            // Arrow head
            let tip = pos2(rect.right(), center_y);
            let back = 6.0;
            let spread = 4.0;
            painter.line_segment(
                [pos2(tip.x - back, tip.y - spread), tip],
                Stroke::new(2.0, arrow_color),
            );
            painter.line_segment(
                [pos2(tip.x - back, tip.y + spread), tip],
                Stroke::new(2.0, arrow_color),
            );
        }
    }

    /// Check if an effect is bypassed.
    fn is_effect_bypassed(&self, effect_type: EffectType, params: &Arc<SharedParams>) -> bool {
        match effect_type {
            EffectType::Preamp => params.bypass.preamp.load(Ordering::Relaxed),
            EffectType::Distortion => params.bypass.distortion.load(Ordering::Relaxed),
            EffectType::Compressor => params.bypass.compressor.load(Ordering::Relaxed),
            EffectType::Gate => params.bypass.gate.load(Ordering::Relaxed),
            EffectType::ParametricEq => params.bypass.eq.load(Ordering::Relaxed),
            EffectType::Wah => params.bypass.wah.load(Ordering::Relaxed),
            EffectType::Chorus => params.bypass.chorus.load(Ordering::Relaxed),
            EffectType::Flanger => params.bypass.flanger.load(Ordering::Relaxed),
            EffectType::Phaser => params.bypass.phaser.load(Ordering::Relaxed),
            EffectType::Tremolo => params.bypass.tremolo.load(Ordering::Relaxed),
            EffectType::Delay => params.bypass.delay.load(Ordering::Relaxed),
            EffectType::Filter => params.bypass.filter.load(Ordering::Relaxed),
            EffectType::MultiVibrato => params.bypass.multivibrato.load(Ordering::Relaxed),
            EffectType::Tape => params.bypass.tape.load(Ordering::Relaxed),
            EffectType::Reverb => params.bypass.reverb.load(Ordering::Relaxed),
        }
    }

    /// Toggle an effect's bypass state.
    fn toggle_effect_bypass(&self, effect_type: EffectType, params: &Arc<SharedParams>) {
        match effect_type {
            EffectType::Preamp => {
                let current = params.bypass.preamp.load(Ordering::Relaxed);
                params.bypass.preamp.store(!current, Ordering::Relaxed);
            }
            EffectType::Distortion => {
                let current = params.bypass.distortion.load(Ordering::Relaxed);
                params.bypass.distortion.store(!current, Ordering::Relaxed);
            }
            EffectType::Compressor => {
                let current = params.bypass.compressor.load(Ordering::Relaxed);
                params.bypass.compressor.store(!current, Ordering::Relaxed);
            }
            EffectType::Gate => {
                let current = params.bypass.gate.load(Ordering::Relaxed);
                params.bypass.gate.store(!current, Ordering::Relaxed);
            }
            EffectType::ParametricEq => {
                let current = params.bypass.eq.load(Ordering::Relaxed);
                params.bypass.eq.store(!current, Ordering::Relaxed);
            }
            EffectType::Wah => {
                let current = params.bypass.wah.load(Ordering::Relaxed);
                params.bypass.wah.store(!current, Ordering::Relaxed);
            }
            EffectType::Chorus => {
                let current = params.bypass.chorus.load(Ordering::Relaxed);
                params.bypass.chorus.store(!current, Ordering::Relaxed);
            }
            EffectType::Flanger => {
                let current = params.bypass.flanger.load(Ordering::Relaxed);
                params.bypass.flanger.store(!current, Ordering::Relaxed);
            }
            EffectType::Phaser => {
                let current = params.bypass.phaser.load(Ordering::Relaxed);
                params.bypass.phaser.store(!current, Ordering::Relaxed);
            }
            EffectType::Tremolo => {
                let current = params.bypass.tremolo.load(Ordering::Relaxed);
                params.bypass.tremolo.store(!current, Ordering::Relaxed);
            }
            EffectType::Delay => {
                let current = params.bypass.delay.load(Ordering::Relaxed);
                params.bypass.delay.store(!current, Ordering::Relaxed);
            }
            EffectType::Filter => {
                let current = params.bypass.filter.load(Ordering::Relaxed);
                params.bypass.filter.store(!current, Ordering::Relaxed);
            }
            EffectType::MultiVibrato => {
                let current = params.bypass.multivibrato.load(Ordering::Relaxed);
                params
                    .bypass
                    .multivibrato
                    .store(!current, Ordering::Relaxed);
            }
            EffectType::Tape => {
                let current = params.bypass.tape.load(Ordering::Relaxed);
                params.bypass.tape.store(!current, Ordering::Relaxed);
            }
            EffectType::Reverb => {
                let current = params.bypass.reverb.load(Ordering::Relaxed);
                params.bypass.reverb.store(!current, Ordering::Relaxed);
            }
        }
    }
}

impl Default for ChainView {
    fn default() -> Self {
        Self::new()
    }
}
