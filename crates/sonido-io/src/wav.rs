//! WAV file reading and writing.

use crate::Result;
use hound::{SampleFormat, WavReader, WavWriter};
use std::path::Path;

/// WAV audio encoding format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WavFormat {
    /// Linear PCM (integer samples).
    Pcm,
    /// IEEE 754 floating-point samples.
    IeeeFloat,
}

/// WAV file metadata extracted without loading sample data.
#[derive(Debug, Clone)]
pub struct WavInfo {
    /// Number of audio channels (1 = mono, 2 = stereo).
    pub channels: u16,
    /// Sample rate in Hz.
    pub sample_rate: u32,
    /// Bit depth per sample.
    pub bits_per_sample: u16,
    /// Total number of sample frames (samples per channel).
    pub num_frames: u64,
    /// Duration in seconds.
    pub duration_secs: f64,
    /// Audio encoding format.
    pub format: WavFormat,
}

/// Read WAV metadata without loading sample data.
///
/// Opens the file, reads the header, and returns a [`WavInfo`] struct
/// with format details and duration. This is much faster than [`read_wav`]
/// for files where you only need metadata.
pub fn read_wav_info<P: AsRef<Path>>(path: P) -> Result<WavInfo> {
    let reader = WavReader::open(path)?;
    let spec = reader.spec();
    let total_samples = reader.len() as u64; // total across all channels
    let num_frames = total_samples / spec.channels as u64;
    let duration_secs = num_frames as f64 / spec.sample_rate as f64;

    let format = match spec.sample_format {
        SampleFormat::Float => WavFormat::IeeeFloat,
        SampleFormat::Int => WavFormat::Pcm,
    };

    Ok(WavInfo {
        channels: spec.channels,
        sample_rate: spec.sample_rate,
        bits_per_sample: spec.bits_per_sample,
        num_frames,
        duration_secs,
        format,
    })
}

/// WAV file specification.
#[derive(Debug, Clone, Copy)]
pub struct WavSpec {
    /// Number of audio channels (1 = mono, 2 = stereo).
    pub channels: u16,
    /// Sample rate in Hz (e.g., 44100, 48000).
    pub sample_rate: u32,
    /// Bit depth per sample (e.g., 16, 24, 32).
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

// StereoSamples is defined in sonido-core and re-exported here for backwards compatibility.
pub use sonido_core::graph::StereoSamples;

/// Read a WAV file and return stereo samples along with the spec.
///
/// Mono files are expanded to stereo by duplicating to both channels.
/// Files with more than 2 channels use only the first two channels.
///
/// # Example
/// ```ignore
/// let (samples, spec) = read_wav_stereo("input.wav")?;
/// println!("Loaded {} samples at {} Hz", samples.len(), spec.sample_rate);
/// ```
pub fn read_wav_stereo<P: AsRef<Path>>(path: P) -> Result<(StereoSamples, WavSpec)> {
    let reader = WavReader::open(path)?;
    let spec = WavSpec::from(reader.spec());
    let channels = spec.channels as usize;

    let all_samples: Vec<f32> = match reader.spec().sample_format {
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

    let stereo = match channels {
        1 => {
            // Mono -> duplicate to both channels
            StereoSamples::from_mono(all_samples)
        }
        2 => {
            // Already stereo -> deinterleave
            StereoSamples::from_interleaved(&all_samples)
        }
        _ => {
            // Multi-channel -> take first two channels
            let samples_per_channel = all_samples.len() / channels;
            let mut left = Vec::with_capacity(samples_per_channel);
            let mut right = Vec::with_capacity(samples_per_channel);

            for chunk in all_samples.chunks(channels) {
                left.push(chunk[0]);
                right.push(chunk.get(1).copied().unwrap_or(chunk[0]));
            }

            StereoSamples::new(left, right)
        }
    };

    Ok((stereo, spec))
}

/// Write stereo samples to a WAV file.
///
/// # Example
/// ```ignore
/// let samples = StereoSamples::new(vec![0.0; 48000], vec![0.0; 48000]);
/// let spec = WavSpec { sample_rate: 48000, channels: 2, ..Default::default() };
/// write_wav_stereo("output.wav", &samples, spec)?;
/// ```
pub fn write_wav_stereo<P: AsRef<Path>>(
    path: P,
    samples: &StereoSamples,
    spec: WavSpec,
) -> Result<()> {
    let mut stereo_spec = spec;
    stereo_spec.channels = 2;

    let hound_spec = hound::WavSpec::from(stereo_spec);
    let mut writer = WavWriter::create(path, hound_spec)?;

    if spec.bits_per_sample == 32 {
        for (l, r) in samples.left.iter().zip(samples.right.iter()) {
            writer.write_sample(*l)?;
            writer.write_sample(*r)?;
        }
    } else {
        let max_val = (1i32 << (spec.bits_per_sample - 1)) as f32;
        for (l, r) in samples.left.iter().zip(samples.right.iter()) {
            let int_l = (*l * max_val).clamp(-max_val, max_val - 1.0) as i32;
            let int_r = (*r * max_val).clamp(-max_val, max_val - 1.0) as i32;
            writer.write_sample(int_l)?;
            writer.write_sample(int_r)?;
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

    #[test]
    fn test_stereo_samples_from_mono() {
        let mono = vec![1.0, 2.0, 3.0];
        let stereo = StereoSamples::from_mono(mono.clone());
        assert_eq!(stereo.left, mono);
        assert_eq!(stereo.right, mono);
    }

    #[test]
    fn test_stereo_samples_to_mono() {
        let stereo = StereoSamples::new(vec![1.0, 2.0], vec![3.0, 4.0]);
        let mono = stereo.to_mono();
        assert_eq!(mono, vec![2.0, 3.0]); // (1+3)/2, (2+4)/2
    }

    #[test]
    fn test_stereo_samples_interleaved() {
        let stereo = StereoSamples::new(vec![1.0, 3.0], vec![2.0, 4.0]);
        let interleaved = stereo.to_interleaved();
        assert_eq!(interleaved, vec![1.0, 2.0, 3.0, 4.0]);

        let back = StereoSamples::from_interleaved(&interleaved);
        assert_eq!(back.left, vec![1.0, 3.0]);
        assert_eq!(back.right, vec![2.0, 4.0]);
    }

    #[test]
    fn test_stereo_roundtrip_f32() {
        let left: Vec<f32> = (0..1000).map(|i| (i as f32 / 1000.0).sin()).collect();
        let right: Vec<f32> = (0..1000).map(|i| (i as f32 / 1000.0).cos()).collect();
        let samples = StereoSamples::new(left.clone(), right.clone());

        let spec = WavSpec {
            channels: 2,
            sample_rate: 48000,
            bits_per_sample: 32,
        };

        let file = NamedTempFile::new().unwrap();
        write_wav_stereo(file.path(), &samples, spec).unwrap();

        let (loaded, loaded_spec) = read_wav_stereo(file.path()).unwrap();
        assert_eq!(loaded_spec.sample_rate, 48000);
        assert_eq!(loaded.len(), samples.len());

        for (a, b) in left.iter().zip(loaded.left.iter()) {
            assert!((a - b).abs() < 1e-6);
        }
        for (a, b) in right.iter().zip(loaded.right.iter()) {
            assert!((a - b).abs() < 1e-6);
        }
    }

    #[test]
    fn test_read_mono_as_stereo() {
        // Write a mono file
        let mono: Vec<f32> = (0..100).map(|i| i as f32 / 100.0).collect();
        let spec = WavSpec {
            channels: 1,
            sample_rate: 44100,
            bits_per_sample: 32,
        };

        let file = NamedTempFile::new().unwrap();
        write_wav(file.path(), &mono, spec).unwrap();

        // Read as stereo (should duplicate)
        let (stereo, _) = read_wav_stereo(file.path()).unwrap();
        assert_eq!(stereo.left, mono);
        assert_eq!(stereo.right, mono);
    }
}
