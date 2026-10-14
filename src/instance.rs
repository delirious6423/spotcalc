//! Single-instance toggle: pressing the hotkey while spotcalc is open closes it.
//!
//! Wayland doesn't let apps grab global hotkeys, so the hotkey (a GNOME custom shortcut)
//! just runs `spotcalc`. The first instance listens on a Unix socket; a second launch
//! connects to it, which tells the open window to close, and then exits itself.

use std::io;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};

/// Outcome of trying to become the running instance.
pub enum Instance {
    /// No other instance was running; we own the socket.
    Primary(UnixListener),
    /// Another instance was running and has been asked to close.
    AlreadyRunning,
}

/// Socket in `$XDG_RUNTIME_DIR` (private to the user), falling back to the temp dir.
#[must_use]
pub fn socket_path() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map_or_else(std::env::temp_dir, PathBuf::from)
        .join("spotcalc.sock")
}

/// Connects to a running instance or, failing that, starts listening.
///
/// # Errors
/// Fails if the socket can be neither reached nor created.
pub fn acquire(path: &Path) -> io::Result<Instance> {
    if UnixStream::connect(path).is_ok() {
        return Ok(Instance::AlreadyRunning);
    }
    // Nobody answered, so any socket file left is stale (e.g. after a crash).
    match std::fs::remove_file(path) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
        _ => {}
    }
    UnixListener::bind(path).map(Instance::Primary)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_launch_reaches_first() {
        let path = std::env::temp_dir().join(format!("spotcalc-test-{}.sock", std::process::id()));
        let Ok(Instance::Primary(listener)) = acquire(&path) else {
            panic!("first launch should be primary");
        };
        assert!(matches!(acquire(&path), Ok(Instance::AlreadyRunning)));
        assert!(
            listener.accept().is_ok(),
            "primary sees the toggle connection"
        );
        drop(listener);
        // Stale socket file from a dead instance is replaced.
        assert!(matches!(acquire(&path), Ok(Instance::Primary(_))));
        std::fs::remove_file(&path).expect("cleanup");
    }
}
