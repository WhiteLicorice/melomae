//! Choose the restore chunk size from the memory available at job start.
//!
//! SRS §6.4 and §12.3 require that a long file does not exhaust RAM. The peak
//! inference memory is modeled from the task 01 measurement
//! (`docs/evidence/01-onnx-spike.md:147`):
//!
//! ```text
//! peak = 0.36 GB + 0.58 GB x (chunk + 2 x pad)
//! ```
//!
//! The engine runs one channel at a time (Stack §C), so the channel-second
//! count is the padded length of one chunk. Task 01 measured the peak working
//! set, so the two values are decimal units (1 GB = 1e9 bytes).

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

/// The peak baseline, in bytes, from task 01.
pub const PEAK_BASE_BYTES: u64 = 360_000_000;

/// The peak growth per padded channel-second, in bytes, from task 01.
pub const PEAK_BYTES_PER_CHANNEL_SECOND: u64 = 580_000_000;

/// The chunk the chooser picks, with the memory numbers behind it.
///
/// A caller logs this with its [`Display`](fmt::Display) implementation
/// (task 05 scope 7). Task 17 writes the text to the job row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkBudget {
    /// The chosen chunk length, in whole seconds.
    pub chunk_seconds: u64,
    /// The predicted peak memory of one padded chunk, in bytes.
    pub predicted_peak_bytes: u64,
    /// The share of the available memory the peak may use, in bytes.
    pub budget_bytes: u64,
    /// The memory available when the chooser ran, in bytes.
    pub available_bytes: u64,
}

impl fmt::Display for ChunkBudget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "chunk {}s: predicted peak {} bytes, budget {} bytes, available {} bytes",
            self.chunk_seconds, self.predicted_peak_bytes, self.budget_bytes, self.available_bytes
        )
    }
}

/// The predicted peak inference memory of one padded chunk, in bytes.
///
/// The model is the task 01 line: a baseline plus a fixed cost per padded
/// channel-second, with both pads counted (task 05 scope 1).
pub fn predicted_peak_bytes(chunk_seconds: u64) -> u64 {
    PEAK_BASE_BYTES + PEAK_BYTES_PER_CHANNEL_SECOND * (chunk_seconds + 2 * PAD_SECONDS)
}

/// The largest chunk in the allowed range whose predicted peak fits the budget.
///
/// Returns `None` when even [`MIN_CHUNK_SECONDS`] does not fit.
pub fn largest_chunk_within(budget_bytes: u64) -> Option<u64> {
    (MIN_CHUNK_SECONDS..=MAX_CHUNK_SECONDS)
        .rev()
        .find(|chunk| predicted_peak_bytes(*chunk) <= budget_bytes)
}

/// Choose a chunk from the memory available at job start.
///
/// The budget is one [`BUDGET_DIVISOR`]th of the available memory. The pure
/// function takes the available memory as an argument, so a caller can inject
/// it. The wrapper [`choose_chunk_for_machine`] reads it from the system.
pub fn choose_chunk(available_bytes: u64) -> Result<ChunkBudget, InferenceError> {
    let budget_bytes = available_bytes / BUDGET_DIVISOR;
    let Some(chunk_seconds) = largest_chunk_within(budget_bytes) else {
        return Err(InferenceError::InsufficientMemory {
            needed: predicted_peak_bytes(MIN_CHUNK_SECONDS),
            available: available_bytes,
        });
    };
    Ok(ChunkBudget {
        chunk_seconds,
        predicted_peak_bytes: predicted_peak_bytes(chunk_seconds),
        budget_bytes,
        available_bytes,
    })
}

/// Read the memory available on this machine and choose a chunk.
///
/// The pipeline calls this once at job start (task 05 scope 2).
pub fn choose_chunk_for_machine() -> Result<ChunkBudget, InferenceError> {
    let mut system = sysinfo::System::new();
    system.refresh_memory();
    choose_chunk(system.available_memory())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One gibibyte, as a binary multiple.
    const GIB: u64 = 1024 * 1024 * 1024;

    #[test]
    fn chunk_budget_chooses_the_largest_chunk_that_fits() {
        let cases = [
            (8 * GIB, 4u64),
            (16 * GIB, 12),
            (64 * GIB, MAX_CHUNK_SECONDS),
        ];
        for (available, expected) in cases {
            let budget = choose_chunk(available).expect("the memory must fit a chunk");
            assert_eq!(
                budget.chunk_seconds, expected,
                "available {available} bytes"
            );
            assert_eq!(budget.available_bytes, available);
            assert_eq!(budget.budget_bytes, available / BUDGET_DIVISOR);
            assert_eq!(
                budget.predicted_peak_bytes,
                predicted_peak_bytes(expected),
                "the result must carry the predicted peak"
            );
            assert!(budget.predicted_peak_bytes <= budget.budget_bytes);
            // The next chunk up, when there is one, must not fit.
            if expected < MAX_CHUNK_SECONDS {
                assert!(
                    predicted_peak_bytes(expected + 1) > budget.budget_bytes,
                    "chunk {} must not fit",
                    expected + 1
                );
            }
        }
    }

    #[test]
    fn chunk_budget_rejects_memory_below_the_floor() {
        for available in [2 * GIB, 4 * GIB] {
            let error = choose_chunk(available).expect_err("the memory must be too small");
            let message = error.to_string();
            let needed = predicted_peak_bytes(MIN_CHUNK_SECONDS);
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
    fn chunk_budget_display_names_the_chunk_and_the_numbers() {
        let budget = choose_chunk(16 * GIB).expect("the memory must fit a chunk");
        assert_eq!(budget.chunk_seconds, 12);
        let text = budget.to_string();
        assert!(text.contains("12"), "the chunk must show: {text}");
        assert!(
            text.contains(&budget.predicted_peak_bytes.to_string()),
            "the predicted peak must show: {text}"
        );
        assert!(
            text.contains(&budget.budget_bytes.to_string()),
            "the budget must show: {text}"
        );
        assert!(
            text.contains(&budget.available_bytes.to_string()),
            "the available memory must show: {text}"
        );
    }

    #[test]
    fn predicted_peak_grows_per_padded_channel_second() {
        // The task 01 line, with both pads counted.
        assert_eq!(predicted_peak_bytes(4), 3_840_000_000);
        assert_eq!(predicted_peak_bytes(6), 5_000_000_000);
        assert_eq!(predicted_peak_bytes(30), 18_920_000_000);
        assert_eq!(
            predicted_peak_bytes(10) - predicted_peak_bytes(9),
            PEAK_BYTES_PER_CHANNEL_SECOND
        );
    }
}
