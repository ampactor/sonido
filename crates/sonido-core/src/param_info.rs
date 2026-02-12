//! Parameter introspection system for discoverable effect parameters.
//!
//! This module provides the [`ParameterInfo`] trait and supporting types that enable
//! runtime discovery and manipulation of effect parameters. This is essential for:
//!
//! - **GUI applications**: Automatically generate parameter controls
//! - **Hardware controllers**: Map MIDI CC or encoder knobs to parameters
//! - **Preset systems**: Save and restore parameter state
//! - **Host automation**: DAW parameter automation support
//!
//! # Design
//!
//! The system uses index-based parameter access for efficiency and simplicity.
//! Each parameter is described by a [`ParamDescriptor`] containing metadata for
//! display and validation.
//!
//! # Example
//!
//! ```rust
//! use sonido_core::{ParameterInfo, ParamDescriptor, ParamUnit};
//!
//! struct SimpleGain {
//!     gain_db: f32,
//! }
//!
//! impl ParameterInfo for SimpleGain {
//!     fn param_count(&self) -> usize { 1 }
//!
//!     fn param_info(&self, index: usize) -> Option<ParamDescriptor> {
//!         match index {
//!             0 => Some(ParamDescriptor {
//!                 name: "Gain",
//!                 short_name: "Gain",
//!                 unit: ParamUnit::Decibels,
//!                 min: -60.0,
//!                 max: 12.0,
//!                 default: 0.0,
//!                 step: 0.1,
//!             }),
//!             _ => None,
//!         }
//!     }
//!
//!     fn get_param(&self, index: usize) -> f32 {
//!         match index {
//!             0 => self.gain_db,
//!             _ => 0.0,
//!         }
//!     }
//!
//!     fn set_param(&mut self, index: usize, value: f32) {
//!         match index {
//!             0 => self.gain_db = value.clamp(-60.0, 12.0),
//!             _ => {}
//!         }
//!     }
//! }
//! ```
//!
//! # no_std Support
//!
//! This module is fully `no_std` compatible with no heap allocations required.

/// Trait for effects that expose introspectable parameters.
///
/// Implement this trait to allow runtime discovery and manipulation of your
/// effect's parameters. This enables automatic GUI generation, preset systems,
/// and MIDI/hardware controller mapping.
///
/// # Parameter Indexing
///
/// Parameters are accessed by zero-based index. The index must be stable for
/// the lifetime of the effect instance. Use [`param_count`](Self::param_count)
/// to determine valid indices.
///
/// # Thread Safety
///
/// This trait does not require thread safety. If you need to access parameters
/// from multiple threads, wrap the effect in appropriate synchronization
/// primitives (e.g., `Mutex` or atomic parameters).
///
/// # Example
///
/// ```rust
/// use sonido_core::{ParameterInfo, ParamDescriptor, ParamUnit};
///
/// struct Compressor {
///     threshold_db: f32,
///     ratio: f32,
///     attack_ms: f32,
///     release_ms: f32,
/// }
///
/// impl ParameterInfo for Compressor {
///     fn param_count(&self) -> usize { 4 }
///
///     fn param_info(&self, index: usize) -> Option<ParamDescriptor> {
///         match index {
///             0 => Some(ParamDescriptor {
///                 name: "Threshold",
///                 short_name: "Thresh",
///                 unit: ParamUnit::Decibels,
///                 min: -60.0,
///                 max: 0.0,
///                 default: -20.0,
///                 step: 0.5,
///             }),
///             1 => Some(ParamDescriptor {
///                 name: "Ratio",
///                 short_name: "Ratio",
///                 unit: ParamUnit::Ratio,
///                 min: 1.0,
///                 max: 20.0,
///                 default: 4.0,
///                 step: 0.1,
///             }),
///             2 => Some(ParamDescriptor {
///                 name: "Attack",
///                 short_name: "Attack",
///                 unit: ParamUnit::Milliseconds,
///                 min: 0.1,
///                 max: 100.0,
///                 default: 10.0,
///                 step: 0.1,
///             }),
///             3 => Some(ParamDescriptor {
///                 name: "Release",
///                 short_name: "Release",
///                 unit: ParamUnit::Milliseconds,
///                 min: 10.0,
///                 max: 1000.0,
///                 default: 100.0,
///                 step: 1.0,
///             }),
///             _ => None,
///         }
///     }
///
///     fn get_param(&self, index: usize) -> f32 {
///         match index {
///             0 => self.threshold_db,
///             1 => self.ratio,
///             2 => self.attack_ms,
///             3 => self.release_ms,
///             _ => 0.0,
///         }
///     }
///
///     fn set_param(&mut self, index: usize, value: f32) {
///         match index {
///             0 => self.threshold_db = value.clamp(-60.0, 0.0),
///             1 => self.ratio = value.clamp(1.0, 20.0),
///             2 => self.attack_ms = value.clamp(0.1, 100.0),
///             3 => self.release_ms = value.clamp(10.0, 1000.0),
///             _ => {}
///         }
///     }
/// }
/// ```
pub trait ParameterInfo {
    /// Returns the number of parameters this effect exposes.
    ///
    /// Valid parameter indices are `0..param_count()`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use sonido_core::ParameterInfo;
    ///
    /// fn iterate_params<T: ParameterInfo>(effect: &T) {
    ///     for i in 0..effect.param_count() {
    ///         if let Some(info) = effect.param_info(i) {
    ///             // Use parameter info
    ///         }
    ///     }
    /// }
    /// ```
    fn param_count(&self) -> usize;

    /// Returns the descriptor for the parameter at the given index.
    ///
    /// Returns `None` if `index >= param_count()`.
    ///
    /// # Arguments
    ///
    /// * `index` - Zero-based parameter index
    ///
    /// # Example
    ///
    /// ```rust
    /// use sonido_core::{ParameterInfo, ParamUnit};
    ///
    /// fn print_param_range<T: ParameterInfo>(effect: &T, index: usize) {
    ///     if let Some(desc) = effect.param_info(index) {
    ///         println!("{}: {} to {}", desc.name, desc.min, desc.max);
    ///     }
    /// }
    /// ```
    fn param_info(&self, index: usize) -> Option<ParamDescriptor>;

    /// Gets the current value of the parameter at the given index.
    ///
    /// Returns `0.0` if `index >= param_count()` (implementations should handle
    /// out-of-bounds gracefully).
    ///
    /// # Arguments
    ///
    /// * `index` - Zero-based parameter index
    ///
    /// # Example
    ///
    /// ```rust
    /// use sonido_core::ParameterInfo;
    ///
    /// fn save_preset<T: ParameterInfo>(effect: &T) -> Vec<f32> {
    ///     (0..effect.param_count())
    ///         .map(|i| effect.get_param(i))
    ///         .collect()
    /// }
    /// ```
    fn get_param(&self, index: usize) -> f32;

    /// Find a parameter index by name (case-insensitive).
    ///
    /// Matches against both [`ParamDescriptor::name`] and
    /// [`ParamDescriptor::short_name`].
    ///
    /// # Returns
    ///
    /// `Some(index)` if found, `None` if no parameter matches.
    fn find_param_by_name(&self, name: &str) -> Option<usize> {
        for i in 0..self.param_count() {
            if let Some(desc) = self.param_info(i)
                && (desc.name.eq_ignore_ascii_case(name)
                    || desc.short_name.eq_ignore_ascii_case(name))
            {
                return Some(i);
            }
        }
        None
    }

    /// Sets the value of the parameter at the given index.
    ///
    /// Implementations should clamp the value to the valid range specified
    /// in the parameter descriptor. Out-of-bounds indices should be ignored.
    ///
    /// # Arguments
    ///
    /// * `index` - Zero-based parameter index
    /// * `value` - New parameter value (will be clamped to valid range)
    ///
    /// # Example
    ///
    /// ```rust
    /// use sonido_core::ParameterInfo;
    ///
    /// fn load_preset<T: ParameterInfo>(effect: &mut T, values: &[f32]) {
    ///     for (i, &value) in values.iter().enumerate() {
    ///         effect.set_param(i, value);
    ///     }
    /// }
    /// ```
    fn set_param(&mut self, index: usize, value: f32);
}

/// Describes a single parameter's metadata for display and validation.
///
/// This struct provides all the information needed to:
/// - Display the parameter in a GUI or on hardware
/// - Validate parameter values
/// - Convert between normalized (0.0-1.0) and actual values
/// - Format values with appropriate units
///
/// # Short Name
///
/// The `short_name` field should be 8 characters or less for compatibility
/// with hardware displays (e.g., LCD screens on MIDI controllers or guitar
/// pedals like DigiTech audioDNA units).
///
/// # Step Size
///
/// The `step` field indicates the recommended increment for encoder-based
/// control. For continuous parameters, use a small value like `0.01`. For
/// discrete parameters, use `1.0`.
///
/// # Example
///
/// ```rust
/// use sonido_core::{ParamDescriptor, ParamUnit};
///
/// let delay_time = ParamDescriptor {
///     name: "Delay Time",
///     short_name: "Time",
///     unit: ParamUnit::Milliseconds,
///     min: 1.0,
///     max: 2000.0,
///     default: 250.0,
///     step: 1.0,
/// };
///
/// // Format for display
/// let value = 500.0;
/// println!("{}: {} ms", delay_time.name, value);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParamDescriptor {
    /// Full parameter name for display (e.g., "Delay Time", "Feedback Level").
    pub name: &'static str,

    /// Short name for hardware displays, max 8 characters (e.g., "Time", "Feedback").
    ///
    /// Keep this concise for compatibility with LCD screens on MIDI controllers
    /// and hardware effects processors.
    pub short_name: &'static str,

    /// Unit type for formatting the parameter value.
    pub unit: ParamUnit,

    /// Minimum allowed value for this parameter.
    pub min: f32,

    /// Maximum allowed value for this parameter.
    pub max: f32,

    /// Default value when the effect is initialized or reset.
    pub default: f32,

    /// Recommended step increment for encoder-based control.
    ///
    /// Use small values (e.g., `0.01`) for continuous parameters and
    /// larger values (e.g., `1.0`) for discrete or coarse parameters.
    pub step: f32,
}

impl ParamDescriptor {
    /// Standard mix parameter (0–100%, default 50%).
    ///
    /// Used by most effects with a wet/dry blend control.
    pub fn mix() -> Self {
        Self {
            name: "Mix",
            short_name: "Mix",
            unit: ParamUnit::Percent,
            min: 0.0,
            max: 100.0,
            default: 50.0,
            step: 1.0,
        }
    }

    /// Standard depth parameter (0–100%, default 50%).
    ///
    /// Used by modulation effects (chorus, flanger, phaser, vibrato).
    pub fn depth() -> Self {
        Self {
            name: "Depth",
            short_name: "Depth",
            unit: ParamUnit::Percent,
            min: 0.0,
            max: 100.0,
            default: 50.0,
            step: 1.0,
        }
    }

    /// Standard feedback parameter (0–95%, default 50%).
    ///
    /// Used by delay-based effects (delay, flanger, chorus).
    /// Capped at 95% to prevent runaway oscillation.
    pub fn feedback() -> Self {
        Self {
            name: "Feedback",
            short_name: "Fdbk",
            unit: ParamUnit::Percent,
            min: 0.0,
            max: 95.0,
            default: 50.0,
            step: 1.0,
        }
    }

    /// Time parameter with custom name and range (milliseconds).
    ///
    /// # Arguments
    ///
    /// * `name` - Full parameter name (e.g., "Delay Time")
    /// * `short_name` - Short name for hardware displays (e.g., "Time")
    /// * `min` - Minimum time in ms
    /// * `max` - Maximum time in ms
    /// * `default` - Default time in ms
    pub fn time_ms(
        name: &'static str,
        short_name: &'static str,
        min: f32,
        max: f32,
        default: f32,
    ) -> Self {
        Self {
            name,
            short_name,
            unit: ParamUnit::Milliseconds,
            min,
            max,
            default,
            step: 1.0,
        }
    }

    /// Gain parameter with custom name and range (decibels).
    ///
    /// # Arguments
    ///
    /// * `name` - Full parameter name (e.g., "Makeup Gain")
    /// * `short_name` - Short name for hardware displays (e.g., "Makeup")
    /// * `min` - Minimum gain in dB
    /// * `max` - Maximum gain in dB
    /// * `default` - Default gain in dB
    pub fn gain_db(
        name: &'static str,
        short_name: &'static str,
        min: f32,
        max: f32,
        default: f32,
    ) -> Self {
        Self {
            name,
            short_name,
            unit: ParamUnit::Decibels,
            min,
            max,
            default,
            step: 0.5,
        }
    }

    /// Clamps a value to this parameter's valid range.
    ///
    /// # Example
    ///
    /// ```rust
    /// use sonido_core::{ParamDescriptor, ParamUnit};
    ///
    /// let desc = ParamDescriptor {
    ///     name: "Gain",
    ///     short_name: "Gain",
    ///     unit: ParamUnit::Decibels,
    ///     min: -60.0,
    ///     max: 12.0,
    ///     default: 0.0,
    ///     step: 0.1,
    /// };
    ///
    /// assert_eq!(desc.clamp(0.0), 0.0);
    /// assert_eq!(desc.clamp(-100.0), -60.0);
    /// assert_eq!(desc.clamp(100.0), 12.0);
    /// ```
    #[inline]
    pub fn clamp(&self, value: f32) -> f32 {
        if value < self.min {
            self.min
        } else if value > self.max {
            self.max
        } else {
            value
        }
    }

    /// Converts a value to normalized range (0.0 to 1.0).
    ///
    /// This is useful for GUI sliders or MIDI CC mapping where the
    /// control range is always 0.0 to 1.0.
    ///
    /// # Example
    ///
    /// ```rust
    /// use sonido_core::{ParamDescriptor, ParamUnit};
    ///
    /// let desc = ParamDescriptor {
    ///     name: "Mix",
    ///     short_name: "Mix",
    ///     unit: ParamUnit::Percent,
    ///     min: 0.0,
    ///     max: 100.0,
    ///     default: 50.0,
    ///     step: 1.0,
    /// };
    ///
    /// assert_eq!(desc.normalize(0.0), 0.0);
    /// assert_eq!(desc.normalize(50.0), 0.5);
    /// assert_eq!(desc.normalize(100.0), 1.0);
    /// ```
    #[inline]
    pub fn normalize(&self, value: f32) -> f32 {
        let range = self.max - self.min;
        if range == 0.0 {
            0.0
        } else {
            (value - self.min) / range
        }
    }

    /// Converts a normalized value (0.0 to 1.0) to the actual parameter range.
    ///
    /// This is the inverse of [`normalize`](Self::normalize).
    ///
    /// # Example
    ///
    /// ```rust
    /// use sonido_core::{ParamDescriptor, ParamUnit};
    ///
    /// let desc = ParamDescriptor {
    ///     name: "Mix",
    ///     short_name: "Mix",
    ///     unit: ParamUnit::Percent,
    ///     min: 0.0,
    ///     max: 100.0,
    ///     default: 50.0,
    ///     step: 1.0,
    /// };
    ///
    /// assert_eq!(desc.denormalize(0.0), 0.0);
    /// assert_eq!(desc.denormalize(0.5), 50.0);
    /// assert_eq!(desc.denormalize(1.0), 100.0);
    /// ```
    #[inline]
    pub fn denormalize(&self, normalized: f32) -> f32 {
        self.min + normalized * (self.max - self.min)
    }
}

/// Unit type for parameter display and formatting.
///
/// This enum helps GUI applications and hardware displays format parameter
/// values with appropriate units and precision.
///
/// # Example
///
/// ```rust
/// use sonido_core::ParamUnit;
///
/// fn format_value(value: f32, unit: ParamUnit) -> String {
///     match unit {
///         ParamUnit::Decibels => format!("{:.1} dB", value),
///         ParamUnit::Hertz => format!("{:.0} Hz", value),
///         ParamUnit::Milliseconds => format!("{:.0} ms", value),
///         ParamUnit::Percent => format!("{:.0}%", value),
///         ParamUnit::Ratio => format!("{:.1}:1", value),
///         ParamUnit::None => format!("{:.2}", value),
///     }
/// }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParamUnit {
    /// Decibels (dB) - for gain, threshold, and level parameters.
    Decibels,

    /// Hertz (Hz) - for frequency parameters like filter cutoff or LFO rate.
    Hertz,

    /// Milliseconds (ms) - for time parameters like delay, attack, release.
    Milliseconds,

    /// Percentage (%) - for mix, blend, and normalized parameters.
    Percent,

    /// Ratio (n:1) - for compressor ratios and similar.
    Ratio,

    /// No unit - for dimensionless or custom parameters.
    None,
}

impl ParamUnit {
    /// Returns the unit suffix string for display.
    ///
    /// # Example
    ///
    /// ```rust
    /// use sonido_core::ParamUnit;
    ///
    /// assert_eq!(ParamUnit::Decibels.suffix(), " dB");
    /// assert_eq!(ParamUnit::Hertz.suffix(), " Hz");
    /// assert_eq!(ParamUnit::None.suffix(), "");
    /// ```
    pub const fn suffix(&self) -> &'static str {
        match self {
            ParamUnit::Decibels => " dB",
            ParamUnit::Hertz => " Hz",
            ParamUnit::Milliseconds => " ms",
            ParamUnit::Percent => "%",
            ParamUnit::Ratio => ":1",
            ParamUnit::None => "",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(feature = "std"))]
    extern crate alloc;
    #[cfg(not(feature = "std"))]
    use alloc::format;

    // Test struct for ParameterInfo implementation
    struct TestEffect {
        gain: f32,
        mix: f32,
    }

    impl TestEffect {
        fn new() -> Self {
            Self {
                gain: 0.0,
                mix: 50.0,
            }
        }
    }

    impl ParameterInfo for TestEffect {
        fn param_count(&self) -> usize {
            2
        }

        fn param_info(&self, index: usize) -> Option<ParamDescriptor> {
            match index {
                0 => Some(ParamDescriptor {
                    name: "Gain",
                    short_name: "Gain",
                    unit: ParamUnit::Decibels,
                    min: -60.0,
                    max: 12.0,
                    default: 0.0,
                    step: 0.1,
                }),
                1 => Some(ParamDescriptor {
                    name: "Mix",
                    short_name: "Mix",
                    unit: ParamUnit::Percent,
                    min: 0.0,
                    max: 100.0,
                    default: 50.0,
                    step: 1.0,
                }),
                _ => None,
            }
        }

        fn get_param(&self, index: usize) -> f32 {
            match index {
                0 => self.gain,
                1 => self.mix,
                _ => 0.0,
            }
        }

        fn set_param(&mut self, index: usize, value: f32) {
            match index {
                0 => {
                    if let Some(desc) = self.param_info(0) {
                        self.gain = desc.clamp(value);
                    }
                }
                1 => {
                    if let Some(desc) = self.param_info(1) {
                        self.mix = desc.clamp(value);
                    }
                }
                _ => {}
            }
        }
    }

    #[test]
    fn test_param_count() {
        let effect = TestEffect::new();
        assert_eq!(effect.param_count(), 2);
    }

    #[test]
    fn test_param_info() {
        let effect = TestEffect::new();

        let gain_info = effect.param_info(0).expect("should have gain param");
        assert_eq!(gain_info.name, "Gain");
        assert_eq!(gain_info.short_name, "Gain");
        assert_eq!(gain_info.unit, ParamUnit::Decibels);
        assert_eq!(gain_info.min, -60.0);
        assert_eq!(gain_info.max, 12.0);

        let mix_info = effect.param_info(1).expect("should have mix param");
        assert_eq!(mix_info.name, "Mix");
        assert_eq!(mix_info.unit, ParamUnit::Percent);

        assert!(effect.param_info(2).is_none());
        assert!(effect.param_info(100).is_none());
    }

    #[test]
    fn test_get_set_param() {
        let mut effect = TestEffect::new();

        assert_eq!(effect.get_param(0), 0.0);
        assert_eq!(effect.get_param(1), 50.0);

        effect.set_param(0, 6.0);
        assert_eq!(effect.get_param(0), 6.0);

        effect.set_param(1, 75.0);
        assert_eq!(effect.get_param(1), 75.0);
    }

    #[test]
    fn test_param_clamping() {
        let mut effect = TestEffect::new();

        // Test clamping to max
        effect.set_param(0, 100.0);
        assert_eq!(effect.get_param(0), 12.0);

        // Test clamping to min
        effect.set_param(0, -100.0);
        assert_eq!(effect.get_param(0), -60.0);

        // Test mix clamping
        effect.set_param(1, 150.0);
        assert_eq!(effect.get_param(1), 100.0);

        effect.set_param(1, -50.0);
        assert_eq!(effect.get_param(1), 0.0);
    }

    #[test]
    fn test_out_of_bounds_index() {
        let mut effect = TestEffect::new();

        // Out of bounds get should return 0.0
        assert_eq!(effect.get_param(99), 0.0);

        // Out of bounds set should do nothing
        effect.set_param(99, 42.0);
        // No panic, and existing params unchanged
        assert_eq!(effect.get_param(0), 0.0);
        assert_eq!(effect.get_param(1), 50.0);
    }

    #[test]
    fn test_descriptor_clamp() {
        let desc = ParamDescriptor {
            name: "Test",
            short_name: "Test",
            unit: ParamUnit::None,
            min: 0.0,
            max: 100.0,
            default: 50.0,
            step: 1.0,
        };

        assert_eq!(desc.clamp(50.0), 50.0);
        assert_eq!(desc.clamp(-10.0), 0.0);
        assert_eq!(desc.clamp(200.0), 100.0);
        assert_eq!(desc.clamp(0.0), 0.0);
        assert_eq!(desc.clamp(100.0), 100.0);
    }

    #[test]
    fn test_normalize_denormalize() {
        let desc = ParamDescriptor {
            name: "Freq",
            short_name: "Freq",
            unit: ParamUnit::Hertz,
            min: 20.0,
            max: 20000.0,
            default: 1000.0,
            step: 1.0,
        };

        // Test normalize
        assert_eq!(desc.normalize(20.0), 0.0);
        assert_eq!(desc.normalize(20000.0), 1.0);

        // Test denormalize
        assert_eq!(desc.denormalize(0.0), 20.0);
        assert_eq!(desc.denormalize(1.0), 20000.0);

        // Test round-trip
        let original = 5000.0;
        let normalized = desc.normalize(original);
        let denormalized = desc.denormalize(normalized);
        assert!((denormalized - original).abs() < 0.001);
    }

    #[test]
    fn test_normalize_zero_range() {
        let desc = ParamDescriptor {
            name: "Fixed",
            short_name: "Fixed",
            unit: ParamUnit::None,
            min: 42.0,
            max: 42.0,
            default: 42.0,
            step: 0.0,
        };

        // Zero range should return 0.0 to avoid division by zero
        assert_eq!(desc.normalize(42.0), 0.0);
    }

    #[test]
    fn test_param_unit_suffix() {
        assert_eq!(ParamUnit::Decibels.suffix(), " dB");
        assert_eq!(ParamUnit::Hertz.suffix(), " Hz");
        assert_eq!(ParamUnit::Milliseconds.suffix(), " ms");
        assert_eq!(ParamUnit::Percent.suffix(), "%");
        assert_eq!(ParamUnit::Ratio.suffix(), ":1");
        assert_eq!(ParamUnit::None.suffix(), "");
    }

    #[test]
    fn test_param_unit_debug() {
        // Ensure Debug is implemented
        let _ = format!("{:?}", ParamUnit::Decibels);
    }

    #[test]
    fn test_descriptor_debug_clone() {
        let desc = ParamDescriptor {
            name: "Test",
            short_name: "Test",
            unit: ParamUnit::None,
            min: 0.0,
            max: 1.0,
            default: 0.5,
            step: 0.01,
        };

        // Test Debug
        let _ = format!("{:?}", desc);

        // Test Clone
        let cloned = desc;
        assert_eq!(cloned.name, desc.name);

        // Test PartialEq
        assert_eq!(desc, cloned);
    }
}
