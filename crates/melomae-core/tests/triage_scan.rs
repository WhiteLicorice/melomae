//! Task 16 scan behavior: the folder walk, the incremental rescan, the row removal, and the proof
//! that a scan never writes an input file.
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
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let mut path = std::env::temp_dir();
        path.push(format!(
            "melomae-triage-{tag}-{}-{nanos}.db",
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
    let frames = (f64::from(rate) * seconds).round() as usize;
    let sweep = log_sweep(rate, 50.0, 15_000.0, 0.4, frames);
    let tone = sine(rate, 440.0, 0.2, frames);
    let mixed = sweep
        .iter()
        .zip(&tone)
        .map(|(a, b)| a + b)
        .collect::<Vec<f32>>();
    write_wav_f32(&dir.join(name), rate, &[mixed]);
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
