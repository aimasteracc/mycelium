//! Append-only journal for incremental Store persistence (RFC-0098 §2).
//!
//! Instead of rewriting the entire Store on every file change, we append a
//! [`DeltaRecord`] describing what changed. On load, the base snapshot is
//! reconstructed and deltas replayed on top. Periodic compaction merges
//! the journal back into a fresh base snapshot.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use anyhow::Context as _;
use serde::{Deserialize, Serialize};

/// On-disk header for the journal file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalHeader {
    /// Format version — must be `1`.
    pub version: u32,
    /// Sequence number of the next delta to be appended.
    pub next_seq: u64,
}

impl Default for JournalHeader {
    fn default() -> Self {
        Self {
            version: 1,
            next_seq: 1,
        }
    }
}

/// A single incremental change to the Store.
///
/// Each record represents the effect of re-indexing one file: remove the
/// old subtree, then merge the new subtree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeltaRecord {
    /// Monotonically increasing sequence number.
    pub seq: u64,
    /// Relative file path that was re-indexed.
    pub file_path: String,
    /// `Base64`-encoded `MessagePack` of the per-file sub-[`super::Store`] to merge in.
    /// Empty if the file was deleted (remove-only).
    pub delta_store: String,
}

/// Manages the append-only journal alongside a base snapshot.
///
/// Directory layout:
/// ```text
/// .mycelium/
///   index.rmp       ← base snapshot (full Store)
///   journal.jsonl    ← append-only delta records (one JSON per line)
/// ```
pub struct Journal {
    dir: PathBuf,
    header: JournalHeader,
    writer: Option<std::fs::File>,
    /// Maximum journal entries before auto-compaction.
    pub compact_threshold: u64,
}

impl Journal {
    /// Open (or create) a journal rooted at the same directory as `snapshot_path`.
    ///
    /// # Errors
    /// Returns an error if the journal directory cannot be created or the file
    /// cannot be opened/created.
    pub fn open(snapshot_path: &Path) -> anyhow::Result<Self> {
        let dir = snapshot_path
            .parent()
            .context("snapshot path has no parent directory")?
            .to_path_buf();
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("creating journal dir {}", dir.display()))?;

        let journal_path = dir.join("journal.jsonl");
        let (header, writer) = if journal_path.exists() {
            let (hdr, next_seq) = Self::read_tail(&journal_path)?;
            let file = std::fs::OpenOptions::new()
                .append(true)
                .open(&journal_path)
                .with_context(|| {
                    format!("opening journal for append {}", journal_path.display())
                })?;
            let mut hdr = hdr;
            hdr.next_seq = next_seq;
            (hdr, file)
        } else {
            let file = std::fs::File::create(&journal_path)
                .with_context(|| format!("creating journal {}", journal_path.display()))?;
            let hdr = JournalHeader::default();
            let hdr_line = serde_json::to_string(&hdr)? + "\n";
            let mut f = &file;
            f.write_all(hdr_line.as_bytes())?;
            (hdr, file)
        };

        Ok(Self {
            dir,
            header,
            writer: Some(writer),
            compact_threshold: 500,
        })
    }

    /// Append a delta: file removed + new sub-store merged.
    ///
    /// # Errors
    /// Returns an error if the delta cannot be serialized or written to disk.
    pub fn append(&mut self, file_path: &str, sub_store: &super::Store) -> anyhow::Result<()> {
        let delta = super::Store::serialize_delta(sub_store);
        let record = DeltaRecord {
            seq: self.header.next_seq,
            file_path: file_path.to_string(),
            delta_store: delta,
        };
        let line = serde_json::to_string(&record)? + "\n";
        if let Some(ref mut w) = self.writer {
            w.write_all(line.as_bytes())
                .context("appending delta to journal")?;
            w.flush().context("flushing journal")?;
        }
        self.header.next_seq += 1;
        Ok(())
    }

    /// How many deltas are pending in the journal.
    #[must_use]
    pub const fn pending_count(&self) -> u64 {
        self.header.next_seq.saturating_sub(1)
    }

    /// Whether compaction should run.
    #[must_use]
    pub const fn should_compact(&self) -> bool {
        self.pending_count() >= self.compact_threshold
    }

    /// Replay all journal deltas onto `store`.
    ///
    /// # Errors
    /// Returns an error if the journal file cannot be read or a delta is corrupt.
    pub fn replay(&self, store: &mut super::Store) -> anyhow::Result<u64> {
        let journal_path = self.dir.join("journal.jsonl");
        if !journal_path.exists() {
            return Ok(0);
        }
        let file = std::fs::File::open(&journal_path)
            .with_context(|| format!("opening journal for replay {}", journal_path.display()))?;
        let reader = BufReader::new(file);
        let mut count: u64 = 0;
        for line in reader.lines() {
            let line = line.context("reading journal line")?;
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with("{\"version\"") {
                continue;
            }
            let record: DeltaRecord = serde_json::from_str(trimmed)
                .with_context(|| format!("parsing delta record: {trimmed}"))?;
            store.remove_file(&record.file_path);
            if !record.delta_store.is_empty() {
                let sub = super::Store::deserialize_delta(&record.delta_store)?;
                store.merge(&sub);
            }
            count += 1;
        }
        Ok(count)
    }

    /// Compact: merge journal into a fresh base snapshot, then truncate journal.
    ///
    /// # Errors
    /// Returns an error if the snapshot cannot be saved or the journal cannot be truncated.
    pub fn compact(&mut self, store: &super::Store) -> anyhow::Result<()> {
        let snapshot_path = self.dir.join("index.rmp");
        let tmp_path = self.dir.join("index.rmp.tmp");

        store
            .save(&tmp_path)
            .context("compacting: writing new base snapshot")?;
        std::fs::rename(&tmp_path, &snapshot_path)
            .with_context(|| "compacting: renaming tmp to snapshot".to_string())?;

        let journal_path = self.dir.join("journal.jsonl");
        if journal_path.exists() {
            std::fs::remove_file(&journal_path).context("compacting: removing old journal")?;
        }

        self.header = JournalHeader::default();
        let file = std::fs::File::create(&journal_path).with_context(|| {
            format!(
                "compacting: creating fresh journal {}",
                journal_path.display()
            )
        })?;
        let hdr_line = serde_json::to_string(&self.header)? + "\n";
        let mut f = &file;
        f.write_all(hdr_line.as_bytes())?;
        self.writer = Some(file);
        Ok(())
    }

    /// Remove journal file (used after full snapshot save bypasses journal).
    ///
    /// # Errors
    /// Returns an error if the journal file cannot be removed.
    pub fn truncate(&mut self) -> anyhow::Result<()> {
        let journal_path = self.dir.join("journal.jsonl");
        if journal_path.exists() {
            std::fs::remove_file(&journal_path)?;
        }
        self.header = JournalHeader::default();
        self.writer = None;
        Ok(())
    }

    fn read_tail(path: &Path) -> anyhow::Result<(JournalHeader, u64)> {
        let file = std::fs::File::open(path)?;
        let reader = BufReader::new(file);
        let mut header = JournalHeader::default();
        let mut next_seq: u64 = 1;
        for line in reader.lines() {
            let line = line?;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Ok(hdr) = serde_json::from_str::<JournalHeader>(trimmed) {
                header = hdr;
                continue;
            }
            if let Ok(rec) = serde_json::from_str::<DeltaRecord>(trimmed) {
                next_seq = rec.seq + 1;
            }
        }
        Ok((header, next_seq))
    }
}
