//! Calculation history, saved as one tab-separated `input\tresult` line per entry.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context as _;

/// Oldest entries are dropped beyond this many.
const MAX_ENTRIES: usize = 200;

/// One committed calculation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub input: String,
    pub result: String,
}

/// History list (oldest first), optionally backed by a file.
#[derive(Debug, Default)]
pub struct History {
    entries: Vec<Entry>,
    path: Option<PathBuf>,
}

impl History {
    /// Loads history from `path`; a missing file gives an empty history.
    ///
    /// # Errors
    /// Fails if the file exists but cannot be read.
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let entries = match fs::read_to_string(path) {
            Ok(text) => parse(&text),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(e) => return Err(e).with_context(|| format!("reading {}", path.display())),
        };
        Ok(Self {
            entries,
            path: Some(path.to_owned()),
        })
    }

    /// Entries, oldest first.
    #[must_use]
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// Appends an entry (skipping an exact repeat of the last one) and saves.
    ///
    /// # Errors
    /// Fails if the history file cannot be written.
    pub fn push(&mut self, input: &str, result: &str) -> anyhow::Result<()> {
        let entry = Entry {
            input: one_line(input),
            result: one_line(result),
        };
        if self.entries.last() != Some(&entry) {
            self.entries.push(entry);
        }
        let excess = self.entries.len().saturating_sub(MAX_ENTRIES);
        self.entries.drain(..excess);
        self.save()
    }

    fn save(&self) -> anyhow::Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        }
        let text: String = self
            .entries
            .iter()
            .map(|e| format!("{}\t{}\n", e.input, e.result))
            .collect();
        fs::write(path, text).with_context(|| format!("writing {}", path.display()))
    }
}

/// Tabs and newlines would break the file format; fend output rarely has them anyway.
fn one_line(s: &str) -> String {
    s.replace(['\t', '\n', '\r'], " ")
}

fn parse(text: &str) -> Vec<Entry> {
    text.lines()
        .filter_map(|line| line.split_once('\t'))
        .map(|(input, result)| Entry {
            input: input.to_owned(),
            result: result.to_owned(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_file() {
        let dir = std::env::temp_dir().join(format!("spotcalc-test-{}", std::process::id()));
        let path = dir.join("history.tsv");
        let mut history = History::load(&path).expect("missing file is fine");
        history.push("1+1", "2").expect("save");
        history.push("1+1", "2").expect("save");
        history.push("a\tb", "x\ny").expect("save");

        let loaded = History::load(&path).expect("load");
        assert_eq!(
            loaded.entries().len(),
            2,
            "duplicate of last entry is skipped"
        );
        assert_eq!(loaded.entries()[1].input, "a b");
        assert_eq!(loaded.entries()[1].result, "x y");
        fs::remove_dir_all(dir).expect("cleanup");
    }

    #[test]
    fn caps_length() {
        let mut history = History::default();
        for i in 0..MAX_ENTRIES + 10 {
            history.push(&i.to_string(), "r").expect("in-memory push");
        }
        assert_eq!(history.entries().len(), MAX_ENTRIES);
        assert_eq!(history.entries()[0].input, "10");
    }
}
