//! Choose the restore chunk size from the memory available at job start.
//!
//! SRS §6.4 and §12.3 require that a long file does not exhaust RAM. The peak
//! memory of a restore is the whole-file reserve plus the peak inference memory
//! of one chunk. The inference peak is modeled as a baseline plus a cost per
//! padded channel-second:
//!
//! ```text
//! peak = PEAK_BASE_BYTES + PEAK_BYTES_PER_CHANNEL_SECOND x (chunk + 2 x pad)
//! ```
//!
//! Task 01 first fitted `0.36 GB + 0.58 GB x channel-seconds` on a single
//! session run. Task 05 measured the real engine and found that the ONNX
//! Runtime CPU memory arena grew the peak across chunks, from about 11 GB after
//! the first chunk to 22 GB after the last on a 60 s input. The engine now
//! turns the arena off (see `inference::OnnxOptions`), and the constants below
//! are fitted on that shipping path. The engine runs one channel at a time
//! (Stack §C), so a padded second is one padded channel-second. The two values
//! are decimal bytes (1 GB = 1e9 bytes).
//!
//! The whole-file reserve ([`file_reserve_bytes`]) does not depend on the
//! chunk. The chooser reserves it before it picks a chunk, because the engine
//! holds whole-file buffers that grow with the file length (review finding B1).

use std::fmt;

use super::InferenceError;

/// The smallest chunk the chooser will pick, in seconds (task 05 scope 4).
pub const MIN_CHUNK_SECONDS: u64 = 4;

/// The largest chunk the chooser will pick, in seconds (task 05 scope 4).
pub const MAX_CHUNK_SECONDS: u64 = 30;

/// The audio inferred beyond each chunk edge and then discarded, in seconds
/// (task 05 scope 4).
pub const PAD_SECONDS: u64 = 1;

/// The available memory is divided by this value to give the peak budget
/// (task 05 scope 3).
pub const BUDGET_DIVISOR: u64 = 2;

/// The peak baseline, in bytes, fitted on the shipping engine path (task 05).
pub const PEAK_BASE_BYTES: u64 = 206_745_941;

/// The peak growth per padded channel-second, in bytes, fitted on the shipping
/// engine path (task 05).
pub const PEAK_BYTES_PER_CHANNEL_SECOND: u64 = 541_471_232;

/// The size of one `f32` sample, in bytes.
const BYTES_PER_SAMPLE: u64 = 4;

/// The frame count and channel count of the file a restore will process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileShape {
    /// The number of frames in the file.
    pub frames: u64,
    /// The number of channels in the file.
    pub channels: u64,
}

/// The whole-file buffers a restore holds for a file of this shape, in bytes.
///
/// The chunked engine (`run_chunked`) allocates two buffers the length
/// of the file, `output_sum` (`inference/mod.rs:362`) and `weight_sum` (`:363`),
/// and one more for the normalized channel (`:411`). It also keeps every
/// finished channel in the restore accumulator (`:467`). While the last channel
/// is normalized, the engine holds `(channels + 2) x frames` samples of `f32`.
/// The caller holds the decoded input as well, `channels x frames` samples of
/// `f32`. The reserve is the sum:
///
/// ```text
/// reserve = (2 x channels + 2) x frames x 4 bytes
/// ```
///
/// This confirms the reviewer's engine estimate of `(channels + 2) x frames x 4`
/// bytes and adds the decoded input, which is resident by the time a chunk runs.
/// The reserve does not depend on the chunk. It does not count the ONNX
/// allocations or the per-chunk tensors, which the inference peak covers.
pub fn file_reserve_bytes(shape: FileShape) -> u64 {
    (2 * shape.channels + 2) * shape.frames * BYTES_PER_SAMPLE
}

/// The chunk the chooser picks, with the memory numbers behind it.
///
/// A caller logs this with its [`Display`](fmt::Display) implementation
/// (task 05 scope 7). Task 17 writes the text to the job row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkBudget {
    /// The chosen chunk length, in whole seconds.
    pub chunk_seconds: u64,
    /// The whole-file reserve for this file, in bytes ([`file_reserve_bytes`]).
    pub reserve_bytes: u64,
    /// The predicted peak memory of one padded chunk, in bytes.
    pub predicted_peak_bytes: u64,
    /// The share of the available memory the restore may use, in bytes.
    pub budget_bytes: u64,
    /// The memory available when the chooser ran, in bytes.
    pub available_bytes: u64,
}

impl ChunkBudget {
    /// The whole-file reserve plus the predicted inference peak, in bytes.
    ///
    /// This is the total the restore may hold against the budget.
    pub fn total_bytes(&self) -> u64 {
        self.reserve_bytes + self.predicted_peak_bytes
    }
}

impl fmt::Display for ChunkBudget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "chunk {}s: reserve {} bytes, predicted inference peak {} bytes, total {} bytes, \
             budget {} bytes, available {} bytes",
            self.chunk_seconds,
            self.reserve_bytes,
            self.predicted_peak_bytes,
            self.total_bytes(),
            self.budget_bytes,
            self.available_bytes
        )
    }
}

/// The predicted peak inference memory of one padded chunk, in bytes.
///
/// The model is a baseline plus a fixed cost per padded channel-second, with
/// both pads counted (task 05 scope 1). The constants are fitted on the
/// shipping engine path (task 05).
pub fn predicted_peak_bytes(chunk_seconds: u64) -> u64 {
    PEAK_BASE_BYTES + PEAK_BYTES_PER_CHANNEL_SECOND * (chunk_seconds + 2 * PAD_SECONDS)
}

/// The largest chunk in the allowed range whose reserve and predicted peak fit
/// the budget.
///
/// Returns `None` when even [`MIN_CHUNK_SECONDS`] does not fit.
pub fn largest_chunk_within(budget_bytes: u64, reserve_bytes: u64) -> Option<u64> {
    (MIN_CHUNK_SECONDS..=MAX_CHUNK_SECONDS)
        .rev()
        .find(|chunk| reserve_bytes + predicted_peak_bytes(*chunk) <= budget_bytes)
}

/// Choose a chunk from the memory available at job start.
///
/// The budget is one [`BUDGET_DIVISOR`]th of the available memory. The chooser
/// reserves the file's whole-file buffers ([`file_reserve_bytes`]) and picks the
/// largest chunk whose reserve plus predicted inference peak fits the budget.
/// The pure function takes the available memory and the file shape as
/// arguments, so a caller can inject them. The wrapper
/// [`choose_chunk_for_machine`] reads the memory from the system.
pub fn choose_chunk(available_bytes: u64, shape: FileShape) -> Result<ChunkBudget, InferenceError> {
    let budget_bytes = available_bytes / BUDGET_DIVISOR;
    let reserve_bytes = file_reserve_bytes(shape);
    let Some(chunk_seconds) = largest_chunk_within(budget_bytes, reserve_bytes) else {
        return Err(InferenceError::InsufficientMemory {
            needed: reserve_bytes + predicted_peak_bytes(MIN_CHUNK_SECONDS),
            budget: budget_bytes,
            available: available_bytes,
        });
    };
    Ok(ChunkBudget {
        chunk_seconds,
        reserve_bytes,
        predicted_peak_bytes: predicted_peak_bytes(chunk_seconds),
        budget_bytes,
        available_bytes,
    })
}

/// Read the memory available on this machine and choose a chunk.
///
/// The pipeline calls this once at job start (task 05 scope 2).
pub fn choose_chunk_for_machine(shape: FileShape) -> Result<ChunkBudget, InferenceError> {
    let mut system = sysinfo::System::new();
    system.refresh_memory();
    choose_chunk(system.available_memory(), shape)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One gibibyte, as a binary multiple.
    const GIB: u64 = 1024 * 1024 * 1024;

    /// One second of audio at 44.1 kHz.
    const SECOND: u64 = 44_100;

    /// The shape of a file of `seconds` seconds and `channels` channels.
    fn shape(seconds: u64, channels: u64) -> FileShape {
        FileShape {
            frames: SECOND * seconds,
            channels,
        }
    }

    #[test]
    fn chunk_budget_chooses_the_largest_chunk_that_fits() {
        // A short file, so the whole-file reserve is small.
        let short = shape(13, 2);
        let cases = [
            (8 * GIB, 5u64),
            (16 * GIB, 13),
            (64 * GIB, MAX_CHUNK_SECONDS),
        ];
        for (available, expected) in cases {
            let budget = choose_chunk(available, short).expect("the memory must fit a chunk");
            assert_eq!(
                budget.chunk_seconds, expected,
                "available {available} bytes"
            );
            assert_eq!(budget.available_bytes, available);
            assert_eq!(budget.budget_bytes, available / BUDGET_DIVISOR);
            assert_eq!(budget.reserve_bytes, file_reserve_bytes(short));
            assert_eq!(
                budget.predicted_peak_bytes,
                predicted_peak_bytes(expected),
                "the result must carry the predicted peak"
            );
            assert_eq!(
                budget.total_bytes(),
                budget.reserve_bytes + budget.predicted_peak_bytes
            );
            assert!(budget.total_bytes() <= budget.budget_bytes);
            // The next chunk up, when there is one, must not fit.
            if expected < MAX_CHUNK_SECONDS {
                assert!(
                    budget.reserve_bytes + predicted_peak_bytes(expected + 1) > budget.budget_bytes,
                    "chunk {} must not fit",
                    expected + 1
                );
            }
        }
    }

    #[test]
    fn chunk_budget_accepts_an_exact_budget_match() {
        // The `<=` predicate must include a chunk whose total equals the budget.
        let short = shape(13, 2);
        let reserve = file_reserve_bytes(short);
        let available = (reserve + predicted_peak_bytes(13)) * BUDGET_DIVISOR;
        let budget = choose_chunk(available, short).expect("the exact match must fit");
        assert_eq!(budget.chunk_seconds, 13);
        assert_eq!(budget.total_bytes(), budget.budget_bytes);
    }

    #[test]
    fn chunk_budget_reserves_the_whole_file_buffers() {
        // The engine holds output_sum, weight_sum, the normalized channel, and
        // every finished channel; the caller holds the decoded input.
        for (seconds, channels) in [(1u64, 1u64), (60, 2), (600, 2), (3_600, 6)] {
            let file = shape(seconds, channels);
            let engine = (channels + 2) * file.frames * 4;
            let input = channels * file.frames * 4;
            assert_eq!(file_reserve_bytes(file), engine + input);
        }
    }

    #[test]
    fn chunk_budget_shortens_the_chunk_for_a_long_stereo_file() {
        let available = 16 * GIB;
        let long = shape(600, 2);
        // With no file reserve, 16 GiB fits a 13 s chunk.
        assert_eq!(
            largest_chunk_within(available / BUDGET_DIVISOR, 0),
            Some(13)
        );
        let budget = choose_chunk(available, long).expect("the memory must fit a chunk");
        assert_eq!(budget.chunk_seconds, 12);
        assert!(budget.total_bytes() <= budget.budget_bytes);
    }

    #[test]
    fn chunk_budget_shortens_the_chunk_for_a_long_mono_file() {
        let available = 16 * GIB;
        let long = shape(3_600, 1);
        assert_eq!(
            largest_chunk_within(available / BUDGET_DIVISOR, 0),
            Some(13)
        );
        let budget = choose_chunk(available, long).expect("the memory must fit a chunk");
        assert_eq!(budget.chunk_seconds, 8);
        assert!(budget.total_bytes() <= budget.budget_bytes);
    }

    #[test]
    fn chunk_budget_rejects_a_one_hour_six_channel_file() {
        // Port of the reviewer's rev05_long_six_channel_file_must_fit_the_restore_budget.
        // At 8649843 the chooser accepted the file even though the engine's
        // whole-file buffers alone exceed the budget.
        let available = 8 * GIB;
        let long = shape(3_600, 6);
        let engine = (long.channels + 2) * long.frames * 4;
        assert!(
            engine > available / BUDGET_DIVISOR,
            "the case must prove the reserve exceeds the budget"
        );
        let error = choose_chunk(available, long).expect_err("the reserve must not fit");
        let message = error.to_string();
        let budget = available / BUDGET_DIVISOR;
        assert!(
            message.contains(&budget.to_string()),
            "the message must name the budget {budget}: {message}"
        );
        assert!(
            message.contains(&available.to_string()),
            "the message must name the available memory {available}: {message}"
        );
        let needed = file_reserve_bytes(long) + predicted_peak_bytes(MIN_CHUNK_SECONDS);
        assert!(
            message.contains(&needed.to_string()),
            "the message must name the total {needed}: {message}"
        );
    }

    #[test]
    fn chunk_budget_rejects_memory_below_the_floor() {
        let short = shape(60, 2);
        for available in [2 * GIB, 4 * GIB] {
            let error = choose_chunk(available, short).expect_err("the memory must be too small");
            let message = error.to_string();
            let needed = file_reserve_bytes(short) + predicted_peak_bytes(MIN_CHUNK_SECONDS);
            assert!(
                message.contains(&needed.to_string()),
                "the message must name the needed memory {needed}: {message}"
            );
            assert!(
                message.contains(&available.to_string()),
                "the message must name the available memory {available}: {message}"
            );
        }
    }

    #[test]
    fn chunk_budget_floor_error_names_the_half_ram_budget() {
        // Port of the reviewer's rev05_floor_error_must_explain_the_half_ram_budget.
        // The message must name the limiting budget, the total, and the available.
        let available = 4 * GIB;
        let short = shape(60, 2);
        let error = choose_chunk(available, short).expect_err("the memory must be too small");
        let message = error.to_string();
        let budget = available / BUDGET_DIVISOR;
        let needed = file_reserve_bytes(short) + predicted_peak_bytes(MIN_CHUNK_SECONDS);
        assert!(
            message.contains(&budget.to_string()),
            "rejection must name the limiting budget {budget}: {message}"
        );
        assert!(
            message.contains(&needed.to_string()),
            "rejection must name the required total {needed}: {message}"
        );
        assert!(
            message.contains(&available.to_string()),
            "rejection must name the available memory {available}: {message}"
        );
    }

    #[test]
    fn chunk_budget_display_names_the_chunk_and_the_numbers() {
        let budget = choose_chunk(16 * GIB, shape(13, 2)).expect("the memory must fit a chunk");
        assert_eq!(budget.chunk_seconds, 13);
        let text = budget.to_string();
        for value in [
            budget.chunk_seconds.to_string(),
            budget.reserve_bytes.to_string(),
            budget.predicted_peak_bytes.to_string(),
            budget.total_bytes().to_string(),
            budget.budget_bytes.to_string(),
            budget.available_bytes.to_string(),
        ] {
            assert!(text.contains(&value), "{value} must show: {text}");
        }
    }

    #[test]
    fn predicted_peak_grows_per_padded_channel_second() {
        // The fitted line, with both pads counted.
        assert_eq!(predicted_peak_bytes(4), 3_455_573_333);
        assert_eq!(predicted_peak_bytes(8), 5_621_458_261);
        assert_eq!(predicted_peak_bytes(12), 7_787_343_189);
        assert_eq!(predicted_peak_bytes(30), 17_533_825_365);
        assert_eq!(
            predicted_peak_bytes(10) - predicted_peak_bytes(9),
            PEAK_BYTES_PER_CHANNEL_SECOND
        );
    }

    #[test]
    fn predicted_peak_matches_the_measured_engine_peaks() {
        // The peaks task 05 measured on the shipping path (arena off), on a
        // 60 s stereo input at chunk 4, 8, and 12 seconds
        // (`SCRATCH/evidence-05/probe-fit.txt`). The fit uses these points, so
        // the model must reproduce each within the 15% acceptance band.
        let measured = [
            (4u64, 3_456_028_672u64),
            (8, 5_620_547_584),
            (12, 7_787_798_528),
        ];
        for (chunk, peak) in measured {
            let predicted = predicted_peak_bytes(chunk);
            let error = (predicted as f64 - peak as f64) / peak as f64;
            assert!(
                error.abs() <= 0.15,
                "chunk {chunk}s: predicted {predicted} against measured {peak} is {error:.4}"
            );
        }
    }
}
