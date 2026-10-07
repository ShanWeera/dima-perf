//! The recent-files list: the ten most recently opened alignments and results files, newest
//! first, without duplicates. Pure data; saving is the store's job, and happens only when the
//! list changes (G57).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Entries kept.
pub const MAX_RECENT: usize = 10;

/// What a recent file is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecentKind {
    /// An alignment.
    Alignment,
    /// A results file.
    Results,
}

/// One entry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecentFile {
    /// Where it is.
    pub path: PathBuf,
    /// When it was last opened (Unix seconds).
    pub opened: u64,
    /// What it is.
    pub kind: RecentKind,
    /// Number of sequences, when known.
    pub sequences: Option<u64>,
    /// `true` when `sequences` is an estimate (large files, D85).
    pub estimated: bool,
    /// Alphabet name, when known.
    pub alphabet: Option<String>,
}

/// The list.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecentFiles {
    /// Newest first.
    pub files: Vec<RecentFile>,
}

impl RecentFiles {
    /// Adds (or moves to the front) an entry; returns `true` when the list changed.
    pub fn add(&mut self, entry: RecentFile) -> bool {
        if self.files.first() == Some(&entry) {
            return false;
        }
        self.files.retain(|f| f.path != entry.path);
        self.files.insert(0, entry);
        self.files.truncate(MAX_RECENT);
        true
    }

    /// Removes an entry; returns `true` when it was there.
    pub fn remove(&mut self, path: &Path) -> bool {
        let before = self.files.len();
        self.files.retain(|f| f.path != path);
        self.files.len() != before
    }

    /// Removes every entry; returns `true` when there were any.
    pub fn clear(&mut self) -> bool {
        let changed = !self.files.is_empty();
        self.files.clear();
        changed
    }

    /// Stored form.
    #[must_use]
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// Reads the stored form; an unreadable file is an empty list.
    #[must_use]
    pub fn from_json(text: &str) -> Self {
        let mut list: Self = serde_json::from_str(text).unwrap_or_default();
        list.files.truncate(MAX_RECENT);
        list
    }
}

/// Seconds since the Unix epoch, now.
#[must_use]
pub fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(p: &str, opened: u64) -> RecentFile {
        RecentFile {
            path: PathBuf::from(p),
            opened,
            kind: RecentKind::Alignment,
            sequences: Some(10),
            estimated: false,
            alphabet: None,
        }
    }

    #[test]
    fn newest_first_without_duplicates_and_bounded() {
        let mut r = RecentFiles::default();
        assert!(r.add(entry("/a", 1)));
        assert!(r.add(entry("/b", 2)));
        assert!(r.add(entry("/a", 3)));
        assert_eq!(r.files.len(), 2);
        assert_eq!(r.files[0].path, PathBuf::from("/a"));
        assert!(
            !r.add(entry("/a", 3)),
            "an identical re-add is not a change"
        );
        for i in 0..20 {
            r.add(entry(&format!("/f{i}"), 10 + i));
        }
        assert_eq!(r.files.len(), MAX_RECENT);
        assert_eq!(r.files[0].path, PathBuf::from("/f19"));
    }

    #[test]
    fn remove_clear_and_storage() {
        let mut r = RecentFiles::default();
        r.add(entry("/a", 1));
        r.add(entry("/b", 2));
        assert!(r.remove(Path::new("/a")));
        assert!(!r.remove(Path::new("/zzz")));
        let back = RecentFiles::from_json(&r.to_json());
        assert_eq!(back, r);
        assert!(r.clear());
        assert!(!r.clear());
        assert_eq!(RecentFiles::from_json("garbage"), RecentFiles::default());
    }
}
