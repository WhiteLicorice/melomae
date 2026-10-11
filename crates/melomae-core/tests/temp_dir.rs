//! Task 34: every temporary fixture directory is unique, even when many tests run at once.
//!
//! `TempDir::new` used to name its directory from the process id and the clock only. On Windows the
//! clock advances in 100 ns steps, so two tests in one binary that called it in the same step got
//! one path. The first test's `Drop` then removed the directory while the other still used it. This
//! failure made the hosted `windows-latest` run red on `round3_fixes` for task 34. These tests pin
//! the uniqueness that fixes it.

mod common;
mod setup;

use std::collections::HashSet;
use std::panic::catch_unwind;
use std::sync::{Arc, Barrier};
use std::thread;

use setup::{Fixtures, TempDir};

/// The number of threads that race to create temporary directories at once.
const PROBE_THREADS: usize = 16;

/// The number of directories each probe thread creates.
const PROBE_ROUNDS: usize = 500;

/// Two `TempDir` values must never share a path, however tightly the threads interleave.
///
/// The threads meet at a barrier before each round, so every round releases all calls in one burst.
/// A name built only from the clock repeats when two calls land in one clock step. A process-wide
/// counter in the name makes every path distinct. Each call is caught, so a panic in one call fails
/// the test instead of leaving the other threads at the barrier.
#[test]
fn temp_dir_new_returns_distinct_paths_under_threads() {
    let barrier = Arc::new(Barrier::new(PROBE_THREADS));
    let mut handles = Vec::with_capacity(PROBE_THREADS);
    for _ in 0..PROBE_THREADS {
        let barrier = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            let mut paths = Vec::with_capacity(PROBE_ROUNDS);
            let mut panics = 0usize;
            for _ in 0..PROBE_ROUNDS {
                barrier.wait();
                match catch_unwind(|| TempDir::new("melomae-unique-probe")) {
                    Ok(dir) => paths.push(dir.path().to_path_buf()),
                    Err(_) => panics += 1,
                }
            }
            (paths, panics)
        }));
    }

    let mut seen = HashSet::new();
    let mut duplicates = Vec::new();
    let mut panics = 0usize;
    for handle in handles {
        let (paths, thread_panics) = handle
            .join()
            .expect("a probe thread panicked outside a call");
        panics += thread_panics;
        for path in paths {
            if !seen.insert(path.clone()) {
                duplicates.push(path);
            }
        }
    }

    assert_eq!(
        panics,
        0,
        "{panics} of {} TempDir calls panicked instead of returning a directory",
        PROBE_THREADS * PROBE_ROUNDS
    );
    assert!(
        duplicates.is_empty(),
        "{} of {} TempDir paths were reused; first duplicate: {}",
        duplicates.len(),
        PROBE_THREADS * PROBE_ROUNDS,
        duplicates[0].display()
    );
}

/// Two parallel fixture builds must keep separate directories. This is the CI symptom through the
/// public fixture entry that the decode and encode tasks build on: `Fixtures::build` on two threads
/// at once.
#[test]
fn e2e_parallel_fixture_builds_keep_separate_directories() {
    let barrier = Arc::new(Barrier::new(2));
    let spawn = |barrier: Arc<Barrier>| {
        thread::spawn(move || {
            barrier.wait();
            Fixtures::build()
        })
    };
    let first = spawn(Arc::clone(&barrier));
    let second = spawn(Arc::clone(&barrier));

    let first = first.join().expect("the first fixture build panicked");
    let second = second.join().expect("the second fixture build panicked");

    assert_ne!(
        first.dir(),
        second.dir(),
        "two parallel fixture builds shared one temporary directory"
    );

    let awkward = "Florida!!!.opus";
    assert!(
        first.path(awkward).is_file(),
        "the first fixture set is missing {awkward}"
    );
    assert!(
        second.path(awkward).is_file(),
        "the second fixture set is missing {awkward}"
    );
}
