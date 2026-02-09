//! Export formats for analysis results.
//!
//! Provides interoperability with standard audio measurement tools:
//! - FRD format (frequency response data, compatible with REW)
//! - CSV format for generic data exchange
//! - PGM format for spectrogram images

use crate::{Spectrogram, ThdResult, TransferFunction};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

/// Export transfer function to FRD format (REW compatible).
///
/// FRD (Frequency Response Data) is a simple text format:
/// - One measurement per line
/// - Three space-separated values: frequency_hz magnitude_db phase_deg
///
/// # Example
///
/// ```rust,ignore
/// use sonido_analysis::{TransferFunction, export::export_frd};
///
/// let tf = TransferFunction::measure(&input, &output, 48000.0, 4096, 0.5);
/// export_frd(&tf, "response.frd")?;
/// ```
pub fn export_frd(tf: &TransferFunction, path: impl AsRef<Path>) -> std::io::Result<()> {
    let mut file = std::fs::File::create(path)?;

    // Write header comment
    writeln!(
        file,
        "* Frequency Response Data exported by sonido-analysis"
    )?;

    for i in 0..tf.frequencies.len() {
        let freq = tf.frequencies[i];
        let mag = tf.magnitude_db[i];
        let phase_deg = tf.phase_rad[i].to_degrees();

        writeln!(file, "{:.6} {:.6} {:.6}", freq, mag, phase_deg)?;
    }

    Ok(())
}

/// Import transfer function from FRD format.
///
/// # Example
///
/// ```rust,ignore
/// use sonido_analysis::export::import_frd;
///
/// let tf = import_frd("response.frd")?;
/// println!("Loaded {} frequency bins", tf.frequencies.len());
/// ```
pub fn import_frd(path: impl AsRef<Path>) -> std::io::Result<TransferFunction> {
    let file = std::fs::File::open(path)?;
    let reader = BufReader::new(file);

    let mut frequencies = Vec::new();
    let mut magnitude_db = Vec::new();
    let mut phase_rad = Vec::new();

    for line in reader.lines() {
        let line = line?;
        let line = line.trim();

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with('#') || line.starts_with('*') {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 3
            && let (Ok(freq), Ok(mag), Ok(phase_deg)) = (
                parts[0].parse::<f32>(),
                parts[1].parse::<f32>(),
                parts[2].parse::<f32>(),
            )
        {
            frequencies.push(freq);
            magnitude_db.push(mag);
            phase_rad.push(phase_deg.to_radians());
        }
    }

    // FRD format doesn't include coherence, so we set it to 1.0 (perfect)
    let coherence = vec![1.0; frequencies.len()];

    Ok(TransferFunction {
        frequencies,
        magnitude_db,
        phase_rad,
        coherence,
    })
}

/// Export spectrogram to CSV format.
///
/// Creates a CSV file with time on rows and frequency bins on columns.
/// First row contains frequency labels, first column contains time labels.
///
/// # Arguments
/// * `spectrogram` - The spectrogram to export
/// * `path` - Output file path
/// * `db_scale` - If true, convert magnitudes to dB scale
///
/// # Example
///
/// ```rust,ignore
/// use sonido_analysis::{StftAnalyzer, export::export_spectrogram_csv};
/// use sonido_analysis::fft::Window;
///
/// let analyzer = StftAnalyzer::new(48000.0, 2048, 512, Window::Hann);
/// let spec = analyzer.analyze(&signal);
/// export_spectrogram_csv(&spec, "spectrogram.csv", true)?;
/// ```
pub fn export_spectrogram_csv(
    spectrogram: &Spectrogram,
    path: impl AsRef<Path>,
    db_scale: bool,
) -> std::io::Result<()> {
    let mut file = std::fs::File::create(path)?;

    // Write header row with frequencies
    write!(file, "time_s")?;
    for bin in 0..spectrogram.num_bins {
        let freq = spectrogram.bin_to_freq(bin);
        write!(file, ",{:.2}", freq)?;
    }
    writeln!(file)?;

    // Write data rows
    for frame in 0..spectrogram.num_frames {
        let time = spectrogram.frame_to_time(frame);
        write!(file, "{:.6}", time)?;

        if let Some(spectrum) = spectrogram.get_frame(frame) {
            for &mag in spectrum {
                let value = if db_scale {
                    20.0 * mag.max(1e-10).log10()
                } else {
                    mag
                };
                write!(file, ",{:.6}", value)?;
            }
        }
        writeln!(file)?;
    }

    Ok(())
}

/// Export spectrogram to PGM grayscale image format.
///
/// PGM is a simple ASCII image format that can be viewed by most image tools.
/// Time is on the X axis, frequency on Y axis (low frequencies at bottom).
///
/// # Arguments
///
/// * `spectrogram` - The spectrogram to export
/// * `path` - Output file path
/// * `db_range` - Dynamic range in dB (values below max-db_range map to black)
pub fn export_spectrogram_pgm(
    spectrogram: &Spectrogram,
    path: impl AsRef<Path>,
    db_range: f32,
) -> std::io::Result<()> {
    let mut file = std::fs::File::create(path)?;

    let width = spectrogram.num_frames;
    let height = spectrogram.num_bins;

    // PGM header
    writeln!(file, "P2")?;
    writeln!(file, "# Spectrogram export from sonido-analysis")?;
    writeln!(file, "# Width: {} frames, Height: {} bins", width, height)?;
    writeln!(file, "{} {}", width, height)?;
    writeln!(file, "255")?;

    // Find max magnitude for normalization
    let mut max_mag = 0.0f32;
    for frame in &spectrogram.data {
        for &mag in frame {
            max_mag = max_mag.max(mag);
        }
    }
    let max_db = 20.0 * max_mag.max(1e-10).log10();

    // Image data (top to bottom = high to low frequency)
    for bin in (0..height).rev() {
        let mut row = Vec::with_capacity(width);
        for frame in 0..width {
            let mag = spectrogram.get(frame, bin).unwrap_or(0.0);
            let db = 20.0 * mag.max(1e-10).log10();

            // Normalize to 0-255
            let normalized = ((db - (max_db - db_range)) / db_range).clamp(0.0, 1.0);
            let pixel = (normalized * 255.0) as u8;
            row.push(pixel);
        }

        // Write row
        for (i, &pixel) in row.iter().enumerate() {
            if i > 0 {
                write!(file, " ")?;
            }
            write!(file, "{}", pixel)?;
        }
        writeln!(file)?;
    }

    Ok(())
}

/// Export distortion analysis results to JSON.
pub fn export_distortion_json(result: &ThdResult, path: impl AsRef<Path>) -> std::io::Result<()> {
    let mut file = std::fs::File::create(path)?;

    let fundamental_db = if result.fundamental_amplitude > 0.0 {
        20.0 * result.fundamental_amplitude.log10()
    } else {
        -120.0
    };

    writeln!(file, "{{")?;
    writeln!(file, "  \"fundamental_hz\": {},", result.fundamental_freq)?;
    writeln!(
        file,
        "  \"fundamental_amplitude\": {},",
        result.fundamental_amplitude
    )?;
    writeln!(file, "  \"fundamental_db\": {},", fundamental_db)?;
    writeln!(file, "  \"thd_ratio\": {},", result.thd_ratio)?;
    writeln!(file, "  \"thd_percent\": {},", result.thd_ratio * 100.0)?;
    writeln!(file, "  \"thd_db\": {},", result.thd_db)?;
    writeln!(file, "  \"thd_plus_noise_ratio\": {},", result.thd_n_ratio)?;
    writeln!(
        file,
        "  \"thd_plus_noise_percent\": {},",
        result.thd_n_ratio * 100.0
    )?;
    writeln!(file, "  \"thd_plus_noise_db\": {},", result.thd_n_db)?;
    writeln!(file, "  \"noise_floor\": {},", result.noise_floor)?;
    writeln!(file, "  \"harmonics\": [")?;

    for (i, &h) in result.harmonics.iter().enumerate() {
        let harmonic_db = if h > 0.0 { 20.0 * h.log10() } else { -120.0 };
        let comma = if i < result.harmonics.len() - 1 {
            ","
        } else {
            ""
        };
        writeln!(file, "    {{")?;
        writeln!(file, "      \"harmonic\": {},", i + 1)?;
        writeln!(
            file,
            "      \"frequency_hz\": {},",
            result.fundamental_freq * (i + 1) as f32
        )?;
        writeln!(file, "      \"amplitude\": {},", h)?;
        writeln!(file, "      \"amplitude_db\": {}", harmonic_db)?;
        writeln!(file, "    }}{}", comma)?;
    }

    writeln!(file, "  ]")?;
    writeln!(file, "}}")?;

    Ok(())
}

/// Export transfer function with group delay to extended FRD format.
///
/// Format: frequency_hz magnitude_db phase_deg group_delay_ms
pub fn export_frd_extended(tf: &TransferFunction, path: impl AsRef<Path>) -> std::io::Result<()> {
    let mut file = std::fs::File::create(path)?;
    let group_delay = tf.group_delay();

    writeln!(file, "* Extended FRD with group delay - sonido-analysis")?;
    writeln!(
        file,
        "* Format: frequency_hz magnitude_db phase_deg group_delay_ms"
    )?;

    for i in 0..tf.frequencies.len() {
        let freq = tf.frequencies[i];
        let mag = tf.magnitude_db[i];
        let phase_deg = tf.phase_rad[i].to_degrees();
        let gd_ms = group_delay.get(i).copied().unwrap_or(0.0) * 1000.0;

        writeln!(file, "{:.6} {:.6} {:.6} {:.6}", freq, mag, phase_deg, gd_ms)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use tempfile::NamedTempFile;

    #[test]
    fn test_frd_roundtrip() {
        let tf = TransferFunction {
            frequencies: vec![100.0, 1000.0, 10000.0],
            magnitude_db: vec![0.0, -3.0, -6.0],
            phase_rad: vec![0.0, -0.5, -1.0],
            coherence: vec![1.0, 0.99, 0.95],
        };

        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        export_frd(&tf, path).unwrap();
        let loaded = import_frd(path).unwrap();

        assert_eq!(loaded.frequencies.len(), tf.frequencies.len());
        for i in 0..tf.frequencies.len() {
            assert!((loaded.frequencies[i] - tf.frequencies[i]).abs() < 0.01);
            assert!((loaded.magnitude_db[i] - tf.magnitude_db[i]).abs() < 0.01);
            assert!((loaded.phase_rad[i] - tf.phase_rad[i]).abs() < 0.01);
        }
    }

    #[test]
    fn test_frd_export_format() {
        let tf = TransferFunction {
            frequencies: vec![1000.0],
            magnitude_db: vec![-3.5],
            phase_rad: vec![-std::f32::consts::FRAC_PI_4], // -45 degrees
            coherence: vec![1.0],
        };

        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        export_frd(&tf, path).unwrap();

        let mut content = String::new();
        std::fs::File::open(path)
            .unwrap()
            .read_to_string(&mut content)
            .unwrap();

        // Should contain frequency, magnitude, and phase in degrees
        assert!(content.contains("1000"), "Should contain frequency 1000");
        assert!(content.contains("-3.5"), "Should contain magnitude -3.5");
        // -0.785398 rad = -44.999... degrees
        assert!(
            content.contains("-44.99") || content.contains("-45.0"),
            "Should contain phase near -45 degrees, got: {}",
            content
        );
    }

    #[test]
    fn test_spectrogram_csv_export() {
        let spec = Spectrogram {
            data: vec![vec![0.1, 0.2, 0.3], vec![0.4, 0.5, 0.6]],
            fft_size: 4,
            hop_size: 2,
            sample_rate: 1000.0,
            num_frames: 2,
            num_bins: 3,
        };

        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        export_spectrogram_csv(&spec, path, false).unwrap();

        let mut content = String::new();
        std::fs::File::open(path)
            .unwrap()
            .read_to_string(&mut content)
            .unwrap();

        // Should have header and data
        assert!(content.contains("time_s"), "Should have time header");
        assert!(content.contains("0.1"), "Should contain magnitude 0.1");
    }

    #[test]
    fn test_spectrogram_pgm_export() {
        let spec = Spectrogram {
            data: vec![
                vec![0.1, 0.5, 1.0],
                vec![0.2, 0.6, 0.8],
                vec![0.3, 0.7, 0.5],
            ],
            fft_size: 4,
            hop_size: 2,
            sample_rate: 1000.0,
            num_frames: 3,
            num_bins: 3,
        };

        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        export_spectrogram_pgm(&spec, path, 60.0).unwrap();

        let mut content = String::new();
        std::fs::File::open(path)
            .unwrap()
            .read_to_string(&mut content)
            .unwrap();

        // Check PGM header
        assert!(content.starts_with("P2"), "Should be P2 format");
        assert!(content.contains("3 3"), "Should have width 3 height 3");
        assert!(content.contains("255"), "Should have max value 255");
    }
}
