//! Visual styling for the Sonido GUI.

use egui::{Color32, CornerRadius, Stroke, Style, Visuals};

/// Theme colors for the GUI.
pub struct Theme {
    /// Main window background color.
    pub background: Color32,
    /// Panel/card background color.
    pub panel_bg: Color32,
    /// Primary accent color for active elements.
    pub accent: Color32,
    /// Dimmed accent color for inactive elements.
    pub accent_dim: Color32,
    /// Primary text color.
    pub text_primary: Color32,
    /// Secondary/muted text color.
    pub text_secondary: Color32,
    /// Meter color for safe signal levels.
    pub meter_green: Color32,
    /// Meter color for hot signal levels.
    pub meter_yellow: Color32,
    /// Meter color for clipping signal levels.
    pub meter_red: Color32,
    /// Knob background track color.
    pub knob_track: Color32,
    /// Knob filled arc color.
    pub knob_fill: Color32,
    /// Bypass indicator color when effect is off.
    pub bypass_off: Color32,
    /// Bypass indicator color when effect is on.
    pub bypass_on: Color32,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            background: Color32::from_rgb(25, 25, 30),
            panel_bg: Color32::from_rgb(35, 35, 42),
            accent: Color32::from_rgb(100, 180, 255),
            accent_dim: Color32::from_rgb(60, 100, 140),
            text_primary: Color32::from_rgb(230, 230, 235),
            text_secondary: Color32::from_rgb(150, 150, 160),
            meter_green: Color32::from_rgb(80, 200, 80),
            meter_yellow: Color32::from_rgb(220, 200, 60),
            meter_red: Color32::from_rgb(220, 60, 60),
            knob_track: Color32::from_rgb(50, 50, 60),
            knob_fill: Color32::from_rgb(100, 180, 255),
            bypass_off: Color32::from_rgb(80, 80, 90),
            bypass_on: Color32::from_rgb(80, 200, 80),
        }
    }
}

impl Theme {
    /// Apply the theme to an egui context.
    pub fn apply(&self, ctx: &egui::Context) {
        let mut style = Style::default();

        // Dark visuals as base
        let mut visuals = Visuals::dark();

        // Window/panel backgrounds
        visuals.window_fill = self.panel_bg;
        visuals.panel_fill = self.panel_bg;
        visuals.extreme_bg_color = self.background;
        visuals.faint_bg_color = Color32::from_rgb(40, 40, 48);

        // Widget colors
        visuals.widgets.noninteractive.bg_fill = self.panel_bg;
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, self.text_secondary);
        visuals.widgets.noninteractive.corner_radius = CornerRadius::same(4);

        visuals.widgets.inactive.bg_fill = Color32::from_rgb(45, 45, 55);
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, self.text_primary);
        visuals.widgets.inactive.corner_radius = CornerRadius::same(4);

        visuals.widgets.hovered.bg_fill = Color32::from_rgb(55, 55, 68);
        visuals.widgets.hovered.fg_stroke = Stroke::new(1.5, self.accent);
        visuals.widgets.hovered.corner_radius = CornerRadius::same(4);

        visuals.widgets.active.bg_fill = Color32::from_rgb(65, 65, 80);
        visuals.widgets.active.fg_stroke = Stroke::new(2.0, self.accent);
        visuals.widgets.active.corner_radius = CornerRadius::same(4);

        // Selection
        visuals.selection.bg_fill = self.accent.gamma_multiply(0.3);
        visuals.selection.stroke = Stroke::new(1.0, self.accent);

        // Override text color
        visuals.override_text_color = Some(self.text_primary);

        style.visuals = visuals;

        // Spacing
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.window_margin = egui::Margin::same(12);
        style.spacing.button_padding = egui::vec2(8.0, 4.0);

        ctx.set_style(style);
    }

    /// Get meter color based on level (0.0 to 1.0+).
    pub fn meter_color(&self, level: f32) -> Color32 {
        if level > 0.95 {
            self.meter_red
        } else if level > 0.7 {
            self.meter_yellow
        } else {
            self.meter_green
        }
    }
}
