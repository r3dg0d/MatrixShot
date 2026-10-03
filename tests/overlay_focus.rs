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

fn keyboard_focus_expr(headers: &[String], id: &str) -> String {
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

/// Idle branch of `cond ? taken : idle`, or the whole expression when it is constant.
fn idle_keyboard_focus(expr: &str) -> String {
    let expr = expr.trim().trim_end_matches(';').trim();
    match expr.split_once('?') {
        Some((cond, rest)) => {
            let (taken, idle) = rest
                .split_once(':')
                .unwrap_or_else(|| panic!("keyboardFocus ternary missing else: {expr}"));
            assert!(!cond.trim().is_empty(), "empty keyboardFocus condition");
            assert!(!taken.trim().is_empty(), "empty keyboardFocus taken branch");
            assert_ne!(
                cond.trim(),
                "true",
                "keyboardFocus must not sit on the editing branch"
            );
            idle.trim().to_string()
        }
        None => expr.to_string(),
    }
}

fn object_with_id(source: &str, id: &str) -> String {
    let needle = format!("id: {id}");
    let id_at = source
        .find(&needle)
        .unwrap_or_else(|| panic!("missing id {id}"));
    let open = source[..id_at]
        .rfind('{')
        .unwrap_or_else(|| panic!("{id} has no opening brace"));
    let mut depth = 0;
    for (index, ch) in source[open..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return source[open..=open + index].to_string();
                }
            }
            _ => {}
        }
    }
    panic!("{id} object never closed");
}

#[test]
fn pointer_only_overlays_do_not_take_keyboard_focus() {
    let source = shell_source();
    let headers = panel_headers(&source);
    assert!(
        headers.len() >= 2,
        "expected chooser and record-config windows, parsed {}",
        headers.len()
    );

    let chooser = keyboard_focus_expr(&headers, "chooserWin");
    assert_eq!(
        chooser, "WlrKeyboardFocus.None",
        "chooser must stay pointer-only with no editing exception"
    );
    assert_eq!(idle_keyboard_focus(&chooser), "WlrKeyboardFocus.None");

    let record = keyboard_focus_expr(&headers, "recCfgWin");
    assert_eq!(
        idle_keyboard_focus(&record),
        "WlrKeyboardFocus.None",
        "screen-record config must idle at pointer-only, got {record}"
    );
    assert!(
        !record.contains("OnDemand"),
        "recCfgWin must not sit at OnDemand (that steals the capture target): {record}"
    );
    assert!(
        record.starts_with("recPathInput.activeFocus ? "),
        "keyboard focus may be taken only while the path field is editing, got {record}"
    );
    let taken = record
        .split_once('?')
        .and_then(|(_, rest)| rest.split_once(':'))
        .map(|(taken, _)| taken.trim())
        .unwrap_or("");
    assert_eq!(
        taken, "WlrKeyboardFocus.Exclusive",
        "a mapped None to OnDemand change does not take Hyprland keys; Exclusive is the editing exception"
    );

    let header = headers
        .iter()
        .find(|header| header.lines().any(|line| line.trim() == "id: recCfgWin"))
        .expect("recCfgWin header");
    assert!(
        header
            .lines()
            .any(|line| line.trim() == "onVisibleChanged: if (!visible) recPathInput.focus = false"),
        "hiding the record config must end the path edit"
    );

    let path_at = source.find("id: recPathInput").expect("recPathInput");
    let before_path = source[..path_at].trim_end();
    assert!(
        before_path
            .trim_end_matches('{')
            .trim_end()
            .ends_with("TextInput"),
        "output path must stay a TextInput"
    );
    let path = object_with_id(&source, "recPathInput");
    assert!(
        path.contains("onAccepted: focus = false"),
        "Enter must release the path field: {path}"
    );
    assert!(
        path.contains("Keys.onEscapePressed: focus = false"),
        "Escape must release the path field: {path}"
    );
}
