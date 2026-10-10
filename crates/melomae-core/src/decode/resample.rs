//! Resampling to the target sample rate with rubato's high-quality sinc resampler.

use std::f64::consts::PI;

use rubato::audioadapter::Adapter;
use rubato::audioadapter_buffers::direct::SequentialSliceOfVecs;
use rubato::{
    Async, FixedAsync, Resampler, SincInterpolationParameters, SincInterpolationType,
    WindowFunction,
};

/// The sinc filter length. 256 taps with the Blackman-Harris window is rubato's high-quality
/// setting and matches the `filter_size=256` of the ffmpeg swr reference.
const SINC_LEN: usize = 256;

/// The number of precomputed sinc phases between two input samples.
const OVERSAMPLING_FACTOR: usize = 256;

/// The near-Nyquist cutoff the owner's old pipeline used (`cutoff=0.98`).
const CUTOFF: f32 = 0.98;

/// The largest ratio change the resampler may make between chunks. The ratio is fixed here, so
/// the value only sets the construction range.
const MAX_RATIO_RELATIVE: f64 = 1.1;

/// The number of frames the resampler processes per chunk.
const CHUNK_FRAMES: usize = 1024;

/// The half-length of the fractional-delay correction filter, in samples.
const CORRECTION_HALF_LEN: i64 = 64;

/// Resample planar channels from `from_rate` to `to_rate`.
///
/// The output is aligned to the input timeline, so output frame `n` holds the input at time
/// `n / to_rate`. rubato's `process_all` removes only the integer part of its own filter delay,
/// so the remaining fraction is measured and removed here. Without that, every resampled file
/// would carry a constant sub-sample offset.
pub(crate) fn planar(input: &[Vec<f32>], from_rate: u32, to_rate: u32) -> Vec<Vec<f32>> {
    let channels = input.len();
    let frames = input.first().map_or(0, Vec::len);
    let ratio = f64::from(to_rate) / f64::from(from_rate);

    let parameters = SincInterpolationParameters::new(SINC_LEN, WindowFunction::BlackmanHarris2)
        .f_cutoff(CUTOFF)
        .oversampling_factor(OVERSAMPLING_FACTOR)
        .interpolation(SincInterpolationType::Cubic);

    let mut resampler = Async::<f32>::new_sinc(
        ratio,
        MAX_RATIO_RELATIVE,
        &parameters,
        CHUNK_FRAMES,
        channels,
        FixedAsync::Input,
    )
    .expect("the sinc resampler parameters are valid constants");

    let residual = residual_delay(&mut resampler, ratio, channels);

    let input = SequentialSliceOfVecs::new(input, channels, frames)
        .expect("the input planes match the channel and frame counts");
    let output = resampler
        .process_all(&input, frames, None)
        .expect("resampling a whole clip cannot fail");

    let output_frames = output.frames();
    let mut planes = vec![vec![0f32; output_frames]; channels];
    for (index, sample) in output.take_data().into_iter().enumerate() {
        planes[index % channels][index / channels] = sample;
    }

    if residual.abs() > 1e-7 {
        for plane in &mut planes {
            *plane = shift(plane, -residual);
        }
    }
    planes
}

/// Measure the sub-sample part of the resampler's delay by locating a resampled impulse.
fn residual_delay(resampler: &mut Async<f32>, ratio: f64, channels: usize) -> f64 {
    const FRAMES: usize = 4096;
    let at = FRAMES / 2;

    let mut impulse = vec![vec![0f32; FRAMES]; channels];
    for plane in &mut impulse {
        plane[at] = 1.0;
    }

    let adapter = SequentialSliceOfVecs::new(&impulse, channels, FRAMES)
        .expect("the calibration input matches the channel count");
    let Ok(output) = resampler.process_all(&adapter, FRAMES, None) else {
        return 0.0;
    };

    let ideal = at as f64 * ratio;
    let centre = ideal.round() as i64;
    // The impulse response of a `SINC_LEN`-tap filter spans about `SINC_LEN * ratio` output
    // samples, so the moment window must cover all of it.
    let half_window = SINC_LEN as i64 + 16;
    let (mut moment, mut mass) = (0.0f64, 0.0f64);
    for n in (centre - half_window)..=(centre + half_window) {
        if n < 0 || n as usize >= output.frames() {
            continue;
        }
        let value = f64::from(output.read_sample(0, n as usize).unwrap_or(0.0));
        moment += n as f64 * value;
        mass += value;
    }
    if mass == 0.0 {
        0.0
    } else {
        moment / mass - ideal
    }
}

/// Shift `input` by `delay` samples (positive delays) with a windowed-sinc fractional-delay filter.
fn shift(input: &[f32], delay: f64) -> Vec<f32> {
    let taps: Vec<f64> = (-CORRECTION_HALF_LEN..=CORRECTION_HALF_LEN)
        .map(|k| kernel(k as f64 - delay))
        .collect();
    let sum: f64 = taps.iter().sum();
    let taps: Vec<f64> = taps.iter().map(|tap| tap / sum).collect();

    let mut out = vec![0f32; input.len()];
    for (n, value) in out.iter_mut().enumerate() {
        let mut acc = 0.0f64;
        for (index, tap) in taps.iter().enumerate() {
            let k = index as i64 - CORRECTION_HALF_LEN;
            let source = n as i64 - k;
            if source < 0 || source as usize >= input.len() {
                continue;
            }
            acc += f64::from(input[source as usize]) * tap;
        }
        *value = acc as f32;
    }
    out
}

/// A Blackman-Harris windowed sinc tap at sample offset `x`.
fn kernel(x: f64) -> f64 {
    let half = CORRECTION_HALF_LEN as f64;
    if x.abs() > half {
        return 0.0;
    }
    let sinc = if x.abs() < f64::EPSILON {
        1.0
    } else {
        (PI * x).sin() / (PI * x)
    };
    let u = x / half;
    let window = 0.35875
        + 0.48829 * (PI * u).cos()
        + 0.14128 * (2.0 * PI * u).cos()
        + 0.01168 * (3.0 * PI * u).cos();
    sinc * window
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_zero_delay_shift_keeps_the_signal() {
        let input: Vec<f32> = (0..512).map(|n| ((n as f64) * 0.1).sin() as f32).collect();
        let output = shift(&input, 0.0);
        for (a, b) in input.iter().zip(&output) {
            assert!((a - b).abs() < 1e-6, "{a} vs {b}");
        }
    }

    #[test]
    fn a_fractional_shift_keeps_a_constant_signal() {
        let input = vec![0.5f32; 512];
        let output = shift(&input, 0.375);
        for value in &output[64..448] {
            assert!((value - 0.5).abs() < 1e-6, "dc gain is not one: {value}");
        }
    }

    #[test]
    fn a_kernel_sums_to_one_across_the_fraction() {
        for step in 0..8 {
            let delay = f64::from(step) / 8.0;
            let sum: f64 = (-CORRECTION_HALF_LEN..=CORRECTION_HALF_LEN)
                .map(|k| kernel(k as f64 - delay))
                .sum();
            assert!((sum - 1.0).abs() < 1e-3, "delay {delay}: sum {sum}");
        }
    }
}
