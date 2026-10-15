//! Picks the EGL driver at launch, from the GPUs present right now.
//!
//! On a machine whose only active GPU is NVIDIA, loading only NVIDIA's EGL driver skips Mesa's,
//! which drags in ~45 MB of LLVM (measured 120 -> 69 MB RSS). Forcing it while another GPU
//! drives the display makes EGL fail ("provided display handle is not supported"), and the GPU
//! setup can change under us: a BIOS update or a hybrid/discrete switch. So the choice is made
//! on every launch, and a forced NVIDIA driver from an older shortcut is dropped when it no
//! longer fits.

use std::ffi::OsStr;
use std::path::Path;

/// The environment variable glvnd reads to pick EGL drivers.
pub const EGL_VENDOR_VAR: &str = "__EGL_VENDOR_LIBRARY_FILENAMES";
/// NVIDIA's glvnd EGL vendor file.
pub const NVIDIA_EGL: &str = "/usr/share/glvnd/egl_vendor.d/10_nvidia.json";
const NVIDIA_PCI_VENDOR: &str = "0x10de";

/// What to change in [`EGL_VENDOR_VAR`] before OpenGL starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EglChoice {
    /// Leave the environment alone.
    Keep,
    /// Load only NVIDIA's driver.
    NvidiaOnly,
    /// Drop a forced NVIDIA driver that this machine can't use now.
    Default,
}

/// Decides from the variable's current value and the GPUs present. Never asks for a change the
/// next launch would undo, so re-running with the result applied always gives [`EglChoice::Keep`].
#[must_use]
pub fn choose(current: Option<&OsStr>, nvidia_only: bool) -> EglChoice {
    match current {
        None if nvidia_only => EglChoice::NvidiaOnly,
        Some(v) if v == OsStr::new(NVIDIA_EGL) && !nvidia_only => EglChoice::Default,
        // Anything else was set by the user on purpose.
        _ => EglChoice::Keep,
    }
}

/// True when NVIDIA's EGL driver is installed and every GPU under `drm` (normally
/// `/sys/class/drm`) is NVIDIA.
#[must_use]
pub fn nvidia_only(drm: &Path) -> bool {
    if !Path::new(NVIDIA_EGL).exists() {
        return false;
    }
    gpu_vendors_are_nvidia(drm)
}

fn gpu_vendors_are_nvidia(drm: &Path) -> bool {
    let vendors: Vec<String> = std::fs::read_dir(drm)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|card| std::fs::read_to_string(card.path().join("device/vendor")).ok())
        .map(|v| v.trim().to_owned())
        .collect();
    !vendors.is_empty() && vendors.iter().all(|v| v == NVIDIA_PCI_VENDOR)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forces_nvidia_only_on_nvidia_only_machines() {
        assert_eq!(choose(None, true), EglChoice::NvidiaOnly);
        assert_eq!(choose(None, false), EglChoice::Keep);
    }

    /// The bug: a shortcut made in discrete-GPU mode forced NVIDIA's driver, and after a BIOS
    /// update switched to hybrid graphics spotcalc failed to start.
    #[test]
    fn drops_stale_nvidia_force_when_another_gpu_is_present() {
        let nvidia = OsStr::new(NVIDIA_EGL);
        assert_eq!(choose(Some(nvidia), false), EglChoice::Default);
        assert_eq!(choose(Some(nvidia), true), EglChoice::Keep);
    }

    #[test]
    fn respects_other_user_settings() {
        let mesa = OsStr::new("/usr/share/glvnd/egl_vendor.d/50_mesa.json");
        assert_eq!(choose(Some(mesa), true), EglChoice::Keep);
        assert_eq!(choose(Some(mesa), false), EglChoice::Keep);
    }

    #[test]
    fn applying_a_choice_is_stable() {
        for nvidia_only in [true, false] {
            let after = match choose(None, nvidia_only) {
                EglChoice::NvidiaOnly => Some(OsStr::new(NVIDIA_EGL)),
                _ => None,
            };
            assert_eq!(choose(after, nvidia_only), EglChoice::Keep);
        }
    }

    fn fake_drm(vendors: &[&str]) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "spotcalc-drm-{}-{}",
            std::process::id(),
            vendors.join("-")
        ));
        std::fs::remove_dir_all(&dir).ok();
        for (i, v) in vendors.iter().enumerate() {
            let dev = dir.join(format!("card{i}/device"));
            std::fs::create_dir_all(&dev).expect("create fake card");
            std::fs::write(dev.join("vendor"), format!("{v}\n")).expect("write vendor");
        }
        // Connector entries like card1-eDP-1 have no device/vendor and are skipped.
        std::fs::create_dir_all(dir.join("card0-eDP-1")).expect("create connector");
        dir
    }

    #[test]
    fn reads_gpu_vendors() {
        for (vendors, expected) in [
            (&["0x10de"][..], true),
            (&["0x8086", "0x10de"][..], false), // hybrid: Intel + NVIDIA
            (&["0x8086"][..], false),
            (&[][..], false),
        ] {
            let dir = fake_drm(vendors);
            assert_eq!(gpu_vendors_are_nvidia(&dir), expected, "{vendors:?}");
            std::fs::remove_dir_all(&dir).expect("cleanup");
        }
    }
}
