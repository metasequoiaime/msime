//! Delivering keys and text to whichever window had focus before a panel opened.
//!
//! Each desktop has its own answer. Wayland compositors hand out no global focus
//! information, so sway is queried over its IPC socket and wtype replays the
//! input; X11 goes through xdotool; Windows keeps a handle to the foreground
//! window. The panels themselves must never take focus, which is why the target
//! is captured before the panel appears and restored after.

#[cfg(target_os = "linux")]
use crate::clipboard_history::write_linux_clipboard;
use crate::*;

#[cfg(target_os = "linux")]
pub(crate) fn focused_sway_container(value: &serde_json::Value) -> Option<u64> {
    if value.get("focused").and_then(serde_json::Value::as_bool) == Some(true) {
        if let Some(id) = value.get("id").and_then(serde_json::Value::as_u64) {
            return Some(id);
        }
    }
    for key in ["nodes", "floating_nodes"] {
        if let Some(nodes) = value.get(key).and_then(serde_json::Value::as_array) {
            for node in nodes {
                if let Some(id) = focused_sway_container(node) {
                    return Some(id);
                }
            }
        }
    }
    None
}

#[cfg(target_os = "linux")]
pub(crate) fn sway_rect_for_container(
    value: &serde_json::Value,
    id: u64,
) -> Option<(f64, f64, f64, f64)> {
    if value.get("id").and_then(serde_json::Value::as_u64) == Some(id) {
        let rect = value.get("rect")?;
        return Some((
            rect.get("x")?.as_f64()?,
            rect.get("y")?.as_f64()?,
            rect.get("width")?.as_f64()?,
            rect.get("height")?.as_f64()?,
        ));
    }
    for key in ["nodes", "floating_nodes"] {
        if let Some(nodes) = value.get(key).and_then(serde_json::Value::as_array) {
            for node in nodes {
                if let Some(rect) = sway_rect_for_container(node, id) {
                    return Some(rect);
                }
            }
        }
    }
    None
}

#[cfg(target_os = "linux")]
pub(crate) fn sway_workspace_for_container(
    value: &serde_json::Value,
    id: u64,
    workspace: Option<(f64, f64, f64, f64)>,
) -> Option<(f64, f64, f64, f64)> {
    let workspace = if value.get("type").and_then(serde_json::Value::as_str) == Some("workspace") {
        value.get("rect").and_then(|rect| {
            Some((
                rect.get("x")?.as_f64()?,
                rect.get("y")?.as_f64()?,
                rect.get("width")?.as_f64()?,
                rect.get("height")?.as_f64()?,
            ))
        })
    } else {
        workspace
    };
    if value.get("id").and_then(serde_json::Value::as_u64) == Some(id) {
        return workspace;
    }
    for key in ["nodes", "floating_nodes"] {
        if let Some(nodes) = value.get(key).and_then(serde_json::Value::as_array) {
            for node in nodes {
                if let Some(rect) = sway_workspace_for_container(node, id, workspace) {
                    return Some(rect);
                }
            }
        }
    }
    None
}

#[cfg(target_os = "linux")]
pub(crate) fn parse_xdotool_geometry(value: &str) -> Option<(f64, f64, f64, f64)> {
    let mut fields = std::collections::HashMap::new();
    for line in value.lines() {
        let (key, value) = line.split_once('=')?;
        fields.insert(key, value.parse::<f64>().ok()?);
    }
    Some((
        *fields.get("X")?,
        *fields.get("Y")?,
        *fields.get("WIDTH")?,
        *fields.get("HEIGHT")?,
    ))
}

#[cfg(target_os = "linux")]
pub(crate) fn panel_position(
    state: &PanelInputState,
    label: &str,
    width: f64,
    height: f64,
) -> Option<tauri::Position> {
    let target = state.0.lock().ok()?.get(label).cloned()?;
    let physical = matches!(&target, PanelInputTarget::X11(_));
    let read = |program: &str, arguments: &[&str], limit: usize| {
        linux_process::read_text(program, arguments, limit, std::time::Duration::from_secs(1))
    };
    let mut logical_workspace = None;
    let rect = match target {
        PanelInputTarget::X11(window) => read(
            "xdotool",
            &["getwindowgeometry", "--shell", window.as_str()],
            4096,
        )
        .and_then(|output| parse_xdotool_geometry(&output)),
        PanelInputTarget::Sway(id) => read("swaymsg", &["-t", "get_tree", "-r"], 1024 * 1024)
            .and_then(|output| serde_json::from_str::<serde_json::Value>(&output).ok())
            .and_then(|tree| {
                logical_workspace = sway_workspace_for_container(&tree, id, None);
                sway_rect_for_container(&tree, id)
            }),
        PanelInputTarget::Wayland | PanelInputTarget::Ydotool | PanelInputTarget::InputMethod => {
            None
        }
    }?;
    if ![rect.0, rect.1, rect.2, rect.3]
        .iter()
        .all(|value| value.is_finite())
        || rect.2 <= 0.0
        || rect.3 <= 0.0
    {
        return None;
    }
    let mut x = rect.0 + (rect.2 - width) / 2.0;
    let mut y = rect.1 + rect.3 + 16.0;
    if let Some((left, top, workspace_width, workspace_height)) = logical_workspace {
        if [left, top, workspace_width, workspace_height]
            .iter()
            .all(|value| value.is_finite())
            && workspace_width > 0.0
            && workspace_height > 0.0
        {
            x = x.clamp(left, left + (workspace_width - width).max(0.0));
            y = y.clamp(top, top + (workspace_height - height).max(0.0));
        }
    }
    if !x.is_finite() || !y.is_finite() {
        return None;
    }
    Some(if physical {
        tauri::Position::Physical(tauri::PhysicalPosition::new(
            x.round() as i32,
            y.round() as i32,
        ))
    } else {
        tauri::Position::Logical(tauri::LogicalPosition::new(x, y))
    })
}

#[cfg(target_os = "linux")]
pub(crate) fn x11_window_is_owned_by_process(pid_output: &str, process_id: u32) -> bool {
    pid_output.trim().parse::<u32>().ok() == Some(process_id)
}

#[cfg(target_os = "linux")]
fn x11_target_is_owned_by_current_process(window: &str) -> bool {
    linux_process::read_text(
        "xdotool",
        &["getwindowpid", window],
        64,
        std::time::Duration::from_secs(1),
    )
    .map(|pid| x11_window_is_owned_by_process(&pid, std::process::id()))
    .unwrap_or(false)
}

#[cfg(target_os = "linux")]
fn capture_panel_input_target() -> Result<PanelInputTarget, HostActionError> {
    let read = |program: &str, arguments: &[&str], limit: usize| {
        linux_process::read_text(program, arguments, limit, std::time::Duration::from_secs(1))
    };
    let sway_target = || {
        let output = read("swaymsg", &["-t", "get_tree", "-r"], 1024 * 1024)?;
        let tree: serde_json::Value = serde_json::from_str(&output).ok()?;
        focused_sway_container(&tree).map(PanelInputTarget::Sway)
    };
    let wayland_session = std::env::var_os("WAYLAND_DISPLAY").is_some()
        || std::env::var("XDG_SESSION_TYPE").as_deref() == Ok("wayland");
    if wayland_session {
        if let Some(target) = sway_target() {
            return Ok(target);
        }
        if read("ydotool", &["type", "--key-delay", "0", ""], 4096).is_some() {
            return Ok(PanelInputTarget::Ydotool);
        }
        // wtype has no --version option. Empty stdin checks the compositor's
        // virtual-keyboard support without emitting any text or key events.
        if read("wtype", &["-"], 4096).is_some() {
            return Ok(PanelInputTarget::Wayland);
        }
    }
    if let Some(output) = read("xdotool", &["getactivewindow"], 64) {
        let id = output.trim();
        if !id.is_empty() && id.bytes().all(|byte| byte.is_ascii_digit()) {
            // Opening an editable panel from the settings window can leave
            // our own Tauri surface focused. Do not capture that window as an
            // editor target: a later paste or virtual key would feed the
            // settings UI instead of the user's application. If the PID
            // probe is unavailable, retain the legacy target so restricted
            // X11 helpers do not make panels unusable.
            let owned = x11_target_is_owned_by_current_process(id);
            if !owned {
                return Ok(PanelInputTarget::X11(id.to_owned()));
            }
        }
    }
    // Do not repeat a failed Sway query in the same Wayland probe sequence.
    if !wayland_session {
        if let Some(target) = sway_target() {
            return Ok(target);
        }
    }
    // GNOME and KDE on Wayland expose neither a virtual keyboard nor a focus query, so without ydotool the only way into the editor is the input method's own connection to it.
    if panel_input_socket().is_some() {
        return Ok(PanelInputTarget::InputMethod);
    }
    Err(HostActionError {
        code: "unavailable",
    })
}

#[cfg(target_os = "linux")]
pub(crate) fn remember_panel_input_target(
    state: &tauri::State<'_, PanelInputState>,
    label: &str,
    replace: bool,
) -> Result<(), HostActionError> {
    let unavailable = || HostActionError {
        code: "unavailable",
    };
    if !replace
        && state
            .0
            .lock()
            .map_err(|_| unavailable())?
            .contains_key(label)
    {
        return Ok(());
    }
    // The probes run session tools with timeouts of their own; send_key and the
    // other panels must not wait on the lock for them.
    let captured = capture_panel_input_target()?;
    let mut target = state.0.lock().map_err(|_| unavailable())?;
    if replace || !target.contains_key(label) {
        target.insert(label.to_owned(), captured);
    }
    Ok(())
}

#[cfg(target_os = "linux")]
pub(crate) fn panel_input_target(
    state: &tauri::State<'_, PanelInputState>,
    label: &str,
) -> Result<PanelInputTarget, HostActionError> {
    state
        .0
        .lock()
        .map_err(|_| HostActionError {
            code: "unavailable",
        })?
        .get(label)
        .cloned()
        .ok_or(HostActionError {
            code: "unavailable",
        })
}

#[cfg(target_os = "linux")]
fn ydotool_key_code(virtual_key: u16) -> Option<u16> {
    let code = match virtual_key {
        0x08 => 14,
        0x09 => 15,
        0x0d => 28,
        0x20 => 57,
        0x2e => 111,
        0x13 => 119,
        0x10 => 42,
        0x11 => 29,
        0x12 => 56,
        0xa1 => 54,
        0xa3 => 97,
        0xa5 => 100,
        0x14 => 58,
        0x1b => 1,
        0x21 => 104,
        0x22 => 109,
        0x23 => 107,
        0x24 => 102,
        0x25 => 105,
        0x26 => 103,
        0x27 => 106,
        0x28 => 108,
        0x2c => 99,
        0x2d => 110,
        0x5b => 125,
        0x5c => 126,
        0x5d => 127,
        0x70..=0x79 => virtual_key - 0x70 + 59,
        0x7a => 87,
        0x7b => 88,
        0x60..=0x69 => [82, 79, 80, 81, 75, 76, 77, 71, 72, 73][(virtual_key - 0x60) as usize],
        0x6a => 55,
        0x6b => 78,
        0x6c => 121,
        0x6d => 74,
        0x6e => 83,
        0x6f => 98,
        0x90 => 69,
        0x91 => 70,
        0xc0 => 41,
        0xbd => 12,
        0xbb => 13,
        0xdb => 26,
        0xdd => 27,
        0xdc => 43,
        0xba => 39,
        0xde => 40,
        0xbc => 51,
        0xbe => 52,
        0xbf => 53,
        0xe2 => 86,
        0x30 => 11,
        0x31 => 2,
        0x32 => 3,
        0x33 => 4,
        0x34 => 5,
        0x35 => 6,
        0x36 => 7,
        0x37 => 8,
        0x38 => 9,
        0x39 => 10,
        0x41 => 30,
        0x42 => 48,
        0x43 => 46,
        0x44 => 32,
        0x45 => 18,
        0x46 => 33,
        0x47 => 34,
        0x48 => 35,
        0x49 => 23,
        0x4a => 36,
        0x4b => 37,
        0x4c => 38,
        0x4d => 50,
        0x4e => 49,
        0x4f => 24,
        0x50 => 25,
        0x51 => 16,
        0x52 => 19,
        0x53 => 31,
        0x54 => 20,
        0x55 => 22,
        0x56 => 47,
        0x57 => 17,
        0x58 => 45,
        0x59 => 21,
        0x5a => 44,
        _ => return None,
    };
    Some(code)
}

#[cfg(target_os = "linux")]
fn run_ydotool(args: &[String]) -> Result<(), HostActionError> {
    let arguments: Vec<&str> = args.iter().map(String::as_str).collect();
    linux_process::run_status("ydotool", &arguments, std::time::Duration::from_secs(3))
        .then_some(())
        .ok_or(HostActionError {
            code: "unavailable",
        })
}

#[cfg(target_os = "linux")]
fn xdotool_key_name(virtual_key: u16) -> Option<String> {
    let name = match virtual_key {
        0x08 => "BackSpace",
        0x09 => "Tab",
        0x0d => "Return",
        0x20 => "space",
        0x2e => "Delete",
        0x13 => "Pause",
        0x10 => "Shift_L",
        0x11 => "Control_L",
        0x12 => "Alt_L",
        0xa1 => "Shift_R",
        0xa3 => "Control_R",
        0xa5 => "Alt_R",
        0x14 => "Caps_Lock",
        0x1b => "Escape",
        0x21 => "Prior",
        0x22 => "Next",
        0x23 => "End",
        0x24 => "Home",
        0x25 => "Left",
        0x26 => "Up",
        0x27 => "Right",
        0x28 => "Down",
        0x2c => "Print",
        0x2d => "Insert",
        0x5b => "Super_L",
        0x5c => "Super_R",
        0x5d => "Menu",
        0x60..=0x69 => return Some(format!("KP_{}", virtual_key - 0x60)),
        0x6a => "KP_Multiply",
        0x6b => "KP_Add",
        0x6c => "KP_Separator",
        0x6d => "KP_Subtract",
        0x6e => "KP_Decimal",
        0x6f => "KP_Divide",
        0x90 => "Num_Lock",
        0x91 => "Scroll_Lock",
        0x70..=0x7b => return Some(format!("F{}", virtual_key - 0x70 + 1)),
        0xc0 => "grave",
        0xbd => "minus",
        0xbb => "equal",
        0xdb => "bracketleft",
        0xdd => "bracketright",
        0xdc => "backslash",
        0xba => "semicolon",
        0xde => "apostrophe",
        0xbc => "comma",
        0xbe => "period",
        0xbf => "slash",
        0xe2 => "less",
        0x30..=0x39 => return char::from_u32(virtual_key as u32).map(|value| value.to_string()),
        0x41..=0x5a => {
            return char::from_u32(virtual_key as u32)
                .map(|value| value.to_ascii_lowercase().to_string());
        }
        _ => return None,
    };
    Some(name.to_owned())
}

#[cfg(target_os = "linux")]
fn xdotool_key_args(request: &KeyboardInputRequest) -> Option<String> {
    let key = xdotool_key_name(request.virtual_key)?;
    let mut parts: Vec<String> = Vec::with_capacity(5);
    if request.include_sticky_modifiers {
        if request.modifiers.ctrl {
            parts.push("ctrl".to_owned());
        }
        if request.modifiers.alt {
            parts.push("alt".to_owned());
        }
        if request.modifiers.win {
            parts.push("super".to_owned());
        }
    }
    if request.shift {
        parts.push("shift".to_owned());
    }
    parts.push(key);
    Some(parts.join("+"))
}

#[cfg(target_os = "linux")]
fn focus_wtype_target(target: &PanelInputTarget) -> Result<(), HostActionError> {
    if let PanelInputTarget::Sway(id) = target {
        let command = format!("[con_id={id}] focus");
        let reply = linux_process::read_text(
            "swaymsg",
            &["-r", &command],
            4096,
            std::time::Duration::from_secs(2),
        )
        .ok_or(HostActionError {
            code: "unavailable",
        })?;
        let results: Vec<serde_json::Value> =
            serde_json::from_str(&reply).map_err(|_| HostActionError {
                code: "unavailable",
            })?;
        if results.is_empty()
            || results.iter().any(|result| {
                result.get("success").and_then(serde_json::Value::as_bool) != Some(true)
            })
        {
            return Err(HostActionError {
                code: "unavailable",
            });
        }
        // A successful command is insufficient when the window disappeared or
        // focus changed. Confirm the actual destination before virtual input.
        let tree = linux_process::read_text(
            "swaymsg",
            &["-t", "get_tree", "-r"],
            1024 * 1024,
            std::time::Duration::from_secs(1),
        )
        .ok_or(HostActionError {
            code: "unavailable",
        })?;
        let tree: serde_json::Value = serde_json::from_str(&tree).map_err(|_| HostActionError {
            code: "unavailable",
        })?;
        if focused_sway_container(&tree) != Some(*id) {
            return Err(HostActionError {
                code: "unavailable",
            });
        }
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn run_wtype(target: &PanelInputTarget, args: &[String]) -> Result<(), HostActionError> {
    if matches!(target, PanelInputTarget::Ydotool) {
        return run_ydotool(args);
    }
    focus_wtype_target(target)?;
    let arguments: Vec<&str> = args.iter().map(String::as_str).collect();
    linux_process::read_text("wtype", &arguments, 64, std::time::Duration::from_secs(3))
        .map(|_| ())
        .ok_or(HostActionError {
            code: "unavailable",
        })
}

#[cfg(target_os = "linux")]
struct PanelFocusRelease {
    windows: Vec<tauri::WebviewWindow>,
}

#[cfg(target_os = "linux")]
impl PanelFocusRelease {
    fn restore(self) {
        // Showing a window does not request focus; this restores a panel that
        // was temporarily hidden for virtual-keyboard injection without
        // stealing the caret back from the external editor.
        for window in self.windows {
            let _ = window.show();
        }
    }
}

#[cfg(target_os = "linux")]
fn release_panel_focus(
    app: &tauri::AppHandle,
    target: &PanelInputTarget,
) -> Result<PanelFocusRelease, HostActionError> {
    if !matches!(
        target,
        PanelInputTarget::Wayland | PanelInputTarget::Ydotool
    ) {
        return Ok(PanelFocusRelease {
            windows: Vec::new(),
        });
    }
    release_focused_panels(app)
}

#[cfg(target_os = "linux")]
const EDITABLE_PANEL_LABELS: [&str; 6] = [
    "handwriting-panel",
    "emoji-panel",
    "clipboard-panel",
    "voice-panel",
    "cloud-clipboard-panel",
    "cloud-dictionary-panel",
];

#[cfg(target_os = "linux")]
fn release_focused_panels(app: &tauri::AppHandle) -> Result<PanelFocusRelease, HostActionError> {
    let windows: Vec<_> = EDITABLE_PANEL_LABELS
        .into_iter()
        .filter_map(|label| app.get_webview_window(label))
        .collect();
    let mut focused = false;
    for window in &windows {
        focused |= window.is_focused().map_err(|_| HostActionError {
            code: "unavailable",
        })?;
    }
    if focused {
        // Hide every editable panel so the compositor cannot focus another one.
        // The screen keyboard never accepts focus and stays available for typing.
        // Annotated because the rollback loop below calls a method on an element
        // before the `push` that would otherwise name the type.
        let mut hidden: Vec<tauri::WebviewWindow> = Vec::with_capacity(EDITABLE_PANEL_LABELS.len());
        for window in windows {
            let visible = window.is_visible().unwrap_or(false);
            if window.hide().is_err() {
                for hidden_window in hidden {
                    let _ = hidden_window.show();
                }
                return Err(HostActionError {
                    code: "unavailable",
                });
            }
            if visible {
                hidden.push(window);
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
        return Ok(PanelFocusRelease { windows: hidden });
    }
    Ok(PanelFocusRelease {
        windows: Vec::new(),
    })
}

#[cfg(target_os = "linux")]
fn with_panel_focus_released<T>(
    app: &tauri::AppHandle,
    target: &PanelInputTarget,
    send: impl FnOnce() -> Result<T, HostActionError>,
) -> Result<T, HostActionError> {
    let release = release_panel_focus(app, target)?;
    let result = send();
    release.restore();
    result
}

// ---- Input method route ----
//
// The MSIME IBus engine and Fcitx5 addon listen on a user-private socket and type into the context they have focused (platforms/linux/src/system/PanelInputChannel.h). That reaches the editor on every session type, including GNOME and KDE on Wayland where no tool can, so it is tried before xdotool, wtype and ydotool, which remain for sessions where another input method is active.

#[cfg(target_os = "linux")]
fn panel_input_socket() -> Option<std::path::PathBuf> {
    discover_session_provider("panel-input.sock")
}

#[cfg(target_os = "linux")]
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ImeReply {
    Ok(Option<u64>),
    Declined,
    Rejected,
}

#[cfg(target_os = "linux")]
#[derive(Debug, PartialEq, Eq)]
enum ImeOutcome {
    Delivered,
    // The host had no focused context, or could not be reached: another route may try.
    Declined,
    // The host explicitly refused the request; a tool route must not bypass it.
    Rejected,
    // The request was sent and no answer came back. The host may still have typed it, so no other route may try, or the text could appear twice.
    Unknown,
}

#[cfg(target_os = "linux")]
pub(crate) fn parse_ime_reply(line: &str) -> Option<ImeReply> {
    let value: serde_json::Value = serde_json::from_str(line.trim()).ok()?;
    match value.get("ok")?.as_bool()? {
        true => Some(ImeReply::Ok(
            value.get("generation").and_then(serde_json::Value::as_u64),
        )),
        false => match value.get("error")?.as_str()? {
            "no_focus" => Some(ImeReply::Declined),
            "restricted" | "invalid" => Some(ImeReply::Rejected),
            _ => None,
        },
    }
}

#[cfg(target_os = "linux")]
fn ime_exchange(
    socket: &std::path::Path,
    request: &serde_json::Value,
) -> Result<ImeReply, ImeOutcome> {
    use std::io::{BufRead, Write};
    let mut stream =
        std::os::unix::net::UnixStream::connect(socket).map_err(|_| ImeOutcome::Declined)?;
    // The host parks a request for up to 700ms while focus moves back to the editor.
    stream
        .set_write_timeout(Some(std::time::Duration::from_millis(500)))
        .and_then(|_| stream.set_read_timeout(Some(std::time::Duration::from_millis(1500))))
        .map_err(|_| ImeOutcome::Declined)?;
    let mut line = request.to_string();
    line.push('\n');
    // The host acts only on a complete line, so a failed write cannot have typed anything.
    stream
        .write_all(line.as_bytes())
        .map_err(|_| ImeOutcome::Declined)?;
    let mut reply = String::new();
    std::io::BufReader::new(std::io::Read::take(&stream, 4096))
        .read_line(&mut reply)
        .map_err(|_| ImeOutcome::Unknown)?;
    parse_ime_reply(&reply).ok_or(ImeOutcome::Unknown)
}

#[cfg(target_os = "linux")]
pub(crate) fn ime_key_request(request: &KeyboardInputRequest) -> Option<serde_json::Value> {
    let key = xdotool_key_name(request.virtual_key)?;
    let sticky = request.include_sticky_modifiers;
    Some(serde_json::json!({
        "op": "key",
        "key": key,
        "keycode": ydotool_key_code(request.virtual_key).unwrap_or(0),
        "shift": request.shift,
        "control": sticky && request.modifiers.ctrl,
        "alt": sticky && request.modifiers.alt,
        "super": sticky && request.modifiers.win,
    }))
}

#[cfg(target_os = "linux")]
fn send_through_input_method(app: &tauri::AppHandle, mut request: serde_json::Value) -> ImeOutcome {
    let Some(socket) = panel_input_socket() else {
        return ImeOutcome::Declined;
    };
    let focused = |window: &tauri::WebviewWindow| window.is_focused().unwrap_or(false);
    let panel_focused = EDITABLE_PANEL_LABELS
        .into_iter()
        .filter_map(|label| app.get_webview_window(label))
        .any(|window| focused(&window));
    // Our own settings window would take the text as readily as any editor. Leave that case to the routes that address the remembered window rather than the focused one.
    let own_window_focused = |app: &tauri::AppHandle| {
        app.webview_windows().into_iter().any(|(label, window)| {
            !EDITABLE_PANEL_LABELS.contains(&label.as_str()) && focused(&window)
        })
    };
    if !panel_focused {
        if own_window_focused(app) {
            return ImeOutcome::Declined;
        }
        return match ime_exchange(&socket, &request) {
            Ok(ImeReply::Ok(_)) => ImeOutcome::Delivered,
            Ok(ImeReply::Declined) => ImeOutcome::Declined,
            Ok(ImeReply::Rejected) => ImeOutcome::Rejected,
            Err(outcome) => outcome,
        };
    }
    // The panel holds the focus, so the input method's focused context is the panel's own web view. Note the focus generation, hide the panels, and ask for a context focused after it.
    let generation = match ime_exchange(&socket, &serde_json::json!({ "op": "generation" })) {
        Ok(ImeReply::Ok(Some(generation))) => generation,
        _ => return ImeOutcome::Declined,
    };
    let Ok(release) = release_focused_panels(app) else {
        return ImeOutcome::Declined;
    };
    let outcome = if own_window_focused(app) {
        ImeOutcome::Declined
    } else {
        request["after_generation"] = generation.into();
        match ime_exchange(&socket, &request) {
            Ok(ImeReply::Ok(_)) => ImeOutcome::Delivered,
            Ok(ImeReply::Declined) => ImeOutcome::Declined,
            Ok(ImeReply::Rejected) => ImeOutcome::Rejected,
            Err(outcome) => outcome,
        }
    };
    release.restore();
    outcome
}

// Runs the input method route and turns its outcome into the answer for the caller: Some when it settled the request, None when the tool routes should try.
#[cfg(target_os = "linux")]
fn settle_through_input_method(
    app: &tauri::AppHandle,
    target: &PanelInputTarget,
    request: serde_json::Value,
) -> Option<Result<(), HostActionError>> {
    match send_through_input_method(app, request) {
        ImeOutcome::Delivered => Some(Ok(())),
        ImeOutcome::Unknown | ImeOutcome::Rejected => Some(Err(HostActionError {
            code: "unavailable",
        })),
        ImeOutcome::Declined if matches!(target, PanelInputTarget::InputMethod) => {
            Some(Err(HostActionError {
                code: "unavailable",
            }))
        }
        ImeOutcome::Declined => None,
    }
}

#[cfg(target_os = "linux")]
fn send_x11_panel_key(window: &str, key: &str) -> Result<(), HostActionError> {
    // Window IDs can be reused after the original editor exits. Re-check the
    // owner immediately before injection so a reused ID cannot target one of
    // this process's own Tauri windows.
    if x11_target_is_owned_by_current_process(window) {
        return Err(HostActionError {
            code: "unavailable",
        });
    }
    // Explicit --window key delivery uses XSendEvent, which many applications
    // reject. Activate first, then use XTEST through the empty window stack.
    // Bound activation as a window manager may decline to focus the target.
    linux_process::read_text(
        "xdotool",
        &["windowactivate", "--sync", window, "key", key],
        64,
        std::time::Duration::from_secs(3),
    )
    .map(|_| ())
    .ok_or(HostActionError {
        code: "unavailable",
    })
}

#[cfg(target_os = "linux")]
pub(crate) fn send_panel_key(
    app: &tauri::AppHandle,
    target: PanelInputTarget,
    request: KeyboardInputRequest,
) -> Result<(), HostActionError> {
    request.validate().map_err(|_| HostActionError {
        code: "invalid_key",
    })?;
    let ime_request = ime_key_request(&request).ok_or(HostActionError {
        code: "invalid_key",
    })?;
    if let Some(result) = settle_through_input_method(app, &target, ime_request) {
        return result;
    }
    if let PanelInputTarget::X11(window) = &target {
        let key = xdotool_key_args(&request).ok_or(HostActionError {
            code: "invalid_key",
        })?;
        return send_x11_panel_key(window, &key);
    }
    if let PanelInputTarget::Ydotool = target {
        let code = ydotool_key_code(request.virtual_key).ok_or(HostActionError {
            code: "invalid_key",
        })?;
        let mut args = Vec::with_capacity(10);
        let mut modifiers = Vec::with_capacity(3);
        if request.include_sticky_modifiers {
            if request.modifiers.ctrl {
                modifiers.push(29u16);
            }
            if request.modifiers.alt {
                modifiers.push(56u16);
            }
            if request.modifiers.win {
                modifiers.push(125u16);
            }
        }
        for modifier in &modifiers {
            args.push(format!("{modifier}:1"));
        }
        if request.shift {
            args.push("42:1".to_owned());
        }
        args.push(format!("{code}:1"));
        args.push(format!("{code}:0"));
        if request.shift {
            args.push("42:0".to_owned());
        }
        for modifier in modifiers.iter().rev() {
            args.push(format!("{modifier}:0"));
        }
        let mut command_args = Vec::with_capacity(args.len() + 1);
        command_args.push("key".to_owned());
        command_args.extend(args);
        return with_panel_focus_released(app, &target, || run_ydotool(&command_args));
    }
    let key = xdotool_key_name(request.virtual_key).ok_or(HostActionError {
        code: "invalid_key",
    })?;
    let mut args = Vec::with_capacity(10);
    if request.include_sticky_modifiers {
        if request.modifiers.ctrl {
            args.extend(["-M".to_owned(), "ctrl".to_owned()]);
        }
        if request.modifiers.alt {
            args.extend(["-M".to_owned(), "alt".to_owned()]);
        }
        if request.modifiers.win {
            args.extend(["-M".to_owned(), "logo".to_owned()]);
        }
    }
    if request.shift {
        args.extend(["-M".to_owned(), "shift".to_owned()]);
    }
    args.extend(["-k".to_owned(), key.to_owned()]);
    with_panel_focus_released(app, &target, || run_wtype(&target, &args))
}

#[cfg(target_os = "linux")]
pub(crate) fn panel_text_requires_clipboard(target: &PanelInputTarget, text: &str) -> bool {
    text.chars()
        .any(|character| matches!(character, '\n' | '\r' | '\t'))
        || ((!text.is_ascii())
            && matches!(target, PanelInputTarget::X11(_) | PanelInputTarget::Ydotool))
}

#[cfg(target_os = "linux")]
fn send_panel_text_to_target(
    app: &tauri::AppHandle,
    target: &PanelInputTarget,
    text: &str,
) -> Result<(), HostActionError> {
    if !text
        .chars()
        .any(|character| matches!(character, '\n' | '\r' | '\t'))
    {
        let request = serde_json::json!({ "op": "text", "text": text });
        if let Some(result) = settle_through_input_method(app, target, request) {
            return result;
        }
    }
    // ydotool types an ASCII key map, while newlines and tabs must remain
    // literal text rather than becoming application shortcuts on any backend.
    let literal_transfer = panel_text_requires_clipboard(target, text);
    if literal_transfer {
        if !write_linux_clipboard(text) {
            return Err(HostActionError {
                code: "unavailable",
            });
        }
        std::thread::sleep(std::time::Duration::from_millis(30));
        return send_panel_ctrl_v(app, target);
    }
    with_panel_focus_released(app, target, || {
        if let PanelInputTarget::X11(window) = target {
            if x11_target_is_owned_by_current_process(window) {
                return Err(HostActionError {
                    code: "unavailable",
                });
            }
            // Use focused XTEST input for applications that reject XSendEvent.
            // --file - reads stdin, keeping the text out of process arguments.
            return linux_process::write_input(
                "xdotool",
                &[
                    "windowactivate",
                    "--sync",
                    window.as_str(),
                    "type",
                    "--delay",
                    "0",
                    "--file",
                    "-",
                ],
                text.as_bytes(),
                std::time::Duration::from_secs(3),
            )
            .then_some(())
            .ok_or(HostActionError {
                code: "unavailable",
            });
        }
        let sent = if matches!(target, PanelInputTarget::Ydotool) {
            // ydotool may hold each ASCII key for 20ms even with key-delay=0.
            // Allow that per-character work while keeping stalls bounded.
            let timeout = std::time::Duration::from_millis(3000 + text.len() as u64 * 30);
            linux_process::write_input(
                "ydotool",
                &["type", "--escape", "0", "--key-delay", "0", "--file", "-"],
                text.as_bytes(),
                timeout,
            )
        } else {
            focus_wtype_target(target)?;
            linux_process::write_input(
                "wtype",
                &["-"],
                text.as_bytes(),
                std::time::Duration::from_secs(3),
            )
        };
        sent.then_some(()).ok_or(HostActionError {
            code: "unavailable",
        })
    })
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
pub(crate) fn record_panel_typing_statistics(
    store: &TypingStatisticsStore,
    text: &str,
    source: TypingSource,
) {
    // One instant for both fields: a day and an hour read separately either side of midnight
    // would file the commit under one day and the other day's hour.
    let now = time::OffsetDateTime::now_local().unwrap_or_else(|_| time::OffsetDateTime::now_utc());
    let date = now.date();
    let day = format!(
        "{:04}-{:02}-{:02}",
        date.year(),
        u8::from(date.month()),
        date.day()
    );
    let _ = store.record(text, source, &day, Some(now.hour()));
}

#[cfg(target_os = "linux")]
pub(crate) async fn send_panel_text(
    app: tauri::AppHandle,
    state: &tauri::State<'_, PanelInputState>,
    typing_statistics: &tauri::State<'_, TypingStatisticsState>,
    label: String,
    text: String,
    source: TypingSource,
) -> Result<(), HostActionError> {
    validate_panel_text(&text)?;
    let target = panel_input_target(state, &label)?;
    deliver_panel_text(app, target, typing_statistics, text, source).await
}

/// Sends text the caller has already validated to one target, off the main thread, and counts it once it arrived.
#[cfg(target_os = "linux")]
async fn deliver_panel_text(
    app: tauri::AppHandle,
    target: PanelInputTarget,
    typing_statistics: &tauri::State<'_, TypingStatisticsState>,
    text: String,
    source: TypingSource,
) -> Result<(), HostActionError> {
    let typing_statistics = typing_statistics.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result = send_panel_text_to_target(&app, &target, &text);
        if result.is_ok() {
            record_panel_typing_statistics(&typing_statistics, &text, source);
        }
        result
    })
    .await
    .map_err(|_| HostActionError {
        code: "unavailable",
    })?
}

#[cfg(target_os = "linux")]
fn validate_panel_text(text: &str) -> Result<(), HostActionError> {
    if text.is_empty()
        || text.len() > 4096
        || msime_client_core::has_disallowed_control_with_options(text, true)
    {
        return Err(HostActionError {
            code: "invalid_text",
        });
    }
    Ok(())
}

/// Sends text from the cloud clipboard panel to the editor captured when this open of the panel began, never to a target another panel or an earlier open left behind.
#[cfg(target_os = "linux")]
pub(crate) async fn send_cloud_clipboard_text(
    app: tauri::AppHandle,
    typing_statistics: &tauri::State<'_, TypingStatisticsState>,
    text: String,
) -> Result<(), HostActionError> {
    validate_panel_text(&text)?;
    let target = cloud_clipboard_input_target(&app).ok_or(HostActionError {
        code: "unavailable",
    })?;
    deliver_panel_text(app, target, typing_statistics, text, TypingSource::Unknown).await
}

#[cfg(target_os = "linux")]
pub(crate) fn send_panel_ctrl_v(
    app: &tauri::AppHandle,
    target: &PanelInputTarget,
) -> Result<(), HostActionError> {
    let request = serde_json::json!({
        "op": "key", "key": "v", "keycode": 47, "control": true,
    });
    if let Some(result) = settle_through_input_method(app, target, request) {
        return result;
    }
    with_panel_focus_released(app, target, || {
        if let PanelInputTarget::X11(window) = target {
            return send_x11_panel_key(window, "ctrl+v");
        }
        if let PanelInputTarget::Ydotool = target {
            return run_ydotool(&[
                "key".to_owned(),
                "29:1".to_owned(),
                "47:1".to_owned(),
                "47:0".to_owned(),
                "29:0".to_owned(),
            ]);
        }
        run_wtype(
            target,
            &[
                "-M".to_owned(),
                "ctrl".to_owned(),
                "-k".to_owned(),
                "v".to_owned(),
            ],
        )
    })
}

#[cfg(target_os = "linux")]
pub(crate) fn send_panel_voice_text(
    app: &tauri::AppHandle,
    target: &PanelInputTarget,
    text: &str,
) -> Result<(), HostActionError> {
    // An independent Tauri panel has no input context of its own, so the text goes to the remembered editor target the way every other panel's text does: through the active MSIME host, or the typing fallbacks when another input method is active. Linux offers no voice commit strategy (see HostCapabilities::voice_commit_mode), so the stored mode is not read here; `tsf` names the default mode only so the shared length and control-character checks run.
    voice_output::submit(text, "tsf", |_, text| {
        send_panel_text_to_target(app, target, text).is_ok()
    })
    .map_err(|error| HostActionError {
        code: match error {
            voice_output::OutputError::InvalidText => "invalid_text",
            voice_output::OutputError::TsfRequiresServer => "tsf_requires_server",
            voice_output::OutputError::Unavailable => "unavailable",
        },
    })
}

// Windows panels are ordinary Tauri windows that never activate, so the host
// injects input on their behalf through the Windows host layer; this shell
// itself stays free of unsafe code.

#[cfg(target_os = "windows")]
pub(crate) fn remember_panel_input_target(
    state: &tauri::State<'_, PanelInputState>,
) -> Result<(), HostActionError> {
    // Editable panels invoke this again after mounting. Do not replace the
    // original editor with our own newly focused webview.
    if !msime_host_windows::foreground_is_external() {
        return state
            .0
            .lock()
            .map_err(|_| HostActionError {
                code: "unavailable",
            })?
            .as_ref()
            .map(|_| ())
            .ok_or(HostActionError {
                code: "unavailable",
            });
    }
    let target = msime_host_windows::foreground_window().ok_or(HostActionError {
        code: "unavailable",
    })?;
    *state.0.lock().map_err(|_| HostActionError {
        code: "unavailable",
    })? = Some(PanelInputTarget(target));
    Ok(())
}

#[cfg(target_os = "windows")]
fn focused_panel_target(state: &tauri::State<'_, PanelInputState>) -> Result<(), HostActionError> {
    let target = state
        .0
        .lock()
        .map_err(|_| HostActionError {
            code: "unavailable",
        })?
        .ok_or(HostActionError {
            code: "unavailable",
        })?;
    focus_panel_target(target)
}

#[cfg(target_os = "windows")]
fn focus_panel_target(target: PanelInputTarget) -> Result<(), HostActionError> {
    msime_host_windows::focus(target.0)
        .then_some(())
        .ok_or(HostActionError {
            code: "unavailable",
        })
}

#[cfg(target_os = "windows")]
pub(crate) fn send_panel_key_windows(
    state: &tauri::State<'_, PanelInputState>,
    request: KeyboardInputRequest,
    keyboard_panel: bool,
) -> Result<(), HostActionError> {
    request.validate().map_err(|_| HostActionError {
        code: "invalid_key",
    })?;
    // The on-screen keyboard follows whatever the user is typing into now, as
    // the reference does with RememberInputTargetWindow on every press. The
    // panel is WS_EX_NOACTIVATE, so the foreground genuinely is the editor;
    // re-focusing the handle captured when the panel opened sent every key to
    // a window the user may have left several clicks ago. Other panels keep
    // their original destination, which is what being edited implies.
    if !keyboard_panel || !msime_host_windows::foreground_is_external() {
        focused_panel_target(state)?;
    }
    // Sticky modifiers only travel with keys the panel marked as inheriting
    // them; shift always applies to the key being sent.
    let sticky = request.include_sticky_modifiers;
    let modifiers = msime_host_windows::Modifiers {
        shift: request.shift,
        ctrl: sticky && request.modifiers.ctrl,
        alt: sticky && request.modifiers.alt,
        win: sticky && request.modifiers.win,
    };
    msime_host_windows::send_key(request.virtual_key, modifiers)
        .then_some(())
        .ok_or(HostActionError {
            code: "unavailable",
        })
}

#[cfg(target_os = "windows")]
pub(crate) fn send_panel_text_windows(
    state: &tauri::State<'_, PanelInputState>,
    text: &str,
) -> Result<(), HostActionError> {
    // Validate before restoring focus. An invalid panel value must not move
    // the user's active editor or otherwise change observable state.
    if !msime_host_windows::valid_text(text) {
        return Err(HostActionError {
            code: "invalid_text",
        });
    }
    focused_panel_target(state)?;
    msime_host_windows::send_text(text)
        .then_some(())
        .ok_or(HostActionError {
            code: "invalid_text",
        })
}

/// Sends text from the cloud clipboard panel to the editor captured when this open of the panel began, never to the target another panel or an earlier open left in [`PanelInputState`].
#[cfg(target_os = "windows")]
pub(crate) fn send_cloud_clipboard_text_windows(
    app: &tauri::AppHandle,
    text: &str,
) -> Result<(), HostActionError> {
    if !msime_host_windows::valid_text(text) {
        return Err(HostActionError {
            code: "invalid_text",
        });
    }
    let target = cloud_clipboard_input_target(app).ok_or(HostActionError {
        code: "unavailable",
    })?;
    focus_panel_target(target)?;
    msime_host_windows::send_text(text)
        .then_some(())
        .ok_or(HostActionError {
            code: "invalid_text",
        })
}

// Panels sit where the native ones did: bottom-centred on the work area, or centred for the panels the shipped product centred.
#[cfg(target_os = "windows")]
pub(crate) fn windows_panel_position(
    width: f64,
    height: f64,
    placement: msime_client_core::host_surface::PanelPlacement,
) -> Option<tauri::Position> {
    use msime_client_core::host_surface::PanelPlacement;
    msime_host_windows::work_area().map(|area| {
        let (x, y) = match placement {
            PanelPlacement::BottomCenter => area.bottom_center(width, height),
            PanelPlacement::Center => area.center(width, height),
        };
        tauri::Position::Physical(tauri::PhysicalPosition::new(
            x.round() as i32,
            y.round() as i32,
        ))
    })
}

// ---- Cloud clipboard input target ----
//
// The cloud clipboard panel types text that came from another device, so it must only ever reach an editor the user was in when they opened it. The shared [`PanelInputState`] cannot promise that: on Windows it is one slot every panel overwrites and that a failed capture leaves as it was, and on Linux a failed capture keeps whatever an earlier open stored for the label. On both, opening the panel from the settings window finds our own window focused, so the slot still names whatever editor some earlier panel captured. The panel therefore keeps its own target, captured for each open and dropped when it closes; without one it can only copy.

#[cfg(any(target_os = "linux", target_os = "windows"))]
pub(crate) const CLOUD_CLIPBOARD_PANEL: &str = "cloud-clipboard-panel";

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[derive(Default)]
pub(crate) struct CloudClipboardInputState(Mutex<FreshInputTarget<PanelInputTarget>>);

/// The editor this open of the cloud clipboard panel may type into, if one was captured.
#[cfg(any(target_os = "linux", target_os = "windows"))]
pub(crate) fn cloud_clipboard_input_target(app: &tauri::AppHandle) -> Option<PanelInputTarget> {
    app.state::<CloudClipboardInputState>()
        .0
        .lock()
        .ok()?
        .target()
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
pub(crate) fn forget_cloud_clipboard_input_target(app: &tauri::AppHandle) {
    if let Ok(mut slot) = app.state::<CloudClipboardInputState>().0.lock() {
        slot.close();
    }
}

/// Remembers the editor a panel that is about to open should type into. Every panel but the cloud clipboard keeps the shared behaviour; the cloud clipboard also captures its own target for this open, keeping it only when it is verifiably another application's window.
#[cfg(target_os = "linux")]
pub(crate) fn remember_opening_panel_target(
    app: &tauri::AppHandle,
    state: &tauri::State<'_, PanelInputState>,
    label: &str,
) {
    if label != CLOUD_CLIPBOARD_PANEL {
        let _ = remember_panel_input_target(state, label, true);
        return;
    }
    let cloud = app.state::<CloudClipboardInputState>();
    let Ok(open) = cloud.0.lock().map(|mut slot| slot.begin_open()) else {
        return;
    };
    // A failed capture leaves the label's previous entry in place, so drop it first: the panel must not be positioned over, or type into, an editor from an earlier open.
    if let Ok(mut targets) = state.0.lock() {
        targets.remove(label);
    }
    let _ = remember_panel_input_target(state, label, true);
    let captured = panel_input_target(state, label)
        .ok()
        .filter(linux_target_is_external);
    let Ok(mut slot) = cloud.0.lock() else {
        return;
    };
    slot.record(open, captured);
}

/// Whether a captured target is provably a window of another process. Only X11 and sway name a window whose owner can be checked; ydotool, wtype and the input method type into whatever holds focus when the text is sent, which from a panel opened in the settings window is that window, so they are never treated as an external editor.
#[cfg(target_os = "linux")]
fn linux_target_is_external(target: &PanelInputTarget) -> bool {
    let read = |program: &str, arguments: &[&str], limit: usize| {
        linux_process::read_text(program, arguments, limit, std::time::Duration::from_secs(1))
    };
    let owner = match target {
        PanelInputTarget::X11(window) => read("xdotool", &["getwindowpid", window], 64)
            .and_then(|output| output.trim().parse::<u32>().ok()),
        PanelInputTarget::Sway(id) => read("swaymsg", &["-t", "get_tree", "-r"], 1024 * 1024)
            .and_then(|output| serde_json::from_str::<serde_json::Value>(&output).ok())
            .and_then(|tree| sway_pid_for_container(&tree, *id)),
        PanelInputTarget::Ydotool | PanelInputTarget::Wayland | PanelInputTarget::InputMethod => {
            None
        }
    };
    owner.is_some_and(|pid| pid != std::process::id())
}

/// The process that owns a sway container, from the `pid` sway reports for each view.
#[cfg(target_os = "linux")]
pub(crate) fn sway_pid_for_container(value: &serde_json::Value, id: u64) -> Option<u32> {
    if value.get("id").and_then(serde_json::Value::as_u64) == Some(id) {
        return value
            .get("pid")
            .and_then(serde_json::Value::as_u64)
            .and_then(|pid| u32::try_from(pid).ok());
    }
    ["nodes", "floating_nodes"]
        .into_iter()
        .filter_map(|key| value.get(key).and_then(serde_json::Value::as_array))
        .flatten()
        .find_map(|node| sway_pid_for_container(node, id))
}

/// Remembers the editor a panel that is about to open should type into. Every panel keeps the shared behaviour; the cloud clipboard also captures its own target for this open, and only when the foreground window belongs to another process.
#[cfg(target_os = "windows")]
pub(crate) fn remember_opening_panel_target(
    app: &tauri::AppHandle,
    state: &tauri::State<'_, PanelInputState>,
    label: &str,
) {
    let _ = remember_panel_input_target(state);
    if label != CLOUD_CLIPBOARD_PANEL {
        return;
    }
    let cloud = app.state::<CloudClipboardInputState>();
    let Ok(mut slot) = cloud.0.lock() else {
        return;
    };
    let open = slot.begin_open();
    let target = msime_host_windows::foreground_is_external()
        .then(msime_host_windows::foreground_window)
        .flatten()
        .map(PanelInputTarget);
    slot.record(open, target);
}
