use std::path::PathBuf;

use anyhow::Context as _;
use eframe::egui;
use spotcalc::app::{SpotCalc, WINDOW_SIZE};
use spotcalc::history::History;
use spotcalc::instance::{self, Instance};

/// `$<var>/spotcalc`, or `~/<fallback>/spotcalc` when the XDG variable is unset or not
/// absolute, as the XDG base-directory spec says.
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

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let listener =
        match instance::acquire(&instance::socket_path()).context("single-instance socket")? {
            // The hotkey toggles: a second press closes the open window.
            Instance::AlreadyRunning => return Ok(()),
            Instance::Primary(listener) => listener,
        };

    let history_path = xdg_dir("XDG_DATA_HOME", ".local/share")?.join("history.tsv");
    let rates_cache = xdg_dir("XDG_CACHE_HOME", ".cache")?.join("rates.json");
    let history = History::load(&history_path).unwrap_or_else(|e| {
        tracing::warn!("starting with empty history: {e:#}");
        History::default()
    });

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_app_id("spotcalc")
            .with_title("spotcalc")
            .with_inner_size(WINDOW_SIZE)
            .with_decorations(false)
            .with_resizable(false)
            .with_always_on_top(),
        // Window size/position is fixed; nothing to persist.
        persist_window: false,
        ..Default::default()
    };
    eframe::run_native(
        "spotcalc",
        options,
        Box::new(move |cc| {
            Ok(Box::new(SpotCalc::new(
                &cc.egui_ctx,
                history,
                listener,
                rates_cache,
            )))
        }),
    )
    .map_err(|e| anyhow::anyhow!("{e}"))?;

    std::fs::remove_file(instance::socket_path()).ok();
    Ok(())
}
