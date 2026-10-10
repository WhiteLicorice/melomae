//! Shared helpers for the decode test targets.
//!
//! Signal synthesis and measurement only. The fixtures themselves are built by `setup`, which
//! encodes synthesized audio with the pinned ffmpeg into a temporary directory.

#![allow(dead_code)]

use std::fs;
use std::path::Path;

/// The sample rate that [`melomae_core::decode::decode_file`] always produces.
pub const TARGET_RATE: u32 = 44_100;

/// Read a raw little-endian `f32` file, as the ffmpeg oracle writes it.
pub fn read_f32le(path: &Path) -> Vec<f32> {
    let bytes = fs::read(path).expect("read the raw f32 reference");
    assert_eq!(
        bytes.len() % 4,
        0,
        "raw f32 reference length is not a multiple of four"
    );
    bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

/// Split interleaved samples into planar channels.
pub fn planar_from_interleaved(interleaved: &[f32], channels: usize) -> Vec<Vec<f32>> {
    assert!(channels > 0, "channel count must be positive");
    let frames = interleaved.len() / channels;
    let mut planes = vec![vec![0.0f32; frames]; channels];
    for (index, sample) in interleaved.iter().enumerate() {
        planes[index % channels][index / channels] = *sample;
    }
    planes
}

/// Interleave planar channels back into a single buffer.
pub fn interleave(planes: &[Vec<f32>]) -> Vec<f32> {
    let channels = planes.len();
    let frames = planes.first().map_or(0, Vec::len);
    let mut out = Vec::with_capacity(frames * channels);
    for frame in 0..frames {
        for plane in planes {
            out.push(plane[frame]);
        }
    }
    out
}

/// A sine of the given frequency and amplitude.
pub fn sine(sample_rate: u32, frequency: f64, amplitude: f64, frames: usize) -> Vec<f32> {
    (0..frames)
        .map(|i| {
            let t = i as f64 / f64::from(sample_rate);
            (amplitude * (2.0 * std::f64::consts::PI * frequency * t).sin()) as f32
        })
        .collect()
}

/// A logarithmic frequency sweep from `start` to `end` Hertz.
pub fn log_sweep(
    sample_rate: u32,
    start: f64,
    end: f64,
    amplitude: f64,
    frames: usize,
) -> Vec<f32> {
    let rate = f64::from(sample_rate);
    let duration = frames as f64 / rate;
    sweep_for_duration(rate, start, end, amplitude, duration, frames)
}

/// The same sweep described by a duration, so the sample times line up across sample rates.
///
/// The phase depends only on time, not on the sample rate, so the sweep sampled at 44.1 kHz is
/// the analytic reference for the same sweep sampled at 48 kHz.
pub fn log_sweep_seconds(
    sample_rate: u32,
    start: f64,
    end: f64,
    amplitude: f64,
    seconds: f64,
) -> Vec<f32> {
    let rate = f64::from(sample_rate);
    let frames = (rate * seconds).round() as usize;
    sweep_for_duration(rate, start, end, amplitude, seconds, frames)
}

fn sweep_for_duration(
    rate: f64,
    start: f64,
    end: f64,
    amplitude: f64,
    duration: f64,
    frames: usize,
) -> Vec<f32> {
    let k = (end / start).ln() / duration;
    (0..frames)
        .map(|i| {
            let t = i as f64 / rate;
            let phase = 2.0 * std::f64::consts::PI * start * ((k * t).exp() - 1.0) / k;
            (amplitude * phase.sin()) as f32
        })
        .collect()
}

/// Write planar 32-bit float samples to a WAV file.
///
/// The helper is deliberately small: it writes one `fmt ` chunk with the IEEE-float format tag
/// and one `data` chunk, which is all the synthesized fixtures need.
pub fn write_wav_f32(path: &Path, sample_rate: u32, planes: &[Vec<f32>]) {
    let channels = planes.len() as u16;
    let interleaved = interleave(planes);
    let data_len = (interleaved.len() * 4) as u32;
    let block_align = channels * 4;
    let byte_rate = sample_rate * u32::from(block_align);

    let mut out = Vec::with_capacity(44 + interleaved.len() * 4);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&3u16.to_le_bytes());
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&32u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for sample in interleaved {
        out.extend_from_slice(&sample.to_le_bytes());
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create the fixture directory");
    }
    fs::write(path, out).expect("write the synthesized WAV");
}

/// Root-mean-square of a signal.
pub fn rms(samples: &[f32]) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum: f64 = samples.iter().map(|s| f64::from(*s) * f64::from(*s)).sum();
    (sum / samples.len() as f64).sqrt()
}

/// The largest absolute difference between two equally long buffers.
pub fn max_abs_diff(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len(), "buffers must be the same length");
    a.iter()
        .zip(b)
        .map(|(x, y)| (x - y).abs())
        .fold(0.0, f32::max)
}

/// Signal-to-noise ratio of `test` against `reference` in decibels.
///
/// The two buffers are compared as-is from index zero; callers trim leading and trailing
/// transients before calling.
pub fn snr_db(reference: &[f32], test: &[f32]) -> f64 {
    let len = reference.len().min(test.len());
    assert!(len > 0, "cannot score an empty signal");
    let mut signal = 0.0f64;
    let mut noise = 0.0f64;
    for i in 0..len {
        let r = f64::from(reference[i]);
        let t = f64::from(test[i]);
        signal += r * r;
        noise += (r - t) * (r - t);
    }
    assert!(noise > 0.0, "the test signal is identical to the reference");
    10.0 * (signal / noise).log10()
}

/// Total harmonic distortion plus noise in decibels, relative to the fundamental.
///
/// The fundamental at `frequency` is removed by a least-squares fit of a sine, a cosine, and a
/// constant over an integer number of periods. The residual is everything else: harmonics,
/// aliasing images, and wideband noise.
pub fn thd_plus_n_db(samples: &[f32], sample_rate: u32, frequency: f64) -> f64 {
    let rate = f64::from(sample_rate);
    let cycles = ((samples.len() as f64) * frequency / rate).floor() as usize;
    assert!(
        cycles >= 2,
        "the window is too short to measure the fundamental"
    );
    let n = (cycles as f64 * rate / frequency).round() as usize;
    let window = &samples[..n];

    let mut sum_s = 0.0f64;
    let mut sum_c = 0.0f64;
    let mut sum_ss = 0.0f64;
    let mut sum_cc = 0.0f64;
    let mut sum_xs = 0.0f64;
    let mut sum_xc = 0.0f64;
    let mut sum_x = 0.0f64;

    for (i, sample) in window.iter().enumerate() {
        let x = f64::from(*sample);
        let phase = 2.0 * std::f64::consts::PI * frequency * i as f64 / rate;
        let s = phase.sin();
        let c = phase.cos();
        sum_s += s;
        sum_c += c;
        sum_ss += s * s;
        sum_cc += c * c;
        sum_xs += x * s;
        sum_xc += x * c;
        sum_x += x;
    }

    // Re-orthogonalize against the constant term so a residual DC offset cannot leak into the
    // fundamental estimate.
    let count = n as f64;
    let mean_s = sum_s / count;
    let mean_c = sum_c / count;
    let mean_x = sum_x / count;
    let var_s = sum_ss / count - mean_s * mean_s;
    let var_c = sum_cc / count - mean_c * mean_c;
    let cov_sx = sum_xs / count - mean_s * mean_x;
    let cov_cx = sum_xc / count - mean_c * mean_x;
    let amplitude_s = cov_sx / var_s;
    let amplitude_c = cov_cx / var_c;

    let mut residual = 0.0f64;
    for (i, sample) in window.iter().enumerate() {
        let x = f64::from(*sample);
        let phase = 2.0 * std::f64::consts::PI * frequency * i as f64 / rate;
        let fitted =
            mean_x + amplitude_s * (phase.sin() - mean_s) + amplitude_c * (phase.cos() - mean_c);
        residual += (x - fitted) * (x - fitted);
    }

    let residual_rms = (residual / count).sqrt();
    let fundamental_rms =
        (amplitude_s * amplitude_s + amplitude_c * amplitude_c).sqrt() / std::f64::consts::SQRT_2;
    20.0 * (residual_rms / fundamental_rms).log10()
}
