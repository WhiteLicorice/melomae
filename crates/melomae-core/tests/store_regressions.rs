//! Regression tests for the store's transition, schema, and concurrency rules.
//!
//! Every test here fails against the reviewed build of the store. They use only
//! the public API that the reviewed build already had, so each failure is
//! behavioral and not a compile error.

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

const ALL_STATES: [JobState; 7] = [
    JobState::Queued,
    JobState::Restoring,
    JobState::Encoding,
    JobState::Tagging,
    JobState::Done,
    JobState::Failed,
    JobState::Cancelled,
];

/// Runs all 49 state pairs for each restore flag through the public API.
#[test]
fn store_transition_matrix_follows_the_flag_contract() {
    use JobState::*;

    let db = TempDb::new("matrix");
    let store = Store::open(db.path()).unwrap();
    let raw = Connection::open(db.path()).unwrap();
    let mut differences = Vec::new();

    for restore_enabled in [false, true] {
        let id = store.insert_job("/music/a.mp3", restore_enabled).unwrap();
        for from in ALL_STATES {
            for to in ALL_STATES {
                raw.execute(
                    "UPDATE jobs SET state = ?1 WHERE id = ?2",
                    rusqlite::params![from.as_str(), id],
                )
                .unwrap();

                let expected = match from {
                    Queued => match to {
                        Restoring => restore_enabled,
                        Encoding => !restore_enabled,
                        Failed | Cancelled => true,
                        _ => false,
                    },
                    Restoring => matches!(to, Encoding | Failed | Cancelled),
                    Encoding => matches!(to, Tagging | Failed | Cancelled),
                    Tagging => matches!(to, Done | Failed | Cancelled),
                    Done | Failed | Cancelled => false,
                };

                match store.transition_job(id, to) {
                    Ok(()) if expected => {}
                    Err(StoreError::IllegalTransition { .. }) if !expected => {
                        assert_eq!(
                            store.job(id).unwrap().state,
                            from,
                            "a rejected move changed the state"
                        );
                    }
                    other => differences.push(format!(
                        "restore_enabled={restore_enabled}, {from:?} -> {to:?}: {other:?}"
                    )),
                }
            }
        }
    }

    assert!(
        differences.is_empty(),
        "98 cases checked, differences: {differences:?}"
    );
}

#[test]
fn store_restore_on_cannot_skip_restoring() {
    let db = TempDb::new("restore-on-negative");
    let store = Store::open(db.path()).unwrap();
    let id = store.insert_job("/music/a.mp3", true).unwrap();

    let result = store.transition_job(id, JobState::Encoding);
    assert!(
        matches!(result, Err(StoreError::IllegalTransition { .. })),
        "a restore-on job must enter restoring first, got {result:?}"
    );
    assert_eq!(store.job(id).unwrap().state, JobState::Queued);
}

#[test]
fn store_restore_off_cannot_enter_restoring() {
    let db = TempDb::new("restore-off-negative");
    let store = Store::open(db.path()).unwrap();
    let id = store.insert_job("/music/a.mp3", false).unwrap();

    let result = store.transition_job(id, JobState::Restoring);
    assert!(
        matches!(result, Err(StoreError::IllegalTransition { .. })),
        "a restore-off job must skip restoring, got {result:?}"
    );
    assert_eq!(store.job(id).unwrap().state, JobState::Queued);
}

#[test]
fn store_a_second_open_keeps_live_jobs() {
    let db = TempDb::new("second-open");
    let first = Store::open(db.path()).unwrap();
    let id = first.insert_job("/music/a.mp3", true).unwrap();
    first.transition_job(id, JobState::Restoring).unwrap();

    let second = Store::open(db.path()).unwrap();

    assert_eq!(first.job(id).unwrap().state, JobState::Restoring);
    assert_eq!(second.job(id).unwrap().state, JobState::Restoring);
}

#[test]
fn store_a_newer_schema_is_rejected_without_a_write() {
    let db = TempDb::new("future-schema");
    let id = {
        let store = Store::open(db.path()).unwrap();
        store.set_setting("output_dir", "/music").unwrap();
        let id = store.insert_job("/music/a.mp3", true).unwrap();
        store.transition_job(id, JobState::Restoring).unwrap();
        id
    };

    let raw = Connection::open(db.path()).unwrap();
    raw.execute_batch("PRAGMA user_version = 3;").unwrap();

    assert!(
        Store::open(db.path()).is_err(),
        "a newer schema must be rejected"
    );

    let version: i64 = raw
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(version, 3, "open changed the schema version");
    let state: String = raw
        .query_row("SELECT state FROM jobs WHERE id = ?1", [id], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(state, "restoring", "open changed a job row");
    let value: String = raw
        .query_row(
            "SELECT value FROM settings WHERE key = 'output_dir'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(value, "/music", "open changed a settings row");
}
