//! Real-time audio streaming via cpal.

use crate::{Error, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, Host, SampleRate, Stream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Audio device information.
#[derive(Debug, Clone)]
pub struct AudioDevice {
    pub name: String,
    pub is_input: bool,
    pub is_output: bool,
    pub default_sample_rate: u32,
}

/// Stream configuration.
#[derive(Debug, Clone)]
pub struct StreamConfig {
    pub sample_rate: u32,
    pub buffer_size: u32,
    pub input_device: Option<String>,
    pub output_device: Option<String>,
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48000,
            buffer_size: 256,
            input_device: None,
            output_device: None,
        }
    }
}

/// List all available audio devices.
pub fn list_devices() -> Result<Vec<AudioDevice>> {
    let host = cpal::default_host();
    let mut devices = Vec::new();

    // Input devices
    if let Ok(inputs) = host.input_devices() {
        for device in inputs {
            if let Ok(name) = device.name() {
                let sample_rate = device
                    .default_input_config()
                    .map(|c| c.sample_rate().0)
                    .unwrap_or(48000);

                // Check if also an output
                let is_output = device.default_output_config().is_ok();

                devices.push(AudioDevice {
                    name,
                    is_input: true,
                    is_output,
                    default_sample_rate: sample_rate,
                });
            }
        }
    }

    // Output-only devices
    if let Ok(outputs) = host.output_devices() {
        for device in outputs {
            if let Ok(name) = device.name() {
                // Skip if already added as input
                if devices.iter().any(|d| d.name == name) {
                    continue;
                }

                let sample_rate = device
                    .default_output_config()
                    .map(|c| c.sample_rate().0)
                    .unwrap_or(48000);

                devices.push(AudioDevice {
                    name,
                    is_input: false,
                    is_output: true,
                    default_sample_rate: sample_rate,
                });
            }
        }
    }

    Ok(devices)
}

/// Get the default audio device info.
pub fn default_device() -> Result<(Option<AudioDevice>, Option<AudioDevice>)> {
    let host = cpal::default_host();

    let input = host.default_input_device().and_then(|d| {
        d.name().ok().map(|name| AudioDevice {
            name,
            is_input: true,
            is_output: false,
            default_sample_rate: d
                .default_input_config()
                .map(|c| c.sample_rate().0)
                .unwrap_or(48000),
        })
    });

    let output = host.default_output_device().and_then(|d| {
        d.name().ok().map(|name| AudioDevice {
            name,
            is_input: false,
            is_output: true,
            default_sample_rate: d
                .default_output_config()
                .map(|c| c.sample_rate().0)
                .unwrap_or(48000),
        })
    });

    Ok((input, output))
}

/// Real-time audio stream with input and output.
pub struct AudioStream {
    #[allow(dead_code)]
    host: Host,
    input_device: Device,
    output_device: Device,
    config: StreamConfig,
    running: Arc<AtomicBool>,
    _input_stream: Option<Stream>,
    _output_stream: Option<Stream>,
}

impl AudioStream {
    /// Create a new audio stream with the given configuration.
    pub fn new(config: StreamConfig) -> Result<Self> {
        let host = cpal::default_host();

        let input_device = match &config.input_device {
            Some(name) => find_input_device(&host, name)?,
            None => host.default_input_device().ok_or(Error::NoDevice)?,
        };

        let output_device = match &config.output_device {
            Some(name) => find_output_device(&host, name)?,
            None => host.default_output_device().ok_or(Error::NoDevice)?,
        };

        Ok(Self {
            host,
            input_device,
            output_device,
            config,
            running: Arc::new(AtomicBool::new(false)),
            _input_stream: None,
            _output_stream: None,
        })
    }

    /// Get the configured sample rate.
    pub fn sample_rate(&self) -> u32 {
        self.config.sample_rate
    }

    /// Run the audio stream with a processing callback.
    ///
    /// The callback receives input samples and must fill the output buffer.
    /// This function blocks until the stream is stopped.
    pub fn run<F>(&mut self, mut process: F) -> Result<()>
    where
        F: FnMut(&[f32], &mut [f32]) + Send + 'static,
    {
        use std::sync::mpsc;

        let _sample_rate = SampleRate(self.config.sample_rate);

        // Get supported configs
        let input_config = self
            .input_device
            .default_input_config()
            .map_err(|e| Error::Stream(e.to_string()))?;

        let output_config = self
            .output_device
            .default_output_config()
            .map_err(|e| Error::Stream(e.to_string()))?;

        // Create channel for passing audio between input and output
        let (tx, rx) = mpsc::sync_channel::<Vec<f32>>(4);

        let running = Arc::clone(&self.running);
        self.running.store(true, Ordering::SeqCst);

        // Input stream - capture audio and send to channel
        let input_running = Arc::clone(&running);
        let input_stream = self
            .input_device
            .build_input_stream(
                &input_config.into(),
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    if input_running.load(Ordering::SeqCst) {
                        let _ = tx.try_send(data.to_vec());
                    }
                },
                |err| eprintln!("Input stream error: {}", err),
                None,
            )
            .map_err(|e| Error::Stream(e.to_string()))?;

        // Output stream - receive processed audio
        let output_running = Arc::clone(&running);
        let mut pending_input: Vec<f32> = Vec::new();
        let output_stream = self
            .output_device
            .build_output_stream(
                &output_config.into(),
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    if !output_running.load(Ordering::SeqCst) {
                        data.fill(0.0);
                        return;
                    }

                    // Collect input samples
                    while let Ok(samples) = rx.try_recv() {
                        pending_input.extend(samples);
                    }

                    // Process if we have enough input
                    if pending_input.len() >= data.len() {
                        let input: Vec<f32> = pending_input.drain(..data.len()).collect();
                        process(&input, data);
                    } else {
                        // Not enough input - output silence
                        data.fill(0.0);
                    }
                },
                |err| eprintln!("Output stream error: {}", err),
                None,
            )
            .map_err(|e| Error::Stream(e.to_string()))?;

        input_stream.play().map_err(|e| Error::Stream(e.to_string()))?;
        output_stream.play().map_err(|e| Error::Stream(e.to_string()))?;

        self._input_stream = Some(input_stream);
        self._output_stream = Some(output_stream);

        // Block until stopped
        while self.running.load(Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }

        Ok(())
    }

    /// Run output-only stream (no input).
    pub fn run_output<F>(&mut self, mut generate: F) -> Result<()>
    where
        F: FnMut(&mut [f32]) + Send + 'static,
    {
        let output_config = self
            .output_device
            .default_output_config()
            .map_err(|e| Error::Stream(e.to_string()))?;

        let running = Arc::clone(&self.running);
        self.running.store(true, Ordering::SeqCst);

        let output_running = Arc::clone(&running);
        let output_stream = self
            .output_device
            .build_output_stream(
                &output_config.into(),
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    if output_running.load(Ordering::SeqCst) {
                        generate(data);
                    } else {
                        data.fill(0.0);
                    }
                },
                |err| eprintln!("Output stream error: {}", err),
                None,
            )
            .map_err(|e| Error::Stream(e.to_string()))?;

        output_stream.play().map_err(|e| Error::Stream(e.to_string()))?;
        self._output_stream = Some(output_stream);

        while self.running.load(Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }

        Ok(())
    }

    /// Run the audio stream with a stereo processing callback.
    ///
    /// The callback receives separate left and right channel inputs
    /// and must fill the left and right output buffers.
    /// This function blocks until the stream is stopped.
    ///
    /// Note: The actual stream may be interleaved stereo depending on the
    /// audio device. This method handles deinterleaving input and
    /// interleaving output automatically.
    pub fn run_stereo<F>(&mut self, mut process: F) -> Result<()>
    where
        F: FnMut(&[f32], &[f32], &mut [f32], &mut [f32]) + Send + 'static,
    {
        use std::sync::mpsc;

        // Get supported configs
        let input_config = self
            .input_device
            .default_input_config()
            .map_err(|e| Error::Stream(e.to_string()))?;

        let output_config = self
            .output_device
            .default_output_config()
            .map_err(|e| Error::Stream(e.to_string()))?;

        let input_channels = input_config.channels() as usize;
        let output_channels = output_config.channels() as usize;

        // Create channel for passing audio between input and output
        let (tx, rx) = mpsc::sync_channel::<Vec<f32>>(4);

        let running = Arc::clone(&self.running);
        self.running.store(true, Ordering::SeqCst);

        // Input stream - capture audio and send to channel
        let input_running = Arc::clone(&running);
        let input_stream = self
            .input_device
            .build_input_stream(
                &input_config.into(),
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    if input_running.load(Ordering::SeqCst) {
                        let _ = tx.try_send(data.to_vec());
                    }
                },
                |err| eprintln!("Input stream error: {}", err),
                None,
            )
            .map_err(|e| Error::Stream(e.to_string()))?;

        // Output stream - receive and process stereo audio
        let output_running = Arc::clone(&running);
        let mut pending_input: Vec<f32> = Vec::new();

        let output_stream = self
            .output_device
            .build_output_stream(
                &output_config.into(),
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    if !output_running.load(Ordering::SeqCst) {
                        data.fill(0.0);
                        return;
                    }

                    // Collect input samples
                    while let Ok(samples) = rx.try_recv() {
                        pending_input.extend(samples);
                    }

                    let frames_needed = data.len() / output_channels;
                    let input_samples_needed = frames_needed * input_channels;

                    // Process if we have enough input
                    if pending_input.len() >= input_samples_needed {
                        let input: Vec<f32> = pending_input.drain(..input_samples_needed).collect();

                        // Deinterleave input to separate L/R buffers
                        let (left_in, right_in) = deinterleave(&input, input_channels);

                        // Prepare output buffers
                        let mut left_out = vec![0.0; frames_needed];
                        let mut right_out = vec![0.0; frames_needed];

                        // Process
                        process(&left_in, &right_in, &mut left_out, &mut right_out);

                        // Interleave output back into the data buffer
                        interleave_into(&left_out, &right_out, data, output_channels);
                    } else {
                        // Not enough input - output silence
                        data.fill(0.0);
                    }
                },
                |err| eprintln!("Output stream error: {}", err),
                None,
            )
            .map_err(|e| Error::Stream(e.to_string()))?;

        input_stream.play().map_err(|e| Error::Stream(e.to_string()))?;
        output_stream.play().map_err(|e| Error::Stream(e.to_string()))?;

        self._input_stream = Some(input_stream);
        self._output_stream = Some(output_stream);

        // Block until stopped
        while self.running.load(Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }

        Ok(())
    }

    /// Stop the audio stream.
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    /// Check if the stream is running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
}

/// Deinterleave input samples into separate left and right channels.
/// Handles mono input by duplicating to both channels.
fn deinterleave(interleaved: &[f32], channels: usize) -> (Vec<f32>, Vec<f32>) {
    let frames = interleaved.len() / channels;
    let mut left = Vec::with_capacity(frames);
    let mut right = Vec::with_capacity(frames);

    match channels {
        1 => {
            // Mono: duplicate to both channels
            for &sample in interleaved {
                left.push(sample);
                right.push(sample);
            }
        }
        _ => {
            // Stereo or more: take first two channels
            for chunk in interleaved.chunks(channels) {
                left.push(chunk[0]);
                right.push(chunk.get(1).copied().unwrap_or(chunk[0]));
            }
        }
    }

    (left, right)
}

/// Interleave left and right channels into an output buffer.
/// Handles output with different channel counts.
fn interleave_into(left: &[f32], right: &[f32], output: &mut [f32], channels: usize) {
    let frames = left.len().min(right.len());

    match channels {
        1 => {
            // Mono output: mix L+R
            for (i, (l, r)) in left.iter().zip(right.iter()).enumerate() {
                if i < output.len() {
                    output[i] = (l + r) * 0.5;
                }
            }
        }
        2 => {
            // Stereo output: interleave L, R
            for i in 0..frames {
                let idx = i * 2;
                if idx + 1 < output.len() {
                    output[idx] = left[i];
                    output[idx + 1] = right[i];
                }
            }
        }
        _ => {
            // Multi-channel: put L/R in first two channels, silence the rest
            for i in 0..frames {
                let idx = i * channels;
                if idx + channels <= output.len() {
                    output[idx] = left[i];
                    output[idx + 1] = right[i];
                    for c in 2..channels {
                        output[idx + c] = 0.0;
                    }
                }
            }
        }
    }
}

fn find_input_device(host: &Host, name: &str) -> Result<Device> {
    host.input_devices()
        .map_err(|e| Error::Stream(e.to_string()))?
        .find(|d| d.name().map(|n| n == name).unwrap_or(false))
        .ok_or_else(|| Error::DeviceNotFound(name.to_string()))
}

fn find_output_device(host: &Host, name: &str) -> Result<Device> {
    host.output_devices()
        .map_err(|e| Error::Stream(e.to_string()))?
        .find(|d| d.name().map(|n| n == name).unwrap_or(false))
        .ok_or_else(|| Error::DeviceNotFound(name.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_devices() {
        // This test just verifies the function doesn't panic
        // Actual device availability depends on the system
        let result = list_devices();
        assert!(result.is_ok());
    }

    #[test]
    fn test_default_device() {
        let result = default_device();
        assert!(result.is_ok());
    }
}
