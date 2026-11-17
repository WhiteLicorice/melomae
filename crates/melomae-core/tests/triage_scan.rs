//! Task 16 scan behavior: the folder walk, the incremental rescan, the row removal, and the proof
//! that a scan never writes an input file. Task 44 adds the unreadable file and the unopenable
//! folder: one bad path must not stop the scan.
//!
//! Every input file is built in test setup and removed in teardown. Nothing is committed.

mod common;
mod setup;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use melomae_core::store::{Store, Verdict};
use melomae_core::triage::{scan_roots, triage_file};

use common::{log_sweep, sine, write_wav_f32};
use setup::{Calibration, TempDir};

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
            "melomae-triage-{tag}-{}-{nanos}-{sequence}.db",
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
        let _ = fs::remove_file(&self.path);
        for suffix in ["-journal", "-wal", "-shm"] {
            let mut sidecar = self.path.clone().into_os_string();
            sidecar.push(suffix);
            let _ = fs::remove_file(PathBuf::from(sidecar));
        }
    }
}

/// Write a short synthesized WAV so a scan has a decodable file to read.
fn write_fixture(dir: &TempDir, name: &str, rate: u32, seconds: f64) {
    write_fixture_at(&dir.join(name), rate, seconds);
}

/// Write the synthesized WAV to one path, creating its folder.
fn write_fixture_at(path: &Path, rate: u32, seconds: f64) {
    let frames = (f64::from(rate) * seconds).round() as usize;
    let sweep = log_sweep(rate, 50.0, 15_000.0, 0.4, frames);
    let tone = sine(rate, 440.0, 0.2, frames);
    let mixed = sweep
        .iter()
        .zip(&tone)
        .map(|(a, b)| a + b)
        .collect::<Vec<f32>>();
    write_wav_f32(path, rate, &[mixed]);
}

/// Every file under `dir`, with its bytes and its modification time, keyed by path.
fn snapshot(dir: &Path) -> BTreeMap<PathBuf, (Vec<u8>, SystemTime)> {
    let mut files = BTreeMap::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(current) = pending.pop() {
        for entry in fs::read_dir(&current).expect("read the snapshot directory") {
            let path = entry.expect("read a snapshot entry").path();
            if path.is_dir() {
                pending.push(path);
            } else {
                let bytes = fs::read(&path).expect("read a snapshot file");
                let modified = fs::metadata(&path)
                    .expect("stat a snapshot file")
                    .modified()
                    .expect("read a modification time");
                files.insert(path, (bytes, modified));
            }
        }
    }
    files
}

#[test]
fn triage_scan_leaves_every_input_file_unchanged() {
    let dir = TempDir::new("melomae-triage-nowrite");
    write_fixture(&dir, "a.wav", 44_100, 1.0);
    write_fixture(&dir, "b.wav", 48_000, 0.5);
    write_fixture(&dir, "nested/c.wav", 44_100, 0.75);

    let before = snapshot(dir.path());

    let db = TempDb::new("nowrite");
    let store = Store::open(db.path()).unwrap();
    let report = scan_roots(&store, &[dir.path().to_path_buf()]).unwrap();
    assert_eq!(report.scanned, 3, "the walk must find all three files");

    let after = snapshot(dir.path());
    assert_eq!(
        before.keys().collect::<Vec<_>>(),
        after.keys().collect::<Vec<_>>(),
        "the scan must not add or remove files"
    );
    for (path, (bytes, modified)) in &before {
        let (after_bytes, after_modified) = after.get(path).expect("the file must still exist");
        assert_eq!(bytes, after_bytes, "{} changed its bytes", path.display());
        assert_eq!(
            modified,
            after_modified,
            "{} changed its modification time",
            path.display()
        );
    }
}

#[test]
fn triage_scan_reads_only_changed_files_on_a_rescan() {
    let dir = TempDir::new("melomae-triage-rescan");
    write_fixture(&dir, "a.wav", 44_100, 0.5);
    write_fixture(&dir, "b.wav", 44_100, 0.5);
    write_fixture(&dir, "c.wav", 44_100, 0.5);

    let db = TempDb::new("rescan");
    let store = Store::open(db.path()).unwrap();
    let roots = [dir.path().to_path_buf()];

    let first = scan_roots(&store, &roots).unwrap();
    assert_eq!(first.scanned, 3);
    assert_eq!(first.skipped, 0);

    // Change only b.wav: a new length, and a modification time one hour ahead.
    write_fixture(&dir, "b.wav", 44_100, 0.9);
    let ahead = SystemTime::now() + std::time::Duration::from_secs(3600);
    fs::File::options()
        .write(true)
        .open(dir.join("b.wav"))
        .unwrap()
        .set_modified(ahead)
        .unwrap();

    let second = scan_roots(&store, &roots).unwrap();
    assert_eq!(
        second.scanned, 1,
        "only the changed file is read on a rescan"
    );
    assert_eq!(second.skipped, 2, "the unchanged files are skipped");
    assert_eq!(second.removed, 0);
}

#[test]
fn triage_scan_removes_rows_for_deleted_files() {
    let dir = TempDir::new("melomae-triage-remove");
    write_fixture(&dir, "keep.wav", 44_100, 0.4);
    write_fixture(&dir, "gone.wav", 44_100, 0.4);

    let db = TempDb::new("remove");
    let store = Store::open(db.path()).unwrap();
    let roots = [dir.path().to_path_buf()];
    scan_roots(&store, &roots).unwrap();

    fs::remove_file(dir.join("gone.wav")).unwrap();
    let report = scan_roots(&store, &roots).unwrap();

    assert_eq!(report.removed, 1);
    assert_eq!(report.scanned, 0);
    assert_eq!(report.skipped, 1);
    assert!(store
        .triage_item(&dir.join("gone.wav").to_string_lossy())
        .unwrap()
        .is_none());
    assert!(store
        .triage_item(&dir.join("keep.wav").to_string_lossy())
        .unwrap()
        .is_some());
}

#[test]
fn e2e_triage_scan_a_folder_into_the_store() {
    let calibration = Calibration::build();
    let db = TempDb::new("e2e");
    let store = Store::open(db.path()).unwrap();

    let report = scan_roots(&store, &[calibration.dir().to_path_buf()]).unwrap();
    assert_eq!(
        report.scanned, 9,
        "the folder holds the source and the eight fixtures"
    );
    assert_eq!(report.skipped, 0);

    let mut rows = store.triage_items().unwrap();
    rows.sort_by(|a, b| a.path.cmp(&b.path));
    assert_eq!(rows.len(), 9);

    for row in &rows {
        assert!(
            row.spectral_cutoff_hz.is_some(),
            "{} must have a measured cutoff",
            row.path
        );
        assert!(row.duration_seconds.is_some());
        assert!(row.sample_rate_hz.is_some());
    }

    let mp3_320 = rows
        .iter()
        .find(|row| row.path.ends_with("cal_mp3_320.mp3"))
        .expect("the 320 kb/s MP3 row");
    assert_eq!(mp3_320.verdict, Verdict::LowBenefit);

    let flac_from_mp3 = rows
        .iter()
        .find(|row| row.path.ends_with("cal_flac_from_mp3_128.flac"))
        .expect("the FLAC-from-MP3 row");
    assert_eq!(flac_from_mp3.verdict, Verdict::SuspectLossless);

    let lossless = rows
        .iter()
        .find(|row| row.path.ends_with("cal_flac_lossless.flac"))
        .expect("the lossless FLAC row");
    assert_eq!(lossless.verdict, Verdict::Lossless);

    let source = rows
        .iter()
        .find(|row| row.path.ends_with("cal_source.wav"))
        .expect("the source WAV row");
    assert_eq!(source.verdict, Verdict::Lossless);

    // The stored row matches a direct triage of the same file. The scan time differs by design.
    let mut direct = triage_file(&calibration.path("cal_mp3_320.mp3")).unwrap();
    direct.scanned_at = mp3_320.scanned_at;
    assert_eq!(&direct, mp3_320);
}

#[test]
fn triage_scan_of_1000_files_stays_within_the_time_budget() {
    let dir = TempDir::new("melomae-triage-1000");
    for index in 0..1000 {
        write_fixture(&dir, &format!("f{index:04}.wav"), 44_100, 0.2);
    }

    let db = TempDb::new("1000");
    let store = Store::open(db.path()).unwrap();

    let started = Instant::now();
    let report = scan_roots(&store, &[dir.path().to_path_buf()]).unwrap();
    let elapsed = started.elapsed();

    eprintln!(
        "1000-file scan: {:?} for {} files ({:.1} files/s)",
        elapsed,
        report.scanned,
        report.scanned as f64 / elapsed.as_secs_f64()
    );

    assert_eq!(report.scanned, 1000);
    assert_eq!(store.triage_items().unwrap().len(), 1000);
    assert!(
        elapsed.as_secs() < 120,
        "the 1000-file scan took {elapsed:?}, over the 120 s budget"
    );
}

// --- Task 44: one unreadable file or folder must not stop the scan ---------------------------

/// A deny on one permission of a temporary path, removed when the guard drops.
///
/// The scan runs in the test process, so the deny names the account that runs the test. Windows
/// denies the right with `icacls`: `(RD)` stops a folder listing and `(R)` stops a file read. Unix
/// clears the mode bits. A root user ignores both, so the tests skip there.
mod deny {
    use std::path::{Path, PathBuf};

    pub struct DeniedPath {
        path: PathBuf,
        #[cfg(windows)]
        account: String,
        #[cfg(unix)]
        mode: u32,
    }

    #[cfg(windows)]
    impl DeniedPath {
        /// Stop the process from listing the folder.
        pub fn listing(dir: &Path) -> DeniedPath {
            DeniedPath::apply(dir, "RD")
        }

        /// Stop the process from reading the file's data.
        pub fn read(file: &Path) -> DeniedPath {
            DeniedPath::apply(file, "R")
        }

        fn apply(path: &Path, right: &str) -> DeniedPath {
            use std::process::Command;

            let account = account();
            let status = Command::new("icacls")
                .arg(path)
                .arg("/deny")
                .arg(format!("{account}:({right})"))
                .status()
                .expect("run icacls to deny a right");
            assert!(
                status.success(),
                "icacls must deny ({right}) on {}",
                path.display()
            );
            DeniedPath {
                path: path.to_path_buf(),
                account,
            }
        }
    }

    #[cfg(windows)]
    impl Drop for DeniedPath {
        fn drop(&mut self) {
            use std::process::Command;

            let _ = Command::new("icacls")
                .arg(&self.path)
                .arg("/remove:d")
                .arg(&self.account)
                .status();
        }
    }

    /// The account that runs the test, in the `domain\user` form that `icacls` accepts.
    #[cfg(windows)]
    fn account() -> String {
        use std::process::Command;

        let output = Command::new("whoami").output().expect("run whoami");
        assert!(output.status.success(), "whoami must report the account");
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    /// Whether a deny stops this user. Windows always denies.
    #[cfg(windows)]
    pub fn deny_blocks_this_user() -> bool {
        true
    }

    #[cfg(unix)]
    impl DeniedPath {
        /// Stop the process from listing the folder.
        pub fn listing(dir: &Path) -> DeniedPath {
            DeniedPath::apply(dir)
        }

        /// Stop the process from reading the file's data.
        pub fn read(file: &Path) -> DeniedPath {
            DeniedPath::apply(file)
        }

        fn apply(path: &Path) -> DeniedPath {
            use std::fs;
            use std::os::unix::fs::PermissionsExt;

            let mode = fs::metadata(path)
                .expect("stat the path to deny")
                .permissions()
                .mode();
            fs::set_permissions(path, fs::Permissions::from_mode(0o000))
                .expect("clear the mode bits");
            DeniedPath {
                path: path.to_path_buf(),
                mode,
            }
        }
    }

    #[cfg(unix)]
    impl Drop for DeniedPath {
        fn drop(&mut self) {
            use std::fs;
            use std::os::unix::fs::PermissionsExt;

            let _ = fs::set_permissions(&self.path, fs::Permissions::from_mode(self.mode));
        }
    }

    /// Whether a deny stops this user. A root user reads and lists anything.
    #[cfg(unix)]
    pub fn deny_blocks_this_user() -> bool {
        use std::process::Command;

        Command::new("id")
            .arg("-u")
            .output()
            .map(|output| String::from_utf8_lossy(&output.stdout).trim() != "0")
            .unwrap_or(false)
    }
}

#[test]
fn triage_scan_gives_a_corrupt_file_an_unreadable_row() {
    let dir = TempDir::new("melomae-triage-corrupt");
    write_fixture(&dir, "good.wav", 44_100, 0.4);
    fs::write(dir.join("broken.mp3"), b"this is not an MP3 stream").unwrap();

    let db = TempDb::new("corrupt");
    let store = Store::open(db.path()).unwrap();
    let report = scan_roots(&store, &[dir.path().to_path_buf()]).expect("the scan must not fail");

    assert_eq!(report.scanned, 2, "both files get a row");
    assert_eq!(
        report.unreadable, 1,
        "the corrupt file is the unreadable one"
    );

    let broken = store
        .triage_item(&dir.join("broken.mp3").to_string_lossy())
        .unwrap()
        .expect("the corrupt file has a row");
    assert_eq!(broken.verdict, Verdict::Unreadable);
    assert!(
        broken.reason.is_some_and(|reason| !reason.is_empty()),
        "the row carries the reason"
    );

    let good = store
        .triage_item(&dir.join("good.wav").to_string_lossy())
        .unwrap()
        .expect("the good file has a row");
    assert_ne!(good.verdict, Verdict::Unreadable);
}

#[test]
fn triage_scan_gives_an_unopenable_file_an_unreadable_row() {
    if !deny::deny_blocks_this_user() {
        eprintln!("skip: this user ignores a deny, so the file stays readable");
        return;
    }
    let dir = TempDir::new("melomae-triage-noread");
    write_fixture(&dir, "good.wav", 44_100, 0.4);
    write_fixture(&dir, "locked.wav", 44_100, 0.4);
    let _denied = deny::DeniedPath::read(&dir.join("locked.wav"));

    let db = TempDb::new("noread");
    let store = Store::open(db.path()).unwrap();
    let report = scan_roots(&store, &[dir.path().to_path_buf()]).expect("the scan must not fail");

    assert_eq!(report.scanned, 2, "both files get a row");
    assert_eq!(
        report.unreadable, 1,
        "the denied file is the unreadable one"
    );

    let locked = store
        .triage_item(&dir.join("locked.wav").to_string_lossy())
        .unwrap()
        .expect("the file that cannot be opened has a row");
    assert_eq!(locked.verdict, Verdict::Unreadable);
    assert!(locked.reason.is_some(), "the row carries the reason");
}

#[test]
fn triage_scan_records_an_unopenable_folder_and_continues() {
    if !deny::deny_blocks_this_user() {
        eprintln!("skip: this user ignores a deny, so the folder stays readable");
        return;
    }
    let dir = TempDir::new("melomae-triage-nolist");
    write_fixture(&dir, "ok.wav", 44_100, 0.4);
    write_fixture_at(&dir.path().join("locked").join("hidden.wav"), 44_100, 0.4);
    let _denied = deny::DeniedPath::listing(&dir.join("locked"));

    let db = TempDb::new("nolist");
    let store = Store::open(db.path()).unwrap();
    let report = scan_roots(&store, &[dir.path().to_path_buf()]).expect("the scan must not fail");

    assert_eq!(report.scanned, 1, "the readable file is still triaged");
    assert_eq!(
        report.skipped_paths.len(),
        1,
        "the folder is one recorded skip"
    );
    assert_eq!(report.skipped_paths[0].path, dir.join("locked"));
    assert!(
        !report.skipped_paths[0].reason.is_empty(),
        "the skip carries the reason"
    );
    let ok = store
        .triage_item(&dir.join("ok.wav").to_string_lossy())
        .unwrap()
        .expect("the readable file has a row");
    assert_ne!(ok.verdict, Verdict::Unreadable);
}

#[test]
fn triage_scan_keeps_the_rows_of_an_unopenable_folder() {
    if !deny::deny_blocks_this_user() {
        eprintln!("skip: this user ignores a deny, so the folder stays readable");
        return;
    }
    let dir = TempDir::new("melomae-triage-keep-rows");
    write_fixture(&dir, "ok.wav", 44_100, 0.4);
    write_fixture_at(&dir.path().join("locked").join("hidden.wav"), 44_100, 0.4);

    let db = TempDb::new("keep-rows");
    let store = Store::open(db.path()).unwrap();
    let roots = [dir.path().to_path_buf()];

    let first = scan_roots(&store, &roots).expect("the first scan must not fail");
    assert_eq!(
        first.scanned, 2,
        "both files are triaged while the folder opens"
    );

    let _denied = deny::DeniedPath::listing(&dir.join("locked"));
    let second = scan_roots(&store, &roots).expect("the second scan must not fail");

    assert_eq!(second.scanned, 0, "the readable file is unchanged");
    assert_eq!(second.skipped, 1);
    assert_eq!(
        second.removed, 0,
        "a folder that cannot be opened is not a deletion"
    );
    let hidden = store
        .triage_item(
            &dir.path()
                .join("locked")
                .join("hidden.wav")
                .to_string_lossy(),
        )
        .unwrap();
    assert!(
        hidden.is_some(),
        "the row of the file inside the unopened folder stays"
    );
}

#[test]
fn triage_scan_fails_when_no_root_opens() {
    let dir = TempDir::new("melomae-triage-no-root");
    let db = TempDb::new("no-root");
    let store = Store::open(db.path()).unwrap();

    let error = scan_roots(&store, &[dir.join("absent")])
        .expect_err("a scan with no openable root must fail");
    assert!(
        error.to_string().contains("absent"),
        "the error names the root: {error}"
    );
}

#[test]
fn triage_scan_records_a_missing_root_beside_a_readable_one() {
    let dir = TempDir::new("melomae-triage-one-root");
    write_fixture(&dir, "ok.wav", 44_100, 0.4);

    let db = TempDb::new("one-root");
    let store = Store::open(db.path()).unwrap();
    let missing = dir.join("absent");

    let report = scan_roots(&store, &[dir.path().to_path_buf(), missing.clone()])
        .expect("one openable root is enough");

    assert_eq!(report.scanned, 1, "the readable root is still triaged");
    assert_eq!(report.skipped_paths.len(), 1);
    assert_eq!(report.skipped_paths[0].path, missing);
}
