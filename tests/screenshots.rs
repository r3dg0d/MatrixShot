//! Exercise the CLI with fake capture tools; no compositor or user config needed.
#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new(clipboard: bool) -> Self {
        let root = std::env::temp_dir().join(format!(
            "matrixshot-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("bin")).unwrap();
        fs::create_dir_all(root.join("config/matrixshot")).unwrap();
        // Paths are passed through the environment, so spaces are exercised too.
        let config = format!(
            "[screenshot]\nsave_directory = {:?}\ncopy_to_clipboard = {clipboard}\n[preview]\nenabled = false\n",
            root.join("capture files").to_str().unwrap()
        );
        fs::write(root.join("config/matrixshot/config.toml"), config).unwrap();
        let fixture = Self(root);
        fixture.script(
            "grim",
            "if [ \"$1\" = -g ]; then shift 2; fi\nprintf image > \"$1\"\n",
        );
        fixture
    }

    fn script(&self, name: &str, body: &str) {
        let path = self.0.join("bin").join(name);
        fs::write(&path, format!("#!/bin/sh\n{body}")).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_matrixshot"))
            .args(args)
            .env("HOME", &self.0)
            .env("XDG_CONFIG_HOME", self.0.join("config"))
            .env("XDG_STATE_HOME", self.0.join("state"))
            .env("PATH", self.0.join("bin"))
            .env("CLIPBOARD_MARKER", self.0.join("clipboard-called"))
            .output()
            .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn captures_with_clipboard_disabled() {
    for args in [
        vec!["fullscreen"],
        vec!["region", "--geometry", "10,20 30x40"],
    ] {
        let fixture = Fixture::new(false);
        // Any attempt to invoke clipboard copying is observable.
        fixture.script("wl-copy", "printf called > \"$CLIPBOARD_MARKER\"\nexit 1\n");
        let output = fixture.run(&args);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let path = PathBuf::from(String::from_utf8(output.stdout).unwrap().trim());
        assert_eq!(fs::read(&path).unwrap(), b"image");
        assert!(!fixture.0.join("clipboard-called").exists());
        assert!(fs::read_to_string(fixture.0.join("state/matrixshot/last"))
            .unwrap()
            .contains(path.to_str().unwrap()));
    }
}

#[test]
fn clipboard_failure_warns_and_preserves_capture() {
    for args in [
        vec!["fullscreen"],
        vec!["region", "--geometry", "10,20 30x40"],
    ] {
        let fixture = Fixture::new(true);
        fixture.script("wl-copy", "exit 7\n");
        let output = fixture.run(&args);
        assert!(output.status.success());
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains("clipboard copy failed"), "{stderr}");
        let path = PathBuf::from(String::from_utf8(output.stdout).unwrap().trim());
        assert_eq!(fs::read(path).unwrap(), b"image");
        assert!(fixture.0.join("state/matrixshot/last").is_file());
    }
}

#[test]
fn failed_capture_does_not_update_last() {
    let fixture = Fixture::new(false);
    fixture.script("grim", "exit 1\n");
    let output = fixture.run(&["fullscreen"]);
    assert!(!output.status.success());
    assert!(!fixture.0.join("state/matrixshot/last").exists());
}
