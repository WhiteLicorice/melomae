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
