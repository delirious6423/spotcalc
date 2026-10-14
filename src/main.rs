use std::path::PathBuf;

use anyhow::Context as _;
use eframe::egui;
use spotcalc::app::{SpotCalc, WINDOW_SIZE};
use spotcalc::history::History;
use spotcalc::instance::{self, Instance};

/// `$XDG_DATA_HOME`, or `~/.local/share` per the XDG spec when unset or not absolute.
fn data_dir() -> anyhow::Result<PathBuf> {
    match std::env::var_os("XDG_DATA_HOME").map(PathBuf::from) {
        Some(dir) if dir.is_absolute() => Ok(dir),
        _ => {
            let home = std::env::var_os("HOME").context("HOME is not set")?;
            Ok(PathBuf::from(home).join(".local/share"))
        }
    }
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

    let history_path = data_dir()?.join("spotcalc").join("history.tsv");
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
        Box::new(move |cc| Ok(Box::new(SpotCalc::new(&cc.egui_ctx, history, listener)))),
    )
    .map_err(|e| anyhow::anyhow!("{e}"))?;

    std::fs::remove_file(instance::socket_path()).ok();
    Ok(())
}
