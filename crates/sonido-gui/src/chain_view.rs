//! Effect chain visualization, reordering, and dynamic add/remove.
//!
//! The [`ChainView`] renders the pedalboard strip, letting users click to
//! select, double-click to bypass, drag to reorder, right-click to remove,
//! and press "+" to add new effects.

use crate::audio_bridge::EffectOrder;
use egui::{Color32, Response, ScrollArea, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use sonido_gui_core::{ParamBridge, SlotIndex};
use sonido_registry::EffectRegistry;

/// Chain view state for drag-and-drop, selection, and pending commands.
pub struct ChainView {
    effect_order: EffectOrder,
    dragging: Option<usize>,
    drag_offset: f32,
    selected: Option<SlotIndex>,
    /// Effect ID to add (set by "+" menu, polled by app).
    pending_add: Option<&'static str>,
    /// Slot index to remove (set by context menu, polled by app).
    pending_remove: Option<SlotIndex>,
}

impl ChainView {
    /// Create a new chain view with no selection.
    pub fn new() -> Self {
        Self {
            effect_order: EffectOrder::default(),
            dragging: None,
            drag_offset: 0.0,
            selected: None,
            pending_add: None,
            pending_remove: None,
        }
    }

    /// Get the current effect order.
    pub fn effect_order(&self) -> &EffectOrder {
        &self.effect_order
    }

    /// Get the currently selected slot index.
    pub fn selected(&self) -> Option<SlotIndex> {
        self.selected
    }

    /// Set the selected slot.
    pub fn set_selected(&mut self, slot: SlotIndex) {
        self.selected = Some(slot);
    }

    /// Clear the current selection.
    pub fn clear_selection(&mut self) {
        self.selected = None;
    }

    /// Take the pending add request (if any), clearing it.
    pub fn take_pending_add(&mut self) -> Option<&'static str> {
        self.pending_add.take()
    }

    /// Take the pending remove request (if any), clearing it.
    pub fn take_pending_remove(&mut self) -> Option<SlotIndex> {
        self.pending_remove.take()
    }

    /// Render the chain view.
    ///
    /// Returns the currently selected slot index (if any). The caller should
    /// also poll [`take_pending_add`](Self::take_pending_add) and
    /// [`take_pending_remove`](Self::take_pending_remove) for user actions.
    pub fn ui(
        &mut self,
        ui: &mut Ui,
        bridge: &dyn ParamBridge,
        registry: &EffectRegistry,
    ) -> Option<SlotIndex> {
        let order = self.effect_order.get();
        let slot_count = bridge.slot_count();

        // Clear selection if the selected slot was removed
        if let Some(sel) = self.selected
            && sel.0 >= slot_count
        {
            self.selected = None;
        }
        let effect_width = 70.0;
        let spacing = 8.0;
        let arrow_width = 20.0;
        let add_button_width = 36.0;

        // Only show slots that exist in both the order and the bridge
        let visible: Vec<usize> = order.iter().copied().filter(|&i| i < slot_count).collect();

        let total_width = if visible.is_empty() {
            add_button_width
        } else {
            visible.len() as f32 * (effect_width + spacing + arrow_width) - arrow_width
                + spacing
                + add_button_width
        };

        // Scroll horizontally when the chain overflows the available width
        let available_width = ui.available_width();
        let needs_scroll = total_width > available_width;

        ScrollArea::horizontal().auto_shrink(true).show(ui, |ui| {
            ui.horizontal(|ui| {
                // Center the chain when it fits
                if !needs_scroll {
                    let available = ui.available_width();
                    if available > total_width {
                        ui.add_space((available - total_width) / 2.0);
                    }
                }

                for (pos, &slot_raw) in visible.iter().enumerate() {
                    let slot_idx = SlotIndex(slot_raw);
                    let effect_id = bridge.effect_id(slot_idx);
                    let short_name = registry
                        .descriptor(effect_id)
                        .map(|d| d.short_name)
                        .unwrap_or("???");

                    let is_selected = self.selected == Some(slot_idx);
                    let is_bypassed = bridge.is_bypassed(slot_idx);

                    let response = self.effect_pedal(
                        ui,
                        short_name,
                        is_selected,
                        is_bypassed,
                        slot_idx,
                        bridge,
                    );

                    // Click → select
                    if response.clicked() {
                        self.selected = Some(slot_idx);
                    }

                    // Right-click → context menu
                    response.context_menu(|ui| {
                        if ui.button("Remove Effect").clicked() {
                            self.pending_remove = Some(slot_idx);
                            ui.close_menu();
                        }
                        if ui
                            .button(if is_bypassed { "Enable" } else { "Bypass" })
                            .clicked()
                        {
                            bridge.set_bypassed(slot_idx, !is_bypassed);
                            ui.close_menu();
                        }
                    });

                    // Handle drag start
                    if response.drag_started() {
                        self.dragging = Some(pos);
                    }

                    // Handle drag
                    if self.dragging == Some(pos) && response.dragged() {
                        let delta = response.drag_delta().x;
                        self.drag_offset += delta;

                        let swap_threshold = effect_width / 2.0 + spacing;
                        if self.drag_offset > swap_threshold && pos < visible.len() - 1 {
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
                    if pos < visible.len() - 1 {
                        ui.add_space(spacing / 2.0);
                        self.draw_arrow(ui, arrow_width);
                        ui.add_space(spacing / 2.0);
                    }
                }

                // "+" button to add new effects
                ui.add_space(spacing);
                self.add_button(ui, registry, add_button_width);
            });
        });

        self.selected
    }

    /// Draw a single effect pedal in the chain.
    fn effect_pedal(
        &self,
        ui: &mut Ui,
        short_name: &str,
        is_selected: bool,
        is_bypassed: bool,
        slot_idx: SlotIndex,
        bridge: &dyn ParamBridge,
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
                short_name,
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
            bridge.set_bypassed(slot_idx, !bridge.is_bypassed(slot_idx));
        }

        response.on_hover_text("Click: select | Double-click: bypass | Right-click: menu")
    }

    /// Draw the "+" button and its popup menu for adding effects.
    fn add_button(&mut self, ui: &mut Ui, registry: &EffectRegistry, width: f32) {
        let size = vec2(width, 50.0);
        let (rect, response) = ui.allocate_exact_size(size, Sense::click());

        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            let body_color = Color32::from_rgb(40, 45, 50);
            painter.rect_filled(rect, 6.0, body_color);
            painter.rect_stroke(
                rect,
                6.0,
                Stroke::new(1.0, Color32::from_rgb(60, 70, 80)),
                StrokeKind::Inside,
            );
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "+",
                egui::FontId::proportional(20.0),
                Color32::from_rgb(120, 140, 160),
            );
        }

        // Popup menu listing all available effects
        let popup_id = ui.make_persistent_id("add_effect_popup");
        if response.clicked() {
            ui.memory_mut(|mem| mem.toggle_popup(popup_id));
        }

        egui::popup_below_widget(
            ui,
            popup_id,
            &response,
            egui::PopupCloseBehavior::CloseOnClick,
            |ui| {
                ui.set_min_width(160.0);
                for desc in registry.all_effects() {
                    if ui.button(desc.name).clicked() {
                        self.pending_add = Some(desc.id);
                    }
                }
            },
        );
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
}

impl Default for ChainView {
    fn default() -> Self {
        Self::new()
    }
}
