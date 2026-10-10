//! End-to-end test for the store.
//!
//! It opens a store on a temporary file, runs one job through every legal
//! state to `done`, closes the store, reopens it, and checks the persisted
//! job, settings, and history row.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use melomae_core::store::{HistoryResult, HistoryRow, JobState, Store, Verdict};

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
fn e2e_store_persists_a_job_settings_and_history_across_a_reopen() {
    let db = TempDb::new("e2e");

    let written = HistoryRow {
        input_path: "/music/in/song.mp3".to_string(),
        input_sha256: "0f1e2d3c".to_string(),
        output_path: Some("/music/restored/song.flac".to_string()),
        verdict: Some(Verdict::Restore),
        settings: Some("restore=true format=flac".to_string()),
        provenance: Some("model=apollo device=cpu".to_string()),
        result: HistoryResult::Restored,
        recorded_at: 1_700_000_000,
    };

    let job_id;
    {
        let store = Store::open(db.path()).unwrap();
        store.set_setting("output_dir", "/music/restored").unwrap();

        job_id = store.insert_job("/music/in/song.mp3", true).unwrap();
        for state in [
            JobState::Restoring,
            JobState::Encoding,
            JobState::Tagging,
            JobState::Done,
        ] {
            store.transition_job(job_id, state).unwrap();
        }

        store.insert_history(&written).unwrap();
    }

    let store = Store::open(db.path()).unwrap();

    let job = store.job(job_id).unwrap();
    assert_eq!(job.state, JobState::Done);
    assert_eq!(job.input_path, "/music/in/song.mp3");
    assert!(job.restore_enabled);

    assert_eq!(
        store.setting("output_dir").unwrap().as_deref(),
        Some("/music/restored")
    );

    assert_eq!(store.history().unwrap(), vec![written]);
}
