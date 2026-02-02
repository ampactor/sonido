# Biosignal Analysis with Sonido

While designed for audio, sonido-analysis works for any time-series signal. The mathematics of spectral analysis are timescale-agnostic.

## Electric Fish (Weakly Electric)

Electric Organ Discharge (EOD) signals from gymnotiform and mormyrid fish are ideal candidates - they occupy the audio frequency range (50-1000 Hz).

### Species Identification via Waveform

```bash
# Record EOD through electrodes (standard audio interface works)
# Pulse-type fish have rich harmonics, wave-type are near-sinusoidal

sonido analyze distortion eigenmannia.wav --fft-size 16384
# Wave-type: THD < 5%

sonido analyze distortion gymnotus.wav --fft-size 16384
# Pulse-type: THD > 30% (rich harmonics from sharp waveform)
```

### Jamming Avoidance Response (JAR)

When two wave-type fish detect similar frequencies, they shift apart:

```bash
# High-resolution spectrogram captures frequency tracking
sonido analyze spectrogram two_fish.wav -o jar.csv --fft-size 4096 --hop 128

# Visualize with Python:
# import pandas as pd
# import matplotlib.pyplot as plt
# df = pd.read_csv('jar.csv', index_col=0)
# plt.imshow(df.T, aspect='auto', origin='lower')
```

### Chirp Detection

Communication signals appear as rapid frequency excursions:

```bash
# Fast time resolution for transient chirps
sonido analyze spectrogram courtship.wav -o chirps.csv --fft-size 512 --hop 32
```

### EOD Stability Measurement

```bash
# Welch averaging for precise frequency estimation
sonido analyze spectrum fish.wav --welch --overlap 0.8 --fft-size 65536 --peaks 3
```

## Slime Mold (Physarum polycephalum)

Slime mold electrical oscillations are extremely slow (0.01-0.1 Hz). The trick: treat your sampling interval as a "virtual sample rate."

### Data Preparation

If you sample voltage every 0.5 seconds:
- Your effective "sample rate" = 2.0 Hz
- 2 hours of data = 14,400 samples
- A 50-second oscillation (0.02 Hz) is detectable

```python
# Convert timeseries to WAV
import numpy as np
from scipy.io import wavfile

# Your voltage readings (mV scaled to -1.0 to 1.0)
voltage_data = np.array([...])  # 14400 samples at 0.5s intervals
sample_rate = 2  # 2 samples per second

# Scale to audio range
normalized = voltage_data / np.max(np.abs(voltage_data))
wavfile.write('physarum.wav', sample_rate, normalized.astype(np.float32))
```

### Analysis

```bash
# Spectrum of oscillation frequencies
sonido analyze spectrum physarum.wav --welch --peaks 5
# Peak at "0.02 Hz" indicates 50-second shuttle streaming rhythm

# Time-frequency analysis
sonido analyze spectrogram physarum.wav -o oscillations.csv --fft-size 128 --hop 16
# Reveals how oscillation frequency changes with stimuli

# Dynamics
sonido analyze dynamics physarum.wav
# Crest factor indicates regularity of oscillations
```

### Multi-Electrode Analysis

For multiple recording sites, compare phase relationships:

```bash
# Record each electrode to separate channel
# Analyze transfer function between sites
sonido analyze transfer electrode1.wav electrode2.wav --group-delay
# Group delay reveals signal propagation direction
```

## Frequency Scale Reference

| Signal | Frequency | Required Sample Rate |
|--------|-----------|---------------------|
| Electric eel pulse | 1-10 Hz | 100+ Hz |
| Pulse-type fish EOD | 20-100 Hz | 1 kHz+ |
| Wave-type fish EOD | 200-600 Hz | 5 kHz+ |
| Slime mold oscillation | 0.01-0.1 Hz | 1 Hz |
| Plant action potential | 0.001-0.01 Hz | 0.1 Hz |

## Tips

1. **Normalize your data** to -1.0 to 1.0 range before conversion to WAV
2. **Use appropriate FFT sizes** - longer for slow signals
3. **Welch averaging** is essential for noisy biological recordings
4. **Export spectrograms to CSV** for visualization in Python/R/MATLAB
5. **Group delay** can reveal signal propagation in distributed systems
