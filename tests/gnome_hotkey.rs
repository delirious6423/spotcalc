//! End-to-end check of hotkey setup against real `gsettings` with GNOME's schemas.
//!
//! Never touches your real settings: it refuses to run unless gsettings is pointed at a
//! throwaway keyfile. Run with:
//!
//! ```sh
//! GSETTINGS_BACKEND=keyfile XDG_CONFIG_HOME=$(mktemp -d) cargo test --test gnome_hotkey -- --ignored
//! ```

use std::path::Path;

use spotcalc::hotkey::{self, DEFAULT_KEY, Status};

#[test]
#[ignore = "needs GNOME schemas and a throwaway GSETTINGS_BACKEND=keyfile; see module docs"]
fn binds_and_detects_the_hotkey() {
    assert_eq!(
        std::env::var("GSETTINGS_BACKEND").as_deref(),
        Ok("keyfile"),
        "refusing to touch real settings; see this file's docs"
    );
    let config = std::env::var("XDG_CONFIG_HOME").expect("XDG_CONFIG_HOME set to a temp dir");
    assert!(
        config.starts_with("/tmp"),
        "XDG_CONFIG_HOME should be a temp dir"
    );

    // Fresh GNOME defaults: Alt+Space belongs to the window menu.
    assert_eq!(
        hotkey::status(),
        Status::Unbound {
            replaces_window_menu: true
        }
    );

    hotkey::bind(DEFAULT_KEY, Path::new("/usr/bin/spotcalc")).expect("bind");
    assert_eq!(hotkey::status(), Status::Bound(DEFAULT_KEY.to_owned()));

    // Binding again is idempotent.
    hotkey::bind(DEFAULT_KEY, Path::new("/usr/bin/spotcalc")).expect("rebind");
    assert_eq!(hotkey::status(), Status::Bound(DEFAULT_KEY.to_owned()));
}
