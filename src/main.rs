use std::os::unix::process::CommandExt as _;

use anyhow::Context as _;
use eframe::egui;
use spotcalc::app::{SpotCalc, WINDOW_SIZE};
use spotcalc::gpu::{self, EglChoice};
use spotcalc::history::History;
use spotcalc::instance::{self, Instance};
use spotcalc::paths::Paths;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    pick_egl_driver();

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

/// Restarts spotcalc with the EGL driver that fits the GPUs present now (see [`gpu`]). glvnd
/// reads the variable when OpenGL loads, and changing our own environment needs `unsafe`, so
/// this re-executes the binary instead (about 1 ms). Falls through if that fails.
fn pick_egl_driver() {
    let current = std::env::var_os(gpu::EGL_VENDOR_VAR);
    let choice = gpu::choose(
        current.as_deref(),
        gpu::nvidia_only(std::path::Path::new("/sys/class/drm")),
    );
    let mut cmd = match std::env::current_exe() {
        Ok(exe) => std::process::Command::new(exe),
        Err(e) => return tracing::warn!("can't find own binary to pick EGL driver: {e}"),
    };
    cmd.args(std::env::args_os().skip(1));
    match choice {
        EglChoice::Keep => return,
        EglChoice::NvidiaOnly => cmd.env(gpu::EGL_VENDOR_VAR, gpu::NVIDIA_EGL),
        EglChoice::Default => cmd.env_remove(gpu::EGL_VENDOR_VAR),
    };
    tracing::debug!("restarting with EGL choice {choice:?}");
    let e = cmd.exec(); // only returns on failure
    tracing::warn!("restart for EGL choice {choice:?} failed: {e}");
}
