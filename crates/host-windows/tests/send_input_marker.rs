// 设置应用注入面板文字时带的标记必须与 tip 跳过直通统计时比对的数值一致，否则同一段文字会被记两次。
#[path = "../src/send_input_marker.rs"]
mod send_input_marker;

use send_input_marker::PANEL_TEXT_SENDINPUT_EXTRA_INFO;

const TIP_POLICY: &str = include_str!("../../../platforms/windows/tsf/IPC/PassthroughStatistics.h");
const TIP_SELF_MARKERS: &str = include_str!("../../../platforms/windows/tsf/IME/MetasequoiaIME.h");

#[test]
fn panel_marker_matches_the_tip() {
    let literal = format!("PanelTextSendInputExtraInfo = {PANEL_TEXT_SENDINPUT_EXTRA_INFO:#X}u;");
    assert!(
        TIP_POLICY.contains(&literal),
        "{literal} is missing from PassthroughStatistics.h"
    );
}

#[test]
fn panel_marker_is_not_one_of_the_tip_self_markers() {
    // tip 对自注入标记整键放行；面板文字若误用其中一个，按键处理也会跟着变。
    let literal = format!("{PANEL_TEXT_SENDINPUT_EXTRA_INFO:#X}u");
    assert!(!TIP_SELF_MARKERS.contains(&literal));
}
