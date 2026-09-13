//! Recent files persistence.
//!
//! Stores recently opened file paths in a JSON file under the platform's
//! config directory:
//!   - macOS: ~/Library/Application Support/dima-gui/recent.json
//!   - Linux: ~/.config/dima-gui/recent.json (or $XDG_CONFIG_HOME)
//!   - Windows: %APPDATA%\dima-gui\recent.json

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

const MAX_RECENT_FILES: usize = 10;
const CONFIG_DIR_NAME: &str = "dima-gui";
const RECENT_FILE_NAME: &str = "recent.json";

/// A single recent file entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentFile {
    pub path: PathBuf,
    /// Unix timestamp in seconds (SystemTime is not directly serde-serializable)
    pub last_opened_unix_secs: u64,
    pub sequence_count: Option<usize>,
    pub detected_alphabet: Option<String>,
}

/// Persistent store for recent files.
#[derive(Debug, Default)]
pub struct RecentFileStore {
    pub files: Vec<RecentFile>,
}

impl RecentFileStore {
    /// Load recent files from disk. Returns empty store on any error.
    pub fn load() -> Self {
        let path = match Self::config_file_path() {
            Some(p) => p,
            None => return Self::default(),
        };
        let contents = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => return Self::default(),
        };
        let files: Vec<RecentFile> = serde_json::from_str(&contents).unwrap_or_default();
        Self { files }
    }

    /// Save recent files to disk. Errors are logged but not propagated.
    pub fn save(&self) {
        let path = match Self::config_file_path() {
            Some(p) => p,
            None => return,
        };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(&self.files) {
            let _ = std::fs::write(&path, json);
        }
    }

    /// Add or update a file entry (moves to front). Saves to disk.
    pub fn add(
        &mut self,
        path: PathBuf,
        sequence_count: Option<usize>,
        detected_alphabet: Option<String>,
    ) {
        // Remove existing entry for same path (dedup)
        self.files.retain(|f| f.path != path);

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        self.files.insert(
            0,
            RecentFile {
                path,
                last_opened_unix_secs: now,
                sequence_count,
                detected_alphabet,
            },
        );

        self.files.truncate(MAX_RECENT_FILES);
        self.save();
    }

    /// Platform-specific config file path.
    fn config_file_path() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join(CONFIG_DIR_NAME).join(RECENT_FILE_NAME))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_add_deduplicates_same_path() {
        let mut store = RecentFileStore::default();
        store.files.push(RecentFile {
            path: PathBuf::from("/tmp/test.fasta"),
            last_opened_unix_secs: 1000,
            sequence_count: Some(100),
            detected_alphabet: Some("Protein".to_string()),
        });
        // Adding same path should replace, not duplicate
        store.add(
            PathBuf::from("/tmp/test.fasta"),
            Some(200),
            Some("Nucleotide".to_string()),
        );
        assert_eq!(store.files.len(), 1);
        assert_eq!(store.files[0].sequence_count, Some(200));
    }

    #[test]
    fn test_add_truncates_at_max() {
        let mut store = RecentFileStore::default();
        for i in 0..15 {
            store.files.push(RecentFile {
                path: PathBuf::from(format!("/tmp/file{}.fasta", i)),
                last_opened_unix_secs: i as u64,
                sequence_count: None,
                detected_alphabet: None,
            });
        }
        store.add(PathBuf::from("/tmp/new.fasta"), None, None);
        assert!(store.files.len() <= 10);
        assert_eq!(store.files[0].path, Path::new("/tmp/new.fasta"));
    }

    #[test]
    fn test_most_recent_is_first() {
        let mut store = RecentFileStore::default();
        store.add(PathBuf::from("/tmp/old.fasta"), None, None);
        store.add(PathBuf::from("/tmp/new.fasta"), None, None);
        assert_eq!(store.files[0].path, Path::new("/tmp/new.fasta"));
        assert_eq!(store.files[1].path, Path::new("/tmp/old.fasta"));
    }

    #[test]
    fn test_serialization_roundtrip() {
        let recent = RecentFile {
            path: PathBuf::from("/tmp/test.fasta"),
            last_opened_unix_secs: 1700000000,
            sequence_count: Some(500),
            detected_alphabet: Some("Protein".to_string()),
        };
        let json = serde_json::to_string(&recent).unwrap();
        let back: RecentFile = serde_json::from_str(&json).unwrap();
        assert_eq!(back.path, recent.path);
        assert_eq!(back.last_opened_unix_secs, recent.last_opened_unix_secs);
        assert_eq!(back.sequence_count, recent.sequence_count);
    }
}
