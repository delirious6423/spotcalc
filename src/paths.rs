//! Where spotcalc keeps its files, following the XDG base-directory spec.

use std::path::PathBuf;

use anyhow::Context as _;

/// spotcalc's file locations.
#[derive(Debug, Clone)]
pub struct Paths {
    /// Calculation history (`~/.local/share/spotcalc/history.tsv`).
    pub history: PathBuf,
    /// Downloaded exchange rates (`~/.cache/spotcalc/rates.json`).
    pub rates_cache: PathBuf,
    /// Exists once the user answered "Not now" to the hotkey setup banner.
    pub hotkey_prompt_dismissed: PathBuf,
}

impl Paths {
    /// Resolves the paths from `$XDG_*_HOME`, falling back to the spec's defaults under `$HOME`.
    ///
    /// # Errors
    /// Fails if an XDG variable is unusable and `HOME` is not set.
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            history: xdg_dir("XDG_DATA_HOME", ".local/share")?.join("history.tsv"),
            rates_cache: xdg_dir("XDG_CACHE_HOME", ".cache")?.join("rates.json"),
            hotkey_prompt_dismissed: xdg_dir("XDG_CONFIG_HOME", ".config")?
                .join("hotkey-prompt-dismissed"),
        })
    }
}

/// `$<var>/spotcalc`, or `~/<fallback>/spotcalc` when the variable is unset or not absolute.
fn xdg_dir(var: &str, fallback: &str) -> anyhow::Result<PathBuf> {
    let base = match std::env::var_os(var).map(PathBuf::from) {
        Some(dir) if dir.is_absolute() => dir,
        _ => {
            let home = std::env::var_os("HOME").context("HOME is not set")?;
            PathBuf::from(home).join(fallback)
        }
    };
    Ok(base.join("spotcalc"))
}
