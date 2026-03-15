//! Dynamic effect loading via cdylib + dlopen.
//!
//! This module defines the C ABI bridge for loading effects compiled as
//! shared libraries at runtime. Enables OWL-style runtime patch swapping
//! and third-party effect development.
//!
//! # Status
//!
//! Types and ABI are defined. Runtime loading is not yet implemented.
//! See `docs/EFFECT_SDK.md` for the developer workflow.
//!
//! # Safety
//!
//! All types in this module use `#[repr(C)]` for a stable ABI. Raw pointers
//! in [`EffectDescriptorC`] must point to valid, null-terminated C strings
//! with `'static` lifetime (e.g., string literals in the shared library).
//! The `instance` pointer in [`EffectVTable`] must be the value returned by
//! the corresponding `create` call. Passing mismatched pointers is undefined
//! behaviour.

/// C-compatible effect descriptor for dynamic loading.
///
/// Populated by the shared library and read by the host to discover the
/// effect's identity and parameter count before instantiation.
///
/// # Safety
///
/// `id` and `name` must be valid, null-terminated C strings. Typically
/// implemented as `b"my_effect\0".as_ptr() as *const core::ffi::c_char`
/// in the exporting crate.
#[repr(C)]
pub struct EffectDescriptorC {
    /// Stable ASCII identifier (e.g., `"my_distortion"`). Used for
    /// persistence and registry lookup.
    pub id: *const core::ffi::c_char,
    /// Human-readable display name (e.g., `"My Distortion"`).
    pub name: *const core::ffi::c_char,
    /// Number of parameters exposed by this effect.
    pub param_count: u32,
    /// Effect version for compatibility checking (semver-encoded as
    /// `major * 10000 + minor * 100 + patch`).
    pub version: u32,
}

/// C-compatible function pointer table for a dynamically loaded effect.
///
/// The host uses this table to call into the shared library without knowing
/// the concrete effect type. Each function receives the opaque `instance`
/// pointer returned by `create`.
///
/// # Safety
///
/// All function pointers must be non-null and point to functions with
/// matching signatures in the shared library. The `instance` pointer
/// passed to each function must be the value originally returned by
/// `create`; using a pointer after `destroy` is undefined behaviour.
#[repr(C)]
pub struct EffectVTable {
    /// Allocate and initialise a new effect instance at the given sample rate.
    ///
    /// Returns an opaque heap pointer; never null on success. The caller
    /// must pass this pointer to every other function and eventually to
    /// `destroy`.
    pub create: extern "C" fn(sample_rate: f32) -> *mut core::ffi::c_void,

    /// Deallocate the effect instance. Must be called exactly once per
    /// `create` call.
    pub destroy: extern "C" fn(instance: *mut core::ffi::c_void),

    /// Process one stereo sample in place.
    ///
    /// `out_left` and `out_right` must be valid, non-null pointers to
    /// writable `f32` locations.
    pub process_stereo: extern "C" fn(
        instance: *mut core::ffi::c_void,
        left: f32,
        right: f32,
        out_left: *mut f32,
        out_right: *mut f32,
    ),

    /// Set a parameter value by index (0-based, up to `param_count - 1`).
    ///
    /// Values are in user-facing units as defined by the effect's descriptors.
    pub set_param: extern "C" fn(instance: *mut core::ffi::c_void, index: u32, value: f32),

    /// Get the current value of a parameter by index.
    pub get_param: extern "C" fn(instance: *mut core::ffi::c_void, index: u32) -> f32,

    /// Reset all internal DSP state (delay buffers, filter history, etc.).
    ///
    /// Equivalent to [`Effect::reset()`](crate::Effect::reset).
    pub reset: extern "C" fn(instance: *mut core::ffi::c_void),
}
