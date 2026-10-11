//! Measure the spectral cutoff of decoded audio.
//!
//! A lossy codec removes the top band with a low-pass filter. The cutoff is the highest frequency
//! that still carries signal, so it tells a lossy source from a lossless one, and a heavily
//! compressed source from a lightly compressed one.
//!
//! The method is Welch's method. The spectrum is averaged over Hann-windowed frames. The frames are
//! taken at start positions spread evenly over the whole file, and every channel is analyzed at
//! every selected position. The average is smoothed, and the peak bin is the reference. The cutoff
//! is the highest frequency that is the top of a run of bins within [`DROP_DB`] of the reference.
//! The run length is at least [`MIN_BAND_HZ`] wide, so a single spectral spike does not count as
//! signal.

use realfft::num_complex::Complex;
use realfft::RealFftPlanner;

use crate::decode::AudioBuffer;

/// The frame length. 8192 samples at 44.1 kHz is 186 ms, or 5.4 Hz per bin.
const WINDOW: usize = 8192;

/// The shortest window the measurement trusts. A file with fewer frames returns a cutoff of zero.
const MIN_FRAMES: usize = 1024;

/// The most analysis positions spread over the file. The cost is the position count times the
/// channel count, so the budget caps positions and not windows per channel.
const MAX_POSITIONS: usize = 128;

/// The smoothing radius, in bins. The moving average spans `2 * (SMOOTH_BINS / 2) + 1` bins, so this
/// value spans five bins.
const SMOOTH_BINS: usize = 4;

/// A band within this many decibels of the spectrum peak counts as signal.
///
/// The value comes from the task 16 calibration on `fixtures/ori_wav.wav` (recorded in the task
/// `Outcome`). A shallower drop, such as 60 dB, follows the source's own high-frequency rolloff: the
/// real source sits at -60 dB by 16 kHz, so a 60 dB drop measures the source and not the codec. At
/// 80 dB the lossless source reaches 22 kHz and each lossy low-pass still reads its own corner. The
/// number is fixed for every file; it is not tuned to one.
const DROP_DB: f64 = 80.0;

/// The shortest run of above-threshold bins that counts as a signal band.
const MIN_BAND_HZ: f64 = 200.0;

/// The spectral cutoff of `audio` in Hertz, using the production threshold.
///
/// The audio is expected at a rate up to 44.1 kHz, so the cutoff never exceeds its Nyquist
/// frequency. Audio with too few frames returns `0.0`.
pub fn measure_cutoff_hz(audio: &AudioBuffer) -> f64 {
    measure_cutoff_hz_at(audio, DROP_DB)
}

/// The spectral cutoff of `audio` in Hertz, with an explicit threshold in decibels below the
/// spectrum peak. [`measure_cutoff_hz`] calls this with the production [`DROP_DB`].
///
/// The calibration test uses this to record how the measured cutoff moves with the threshold, so
/// the chosen [`DROP_DB`] has recorded evidence.
pub fn measure_cutoff_hz_at(audio: &AudioBuffer, drop_db: f64) -> f64 {
    let Some(spectrum) = averaged_spectrum(audio) else {
        return 0.0;
    };
    let peak = spectrum.smoothed.iter().copied().fold(0.0f64, f64::max);
    if peak <= 0.0 {
        return 0.0;
    }
    let threshold = peak * 10f64.powf(-drop_db / 10.0);
    let run_bins = spectrum.min_band_bins();

    let mut run = 0usize;
    for bin in (1..spectrum.smoothed.len()).rev() {
        if spectrum.smoothed[bin] >= threshold {
            run += 1;
            if run >= run_bins {
                let top = bin + run - 1;
                return (top as f64 * spectrum.bin_width).min(spectrum.nyquist);
            }
        } else {
            run = 0;
        }
    }
    0.0
}

/// An averaged, smoothed power spectrum.
struct Spectrum {
    smoothed: Vec<f64>,
    bin_width: f64,
    nyquist: f64,
}

impl Spectrum {
    /// The number of bins in a run that spans at least [`MIN_BAND_HZ`].
    fn min_band_bins(&self) -> usize {
        ((MIN_BAND_HZ / self.bin_width).ceil() as usize).max(2)
    }
}

/// Average the magnitude-squared spectrum of `audio` over Hann-windowed frames, and smooth it.
///
/// The frames are taken at up to [`MAX_POSITIONS`] start positions spread evenly over the whole
/// file, from the first valid start to the last, so the start, the middle, and the end are always
/// included. Every channel is analyzed at every selected position. A silent or narrow-band opening
/// therefore cannot hide the rest of the file, and a silent channel cannot hide the others. The
/// position count is capped, so the cost is positions times channels.
///
/// Returns `None` when the audio is too short to window.
fn averaged_spectrum(audio: &AudioBuffer) -> Option<Spectrum> {
    let rate = f64::from(audio.sample_rate());
    let frames = audio.frames();
    if rate <= 0.0 || frames < MIN_FRAMES {
        return None;
    }
    let window = WINDOW.min(highest_power_of_two(frames));
    if window < MIN_FRAMES {
        return None;
    }
    let bins = window / 2 + 1;
    let hann: Vec<f32> = (0..window)
        .map(|index| {
            let phase = std::f64::consts::PI * index as f64 / window as f64;
            (phase.sin().powi(2)) as f32
        })
        .collect();

    let mut planner = RealFftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(window);
    let mut input = vec![0f32; window];
    let mut spectrum = vec![Complex::default(); bins];
    let mut power = vec![0f64; bins];

    // The valid start positions run from 0 to `span`. Never take more positions than half-overlapping
    // windows would use, so a short file is not sampled at near-identical positions.
    let hop = (window / 2).max(1);
    let span = frames - window;
    let positions = MAX_POSITIONS.min(span / hop + 1).max(1);

    let mut turns = 0usize;
    for position in 0..positions {
        let start = if positions == 1 {
            0
        } else {
            span * position / (positions - 1)
        };
        for channel in audio.channels() {
            for index in 0..window {
                input[index] = channel[start + index] * hann[index];
            }
            fft.process(&mut input, &mut spectrum)
                .expect("the real FFT of a fixed-size window cannot fail");
            for (bin, value) in power.iter_mut().zip(&spectrum) {
                *bin += f64::from(value.re) * f64::from(value.re)
                    + f64::from(value.im) * f64::from(value.im);
            }
            turns += 1;
        }
    }

    if turns == 0 {
        return None;
    }
    for bin in &mut power {
        *bin /= turns as f64;
    }
    Some(Spectrum {
        smoothed: moving_average(&power, SMOOTH_BINS),
        bin_width: rate / window as f64,
        nyquist: rate / 2.0,
    })
}

/// The largest power of two that is no greater than `value`.
fn highest_power_of_two(value: usize) -> usize {
    if value == 0 {
        return 0;
    }
    1usize << (usize::BITS - 1 - value.leading_zeros())
}

/// The moving average of `values` over a window of `width` bins.
fn moving_average(values: &[f64], width: usize) -> Vec<f64> {
    let half = width / 2;
    values
        .iter()
        .enumerate()
        .map(|(index, _)| {
            let low = index.saturating_sub(half);
            let high = (index + half + 1).min(values.len());
            values[low..high].iter().sum::<f64>() / (high - low) as f64
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Deterministic white noise, amplitude +/-0.3.
    fn noise(frames: usize, seed: u64) -> Vec<f32> {
        let mut state = seed;
        (0..frames)
            .map(|_| {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                (((state >> 40) as f32 / (1u32 << 24) as f32) - 0.5) * 0.6
            })
            .collect()
    }

    /// White noise with every bin above `cutoff_hz` zeroed, as an exact brick-wall low-pass.
    fn band_limited_noise(frames: usize, rate: u32, cutoff_hz: f64) -> Vec<f32> {
        let mut planner = RealFftPlanner::<f32>::new();
        let forward = planner.plan_fft_forward(frames);
        let inverse = planner.plan_fft_inverse(frames);
        let mut input = noise(frames, 0x1234_5678);
        let mut spectrum = vec![Complex::default(); frames / 2 + 1];
        forward.process(&mut input, &mut spectrum).unwrap();
        let bin_width = f64::from(rate) / frames as f64;
        for (index, value) in spectrum.iter_mut().enumerate() {
            if index as f64 * bin_width > cutoff_hz {
                *value = Complex::default();
            }
        }
        let mut output = vec![0f32; frames];
        inverse.process(&mut spectrum, &mut output).unwrap();
        let scale = 1.0 / frames as f32;
        for sample in &mut output {
            *sample *= scale;
        }
        output
    }

    #[test]
    fn a_full_band_signal_reaches_nyquist() {
        let audio = AudioBuffer::new(44_100, vec![noise(44_100, 0xabcd_ef01)]);
        let cutoff = measure_cutoff_hz(&audio);
        assert!(
            cutoff > 20_000.0,
            "full-band noise must reach the top of the band, got {cutoff}"
        );
    }

    #[test]
    fn a_brick_wall_low_pass_is_found_at_its_corner() {
        for corner in [8_000.0, 12_000.0, 16_000.0] {
            let samples = band_limited_noise(44_100, 44_100, corner);
            let audio = AudioBuffer::new(44_100, vec![samples]);
            let cutoff = measure_cutoff_hz(&audio);
            assert!(
                (cutoff - corner).abs() < 1_000.0,
                "the {corner} Hz low-pass was measured at {cutoff} Hz"
            );
        }
    }

    #[test]
    fn silence_has_no_cutoff() {
        let audio = AudioBuffer::new(44_100, vec![vec![0.0; 44_100]]);
        assert_eq!(measure_cutoff_hz(&audio), 0.0);
    }

    #[test]
    fn a_too_short_signal_has_no_cutoff() {
        let audio = AudioBuffer::new(44_100, vec![vec![0.1; 512]]);
        assert_eq!(measure_cutoff_hz(&audio), 0.0);
    }

    #[test]
    fn analysis_starts_include_the_first_central_and_last_start() {
        let window = 8192usize;
        let hop = window / 2;
        for span in [0usize, hop / 2, hop, hop * 3] {
            let starts = analysis_starts(window + span, window);

            let mut sorted = starts.clone();
            sorted.sort_unstable();
            sorted.dedup();
            assert_eq!(
                sorted, starts,
                "the starts are not sorted and unique for span {span}"
            );
            assert!(
                starts.iter().all(|start| *start <= span),
                "a start is past the last valid start for span {span}"
            );

            if span == 0 {
                assert_eq!(starts, vec![0], "a zero span must use one start");
                continue;
            }
            assert!(
                starts.contains(&0),
                "the first start is missing for span {span}"
            );
            assert!(
                starts.contains(&span),
                "the last start is missing for span {span}"
            );
            assert!(
                starts.contains(&(span / 2)),
                "the central start is missing for span {span}"
            );
            assert!(starts.len() >= 3, "too few starts for span {span}");
        }
    }
}
