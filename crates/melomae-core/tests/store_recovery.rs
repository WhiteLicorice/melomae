//! Startup recovery and schema-version tests for the store.
//!
//! These tests use the review fix API: `Store::recover_interrupted_jobs` and
//! `StoreError::UnsupportedSchema`.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use melomae_core::store::{JobState, Store, StoreError};
use rusqlite::Connection;

/// A temporary database file that is deleted when it drops.
struct TempDb {
    path: PathBuf,
}

impl TempDb {
    fn new(tag: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let mut path = std::env::temp_dir();
        path.push(format!(
            "melomae-store-{tag}-{}-{nanos}.db",
            std::process::id()
        ));
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDb {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
        for suffix in ["-journal", "-wal", "-shm"] {
            let mut sidecar = self.path.clone().into_os_string();
            sidecar.push(suffix);
            let _ = std::fs::remove_file(PathBuf::from(sidecar));
        }
    }
}

#[test]
fn store_recover_interrupted_jobs_requeues_in_flight_and_keeps_terminal() {
    let db = TempDb::new("recover");
    let (restoring, encoding, tagging, done, cancelled, failed) = {
        let store = Store::open(db.path()).unwrap();

        let restoring = store.insert_job("/music/a.mp3", true).unwrap();
        store
            .transition_job(restoring, JobState::Restoring)
            .unwrap();

        let encoding = store.insert_job("/music/b.mp3", true).unwrap();
        store.transition_job(encoding, JobState::Restoring).unwrap();
        store.transition_job(encoding, JobState::Encoding).unwrap();

        let tagging = store.insert_job("/music/c.mp3", true).unwrap();
        store.transition_job(tagging, JobState::Restoring).unwrap();
        store.transition_job(tagging, JobState::Encoding).unwrap();
        store.transition_job(tagging, JobState::Tagging).unwrap();

        let done = store.insert_job("/music/d.mp3", true).unwrap();
        store.transition_job(done, JobState::Restoring).unwrap();
        store.transition_job(done, JobState::Encoding).unwrap();
        store.transition_job(done, JobState::Tagging).unwrap();
        store.transition_job(done, JobState::Done).unwrap();

        let cancelled = store.insert_job("/music/e.mp3", true).unwrap();
        store
            .transition_job(cancelled, JobState::Cancelled)
            .unwrap();

        let failed = store.insert_job("/music/f.mp3", true).unwrap();
        store.transition_job(failed, JobState::Restoring).unwrap();
        store.transition_job(failed, JobState::Failed).unwrap();

        (restoring, encoding, tagging, done, cancelled, failed)
    };

    let store = Store::open(db.path()).unwrap();
    assert_eq!(store.recover_interrupted_jobs().unwrap(), 3);

    for in_flight in [restoring, encoding, tagging] {
        assert_eq!(store.job(in_flight).unwrap().state, JobState::Queued);
    }
    assert_eq!(store.job(done).unwrap().state, JobState::Done);
    assert_eq!(store.job(cancelled).unwrap().state, JobState::Cancelled);
    assert_eq!(store.job(failed).unwrap().state, JobState::Failed);

    assert_eq!(store.recover_interrupted_jobs().unwrap(), 0);
}

#[test]
fn store_a_newer_schema_returns_a_typed_error_naming_both_versions() {
    let db = TempDb::new("future-typed");
    {
        let store = Store::open(db.path()).unwrap();
        store.set_setting("output_dir", "/music").unwrap();
    }
    let raw = Connection::open(db.path()).unwrap();
    raw.execute_batch("PRAGMA user_version = 3;").unwrap();

    let error = Store::open(db.path())
        .err()
        .expect("a newer schema must be rejected");
    match error {
        StoreError::UnsupportedSchema { found, supported } => {
            assert_eq!(found, 3);
            assert_eq!(supported, 2);
        }
        other => panic!("expected UnsupportedSchema, got {other:?}"),
    }
}
