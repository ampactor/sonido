//! Rotary knob control widget.
//!
//! Professional audio-style knob with:
//! - Drag to adjust value
//! - Fine control with Shift key
//! - Double-click to reset
//! - Value display below knob

use egui::{Color32, Pos2, Response, Sense, Stroke, Ui, Widget, pos2, vec2};
use std::f32::consts::PI;

/// Rotary knob parameters.
pub struct Knob<'a> {
    value: &'a mut f32,
    min: f32,
    max: f32,
    default: f32,
    label: &'a str,
    format_value: Option<Box<dyn Fn(f32) -> String + 'a>>,
    diameter: f32,
    sensitivity: f32,
}

impl<'a> Knob<'a> {
    /// Create a new knob.
    pub fn new(value: &'a mut f32, min: f32, max: f32, label: &'a str) -> Self {
        Self {
            value,
            min,
            max,
            default: (min + max) / 2.0,
            label,
            format_value: None,
            diameter: 60.0,
            sensitivity: 0.004,
        }
    }

    /// Set the default (reset) value.
    pub fn default(mut self, default: f32) -> Self {
        self.default = default;
        self
    }

    /// Set a custom value formatter.
    pub fn format(mut self, formatter: impl Fn(f32) -> String + 'a) -> Self {
        self.format_value = Some(Box::new(formatter));
        self
    }

    /// Set knob diameter in pixels.
    pub fn diameter(mut self, diameter: f32) -> Self {
        self.diameter = diameter;
        self
    }

    /// Set sensitivity (value change per pixel dragged).
    pub fn sensitivity(mut self, sensitivity: f32) -> Self {
        self.sensitivity = sensitivity;
        self
    }

    /// Format as decibels.
    pub fn format_db(self) -> Self {
        self.format(|v| format!("{:.1} dB", v))
    }

    /// Format as Hertz.
    pub fn format_hz(self) -> Self {
        self.format(|v| {
            if v >= 1000.0 {
                format!("{:.1} kHz", v / 1000.0)
            } else {
                format!("{:.0} Hz", v)
            }
        })
    }

    /// Format as milliseconds.
    pub fn format_ms(self) -> Self {
        self.format(|v| {
            if v >= 1000.0 {
                format!("{:.2} s", v / 1000.0)
            } else {
                format!("{:.1} ms", v)
            }
        })
    }

    /// Format as percentage.
    pub fn format_percent(self) -> Self {
        self.format(|v| format!("{:.0}%", v * 100.0))
    }

    /// Format as ratio (e.g., "4:1").
    pub fn format_ratio(self) -> Self {
        self.format(|v| format!("{:.1}:1", v))
    }
}

impl Widget for Knob<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let size = vec2(self.diameter, self.diameter + 35.0); // Extra space for label
        let (rect, mut response) = ui.allocate_exact_size(size, Sense::click_and_drag());

        let center = pos2(rect.center().x, rect.top() + self.diameter / 2.0);
        let radius = self.diameter / 2.0 - 4.0;

        // Handle interaction
        let mut changed = false;

        // Double-click to reset
        if response.double_clicked() {
            *self.value = self.default;
            changed = true;
        }

        // Drag to adjust
        if response.dragged() {
            let delta = response.drag_delta();
            let sensitivity = if ui.input(|i| i.modifiers.shift) {
                self.sensitivity * 0.1 // Fine control
            } else {
                self.sensitivity
            };

            // Vertical drag changes value (up = increase)
            let value_delta = -delta.y * sensitivity * (self.max - self.min);
            *self.value = (*self.value + value_delta).clamp(self.min, self.max);
            changed = true;
        }

        // Visual state
        let is_active = response.dragged() || response.has_focus();

        // Draw knob
        if ui.is_rect_visible(rect) {
            let painter = ui.painter();

            // Knob arc angles (270 degree sweep, starting from bottom-left)
            let start_angle = PI * 0.75; // 135 degrees
            let end_angle = PI * 2.25; // 405 degrees (wraps around)
            let sweep = end_angle - start_angle;

            // Normalized value position
            let normalized = (*self.value - self.min) / (self.max - self.min);
            let value_angle = start_angle + normalized * sweep;

            // Track (background arc)
            let track_color = Color32::from_rgb(50, 50, 60);
            draw_arc(
                painter,
                center,
                radius - 2.0,
                start_angle,
                end_angle,
                track_color,
                6.0,
            );

            // Value arc (filled portion)
            let fill_color = if is_active {
                Color32::from_rgb(120, 200, 255)
            } else {
                Color32::from_rgb(100, 180, 255)
            };
            if normalized > 0.001 {
                draw_arc(
                    painter,
                    center,
                    radius - 2.0,
                    start_angle,
                    value_angle,
                    fill_color,
                    6.0,
                );
            }

            // Knob body
            let body_color = if is_active {
                Color32::from_rgb(65, 65, 78)
            } else {
                Color32::from_rgb(55, 55, 68)
            };
            painter.circle_filled(center, radius - 8.0, body_color);

            // Pointer line
            let pointer_len = radius - 14.0;
            let pointer_end = pos2(
                center.x + value_angle.cos() * pointer_len,
                center.y + value_angle.sin() * pointer_len,
            );
            painter.line_segment([center, pointer_end], Stroke::new(3.0, fill_color));

            // Center dot
            painter.circle_filled(center, 3.0, fill_color);

            // Label
            let label_pos = pos2(rect.center().x, center.y + radius + 8.0);
            painter.text(
                label_pos,
                egui::Align2::CENTER_TOP,
                self.label,
                egui::FontId::proportional(12.0),
                Color32::from_rgb(180, 180, 190),
            );

            // Value text
            let value_text = if let Some(ref formatter) = self.format_value {
                formatter(*self.value)
            } else {
                format!("{:.2}", *self.value)
            };
            let value_pos = pos2(rect.center().x, center.y + radius + 22.0);
            painter.text(
                value_pos,
                egui::Align2::CENTER_TOP,
                value_text,
                egui::FontId::proportional(11.0),
                Color32::from_rgb(150, 150, 160),
            );
        }

        if changed {
            response.mark_changed();
        }

        response
    }
}

/// Draw an arc using line segments.
fn draw_arc(
    painter: &egui::Painter,
    center: Pos2,
    radius: f32,
    start_angle: f32,
    end_angle: f32,
    color: Color32,
    stroke_width: f32,
) {
    let segments = 32;
    let sweep = end_angle - start_angle;

    let points: Vec<Pos2> = (0..=segments)
        .map(|i| {
            let t = i as f32 / segments as f32;
            let angle = start_angle + t * sweep;
            pos2(
                center.x + angle.cos() * radius,
                center.y + angle.sin() * radius,
            )
        })
        .collect();

    for window in points.windows(2) {
        painter.line_segment([window[0], window[1]], Stroke::new(stroke_width, color));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_knob_default_value() {
        let mut value = 0.5;
        let knob = Knob::new(&mut value, 0.0, 1.0, "Test").default(0.25);
        assert_eq!(knob.default, 0.25);
    }
}
