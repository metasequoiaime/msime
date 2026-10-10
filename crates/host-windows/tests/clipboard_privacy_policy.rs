// 不碰剪贴板，在任何主机上检验设置应用侧的隐私判定，并对照 Server 侧的同一份格式名单。
#[path = "../src/clipboard_privacy.rs"]
mod clipboard_privacy;

use clipboard_privacy::{
    ClipboardPermission, ClipboardPrivacyMarkers, CLOUD_PERMISSION_FORMAT, EXCLUDE_MONITOR_FORMAT,
    HISTORY_PERMISSION_FORMAT, VIEWER_IGNORE_FORMAT, VOICE_CLIPBOARD_MARKERS,
};

const SERVER_POLICY: &str =
    include_str!("../../../platforms/windows/src/clipboard/ClipboardPrivacyPolicy.h");
const SERVER_VOICE_POLICY: &str =
    include_str!("../../../platforms/windows/src/voice/VoiceCommitPolicy.h");

/// Server 原生语音 ctrl_v 写入的格式名单（`voice_clipboard_markers` 数组里的每一项）。
fn server_voice_markers() -> Vec<String> {
    let start = SERVER_VOICE_POLICY
        .find("voice_clipboard_markers[] = {")
        .expect("VoiceCommitPolicy.h declares voice_clipboard_markers");
    let body = &SERVER_VOICE_POLICY[start..];
    let body = &body[..body.find("};").expect("the marker array is closed")];
    body.lines()
        .filter_map(|line| {
            let line = line.trim();
            let name = line.strip_prefix("{L\"")?;
            let (name, rest) = name.split_once('"')?;
            assert_eq!(
                rest.trim(),
                ", 0},",
                "every voice marker is DWORD 0: {line}"
            );
            Some(name.to_owned())
        })
        .collect()
}

#[test]
fn panel_voice_paste_marks_the_same_formats_as_the_server() {
    // 共享语音面板的 ctrl_v 与 Server 原生 ctrl_v 必须写同一份隐私标记，否则同一段识别文本走面板就会进剪贴板历史。
    assert_eq!(
        VOICE_CLIPBOARD_MARKERS.map(str::to_owned).to_vec(),
        server_voice_markers()
    );
    // 名单里的每一项都是剪贴板历史采集会据以跳过样本的标记。
    for name in VOICE_CLIPBOARD_MARKERS {
        assert!(SERVER_POLICY.contains(&format!("L\"{name}\"")), "{name}");
    }
}

#[test]
fn format_names_match_the_server_monitor() {
    for name in [
        EXCLUDE_MONITOR_FORMAT,
        VIEWER_IGNORE_FORMAT,
        HISTORY_PERMISSION_FORMAT,
        CLOUD_PERMISSION_FORMAT,
    ] {
        assert!(
            SERVER_POLICY.contains(&format!("L\"{name}\"")),
            "{name} is missing from ClipboardPrivacyPolicy.h"
        );
    }
}

#[test]
fn permission_dword_zero_denies_and_unreadable_denies() {
    assert_eq!(
        ClipboardPermission::from_data(Some(&0u32.to_le_bytes())),
        ClipboardPermission::Denied
    );
    assert_eq!(
        ClipboardPermission::from_data(Some(&1u32.to_le_bytes())),
        ClipboardPermission::Allowed
    );
    assert_eq!(
        ClipboardPermission::from_data(None),
        ClipboardPermission::Denied
    );
    assert_eq!(
        ClipboardPermission::from_data(Some(&[1, 0])),
        ClipboardPermission::Denied
    );
}

#[test]
fn any_privacy_marker_excludes_the_sample() {
    assert!(!ClipboardPrivacyMarkers::default().excluded());
    for markers in [
        ClipboardPrivacyMarkers {
            exclude_from_monitor: true,
            ..Default::default()
        },
        ClipboardPrivacyMarkers {
            viewer_ignore: true,
            ..Default::default()
        },
        ClipboardPrivacyMarkers {
            history: ClipboardPermission::Denied,
            ..Default::default()
        },
        ClipboardPrivacyMarkers {
            cloud: ClipboardPermission::Denied,
            ..Default::default()
        },
    ] {
        assert!(markers.excluded(), "{markers:?}");
    }
    assert!(!ClipboardPrivacyMarkers {
        history: ClipboardPermission::Allowed,
        cloud: ClipboardPermission::Allowed,
        ..Default::default()
    }
    .excluded());
}
