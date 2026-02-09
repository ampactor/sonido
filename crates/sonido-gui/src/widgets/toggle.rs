//! Bypass toggle widget for effects.

use egui::{Color32, Response, Sense, Stroke, StrokeKind, Ui, Widget, pos2, vec2};

/// A bypass toggle button for effects.
pub struct BypassToggle<'a> {
    active: &'a mut bool,
    label: &'a str,
    size: f32,
}

impl<'a> BypassToggle<'a> {
    /// Create a new bypass toggle.
    ///
    /// `active` is true when the effect is ON (not bypassed).
    pub fn new(active: &'a mut bool, label: &'a str) -> Self {
        Self {
            active,
            label,
            size: 20.0,
        }
    }

    /// Set the button size.
    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }
}

impl Widget for BypassToggle<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let total_width = self.size
            + 8.0
            + ui.fonts(|f| {
                f.glyph_width(&egui::FontId::proportional(12.0), 'M') * self.label.len() as f32
            });
        let size = vec2(total_width.max(60.0), self.size + 4.0);

        let (rect, response) = ui.allocate_exact_size(size, Sense::click());

        if response.clicked() {
            *self.active = !*self.active;
        }

        if ui.is_rect_visible(rect) {
            let painter = ui.painter();

            // Toggle indicator (circle)
            let indicator_center = pos2(rect.left() + self.size / 2.0 + 2.0, rect.center().y);
            let indicator_radius = self.size / 2.0 - 2.0;

            // Background ring
            painter.circle_stroke(
                indicator_center,
                indicator_radius,
                Stroke::new(2.0, Color32::from_rgb(60, 60, 70)),
            );

            // Filled circle when active
            if *self.active {
                painter.circle_filled(
                    indicator_center,
                    indicator_radius - 3.0,
                    Color32::from_rgb(80, 200, 80),
                );
            }

            // Hover effect
            if response.hovered() {
                painter.circle_stroke(
                    indicator_center,
                    indicator_radius + 2.0,
                    Stroke::new(1.0, Color32::from_rgb(100, 180, 255).gamma_multiply(0.5)),
                );
            }

            // Label
            let label_pos = pos2(rect.left() + self.size + 8.0, rect.center().y);
            let text_color = if *self.active {
                Color32::from_rgb(200, 200, 210)
            } else {
                Color32::from_rgb(120, 120, 130)
            };
            painter.text(
                label_pos,
                egui::Align2::LEFT_CENTER,
                self.label,
                egui::FontId::proportional(12.0),
                text_color,
            );
        }

        response
    }
}

/// A larger footswitch-style toggle for the chain view.
pub struct FootswitchToggle<'a> {
    active: &'a mut bool,
    label: &'a str,
}

impl<'a> FootswitchToggle<'a> {
    pub fn new(active: &'a mut bool, label: &'a str) -> Self {
        Self { active, label }
    }
}

impl Widget for FootswitchToggle<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let size = vec2(70.0, 50.0);
        let (rect, response) = ui.allocate_exact_size(size, Sense::click());

        if response.clicked() {
            *self.active = !*self.active;
        }

        if ui.is_rect_visible(rect) {
            let painter = ui.painter();

            // Pedal body
            let body_color = if *self.active {
                Color32::from_rgb(50, 60, 55)
            } else {
                Color32::from_rgb(40, 40, 48)
            };
            painter.rect_filled(rect, 6.0, body_color);
            painter.rect_stroke(
                rect,
                6.0,
                Stroke::new(1.0, Color32::from_rgb(70, 70, 80)),
                StrokeKind::Inside,
            );

            // LED indicator
            let led_pos = pos2(rect.center().x, rect.top() + 12.0);
            let led_color = if *self.active {
                Color32::from_rgb(100, 255, 100)
            } else {
                Color32::from_rgb(50, 60, 50)
            };
            painter.circle_filled(led_pos, 5.0, led_color);
            if *self.active {
                // Glow effect
                painter.circle_filled(led_pos, 8.0, led_color.gamma_multiply(0.3));
            }

            // Label
            let label_pos = pos2(rect.center().x, rect.bottom() - 12.0);
            let text_color = if *self.active {
                Color32::from_rgb(200, 200, 210)
            } else {
                Color32::from_rgb(100, 100, 110)
            };
            painter.text(
                label_pos,
                egui::Align2::CENTER_CENTER,
                self.label,
                egui::FontId::proportional(10.0),
                text_color,
            );
        }

        response
    }
}
