//! Deterministic concurrency regressions for the store.
//!
//! Each test installs a SQLite busy callback on the store's own connection. The
//! callback fires while the store's write waits on a lock that another
//! connection holds. It proves the transition already ran its read and its
//! validation, so the test can then change the row and release the lock. The
//! handshakes use bounded timeouts, so a wrong order fails the test instead of
//! hanging it.
//!
//! A plain timeout or a barrier cannot force this order. A callback that fires
//! at the write boundary can.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::Connection;

use super::{JobState, Store, StoreError};

/// The upper bound on every handshake. A missed step fails the test.
const TIMEOUT: Duration = Duration::from_secs(10);

/// A temporary database file that is deleted when it drops.
struct TempDb {
    path: PathBuf,
}

impl TempDb {
    fn new(tag: &str) -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};

        static COUNTER: AtomicU64 = AtomicU64::new(0);

        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
        let mut path = std::env::temp_dir();
        path.push(format!(
            "melomae-store-{tag}-{}-{nanos}-{sequence}.db",
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

/// A blocked-write signal and the matching release from the test.
type Handshake = (mpsc::Sender<()>, mpsc::Receiver<()>);

/// Signals that the store write is blocked, waits for the release, then retries.
///
/// The first call completes the handshake and sets `released`. Every later call
/// returns `true`, so the write keeps retrying until the lock frees. That is
/// what lets two writers actually reach their write. A call that cannot complete
/// the handshake returns `false`, which turns the wait into a `SQLITE_BUSY`
/// error instead of a hang.
fn retry_after_release(handshake: &Mutex<Option<Handshake>>, released: &AtomicBool) -> bool {
    if released.load(Ordering::SeqCst) {
        return true;
    }
    let taken = match handshake.lock() {
        Ok(mut guard) => guard.take(),
        Err(poisoned) => poisoned.into_inner().take(),
    };
    match taken {
        Some((blocked, release)) => {
            if blocked.send(()).is_err() || release.recv_timeout(TIMEOUT).is_err() {
                return false;
            }
            released.store(true, Ordering::SeqCst);
            true
        }
        None => false,
    }
}

static CANCEL_HANDSHAKE: Mutex<Option<Handshake>> = Mutex::new(None);
static CANCEL_RELEASED: AtomicBool = AtomicBool::new(false);

fn cancel_busy(_count: i32) -> bool {
    retry_after_release(&CANCEL_HANDSHAKE, &CANCEL_RELEASED)
}

static COMPETE_LEFT: Mutex<Option<Handshake>> = Mutex::new(None);
static COMPETE_LEFT_RELEASED: AtomicBool = AtomicBool::new(false);
static COMPETE_RIGHT: Mutex<Option<Handshake>> = Mutex::new(None);
static COMPETE_RIGHT_RELEASED: AtomicBool = AtomicBool::new(false);

fn compete_left_busy(_count: i32) -> bool {
    retry_after_release(&COMPETE_LEFT, &COMPETE_LEFT_RELEASED)
}

fn compete_right_busy(_count: i32) -> bool {
    retry_after_release(&COMPETE_RIGHT, &COMPETE_RIGHT_RELEASED)
}

#[test]
fn a_cancelled_job_cannot_be_overwritten_by_a_stale_transition() {
    let db = TempDb::new("race-cancel");
    let path = db.path().to_path_buf();
    let store = Store::open(&path).unwrap();
    let id = store.insert_job("/music/a.mp3", true).unwrap();
    store.transition_job(id, JobState::Restoring).unwrap();

    // A second connection cancels the job in an open write transaction.
    let rival = Connection::open(&path).unwrap();
    rival.execute_batch("BEGIN IMMEDIATE").unwrap();
    rival
        .execute("UPDATE jobs SET state = 'cancelled' WHERE id = ?1", [id])
        .unwrap();

    // The busy callback fires only after the transition read `restoring` and
    // validated it, and its write is now waiting on the rival. Hold the
    // cancellation until then, so the read cannot miss it.
    let (blocked_tx, blocked_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    *CANCEL_HANDSHAKE.lock().unwrap() = Some((blocked_tx, release_rx));
    store.connection.busy_handler(Some(cancel_busy)).unwrap();

    let committer = std::thread::spawn(move || {
        blocked_rx
            .recv_timeout(TIMEOUT)
            .expect("the transition never blocked on the rival write");
        rival.execute_batch("COMMIT").unwrap();
        release_tx.send(()).unwrap();
    });

    let result = store.transition_job(id, JobState::Encoding);
    committer.join().unwrap();

    assert!(
        matches!(result, Err(StoreError::IllegalTransition { .. })),
        "a stale transition overwrote the cancel: {result:?}"
    );
    assert_eq!(store.job(id).unwrap().state, JobState::Cancelled);
}

#[test]
fn only_one_of_two_competing_transitions_wins() {
    let db = TempDb::new("race-compete");
    let path = db.path().to_path_buf();

    let left_store = Store::open(&path).unwrap();
    let id = left_store.insert_job("/music/a.mp3", true).unwrap();
    let right_store = Store::open(&path).unwrap();

    // A third connection holds the write lock, so neither store can write yet.
    let gate = Connection::open(&path).unwrap();
    gate.execute_batch("BEGIN IMMEDIATE").unwrap();

    let (left_blocked_tx, left_blocked_rx) = mpsc::channel();
    let (left_release_tx, left_release_rx) = mpsc::channel();
    *COMPETE_LEFT.lock().unwrap() = Some((left_blocked_tx, left_release_rx));
    left_store
        .connection
        .busy_handler(Some(compete_left_busy))
        .unwrap();

    let (right_blocked_tx, right_blocked_rx) = mpsc::channel();
    let (right_release_tx, right_release_rx) = mpsc::channel();
    *COMPETE_RIGHT.lock().unwrap() = Some((right_blocked_tx, right_release_rx));
    right_store
        .connection
        .busy_handler(Some(compete_right_busy))
        .unwrap();

    let left = std::thread::spawn(move || left_store.transition_job(id, JobState::Restoring));
    let right = std::thread::spawn(move || right_store.transition_job(id, JobState::Restoring));

    // Both callers must reach their blocked write before the lock is released.
    // That forces both to read `queued` before either writes.
    left_blocked_rx
        .recv_timeout(TIMEOUT)
        .expect("the left transition never blocked");
    right_blocked_rx
        .recv_timeout(TIMEOUT)
        .expect("the right transition never blocked");
    gate.execute_batch("COMMIT").unwrap();
    left_release_tx.send(()).unwrap();
    right_release_tx.send(()).unwrap();

    let left = left.join().unwrap();
    let right = right.join().unwrap();

    let winners = [left.is_ok(), right.is_ok()]
        .into_iter()
        .filter(|won| *won)
        .count();
    assert_eq!(
        winners, 1,
        "exactly one transition must win: {left:?} and {right:?}"
    );

    let store = Store::open(&path).unwrap();
    assert_eq!(store.job(id).unwrap().state, JobState::Restoring);
}
