//! Audio processor for sonido CLAP plugins.
//!
//! Handles the real-time audio callback: reads parameter change events from
//! the host, updates the effect, and processes stereo audio buffers.

use crate::main_thread::SonidoMainThread;
use crate::shared::SonidoShared;
use clack_extensions::params::PluginAudioProcessorParams;
use clack_plugin::prelude::*;
use sonido_registry::{EffectRegistry, EffectWithParams};

/// Audio-thread processor wrapping a sonido effect.
///
/// Created during `activate()`, destroyed during `deactivate()`.
/// Owns the actual `Effect` instance and processes audio in real time.
pub struct SonidoAudioProcessor<'a> {
    shared: &'a SonidoShared,
    effect: Box<dyn EffectWithParams + Send>,
}

impl<'a> PluginAudioProcessor<'a, SonidoShared, SonidoMainThread<'a>> for SonidoAudioProcessor<'a> {
    fn activate(
        _host: HostAudioProcessorHandle<'a>,
        _main_thread: &mut SonidoMainThread<'a>,
        shared: &'a SonidoShared,
        audio_config: PluginAudioConfiguration,
    ) -> Result<Self, PluginError> {
        let registry = EffectRegistry::new();
        let mut effect = registry
            .create(shared.effect_id(), audio_config.sample_rate as f32)
            .ok_or(PluginError::Message("Failed to create effect"))?;

        // Initialize effect parameters from shared atomic state.
        shared.apply_to_effect(effect.as_mut());

        Ok(Self { shared, effect })
    }

    fn process(
        &mut self,
        _process: Process,
        mut audio: Audio,
        events: Events,
    ) -> Result<ProcessStatus, PluginError> {
        // Apply parameter changes from host automation events.
        self.handle_events(events.input);

        // Process audio through the effect.
        self.process_audio(&mut audio)?;

        Ok(ProcessStatus::ContinueIfNotQuiet)
    }

    fn deactivate(self, _main_thread: &mut SonidoMainThread<'_>) {
        // Effect is dropped here, releasing all DSP state.
    }

    fn reset(&mut self) {
        self.effect.reset();
    }
}

impl SonidoAudioProcessor<'_> {
    /// Handle incoming parameter change events from the host.
    ///
    /// Updates both the shared atomic state (so the main thread sees the
    /// latest values) and the effect's internal parameters directly.
    fn handle_events(&mut self, input: &InputEvents) {
        for event in input {
            if let Some(clack_plugin::events::spaces::CoreEventSpace::ParamValue(ev)) =
                event.as_core_event()
                && let Some(param_id) = ev.param_id()
            {
                let id = param_id.get();
                if let Some(index) = self.shared.index_by_id(id) {
                    let value = ev.value() as f32;
                    self.shared.set_value(index, value);
                    self.effect.effect_set_param(index, value);
                }
            }
        }
    }

    /// Process stereo audio through the effect.
    fn process_audio(&mut self, audio: &mut Audio) -> Result<(), PluginError> {
        for mut port_pair in audio {
            let channels = port_pair.channels()?;

            // Extract f32 channels, skip f64-only ports.
            let Some(mut channels) = channels.into_f32() else {
                continue;
            };

            let pair_count = channels.channel_pair_count();
            match pair_count {
                0 => {}
                1 => {
                    // Mono: process single channel.
                    if let Some(pair) = channels.channel_pair(0) {
                        self.process_channel_pair(pair);
                    }
                }
                _ => {
                    // Stereo: get L and R channel pairs.
                    let left = channels.channel_pair(0);
                    let right = channels.channel_pair(1);

                    match (left, right) {
                        (
                            Some(ChannelPair::InputOutput(left_in, left_out)),
                            Some(ChannelPair::InputOutput(right_in, right_out)),
                        ) => {
                            self.effect
                                .process_block_stereo(left_in, right_in, left_out, right_out);
                        }
                        (Some(ChannelPair::InPlace(left)), Some(ChannelPair::InPlace(right))) => {
                            self.effect.process_block_stereo_inplace(left, right);
                        }
                        _ => {
                            // Asymmetric layout — process channels independently.
                            if let Some(pair) = channels.channel_pair(0) {
                                self.process_channel_pair(pair);
                            }
                            if let Some(pair) = channels.channel_pair(1) {
                                self.process_channel_pair(pair);
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Process a single channel pair (mono fallback).
    fn process_channel_pair(&mut self, pair: ChannelPair<f32>) {
        match pair {
            ChannelPair::InputOutput(input, output) => {
                self.effect.process_block(input, output);
            }
            ChannelPair::InPlace(buf) => {
                self.effect.process_block_inplace(buf);
            }
            _ => {}
        }
    }
}

impl PluginAudioProcessorParams for SonidoAudioProcessor<'_> {
    fn flush(&mut self, input: &InputEvents, _output: &mut OutputEvents) {
        self.handle_events(input);
    }
}
