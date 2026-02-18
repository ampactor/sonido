//! CLAP plugin GUI: parameter bridge and egui-baseview window management.
//!
//! Connects the CLAP host's parent window handle to egui rendering via
//! egui-baseview (OpenGL). Each plugin instance gets an independent
//! `egui::Context` and GL context — no shared global state.

// Required for HasRawWindowHandle impl on ParentWindow (rwh 0.5 trait is unsafe)
#![allow(unsafe_code)]

use std::sync::Arc;

use baseview::{Size, WindowHandle, WindowOpenOptions, WindowScalePolicy};
use egui_baseview::{EguiWindow, GraphicsConfig};
use raw_window_handle::{HasRawWindowHandle, RawWindowHandle};

use egui_baseview::egui;
use sonido_core::ParamDescriptor;
use sonido_gui_core::{
    effects_ui::{EffectPanel, create_panel},
    param_bridge::{ParamBridge, ParamIndex, SlotIndex},
};

use crate::shared::SonidoShared;

// ── Fixed window size ──────────────────────────────────────────────────────────

/// Plugin window width in physical pixels.
pub const PLUGIN_WIDTH: u32 = 480;

/// Plugin window height in physical pixels.
pub const PLUGIN_HEIGHT: u32 = 380;

// ── Parameter bridge ──────────────────────────────────────────────────────────

/// Implements [`ParamBridge`] backed by `SonidoShared` lock-free atomics.
///
/// Single-slot (slot 0 = the effect). Thread-safe reads/writes via `Acquire`/`Release`
/// atomics — safe to call from the egui render closure on any thread.
///
/// # V1 Limitations
///
/// `begin_set`/`end_set` are no-ops. Automation gesture events (`ParamGestureBeginEvent`)
/// are deferred to v2; parameter value changes are still applied to the DSP via atomics.
pub struct PluginParamBridge {
    shared: SonidoShared,
}

impl PluginParamBridge {
    /// Creates a new bridge backed by the given shared parameter store.
    pub fn new(shared: SonidoShared) -> Self {
        Self { shared }
    }
}

impl ParamBridge for PluginParamBridge {
    fn slot_count(&self) -> usize {
        1 // plugin exposes a single effect slot
    }

    fn effect_id(&self, _slot: SlotIndex) -> &str {
        self.shared.effect_id()
    }

    fn param_count(&self, _slot: SlotIndex) -> usize {
        self.shared.param_count()
    }

    fn param_descriptor(&self, _slot: SlotIndex, param: ParamIndex) -> Option<ParamDescriptor> {
        self.shared.descriptor(param.0).cloned()
    }

    fn get(&self, _slot: SlotIndex, param: ParamIndex) -> f32 {
        self.shared.get_value(param.0).unwrap_or(0.0)
    }

    fn set(&self, _slot: SlotIndex, param: ParamIndex, value: f32) {
        self.shared.set_value(param.0, value);
    }

    fn is_bypassed(&self, _slot: SlotIndex) -> bool {
        false
    }

    fn set_bypassed(&self, _slot: SlotIndex, _bypassed: bool) {}
}

// ── Parent window wrapper ─────────────────────────────────────────────────────

/// Wraps a [`RawWindowHandle`] from the CLAP host for use with egui-baseview's
/// `open_parented`, which requires [`HasRawWindowHandle`] (raw-window-handle 0.5).
///
/// # Safety
///
/// The raw window handle is valid for the entire embedded window lifetime.
/// The CLAP specification guarantees the host's parent window outlives the
/// plugin's child window (`destroy()` is called before host closes the parent).
struct ParentWindow(RawWindowHandle);

// SAFETY: RawWindowHandle contains pointer-sized integers; the window lives on
// the host's main thread for the full plugin GUI lifecycle.
unsafe impl HasRawWindowHandle for ParentWindow {
    fn raw_window_handle(&self) -> RawWindowHandle {
        self.0
    }
}

// SAFETY: The handle is only used on the main thread (CLAP gui extension
// contract), but baseview requires Send for the parent parameter type.
unsafe impl Send for ParentWindow {}
unsafe impl Sync for ParentWindow {}

// ── Editor ────────────────────────────────────────────────────────────────────

/// Holds the egui-baseview rendering window for one plugin instance.
///
/// Dropping this value closes the plugin GUI window (baseview `WindowHandle`
/// cleanup is handled on drop).
pub struct SonidoEditor {
    _window: WindowHandle,
}

impl SonidoEditor {
    /// Opens an egui child window inside the host's parent window.
    ///
    /// Returns `None` if no UI panel is registered for the effect's ID
    /// (should not happen for the 15 built-in effects).
    pub fn open(parent_rwh: RawWindowHandle, shared: SonidoShared, scale: f64) -> Option<Self> {
        let effect_id = shared.effect_id().to_owned();
        let panel: Box<dyn EffectPanel + Send + Sync> = create_panel(&effect_id)?;
        let bridge = Arc::new(PluginParamBridge::new(shared));

        struct GuiState {
            bridge: Arc<PluginParamBridge>,
            panel: Box<dyn EffectPanel + Send + Sync>,
        }

        let state = GuiState { bridge, panel };

        let settings = WindowOpenOptions {
            title: effect_id,
            size: Size::new(f64::from(PLUGIN_WIDTH), f64::from(PLUGIN_HEIGHT)),
            scale: WindowScalePolicy::ScaleFactor(scale),
            gl_config: None,
        };

        let window = EguiWindow::open_parented(
            &ParentWindow(parent_rwh),
            settings,
            GraphicsConfig::default(),
            state,
            // build: one-time setup (fonts, theme — none needed, gui-core theme applies)
            |_ctx, _queue, _state| {},
            // update: called each frame by egui-baseview's render loop
            |ctx, _queue, state| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    state
                        .panel
                        .ui(ui, state.bridge.as_ref() as &dyn ParamBridge, SlotIndex(0));
                });
            },
        );

        Some(Self { _window: window })
    }
}
