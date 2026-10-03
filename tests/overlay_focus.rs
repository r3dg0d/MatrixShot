//! Lock layer-shell keyboard focus for the pointer-only capture overlays.
//! Reads the Quickshell source; no compositor session.

use std::fs;
use std::path::PathBuf;

fn shell_source() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("quickshell/shell.qml");
    fs::read_to_string(&path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()))
}

/// Property block of a `PanelWindow` before its first nested object.
fn panel_headers(source: &str) -> Vec<String> {
    let mut headers = Vec::new();
    let mut rest = source;
    while let Some(at) = rest.find("PanelWindow {") {
        let body = &rest[at + "PanelWindow".len()..];
        let open = body.find('{').expect("PanelWindow brace");
        let mut depth = 0;
        let mut header_end = None;
        for (index, ch) in body[open..].char_indices() {
            match ch {
                '{' => {
                    if depth == 0 {
                        depth = 1;
                    } else {
                        header_end = Some(open + index);
                        break;
                    }
                }
                '}' => panic!("PanelWindow closed before a child or property block"),
                _ => {}
            }
        }
        let header_end = header_end.expect("PanelWindow header");
        headers.push(body[open..header_end].to_string());
        rest = &body[open + 1..];
    }
    headers
}

fn focus_of(headers: &[String], id: &str) -> String {
    let needle = format!("id: {id}");
    let header = headers
        .iter()
        .find(|header| header.lines().any(|line| line.trim() == needle))
        .unwrap_or_else(|| panic!("missing PanelWindow id {id}"));
    let focuses: Vec<_> = header
        .lines()
        .filter_map(|line| {
            line.trim()
                .strip_prefix("WlrLayershell.keyboardFocus:")
                .map(str::trim)
        })
        .collect();
    assert_eq!(
        focuses.len(),
        1,
        "{id} must set keyboard focus exactly once in its own property block, got {focuses:?}"
    );
    focuses[0].trim_end_matches(';').to_string()
}

#[test]
fn pointer_only_overlays_do_not_take_keyboard_focus() {
    let headers = panel_headers(&shell_source());
    assert!(
        headers.len() >= 2,
        "expected chooser and record-config windows, parsed {}",
        headers.len()
    );
    assert_eq!(
        focus_of(&headers, "chooserWin"),
        "WlrKeyboardFocus.None",
        "chooser must stay pointer-only"
    );
    assert_eq!(
        focus_of(&headers, "recCfgWin"),
        "WlrKeyboardFocus.None",
        "screen-record config must not move keyboard focus"
    );
}
