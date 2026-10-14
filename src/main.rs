use anyhow::Context as _;
use eframe::egui;
use spotcalc::app::{SpotCalc, WINDOW_SIZE};
use spotcalc::history::History;
use spotcalc::instance::{self, Instance};
use spotcalc::paths::Paths;

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

    let paths = Paths::from_env()?;
    let history = History::load(&paths.history).unwrap_or_else(|e| {
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
                paths,
            )))
        }),
    )
    .map_err(|e| anyhow::anyhow!("{e}"))?;

    std::fs::remove_file(instance::socket_path()).ok();
    Ok(())
}
