//! GNOME hotkey setup from inside the app, for installs (like the .deb) that can't run a
//! per-user setup script. Mirrors `scripts/install.sh`: a custom shortcut under
//! `org.gnome.settings-daemon.plugins.media-keys` that runs spotcalc.
//!
//! Talks to the `gsettings` CLI rather than linking GLib: it's always present on GNOME, and a
//! few process spawns on a background thread cost nothing noticeable.

use std::path::Path;
use std::process::Command;

use anyhow::{Context as _, bail, ensure};

/// The hotkey offered on first launch, in GTK accelerator syntax.
pub const DEFAULT_KEY: &str = "<Alt>space";

const MEDIA_KEYS: &str = "org.gnome.settings-daemon.plugins.media-keys";
const ENTRY: &str = "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/spotcalc/";
const WM_KEYS: &str = "org.gnome.desktop.wm.keybindings";
/// GNOME binds Alt+Space to this; a custom shortcut on the same key would never fire.
const WINDOW_MENU: &str = "activate-window-menu";
const NVIDIA_EGL: &str = "/usr/share/glvnd/egl_vendor.d/10_nvidia.json";

/// Whether spotcalc can be opened with a hotkey.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    /// Not GNOME (no media-keys schema), so the user must bind a key in their desktop's settings.
    Unsupported,
    /// A GNOME custom shortcut runs spotcalc, on this key.
    Bound(String),
    /// GNOME, but no shortcut runs spotcalc yet.
    Unbound {
        /// GNOME's window menu currently uses [`DEFAULT_KEY`]; setup will take it over.
        replaces_window_menu: bool,
    },
}

/// Checks the user's GNOME shortcuts for one that runs spotcalc.
#[must_use]
pub fn status() -> Status {
    let Ok(list) = gsettings(&["get", MEDIA_KEYS, "custom-keybindings"]) else {
        return Status::Unsupported;
    };
    for entry in parse_str_array(&list) {
        let schema = custom_schema(&entry);
        let runs_spotcalc = gsettings(&["get", &schema, "command"])
            .is_ok_and(|command| command.contains("spotcalc"));
        if runs_spotcalc
            && let Ok(binding) = gsettings(&["get", &schema, "binding"])
            && let Some(key) = parse_str(&binding).filter(|key| !key.is_empty())
        {
            return Status::Bound(key);
        }
    }
    let replaces_window_menu = gsettings(&["get", WM_KEYS, WINDOW_MENU])
        .is_ok_and(|keys| parse_str_array(&keys).iter().any(|k| k == DEFAULT_KEY));
    Status::Unbound {
        replaces_window_menu,
    }
}

/// Binds `key` to run `exe`, freeing it from GNOME's window menu if needed.
///
/// # Errors
/// Fails if gsettings fails or another custom shortcut already uses `key`.
pub fn bind(key: &str, exe: &Path) -> anyhow::Result<()> {
    let mut entries = parse_str_array(&gsettings(&["get", MEDIA_KEYS, "custom-keybindings"])?);
    for entry in entries.iter().filter(|e| *e != ENTRY) {
        let schema = custom_schema(entry);
        if gsettings(&["get", &schema, "binding"])
            .ok()
            .and_then(|b| parse_str(&b))
            .is_some_and(|b| b == key)
        {
            let name = gsettings(&["get", &schema, "name"])
                .ok()
                .and_then(|n| parse_str(&n))
                .unwrap_or_default();
            bail!("{key} is already used by the \u{201c}{name}\u{201d} shortcut");
        }
    }

    let window_menu = parse_str_array(&gsettings(&["get", WM_KEYS, WINDOW_MENU])?);
    if window_menu.iter().any(|k| k == key) {
        let kept: Vec<String> = window_menu.into_iter().filter(|k| k != key).collect();
        gsettings(&["set", WM_KEYS, WINDOW_MENU, &format_str_array(&kept)])?;
    }

    if !entries.iter().any(|e| e == ENTRY) {
        entries.push(ENTRY.to_owned());
        gsettings(&[
            "set",
            MEDIA_KEYS,
            "custom-keybindings",
            &format_str_array(&entries),
        ])?;
    }
    let schema = custom_schema(ENTRY);
    let command = shortcut_command(exe, nvidia_only());
    gsettings(&["set", &schema, "name", &format_str("spotcalc")])?;
    gsettings(&["set", &schema, "command", &format_str(&command)])?;
    gsettings(&["set", &schema, "binding", &format_str(key)])?;
    Ok(())
}

/// Human-readable form of a GTK accelerator: `<Alt>space` → `Alt+Space`.
#[must_use]
pub fn display_name(key: &str) -> String {
    let mut name = key
        .replace("<Primary>", "Ctrl+")
        .replace("<Control>", "Ctrl+")
        .replace("<Alt>", "Alt+")
        .replace("<Super>", "Super+")
        .replace("<Shift>", "Shift+");
    if let Some(last) = name.rsplit('+').next().filter(|k| !k.is_empty()) {
        let capitalized = last[..1].to_uppercase() + &last[1..];
        name.truncate(name.len() - last.len());
        name.push_str(&capitalized);
    }
    name
}

/// False for binaries run from a cargo `target/` directory, so a `cargo run` during
/// development never offers to bind the hotkey to a throwaway build.
#[must_use]
pub fn is_installed_binary(exe: &Path) -> bool {
    let parts: Vec<_> = exe.components().map(|c| c.as_os_str()).collect();
    !parts
        .windows(2)
        .any(|w| w[0] == "target" && (w[1] == "debug" || w[1] == "release"))
}

/// The command the shortcut runs. On NVIDIA-only machines it skips loading Mesa's EGL driver,
/// which pulls in ~45 MB of LLVM (see the README's Performance section).
fn shortcut_command(exe: &Path, nvidia_only: bool) -> String {
    let exe = exe.display();
    if nvidia_only {
        format!("env __EGL_VENDOR_LIBRARY_FILENAMES={NVIDIA_EGL} {exe}")
    } else {
        exe.to_string()
    }
}

/// True when NVIDIA's EGL driver is installed and every GPU is NVIDIA (PCI vendor 0x10de).
fn nvidia_only() -> bool {
    if !Path::new(NVIDIA_EGL).exists() {
        return false;
    }
    let vendors: Vec<String> = std::fs::read_dir("/sys/class/drm")
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|card| std::fs::read_to_string(card.path().join("device/vendor")).ok())
        .map(|v| v.trim().to_owned())
        .collect();
    !vendors.is_empty() && vendors.iter().all(|v| v == "0x10de")
}

fn custom_schema(entry: &str) -> String {
    format!("{MEDIA_KEYS}.custom-keybinding:{entry}")
}

fn gsettings(args: &[&str]) -> anyhow::Result<String> {
    let output = Command::new("gsettings")
        .args(args)
        .output()
        .context("running gsettings")?;
    ensure!(
        output.status.success(),
        "gsettings {}: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr).trim()
    );
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// Parses gsettings' GVariant text for a string, e.g. `'<Alt>space'`.
fn parse_str(text: &str) -> Option<String> {
    let inner = text.trim().strip_prefix('\'')?.strip_suffix('\'')?;
    Some(inner.replace("\\'", "'").replace("\\\\", "\\"))
}

/// Parses a GVariant string array, e.g. `['/a/', '/b/']` or `@as []`.
fn parse_str_array(text: &str) -> Vec<String> {
    let text = text.trim().trim_start_matches("@as").trim();
    let Some(inner) = text.strip_prefix('[').and_then(|t| t.strip_suffix(']')) else {
        return Vec::new();
    };
    // Shortcut paths and accelerators never contain commas or quotes.
    inner.split(", ").filter_map(parse_str).collect()
}

fn format_str(s: &str) -> String {
    format!("'{}'", s.replace('\\', "\\\\").replace('\'', "\\'"))
}

fn format_str_array(items: &[String]) -> String {
    let quoted: Vec<String> = items.iter().map(|s| format_str(s)).collect();
    format!("[{}]", quoted.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_gsettings_arrays() {
        assert_eq!(parse_str_array("@as []"), Vec::<String>::new());
        assert_eq!(parse_str_array("['/a/']"), vec!["/a/"]);
        assert_eq!(
            parse_str_array("['<Alt>space', 'XF86Keyboard']"),
            vec!["<Alt>space", "XF86Keyboard"]
        );
    }

    #[test]
    fn string_round_trip() {
        for s in ["<Alt>space", "it's", r"back\slash", ""] {
            assert_eq!(parse_str(&format_str(s)).as_deref(), Some(s));
        }
        assert_eq!(parse_str("unquoted"), None);
    }

    #[test]
    fn array_round_trip() {
        let items = vec!["/a/".to_owned(), "/b/".to_owned()];
        assert_eq!(parse_str_array(&format_str_array(&items)), items);
        assert_eq!(format_str_array(&[]), "[]");
    }

    #[test]
    fn display_names() {
        assert_eq!(display_name("<Alt>space"), "Alt+Space");
        assert_eq!(display_name("<Super>c"), "Super+C");
        assert_eq!(display_name("<Primary><Shift>F2"), "Ctrl+Shift+F2");
    }

    #[test]
    fn dev_builds_are_not_installed_binaries() {
        assert!(is_installed_binary(Path::new("/usr/bin/spotcalc")));
        assert!(is_installed_binary(Path::new(
            "/home/u/.local/bin/spotcalc"
        )));
        assert!(!is_installed_binary(Path::new(
            "/home/u/code/spotcalc/target/debug/spotcalc"
        )));
        assert!(!is_installed_binary(Path::new(
            "/x/target/release/spotcalc"
        )));
    }

    #[test]
    fn command_skips_mesa_only_on_nvidia_only_machines() {
        let exe = Path::new("/usr/bin/spotcalc");
        assert_eq!(shortcut_command(exe, false), "/usr/bin/spotcalc");
        assert_eq!(
            shortcut_command(exe, true),
            format!("env __EGL_VENDOR_LIBRARY_FILENAMES={NVIDIA_EGL} /usr/bin/spotcalc")
        );
    }
}
