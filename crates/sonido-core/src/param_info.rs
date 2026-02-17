//! Parameter introspection system for discoverable effect parameters.
//!
//! This module provides the [`ParameterInfo`] trait and supporting types that enable
//! runtime discovery and manipulation of effect parameters. This is essential for:
//!
//! - **GUI applications**: Automatically generate parameter controls
//! - **Hardware controllers**: Map MIDI CC or encoder knobs to parameters
//! - **Preset systems**: Save and restore parameter state
//! - **Host automation**: DAW parameter automation and CLAP/VST3 integration
//!
//! # Design
//!
//! The system uses index-based parameter access for efficiency and simplicity.
//! Each parameter is described by a [`ParamDescriptor`] containing metadata for
//! display, validation, and plugin host communication. Parameters also carry:
//!
//! - [`ParamId`] — stable numeric ID for automation recording and preset persistence
//! - [`ParamScale`] — normalization curve (linear, logarithmic, power)
//! - [`ParamFlags`] — capability flags for plugin hosts (automatable, stepped, etc.)
//! - `string_id` — human-readable stable ID for debugging and serialization
//! - `group` — parameter grouping for host tree display
//!
//! # Example
//!
//! ```rust
//! use sonido_core::{ParameterInfo, ParamDescriptor, ParamUnit, ParamId, ParamScale, ParamFlags};
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
//!             0 => Some(ParamDescriptor::gain_db("Gain", "Gain", -60.0, 12.0, 0.0)
//!                 .with_id(ParamId(100), "gain_level")),
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

/// Scaling curve for parameter normalization.
///
/// Determines how a parameter's plain value maps to normalized \[0.0, 1.0\] space.
/// Linear is default. Use Logarithmic for frequency parameters (20 Hz–20 kHz),
/// Power for parameters that need more resolution at one end.
///
/// # Normalization Formulas
///
/// - **Linear**: `normalized = (value - min) / (max - min)`
/// - **Logarithmic**: `normalized = ln(value/min) / ln(max/min)`
/// - **Power(exp)**: `normalized = ((value - min) / (max - min)).powf(1.0 / exp)`
///
/// Reference: JUCE `NormalisableRange` (skew factor), iPlug2 `ShapePowCurve`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ParamScale {
    /// Linear mapping (default). Equal resolution across the range.
    #[default]
    Linear,
    /// Logarithmic mapping. More resolution at low values.
    /// Ideal for frequency parameters (20 Hz → 20 kHz).
    /// Requires `min > 0.0`.
    Logarithmic,
    /// Power curve mapping with configurable exponent.
    /// exponent < 1.0 → more resolution at low end.
    /// exponent > 1.0 → more resolution at high end.
    /// Equivalent to JUCE's `NormalisableRange` skew factor.
    Power(f32),
}

/// Stable parameter identifier that survives reordering.
///
/// Used by plugin hosts for automation recording, preset save/restore,
/// and parameter mapping. Once assigned, a `ParamId` MUST NEVER change
/// for a given parameter — it's part of the public API contract.
///
/// Maps directly to CLAP `clap_id` and VST3 `ParamID`.
///
/// # Convention
///
/// Each effect gets a base ID; params are sequential from there:
/// - Distortion: 200, 201, 202, 203
/// - Reverb: 1500, 1501, 1502, ...
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ParamId(pub u32);

/// Parameter capability flags for plugin host communication.
///
/// Bitflag type that maps to CLAP `clap_param_info_flags` and
/// VST3 `ParameterInfo::flags`. Use [`union`](Self::union) to combine.
///
/// # Example
///
/// ```rust
/// use sonido_core::ParamFlags;
///
/// let flags = ParamFlags::AUTOMATABLE.union(ParamFlags::STEPPED);
/// assert!(flags.contains(ParamFlags::AUTOMATABLE));
/// assert!(flags.contains(ParamFlags::STEPPED));
/// assert!(!flags.contains(ParamFlags::HIDDEN));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParamFlags(u8);

impl ParamFlags {
    /// No flags set.
    pub const NONE: Self = Self(0);
    /// Host can automate this parameter (default for all params).
    pub const AUTOMATABLE: Self = Self(1 << 0);
    /// Parameter has discrete steps (enum-like, integer values).
    pub const STEPPED: Self = Self(1 << 1);
    /// Parameter should be hidden from generic host UI.
    pub const HIDDEN: Self = Self(1 << 2);
    /// Parameter is read-only (metering, display only).
    pub const READ_ONLY: Self = Self(1 << 3);
    /// Parameter supports non-destructive modulation (CLAP `CLAP_PARAM_IS_MODULATABLE`).
    ///
    /// Modulated value = base value + modulation offset. The base value is preserved
    /// when the modulation source stops. No VST3 equivalent — VST3 treats all
    /// parameter changes as automation.
    ///
    /// Requires `modulation_id` in the descriptor for CLAP host routing.
    pub const MODULATABLE: Self = Self(1 << 4);

    /// Returns `true` if all bits in `other` are set in `self`.
    #[inline]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Returns the union of two flag sets.
    #[inline]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

impl Default for ParamFlags {
    fn default() -> Self {
        Self::AUTOMATABLE
    }
}

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
/// use sonido_core::{ParameterInfo, ParamDescriptor, ParamId};
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
///             0 => Some(ParamDescriptor::gain_db("Threshold", "Thresh", -60.0, 0.0, -20.0)
///                 .with_id(ParamId(300), "comp_thresh")),
///             1 => Some(ParamDescriptor::time_ms("Ratio", "Ratio", 1.0, 20.0, 4.0)
///                 .with_id(ParamId(301), "comp_ratio")),
///             2 => Some(ParamDescriptor::time_ms("Attack", "Attack", 0.1, 100.0, 10.0)
///                 .with_id(ParamId(302), "comp_attack")),
///             3 => Some(ParamDescriptor::time_ms("Release", "Release", 10.0, 1000.0, 100.0)
///                 .with_id(ParamId(303), "comp_release")),
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

    /// Returns the stable [`ParamId`] for the parameter at the given index.
    ///
    /// Default implementation reads it from the descriptor. Returns `None`
    /// if the index is out of range.
    fn param_id(&self, index: usize) -> Option<ParamId> {
        self.param_info(index).map(|d| d.id)
    }

    /// Finds a parameter index by its stable [`ParamId`].
    ///
    /// Scans all parameters (O(n)) — suitable for setup paths, not audio.
    fn param_index_by_id(&self, id: ParamId) -> Option<usize> {
        (0..self.param_count()).find(|&i| self.param_info(i).is_some_and(|d| d.id == id))
    }
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
/// use sonido_core::{ParamDescriptor, ParamId};
///
/// let delay_time = ParamDescriptor::time_ms("Delay Time", "Time", 1.0, 2000.0, 250.0)
///     .with_id(ParamId(1100), "dly_time");
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

    /// Stable numeric ID for plugin host automation and preset persistence.
    ///
    /// Maps directly to CLAP `clap_id` and VST3 `ParamID`. Once assigned,
    /// this value must never change for a given parameter.
    /// Default: `ParamId(0)` (unassigned).
    pub id: ParamId,

    /// Human-readable stable ID for presets, debugging, and serialization.
    ///
    /// Convention: `"effect_param"` (e.g., `"dist_drive"`, `"rev_decay"`).
    /// Default: `""` (unassigned).
    pub string_id: &'static str,

    /// Normalization curve for mapping between plain and normalized values.
    ///
    /// Default: [`ParamScale::Linear`].
    pub scale: ParamScale,

    /// Capability flags for plugin host communication.
    ///
    /// Default: [`ParamFlags::AUTOMATABLE`].
    pub flags: ParamFlags,

    /// Parameter group for host tree display (e.g., `"filter"`, `"modulation"`).
    ///
    /// Empty string means top-level (ungrouped). Used by CLAP hosts to
    /// organize parameters hierarchically.
    pub group: &'static str,

    /// Optional modulation routing ID for CLAP hosts.
    ///
    /// When `Some`, the host can apply non-destructive modulation to this parameter.
    /// Must be unique across all modulatable parameters in the plugin.
    /// Mirrors nih-plug's `poly_modulation_id()` approach.
    pub modulation_id: Option<u32>,
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
            id: ParamId(0),
            string_id: "",
            scale: ParamScale::Linear,
            flags: ParamFlags::AUTOMATABLE,
            group: "",
            modulation_id: None,
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
            id: ParamId(0),
            string_id: "",
            scale: ParamScale::Linear,
            flags: ParamFlags::AUTOMATABLE,
            group: "",
            modulation_id: None,
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
            id: ParamId(0),
            string_id: "",
            scale: ParamScale::Linear,
            flags: ParamFlags::AUTOMATABLE,
            group: "",
            modulation_id: None,
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
            id: ParamId(0),
            string_id: "",
            scale: ParamScale::Linear,
            flags: ParamFlags::AUTOMATABLE,
            group: "",
            modulation_id: None,
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
            id: ParamId(0),
            string_id: "",
            scale: ParamScale::Linear,
            flags: ParamFlags::AUTOMATABLE,
            group: "",
            modulation_id: None,
        }
    }

    /// Standard LFO rate parameter in Hz.
    ///
    /// Uses logarithmic scaling for perceptually uniform rate control.
    ///
    /// # Arguments
    ///
    /// * `min` - Minimum rate in Hz
    /// * `max` - Maximum rate in Hz
    /// * `default` - Default rate in Hz
    pub fn rate_hz(min: f32, max: f32, default: f32) -> Self {
        Self {
            name: "Rate",
            short_name: "Rate",
            unit: ParamUnit::Hertz,
            min,
            max,
            default,
            step: 0.05,
            id: ParamId(0),
            string_id: "",
            scale: ParamScale::Logarithmic,
            flags: ParamFlags::AUTOMATABLE,
            group: "",
            modulation_id: None,
        }
    }

    /// Sets the stable parameter ID and string ID.
    ///
    /// Builder pattern — call after a factory method or struct literal.
    ///
    /// # Example
    ///
    /// ```rust
    /// use sonido_core::{ParamDescriptor, ParamId};
    ///
    /// let desc = ParamDescriptor::mix().with_id(ParamId(700), "chor_mix");
    /// assert_eq!(desc.id, ParamId(700));
    /// assert_eq!(desc.string_id, "chor_mix");
    /// ```
    pub const fn with_id(mut self, id: ParamId, string_id: &'static str) -> Self {
        self.id = id;
        self.string_id = string_id;
        self
    }

    /// Sets the normalization scale.
    ///
    /// Builder pattern — call after a factory method or struct literal.
    pub const fn with_scale(mut self, scale: ParamScale) -> Self {
        self.scale = scale;
        self
    }

    /// Sets the parameter flags.
    ///
    /// Builder pattern — call after a factory method or struct literal.
    pub const fn with_flags(mut self, flags: ParamFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Sets the parameter group.
    ///
    /// Builder pattern — call after a factory method or struct literal.
    pub const fn with_group(mut self, group: &'static str) -> Self {
        self.group = group;
        self
    }

    /// Sets the modulation routing ID for CLAP host integration.
    ///
    /// Builder pattern — call after a factory method or struct literal.
    /// Setting this also implies the parameter is modulatable.
    ///
    /// # Example
    ///
    /// ```rust
    /// use sonido_core::{ParamDescriptor, ParamId, ParamFlags};
    ///
    /// let desc = ParamDescriptor::mix()
    ///     .with_id(ParamId(700), "chor_mix")
    ///     .with_flags(ParamFlags::AUTOMATABLE.union(ParamFlags::MODULATABLE))
    ///     .with_modulation_id(700);
    /// assert_eq!(desc.modulation_id, Some(700));
    /// ```
    pub const fn with_modulation_id(mut self, id: u32) -> Self {
        self.modulation_id = Some(id);
        self
    }

    /// Clamps a value to this parameter's valid range.
    ///
    /// # Example
    ///
    /// ```rust
    /// use sonido_core::ParamDescriptor;
    ///
    /// let desc = ParamDescriptor::gain_db("Gain", "Gain", -60.0, 12.0, 0.0);
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

    /// Converts a plain value to normalized range (0.0 to 1.0).
    ///
    /// Respects the parameter's [`ParamScale`]:
    /// - **Linear**: `(value - min) / (max - min)`
    /// - **Logarithmic**: `ln(value/min) / ln(max/min)` — requires `min > 0`
    /// - **Power(exp)**: `((value - min) / (max - min)).powf(1.0 / exp)`
    ///
    /// # Example
    ///
    /// ```rust
    /// use sonido_core::ParamDescriptor;
    ///
    /// let desc = ParamDescriptor::mix();
    /// assert_eq!(desc.normalize(0.0), 0.0);
    /// assert_eq!(desc.normalize(50.0), 0.5);
    /// assert_eq!(desc.normalize(100.0), 1.0);
    /// ```
    #[inline]
    pub fn normalize(&self, value: f32) -> f32 {
        let range = self.max - self.min;
        if range == 0.0 {
            return 0.0;
        }
        match self.scale {
            ParamScale::Linear => (value - self.min) / range,
            ParamScale::Logarithmic => {
                if self.min <= 0.0 || value <= 0.0 {
                    return 0.0;
                }
                libm::logf(value / self.min) / libm::logf(self.max / self.min)
            }
            ParamScale::Power(exp) => {
                let linear = (value - self.min) / range;
                libm::powf(linear, 1.0 / exp)
            }
        }
    }

    /// Converts a normalized value (0.0 to 1.0) to the actual parameter range.
    ///
    /// Inverse of [`normalize`](Self::normalize), respecting [`ParamScale`].
    ///
    /// # Example
    ///
    /// ```rust
    /// use sonido_core::ParamDescriptor;
    ///
    /// let desc = ParamDescriptor::mix();
    /// assert_eq!(desc.denormalize(0.0), 0.0);
    /// assert_eq!(desc.denormalize(0.5), 50.0);
    /// assert_eq!(desc.denormalize(1.0), 100.0);
    /// ```
    #[inline]
    pub fn denormalize(&self, normalized: f32) -> f32 {
        match self.scale {
            ParamScale::Linear => self.min + normalized * (self.max - self.min),
            ParamScale::Logarithmic => {
                if self.min <= 0.0 {
                    return self.min;
                }
                self.min * libm::powf(self.max / self.min, normalized)
            }
            ParamScale::Power(exp) => {
                let curved = libm::powf(normalized, exp);
                self.min + curved * (self.max - self.min)
            }
        }
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
                0 => Some(
                    ParamDescriptor::gain_db("Gain", "Gain", -60.0, 12.0, 0.0)
                        .with_id(ParamId(100), "test_gain"),
                ),
                1 => Some(ParamDescriptor::mix().with_id(ParamId(101), "test_mix")),
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
        let desc = ParamDescriptor::mix(); // 0..100
        assert_eq!(desc.clamp(50.0), 50.0);
        assert_eq!(desc.clamp(-10.0), 0.0);
        assert_eq!(desc.clamp(200.0), 100.0);
        assert_eq!(desc.clamp(0.0), 0.0);
        assert_eq!(desc.clamp(100.0), 100.0);
    }

    #[test]
    fn test_normalize_denormalize_linear() {
        let desc = ParamDescriptor::mix(); // 0..100, linear

        assert_eq!(desc.normalize(0.0), 0.0);
        assert_eq!(desc.normalize(50.0), 0.5);
        assert_eq!(desc.normalize(100.0), 1.0);

        assert_eq!(desc.denormalize(0.0), 0.0);
        assert_eq!(desc.denormalize(0.5), 50.0);
        assert_eq!(desc.denormalize(1.0), 100.0);

        // Round-trip
        let original = 73.0;
        let rt = desc.denormalize(desc.normalize(original));
        assert!((rt - original).abs() < 0.001);
    }

    #[test]
    fn test_normalize_denormalize_logarithmic() {
        let desc = ParamDescriptor::rate_hz(20.0, 20000.0, 1000.0); // logarithmic

        // Endpoints
        assert!((desc.normalize(20.0) - 0.0).abs() < 1e-6);
        assert!((desc.normalize(20000.0) - 1.0).abs() < 1e-6);

        // Midpoint in log space: sqrt(20 * 20000) ≈ 632.5
        let mid = desc.denormalize(0.5);
        let expected_mid = libm::sqrtf(20.0 * 20000.0);
        assert!(
            (mid - expected_mid).abs() < 1.0,
            "log midpoint: expected ~{expected_mid}, got {mid}"
        );

        // Round-trip
        for &val in &[20.0, 100.0, 1000.0, 5000.0, 20000.0] {
            let rt = desc.denormalize(desc.normalize(val));
            assert!(
                (rt - val).abs() / val < 1e-4,
                "log round-trip failed for {val}: got {rt}"
            );
        }
    }

    #[test]
    fn test_normalize_denormalize_power() {
        let desc = ParamDescriptor::depth().with_scale(ParamScale::Power(2.0));

        // Endpoints unchanged
        assert_eq!(desc.normalize(0.0), 0.0);
        assert_eq!(desc.normalize(100.0), 1.0);

        // Power(2): normalize(x) = sqrt(x/100), denormalize(n) = n^2 * 100
        let n = desc.normalize(25.0); // sqrt(0.25) = 0.5
        assert!(
            (n - 0.5).abs() < 1e-6,
            "power normalize: expected 0.5, got {n}"
        );

        let v = desc.denormalize(0.5); // 0.5^2 * 100 = 25
        assert!(
            (v - 25.0).abs() < 1e-4,
            "power denormalize: expected 25, got {v}"
        );

        // Round-trip
        for &val in &[0.0, 10.0, 25.0, 50.0, 75.0, 100.0] {
            let rt = desc.denormalize(desc.normalize(val));
            assert!(
                (rt - val).abs() < 0.01,
                "power round-trip failed for {val}: got {rt}"
            );
        }
    }

    #[test]
    fn test_normalize_zero_range() {
        let desc = ParamDescriptor::gain_db("Fixed", "Fixed", 42.0, 42.0, 42.0);
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
        let _ = format!("{:?}", ParamUnit::Decibels);
    }

    #[test]
    fn test_descriptor_debug_clone() {
        let desc = ParamDescriptor::depth();

        // Test Debug
        let _ = format!("{:?}", desc);

        // Test Clone
        let cloned = desc;
        assert_eq!(cloned.name, desc.name);

        // Test PartialEq
        assert_eq!(desc, cloned);
    }

    #[test]
    fn test_rate_hz_factory() {
        let desc = ParamDescriptor::rate_hz(0.05, 5.0, 0.5);
        assert_eq!(desc.name, "Rate");
        assert_eq!(desc.short_name, "Rate");
        assert_eq!(desc.unit, ParamUnit::Hertz);
        assert_eq!(desc.min, 0.05);
        assert_eq!(desc.max, 5.0);
        assert_eq!(desc.default, 0.5);
        assert_eq!(desc.step, 0.05);
        assert_eq!(desc.scale, ParamScale::Logarithmic);
    }

    #[test]
    fn test_param_id_lookup() {
        let effect = TestEffect::new();

        assert_eq!(effect.param_id(0), Some(ParamId(100)));
        assert_eq!(effect.param_id(1), Some(ParamId(101)));
        assert_eq!(effect.param_id(2), None);

        assert_eq!(effect.param_index_by_id(ParamId(100)), Some(0));
        assert_eq!(effect.param_index_by_id(ParamId(101)), Some(1));
        assert_eq!(effect.param_index_by_id(ParamId(999)), None);
    }

    #[test]
    fn test_param_flags() {
        assert!(ParamFlags::AUTOMATABLE.contains(ParamFlags::AUTOMATABLE));
        assert!(!ParamFlags::AUTOMATABLE.contains(ParamFlags::STEPPED));
        assert!(!ParamFlags::NONE.contains(ParamFlags::AUTOMATABLE));

        let combined = ParamFlags::AUTOMATABLE.union(ParamFlags::STEPPED);
        assert!(combined.contains(ParamFlags::AUTOMATABLE));
        assert!(combined.contains(ParamFlags::STEPPED));
        assert!(!combined.contains(ParamFlags::HIDDEN));
    }

    #[test]
    fn test_with_id_builder() {
        let desc = ParamDescriptor::mix().with_id(ParamId(42), "test_mix");
        assert_eq!(desc.id, ParamId(42));
        assert_eq!(desc.string_id, "test_mix");
        assert_eq!(desc.name, "Mix"); // unchanged
    }

    #[test]
    fn test_with_scale_builder() {
        let desc = ParamDescriptor::depth().with_scale(ParamScale::Power(3.0));
        assert_eq!(desc.scale, ParamScale::Power(3.0));
        assert_eq!(desc.name, "Depth"); // unchanged
    }

    #[test]
    fn test_with_flags_builder() {
        let desc =
            ParamDescriptor::mix().with_flags(ParamFlags::AUTOMATABLE.union(ParamFlags::STEPPED));
        assert!(desc.flags.contains(ParamFlags::STEPPED));
    }

    #[test]
    fn test_defaults() {
        let desc = ParamDescriptor::mix();
        assert_eq!(desc.id, ParamId(0));
        assert_eq!(desc.string_id, "");
        assert_eq!(desc.scale, ParamScale::Linear);
        assert_eq!(desc.flags, ParamFlags::AUTOMATABLE);
        assert_eq!(desc.group, "");
        assert_eq!(desc.modulation_id, None);
    }

    #[test]
    fn test_modulatable_flag() {
        let flags = ParamFlags::AUTOMATABLE.union(ParamFlags::MODULATABLE);
        assert!(flags.contains(ParamFlags::MODULATABLE));
        assert!(flags.contains(ParamFlags::AUTOMATABLE));
        assert!(!flags.contains(ParamFlags::STEPPED));
    }

    #[test]
    fn test_modulation_id_builder() {
        let desc = ParamDescriptor::mix().with_modulation_id(42);
        assert_eq!(desc.modulation_id, Some(42));

        let desc_none = ParamDescriptor::mix();
        assert_eq!(desc_none.modulation_id, None);
    }
}
