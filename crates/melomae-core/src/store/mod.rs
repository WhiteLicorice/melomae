//! Persistence for settings, jobs, triage results, and history.
//!
//! One SQLite database holds all four. The caller passes the database path, so
//! the app passes its data directory and tests pass a temporary file. Schema
//! migrations are numbered by `PRAGMA user_version`.

use std::fmt;
use std::path::Path;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, Row};

/// The schema version this build writes. Each migration increments it.
const SCHEMA_VERSION: i64 = 1;

/// The tables and columns of migration 1.
const MIGRATION_1: &str = r#"
CREATE TABLE settings (
    key   TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
);

CREATE TABLE jobs (
    id              INTEGER PRIMARY KEY,
    input_path      TEXT    NOT NULL,
    output_path     TEXT,
    restore_enabled INTEGER NOT NULL,
    state           TEXT    NOT NULL,
    error           TEXT,
    created_at      INTEGER NOT NULL
);

CREATE TABLE triage_items (
    id                 INTEGER PRIMARY KEY,
    path               TEXT    NOT NULL UNIQUE,
    size_bytes         INTEGER NOT NULL,
    modified_at        INTEGER NOT NULL,
    container          TEXT,
    codec              TEXT,
    bitrate_kbps       INTEGER,
    sample_rate_hz     INTEGER,
    bit_depth          INTEGER,
    duration_seconds   REAL,
    spectral_cutoff_hz REAL,
    verdict            TEXT    NOT NULL,
    reason             TEXT,
    scanned_at         INTEGER NOT NULL
);

CREATE TABLE history (
    id           INTEGER PRIMARY KEY,
    input_path   TEXT    NOT NULL,
    input_sha256 TEXT    NOT NULL,
    output_path  TEXT,
    verdict      TEXT,
    settings     TEXT,
    provenance   TEXT,
    result       TEXT    NOT NULL,
    recorded_at  INTEGER NOT NULL
);
"#;

/// The row identifier of a job.
pub type JobId = i64;

/// Every state of the job state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JobState {
    Queued,
    Restoring,
    Encoding,
    Tagging,
    Done,
    Failed,
    Cancelled,
}

impl JobState {
    /// The value stored in the database.
    pub fn as_str(self) -> &'static str {
        match self {
            JobState::Queued => "queued",
            JobState::Restoring => "restoring",
            JobState::Encoding => "encoding",
            JobState::Tagging => "tagging",
            JobState::Done => "done",
            JobState::Failed => "failed",
            JobState::Cancelled => "cancelled",
        }
    }

    /// Whether the state ends the job. A terminal state has no exit.
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            JobState::Done | JobState::Failed | JobState::Cancelled
        )
    }

    /// Whether a job in this state may move to `next` (task 14 scope item 4).
    ///
    /// A job with restore on moves `queued -> restoring`. A job with restore
    /// off moves `queued -> encoding`. `queued -> failed` and
    /// `queued -> cancelled` are legal for either flag. Every active state
    /// accepts `failed` and `cancelled`. `tagging -> done` ends the job.
    /// Terminal states accept nothing.
    pub fn can_transition_to(self, next: JobState, restore_enabled: bool) -> bool {
        use JobState::*;

        match self {
            Queued => match next {
                Restoring => restore_enabled,
                Encoding => !restore_enabled,
                Failed | Cancelled => true,
                _ => false,
            },
            Restoring => matches!(next, Encoding | Failed | Cancelled),
            Encoding => matches!(next, Tagging | Failed | Cancelled),
            Tagging => matches!(next, Done | Failed | Cancelled),
            Done | Failed | Cancelled => false,
        }
    }
}

impl fmt::Display for JobState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for JobState {
    type Err = StoreError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "queued" => Ok(JobState::Queued),
            "restoring" => Ok(JobState::Restoring),
            "encoding" => Ok(JobState::Encoding),
            "tagging" => Ok(JobState::Tagging),
            "done" => Ok(JobState::Done),
            "failed" => Ok(JobState::Failed),
            "cancelled" => Ok(JobState::Cancelled),
            other => Err(StoreError::UnknownValue {
                column: "state",
                value: other.to_string(),
            }),
        }
    }
}

/// The triage verdict of one input file (SRS §4.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Restore,
    LowBenefit,
    SuspectLossless,
    Lossless,
    Unreadable,
}

impl Verdict {
    /// The value stored in the database.
    pub fn as_str(self) -> &'static str {
        match self {
            Verdict::Restore => "restore",
            Verdict::LowBenefit => "low-benefit",
            Verdict::SuspectLossless => "suspect-lossless",
            Verdict::Lossless => "lossless",
            Verdict::Unreadable => "unreadable",
        }
    }
}

impl FromStr for Verdict {
    type Err = StoreError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "restore" => Ok(Verdict::Restore),
            "low-benefit" => Ok(Verdict::LowBenefit),
            "suspect-lossless" => Ok(Verdict::SuspectLossless),
            "lossless" => Ok(Verdict::Lossless),
            "unreadable" => Ok(Verdict::Unreadable),
            other => Err(StoreError::UnknownValue {
                column: "verdict",
                value: other.to_string(),
            }),
        }
    }
}

/// The result of one history row (SRS §9.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryResult {
    Restored,
    Skipped,
    Failed,
}

impl HistoryResult {
    /// The value stored in the database.
    pub fn as_str(self) -> &'static str {
        match self {
            HistoryResult::Restored => "restored",
            HistoryResult::Skipped => "skipped",
            HistoryResult::Failed => "failed",
        }
    }
}

impl FromStr for HistoryResult {
    type Err = StoreError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "restored" => Ok(HistoryResult::Restored),
            "skipped" => Ok(HistoryResult::Skipped),
            "failed" => Ok(HistoryResult::Failed),
            other => Err(StoreError::UnknownValue {
                column: "result",
                value: other.to_string(),
            }),
        }
    }
}

/// One job in the queue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Job {
    pub id: JobId,
    pub input_path: String,
    pub output_path: Option<String>,
    pub restore_enabled: bool,
    pub state: JobState,
    pub error: Option<String>,
    pub created_at: i64,
}

/// One triaged input file. The fields match the task 16 measurements.
#[derive(Debug, Clone, PartialEq)]
pub struct TriageItem {
    pub path: String,
    pub size_bytes: i64,
    pub modified_at: i64,
    pub container: Option<String>,
    pub codec: Option<String>,
    pub bitrate_kbps: Option<i64>,
    pub sample_rate_hz: Option<i64>,
    pub bit_depth: Option<i64>,
    pub duration_seconds: Option<f64>,
    pub spectral_cutoff_hz: Option<f64>,
    pub verdict: Verdict,
    pub reason: Option<String>,
    pub scanned_at: i64,
}

/// One restore recorded in the history (SRS §8.8).
#[derive(Debug, Clone, PartialEq)]
pub struct HistoryRow {
    pub input_path: String,
    pub input_sha256: String,
    pub output_path: Option<String>,
    pub verdict: Option<Verdict>,
    pub settings: Option<String>,
    pub provenance: Option<String>,
    pub result: HistoryResult,
    pub recorded_at: i64,
}

/// A failure from the store.
#[derive(Debug)]
pub enum StoreError {
    /// A SQLite call failed.
    Database(rusqlite::Error),
    /// The job cannot move from its current state to the requested state.
    IllegalTransition { from: JobState, to: JobState },
    /// A stored text value is not a known value for its column.
    UnknownValue { column: &'static str, value: String },
    /// No job has the requested id.
    JobNotFound(JobId),
    /// The database schema is newer than this build supports.
    UnsupportedSchema { found: i64, supported: i64 },
}

impl fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::Database(error) => write!(formatter, "database error: {error}"),
            StoreError::IllegalTransition { from, to } => {
                write!(formatter, "a job cannot move from {from} to {to}")
            }
            StoreError::UnknownValue { column, value } => {
                write!(formatter, "unknown {column} value: {value}")
            }
            StoreError::JobNotFound(id) => write!(formatter, "no job with id {id}"),
            StoreError::UnsupportedSchema { found, supported } => write!(
                formatter,
                "the database schema is version {found}, but this build supports version {supported}"
            ),
        }
    }
}

impl std::error::Error for StoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            StoreError::Database(error) => Some(error),
            _ => None,
        }
    }
}

impl From<rusqlite::Error> for StoreError {
    fn from(error: rusqlite::Error) -> Self {
        StoreError::Database(error)
    }
}

/// A handle to the SQLite database.
pub struct Store {
    connection: Connection,
}

impl Store {
    /// Opens the database at `path`, creating it when absent and running the
    /// schema migrations it needs.
    ///
    /// `open` does not touch the jobs. A second open while work still runs must
    /// not disturb that work. Call [`Store::recover_interrupted_jobs`] once at
    /// startup to requeue jobs that an earlier run left in flight.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let mut connection = Connection::open(path)?;
        let found = read_user_version(&connection)?;
        if found > SCHEMA_VERSION {
            return Err(StoreError::UnsupportedSchema {
                found,
                supported: SCHEMA_VERSION,
            });
        }
        migrate(&mut connection, found)?;
        Ok(Self { connection })
    }

    /// Moves every interrupted job back to `queued` and returns how many moved.
    ///
    /// An interrupted job is one in `restoring`, `encoding`, or `tagging`,
    /// because no worker in this run owns it. The app calls this once at
    /// startup, before it starts any worker and before it opens a second
    /// handle. `open` does not call it, so a second open leaves live jobs
    /// alone.
    pub fn recover_interrupted_jobs(&self) -> Result<usize, StoreError> {
        let moved = self.connection.execute(
            "UPDATE jobs SET state = 'queued'
             WHERE state IN ('restoring', 'encoding', 'tagging')",
            [],
        )?;
        Ok(moved)
    }

    /// The schema version stored in `PRAGMA user_version`.
    pub fn schema_version(&self) -> Result<i64, StoreError> {
        read_user_version(&self.connection)
    }

    /// Writes a setting, replacing any earlier value.
    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), StoreError> {
        self.connection.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    /// Reads a setting, or `None` when it was never written.
    pub fn setting(&self, key: &str) -> Result<Option<String>, StoreError> {
        let mut statement = self
            .connection
            .prepare("SELECT value FROM settings WHERE key = ?1")?;
        let mut rows = statement.query([key])?;
        match rows.next()? {
            Some(row) => Ok(Some(row.get(0)?)),
            None => Ok(None),
        }
    }

    /// Adds a job in the `queued` state and returns its id.
    pub fn insert_job(&self, input_path: &str, restore_enabled: bool) -> Result<JobId, StoreError> {
        self.connection.execute(
            "INSERT INTO jobs (input_path, restore_enabled, state, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                input_path,
                restore_enabled as i64,
                JobState::Queued.as_str(),
                now_unix()
            ],
        )?;
        Ok(self.connection.last_insert_rowid())
    }

    /// Reads one job by id.
    pub fn job(&self, id: JobId) -> Result<Job, StoreError> {
        let mut statement = self.connection.prepare(
            "SELECT id, input_path, output_path, restore_enabled, state, error, created_at
             FROM jobs WHERE id = ?1",
        )?;
        let mut rows = statement.query([id])?;
        match rows.next()? {
            Some(row) => job_from_row(row),
            None => Err(StoreError::JobNotFound(id)),
        }
    }

    /// Moves a job to `next`.
    ///
    /// The validation covers the job's state and its restore setting. The write
    /// is conditional on the state that was read, so a change from another
    /// writer cannot be overwritten. On that conflict the store re-reads the
    /// job and returns [`StoreError::IllegalTransition`].
    pub fn transition_job(&self, id: JobId, next: JobState) -> Result<(), StoreError> {
        let from = self.job(id)?.state;
        if !from.can_transition_to(next, self.job(id)?.restore_enabled) {
            return Err(StoreError::IllegalTransition { from, to: next });
        }
        let changed = self.connection.execute(
            "UPDATE jobs SET state = ?1 WHERE id = ?2 AND state = ?3",
            params![next.as_str(), id, from.as_str()],
        )?;
        if changed == 1 {
            return Ok(());
        }
        let from = self.job(id)?.state;
        Err(StoreError::IllegalTransition { from, to: next })
    }

    /// Writes one triage result, replacing an earlier row for the same path.
    pub fn upsert_triage_item(&self, item: &TriageItem) -> Result<(), StoreError> {
        self.connection.execute(
            "INSERT INTO triage_items (
                 path, size_bytes, modified_at, container, codec, bitrate_kbps,
                 sample_rate_hz, bit_depth, duration_seconds, spectral_cutoff_hz,
                 verdict, reason, scanned_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
             ON CONFLICT(path) DO UPDATE SET
                 size_bytes = excluded.size_bytes,
                 modified_at = excluded.modified_at,
                 container = excluded.container,
                 codec = excluded.codec,
                 bitrate_kbps = excluded.bitrate_kbps,
                 sample_rate_hz = excluded.sample_rate_hz,
                 bit_depth = excluded.bit_depth,
                 duration_seconds = excluded.duration_seconds,
                 spectral_cutoff_hz = excluded.spectral_cutoff_hz,
                 verdict = excluded.verdict,
                 reason = excluded.reason,
                 scanned_at = excluded.scanned_at",
            params![
                item.path,
                item.size_bytes,
                item.modified_at,
                item.container,
                item.codec,
                item.bitrate_kbps,
                item.sample_rate_hz,
                item.bit_depth,
                item.duration_seconds,
                item.spectral_cutoff_hz,
                item.verdict.as_str(),
                item.reason,
                item.scanned_at,
            ],
        )?;
        Ok(())
    }

    /// Reads the triage result for one path, or `None` when absent.
    pub fn triage_item(&self, path: &str) -> Result<Option<TriageItem>, StoreError> {
        let mut statement = self.connection.prepare(
            "SELECT path, size_bytes, modified_at, container, codec, bitrate_kbps,
                    sample_rate_hz, bit_depth, duration_seconds, spectral_cutoff_hz,
                    verdict, reason, scanned_at
             FROM triage_items WHERE path = ?1",
        )?;
        let mut rows = statement.query([path])?;
        match rows.next()? {
            Some(row) => Ok(Some(triage_item_from_row(row)?)),
            None => Ok(None),
        }
    }

    /// Reads every triage result, ordered by path.
    pub fn triage_items(&self) -> Result<Vec<TriageItem>, StoreError> {
        let mut statement = self.connection.prepare(
            "SELECT path, size_bytes, modified_at, container, codec, bitrate_kbps,
                    sample_rate_hz, bit_depth, duration_seconds, spectral_cutoff_hz,
                    verdict, reason, scanned_at
             FROM triage_items ORDER BY path",
        )?;
        let mut rows = statement.query([])?;
        let mut items = Vec::new();
        while let Some(row) = rows.next()? {
            items.push(triage_item_from_row(row)?);
        }
        Ok(items)
    }

    /// Removes the triage result for one path. A path with no row is not an error.
    pub fn delete_triage_item(&self, path: &str) -> Result<(), StoreError> {
        self.connection
            .execute("DELETE FROM triage_items WHERE path = ?1", [path])?;
        Ok(())
    }

    /// Appends one history row and returns its id.
    pub fn insert_history(&self, row: &HistoryRow) -> Result<i64, StoreError> {
        self.connection.execute(
            "INSERT INTO history (
                 input_path, input_sha256, output_path, verdict, settings,
                 provenance, result, recorded_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                row.input_path,
                row.input_sha256,
                row.output_path,
                row.verdict.map(Verdict::as_str),
                row.settings,
                row.provenance,
                row.result.as_str(),
                row.recorded_at,
            ],
        )?;
        Ok(self.connection.last_insert_rowid())
    }

    /// Reads the history in insertion order.
    pub fn history(&self) -> Result<Vec<HistoryRow>, StoreError> {
        let mut statement = self.connection.prepare(
            "SELECT input_path, input_sha256, output_path, verdict, settings,
                    provenance, result, recorded_at
             FROM history ORDER BY id",
        )?;
        let mut rows = statement.query([])?;
        let mut items = Vec::new();
        while let Some(row) = rows.next()? {
            items.push(history_from_row(row)?);
        }
        Ok(items)
    }
}

/// Runs every migration above the stored version, one time each.
///
/// Each block is guarded by its own version, so a later migration 2 does not
/// replay migration 1. A caller rejects a version above `SCHEMA_VERSION`
/// before this runs.
fn migrate(connection: &mut Connection, from: i64) -> Result<(), StoreError> {
    if from < 1 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(MIGRATION_1)?;
        transaction.execute_batch("PRAGMA user_version = 1;")?;
        transaction.commit()?;
    }
    Ok(())
}

fn read_user_version(connection: &Connection) -> Result<i64, StoreError> {
    let version = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    Ok(version)
}

fn job_from_row(row: &Row<'_>) -> Result<Job, StoreError> {
    Ok(Job {
        id: row.get(0)?,
        input_path: row.get(1)?,
        output_path: row.get(2)?,
        restore_enabled: row.get::<_, i64>(3)? != 0,
        state: row.get::<_, String>(4)?.parse()?,
        error: row.get(5)?,
        created_at: row.get(6)?,
    })
}

fn triage_item_from_row(row: &Row<'_>) -> Result<TriageItem, StoreError> {
    Ok(TriageItem {
        path: row.get(0)?,
        size_bytes: row.get(1)?,
        modified_at: row.get(2)?,
        container: row.get(3)?,
        codec: row.get(4)?,
        bitrate_kbps: row.get(5)?,
        sample_rate_hz: row.get(6)?,
        bit_depth: row.get(7)?,
        duration_seconds: row.get(8)?,
        spectral_cutoff_hz: row.get(9)?,
        verdict: row.get::<_, String>(10)?.parse()?,
        reason: row.get(11)?,
        scanned_at: row.get(12)?,
    })
}

fn history_from_row(row: &Row<'_>) -> Result<HistoryRow, StoreError> {
    let verdict = row
        .get::<_, Option<String>>(3)?
        .map(|value| value.parse())
        .transpose()?;
    Ok(HistoryRow {
        input_path: row.get(0)?,
        input_sha256: row.get(1)?,
        output_path: row.get(2)?,
        verdict,
        settings: row.get(4)?,
        provenance: row.get(5)?,
        result: row.get::<_, String>(6)?.parse()?,
        recorded_at: row.get(7)?,
    })
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as i64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

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

    fn columns(store: &Store, table: &str) -> Vec<String> {
        let mut statement = store
            .connection
            .prepare(&format!("PRAGMA table_info({table})"))
            .unwrap();
        statement
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
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

    #[test]
    fn transition_table_accepts_legal_and_rejects_illegal() {
        use JobState::*;

        for restore_enabled in [false, true] {
            for from in ALL_STATES {
                for to in ALL_STATES {
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
                    assert_eq!(
                        from.can_transition_to(to, restore_enabled),
                        expected,
                        "unexpected rule for restore_enabled={restore_enabled}, {from:?} -> {to:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn illegal_transition_returns_a_typed_error_and_keeps_the_state() {
        let db = TempDb::new("illegal");
        let store = Store::open(db.path()).unwrap();
        let id = store.insert_job("/music/a.mp3", true).unwrap();

        let error = store.transition_job(id, JobState::Done).unwrap_err();
        match error {
            StoreError::IllegalTransition { from, to } => {
                assert_eq!(from, JobState::Queued);
                assert_eq!(to, JobState::Done);
            }
            other => panic!("expected an IllegalTransition, got {other:?}"),
        }
        assert_eq!(store.job(id).unwrap().state, JobState::Queued);

        store.transition_job(id, JobState::Restoring).unwrap();
        assert_eq!(store.job(id).unwrap().state, JobState::Restoring);
    }

    #[test]
    fn migration_builds_the_schema_from_an_empty_file() {
        let db = TempDb::new("migrate");
        let store = Store::open(db.path()).unwrap();

        assert_eq!(store.schema_version().unwrap(), 2);

        let triage = columns(&store, "triage_items");
        for column in [
            "path",
            "size_bytes",
            "modified_at",
            "container",
            "codec",
            "bitrate_kbps",
            "sample_rate_hz",
            "bit_depth",
            "duration_seconds",
            "spectral_cutoff_hz",
            "verdict",
            "reason",
            "scanned_at",
        ] {
            assert!(
                triage.iter().any(|name| name == column),
                "triage_items is missing {column}"
            );
        }

        let history = columns(&store, "history");
        for column in [
            "input_path",
            "input_sha256",
            "output_path",
            "verdict",
            "settings",
            "provenance",
            "result",
            "recorded_at",
        ] {
            assert!(
                history.iter().any(|name| name == column),
                "history is missing {column}"
            );
        }
    }

    #[test]
    fn migration_2_clears_triage_rows_and_keeps_the_other_tables() {
        let db = TempDb::new("migrate2");
        {
            let connection = Connection::open(db.path()).unwrap();
            connection.execute_batch(MIGRATION_1).unwrap();
            connection
                .execute_batch("PRAGMA user_version = 1;")
                .unwrap();
            connection
                .execute(
                    "INSERT INTO triage_items
                         (path, size_bytes, modified_at, verdict, scanned_at)
                     VALUES ('/music/a.mp3', 4200000, 1700000000, 'restore', 1700000000)",
                    [],
                )
                .unwrap();
            connection
                .execute(
                    "INSERT INTO settings (key, value) VALUES ('output_dir', '/music/out')",
                    [],
                )
                .unwrap();
            connection
                .execute(
                    "INSERT INTO jobs (input_path, restore_enabled, state, created_at)
                     VALUES ('/music/a.mp3', 1, 'queued', 1700000000)",
                    [],
                )
                .unwrap();
        }

        let store = Store::open(db.path()).unwrap();

        assert_eq!(store.schema_version().unwrap(), 2);
        assert!(
            store.triage_items().unwrap().is_empty(),
            "migration 2 clears every cached triage row"
        );
        assert_eq!(
            store.setting("output_dir").unwrap().as_deref(),
            Some("/music/out")
        );
        assert_eq!(store.job(1).unwrap().input_path, "/music/a.mp3");
    }

    #[test]
    fn a_restore_off_job_moves_from_queued_to_encoding() {
        let db = TempDb::new("restore-off");
        let store = Store::open(db.path()).unwrap();
        let id = store.insert_job("/music/a.mp3", false).unwrap();
        assert!(!store.job(id).unwrap().restore_enabled);

        store.transition_job(id, JobState::Encoding).unwrap();
        assert_eq!(store.job(id).unwrap().state, JobState::Encoding);
    }

    #[test]
    fn triage_items_round_trip_and_update_in_place() {
        let db = TempDb::new("triage");
        let store = Store::open(db.path()).unwrap();
        let item = TriageItem {
            path: "/music/a.mp3".to_string(),
            size_bytes: 4_200_000,
            modified_at: 1_700_000_000,
            container: Some("MP3".to_string()),
            codec: Some("mp3".to_string()),
            bitrate_kbps: Some(128),
            sample_rate_hz: Some(44_100),
            bit_depth: None,
            duration_seconds: Some(180.0),
            spectral_cutoff_hz: Some(16_000.0),
            verdict: Verdict::Restore,
            reason: None,
            scanned_at: 1_700_000_100,
        };
        store.upsert_triage_item(&item).unwrap();
        assert_eq!(
            store.triage_item("/music/a.mp3").unwrap(),
            Some(item.clone())
        );

        let resized = TriageItem {
            size_bytes: 9_900_000,
            modified_at: 1_700_000_200,
            verdict: Verdict::LowBenefit,
            ..item.clone()
        };
        store.upsert_triage_item(&resized).unwrap();
        assert_eq!(store.triage_item("/music/a.mp3").unwrap(), Some(resized));
        assert_eq!(store.triage_item("/music/missing.mp3").unwrap(), None);
    }
}

#[cfg(test)]
mod race_tests;
