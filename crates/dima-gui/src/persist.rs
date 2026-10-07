//! Where settings and recent files live, and how they are written.
//!
//! Files go to the platform configuration directory (`~/.config/dima-gui` on Linux,
//! `~/Library/Application Support/dima-gui` on macOS, `%APPDATA%\dima-gui` on Windows). Writes
//! are serialised on one background thread and atomic (the library's [`Sink`]: temporary file,
//! sync, rename), so two saves can never interleave or leave a truncated file (G28), and only
//! the latest content of each file is written (G57). Tests use an in-memory or temporary store
//! and never touch the user's files (G15).

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use dima_core::output::{OutputCompression, Sink, Target};

/// File names inside the configuration directory.
pub const SETTINGS_FILE: &str = "settings.json";
/// Recent files.
pub const RECENT_FILE: &str = "recent.json";

enum Request {
    Write(&'static str, String),
    Flush(Sender<()>),
}

/// The configuration store.
pub struct ConfigStore {
    dir: Option<PathBuf>,
    /// In-memory contents (always up to date; the only storage when `dir` is `None`).
    memory: Mutex<BTreeMap<&'static str, String>>,
    writer: Mutex<Option<Sender<Request>>>,
}

impl ConfigStore {
    /// The user's configuration directory (the app's normal store).
    #[must_use]
    pub fn system() -> Self {
        Self::new(dirs::config_dir().map(|d| d.join("dima-gui")))
    }

    /// A store that keeps everything in memory (tests).
    #[cfg(test)]
    #[must_use]
    pub fn in_memory() -> Self {
        Self::new(None)
    }

    /// A store in `dir` (tests).
    #[cfg(test)]
    #[must_use]
    pub fn at(dir: PathBuf) -> Self {
        Self::new(Some(dir))
    }

    fn new(dir: Option<PathBuf>) -> Self {
        Self {
            dir,
            memory: Mutex::new(BTreeMap::new()),
            writer: Mutex::new(None),
        }
    }

    /// The stored text of `name`, if any.
    #[must_use]
    pub fn read(&self, name: &'static str) -> Option<String> {
        if let Some(text) = self.lock_memory().get(name) {
            return Some(text.clone());
        }
        let dir = self.dir.as_ref()?;
        std::fs::read_to_string(dir.join(name)).ok()
    }

    /// Stores `text` as `name`; written to disk in the background.
    pub fn write(&self, name: &'static str, text: String) {
        self.lock_memory().insert(name, text.clone());
        let Some(dir) = self.dir.clone() else {
            return;
        };
        let mut writer = self
            .writer
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if writer.is_none() {
            let (tx, rx) = mpsc::channel();
            let background = dir.clone();
            let spawned = std::thread::Builder::new()
                .name("dima-gui-settings".to_owned())
                .spawn(move || writer_loop(&background, &rx));
            if spawned.is_err() {
                // The system refused a thread: write this one now (a small file), and try the
                // background writer again next time instead of losing every later write.
                if std::fs::create_dir_all(&dir).is_ok() {
                    let _ = write_atomically(&dir.join(name), &text);
                }
                return;
            }
            *writer = Some(tx);
        }
        if let Some(tx) = writer.as_ref() {
            let _ = tx.send(Request::Write(name, text));
        }
    }

    /// Waits (at most two seconds) until every pending write is on disk.
    pub fn flush(&self) {
        let writer = self
            .writer
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(tx) = writer.as_ref() {
            let (done_tx, done_rx) = mpsc::channel();
            if tx.send(Request::Flush(done_tx)).is_ok() {
                let _ = done_rx.recv_timeout(Duration::from_secs(2));
            }
        }
    }

    fn lock_memory(&self) -> std::sync::MutexGuard<'_, BTreeMap<&'static str, String>> {
        self.memory
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

fn writer_loop(dir: &std::path::Path, rx: &Receiver<Request>) {
    while let Ok(first) = rx.recv() {
        // Coalesce: only the latest content of each file is written.
        let mut pending: BTreeMap<&'static str, String> = BTreeMap::new();
        let mut flushes = Vec::new();
        let mut next = Some(first);
        while let Some(req) = next {
            match req {
                Request::Write(name, text) => {
                    pending.insert(name, text);
                }
                Request::Flush(done) => flushes.push(done),
            }
            next = rx.try_recv().ok();
        }
        if !pending.is_empty() && std::fs::create_dir_all(dir).is_ok() {
            for (name, text) in pending {
                // A configuration file is a convenience: a failed write is not worth
                // interrupting the user for.
                let _ = write_atomically(&dir.join(name), &text);
            }
        }
        for done in flushes {
            let _ = done.send(());
        }
    }
}

fn write_atomically(path: &std::path::Path, text: &str) -> dima_core::Result<()> {
    let mut sink = Sink::create(&Target::Path(path.to_path_buf()), OutputCompression::None)?;
    sink.write_all(text.as_bytes()).map_err(|e| sink.error(e))?;
    sink.commit().map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_store_never_touches_disk() {
        let s = ConfigStore::in_memory();
        assert_eq!(s.read(SETTINGS_FILE), None);
        s.write(SETTINGS_FILE, "{}".into());
        assert_eq!(s.read(SETTINGS_FILE).as_deref(), Some("{}"));
        s.flush();
    }

    #[test]
    fn writes_are_atomic_serialised_and_coalesced() {
        let dir = tempfile::tempdir().unwrap();
        let s = ConfigStore::at(dir.path().join("cfg"));
        for i in 0..200 {
            s.write(RECENT_FILE, format!("{{\"n\": {i}}}"));
        }
        s.flush();
        let text = std::fs::read_to_string(dir.path().join("cfg").join(RECENT_FILE)).unwrap();
        assert_eq!(text, "{\"n\": 199}", "the last write wins");
        let leftovers: Vec<_> = std::fs::read_dir(dir.path().join("cfg"))
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "no temporary files remain");
        // A fresh store reads what was written.
        let again = ConfigStore::at(dir.path().join("cfg"));
        assert_eq!(again.read(RECENT_FILE).as_deref(), Some("{\"n\": 199}"));
    }
}
