//! WAV file reading and writing.

use crate::{Error, Result};
use hound::{SampleFormat, WavReader, WavWriter};
use std::path::Path;

/// WAV file specification.
#[derive(Debug, Clone, Copy)]
pub struct WavSpec {
    pub channels: u16,
    pub sample_rate: u32,
    pub bits_per_sample: u16,
}

impl Default for WavSpec {
    fn default() -> Self {
        Self {
            channels: 1,
            sample_rate: 48000,
            bits_per_sample: 32,
        }
    }
}

impl From<hound::WavSpec> for WavSpec {
    fn from(spec: hound::WavSpec) -> Self {
        Self {
            channels: spec.channels,
            sample_rate: spec.sample_rate,
            bits_per_sample: spec.bits_per_sample,
        }
    }
}

impl From<WavSpec> for hound::WavSpec {
    fn from(spec: WavSpec) -> Self {
        hound::WavSpec {
            channels: spec.channels,
            sample_rate: spec.sample_rate,
            bits_per_sample: spec.bits_per_sample,
            sample_format: if spec.bits_per_sample == 32 {
                SampleFormat::Float
            } else {
                SampleFormat::Int
            },
        }
    }
}

/// Read a WAV file and return samples as f32 along with the spec.
///
/// Multi-channel files are mixed down to mono by averaging channels.
///
/// # Example
/// ```ignore
/// let (samples, spec) = read_wav("input.wav")?;
/// println!("Loaded {} samples at {} Hz", samples.len(), spec.sample_rate);
/// ```
pub fn read_wav<P: AsRef<Path>>(path: P) -> Result<(Vec<f32>, WavSpec)> {
    let reader = WavReader::open(path)?;
    let spec = WavSpec::from(reader.spec());
    let channels = spec.channels as usize;

    let samples: Vec<f32> = match reader.spec().sample_format {
        SampleFormat::Float => reader
            .into_samples::<f32>()
            .collect::<std::result::Result<Vec<_>, _>>()?,
        SampleFormat::Int => {
            let bits = spec.bits_per_sample;
            let max_val = (1i32 << (bits - 1)) as f32;
            reader
                .into_samples::<i32>()
                .map(|s| s.map(|v| v as f32 / max_val))
                .collect::<std::result::Result<Vec<_>, _>>()?
        }
    };

    // Mix down to mono if multi-channel
    let mono_samples = if channels > 1 {
        samples
            .chunks(channels)
            .map(|chunk| chunk.iter().sum::<f32>() / channels as f32)
            .collect()
    } else {
        samples
    };

    Ok((mono_samples, spec))
}

/// Write samples to a WAV file.
///
/// # Example
/// ```ignore
/// let samples = vec![0.0f32; 48000]; // 1 second of silence
/// let spec = WavSpec { sample_rate: 48000, ..Default::default() };
/// write_wav("output.wav", &samples, spec)?;
/// ```
pub fn write_wav<P: AsRef<Path>>(path: P, samples: &[f32], spec: WavSpec) -> Result<()> {
    let hound_spec = hound::WavSpec::from(spec);
    let mut writer = WavWriter::create(path, hound_spec)?;

    if spec.bits_per_sample == 32 {
        for &sample in samples {
            writer.write_sample(sample)?;
        }
    } else {
        let max_val = (1i32 << (spec.bits_per_sample - 1)) as f32;
        for &sample in samples {
            let int_sample = (sample * max_val).clamp(-max_val, max_val - 1.0) as i32;
            writer.write_sample(int_sample)?;
        }
    }

    writer.finalize()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_roundtrip_f32() {
        let samples: Vec<f32> = (0..1000).map(|i| (i as f32 / 1000.0).sin()).collect();
        let spec = WavSpec {
            channels: 1,
            sample_rate: 48000,
            bits_per_sample: 32,
        };

        let file = NamedTempFile::new().unwrap();
        write_wav(file.path(), &samples, spec).unwrap();

        let (loaded, loaded_spec) = read_wav(file.path()).unwrap();
        assert_eq!(loaded_spec.sample_rate, 48000);
        assert_eq!(loaded.len(), samples.len());

        for (a, b) in samples.iter().zip(loaded.iter()) {
            assert!((a - b).abs() < 1e-6);
        }
    }

    #[test]
    fn test_roundtrip_i16() {
        let samples: Vec<f32> = (0..1000).map(|i| (i as f32 / 1000.0).sin() * 0.9).collect();
        let spec = WavSpec {
            channels: 1,
            sample_rate: 44100,
            bits_per_sample: 16,
        };

        let file = NamedTempFile::new().unwrap();
        write_wav(file.path(), &samples, spec).unwrap();

        let (loaded, loaded_spec) = read_wav(file.path()).unwrap();
        assert_eq!(loaded_spec.sample_rate, 44100);
        assert_eq!(loaded.len(), samples.len());

        // 16-bit has less precision
        for (a, b) in samples.iter().zip(loaded.iter()) {
            assert!((a - b).abs() < 0.001);
        }
    }
}
