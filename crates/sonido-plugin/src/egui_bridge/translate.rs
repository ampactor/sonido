//! Translate baseview input events into egui's [`RawInput`] format.
//!
//! Only the events relevant to audio plugin UIs are translated:
//! - Mouse movement, clicks, scroll (knob/slider/combo interaction)
//! - Keyboard modifier state (shift for fine-control on knobs)
//!
//! Clipboard, cursor icon, text input, and IME are intentionally omitted —
//! no sonido effect panel uses text fields or clipboard operations.

use baseview::{MouseButton as BvMouseButton, MouseEvent, ScrollDelta, WindowEvent};
use egui::{Event as EguiEvent, Modifiers, PointerButton, Pos2, RawInput, Vec2};
use keyboard_types::Modifiers as KbModifiers;

/// Translate a baseview [`MouseEvent`] into egui events.
///
/// Appends zero or more [`egui::Event`] variants to `raw_input.events`.
/// Updates `mouse_pos` to track the current logical cursor position.
///
/// # Coordinate system
///
/// Baseview reports physical pixel coordinates. We divide by `scale` to get
/// egui's logical coordinates (matching `pixels_per_point`).
pub fn translate_mouse(
    event: &MouseEvent,
    scale: f64,
    raw_input: &mut RawInput,
    mouse_pos: &mut Pos2,
) {
    let scale_recip = 1.0 / scale as f32;

    match event {
        MouseEvent::CursorMoved {
            position,
            modifiers,
        } => {
            let pos = Pos2::new(
                position.x as f32 * scale_recip,
                position.y as f32 * scale_recip,
            );
            *mouse_pos = pos;
            raw_input.modifiers = map_modifiers(*modifiers);
            raw_input.events.push(EguiEvent::PointerMoved(pos));
        }

        MouseEvent::ButtonPressed { button, modifiers } => {
            raw_input.modifiers = map_modifiers(*modifiers);
            if let Some(egui_button) = map_mouse_button(*button) {
                raw_input.events.push(EguiEvent::PointerButton {
                    pos: *mouse_pos,
                    button: egui_button,
                    pressed: true,
                    modifiers: raw_input.modifiers,
                });
            }
        }

        MouseEvent::ButtonReleased { button, modifiers } => {
            raw_input.modifiers = map_modifiers(*modifiers);
            if let Some(egui_button) = map_mouse_button(*button) {
                raw_input.events.push(EguiEvent::PointerButton {
                    pos: *mouse_pos,
                    button: egui_button,
                    pressed: false,
                    modifiers: raw_input.modifiers,
                });
            }
        }

        MouseEvent::WheelScrolled {
            delta,
            modifiers: _,
        } => {
            let (x, y) = match delta {
                ScrollDelta::Lines { x, y } => (x * 24.0, y * 24.0),
                ScrollDelta::Pixels { x, y } => (x * scale_recip, y * scale_recip),
            };
            raw_input.events.push(EguiEvent::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: Vec2::new(x, y),
                modifiers: raw_input.modifiers,
            });
        }

        MouseEvent::CursorEntered => {
            // egui infers cursor presence from pointer events; no action needed.
        }

        MouseEvent::CursorLeft => {
            raw_input.events.push(EguiEvent::PointerGone);
        }

        _ => {}
    }
}

/// Translate a baseview [`WindowEvent`] into egui state updates.
///
/// Handles focus changes and window resize. Returns `true` if the window
/// was resized (caller should update viewport dimensions).
pub fn translate_window(
    event: &WindowEvent,
    raw_input: &mut RawInput,
    physical_width: &mut u32,
    physical_height: &mut u32,
) -> bool {
    match event {
        WindowEvent::Focused => {
            raw_input.focused = true;
            false
        }
        WindowEvent::Unfocused => {
            raw_input.focused = false;
            false
        }
        WindowEvent::Resized(info) => {
            *physical_width = info.physical_size().width;
            *physical_height = info.physical_size().height;
            true
        }
        _ => false,
    }
}

/// Extract modifier state from a baseview keyboard event.
///
/// Called on keyboard events to keep `raw_input.modifiers` up to date.
/// Individual key presses are not translated — only shift is needed
/// (for fine-control knob dragging), and that is tracked via modifiers.
pub fn update_modifiers_from_keyboard(modifiers: KbModifiers, raw_input: &mut RawInput) {
    raw_input.modifiers = map_modifiers(modifiers);
}

/// Map baseview modifier flags to egui [`Modifiers`].
fn map_modifiers(mods: KbModifiers) -> Modifiers {
    Modifiers {
        alt: mods.contains(KbModifiers::ALT),
        ctrl: mods.contains(KbModifiers::CONTROL),
        shift: mods.contains(KbModifiers::SHIFT),
        mac_cmd: false,
        command: mods.contains(KbModifiers::META),
    }
}

/// Map a baseview mouse button to an egui [`PointerButton`].
fn map_mouse_button(button: BvMouseButton) -> Option<PointerButton> {
    match button {
        BvMouseButton::Left => Some(PointerButton::Primary),
        BvMouseButton::Right => Some(PointerButton::Secondary),
        BvMouseButton::Middle => Some(PointerButton::Middle),
        _ => None,
    }
}
