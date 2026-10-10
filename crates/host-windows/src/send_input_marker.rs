//! 面板注入文字时 `SendInput` 带的 `dwExtraInfo` 标记。
//!
//! 设置应用在 `send_text` 成功后自己按来源（手写、本地输入、语音）记打字统计；水杉的 tip 看到带这个标记的按键就不再把它当作直通字符再记一次（`platforms/windows/tsf/IPC/PassthroughStatistics.h` 的 `PanelTextSendInputExtraInfo`，两处数值必须一致）。不碰 Win32，`tests/send_input_marker.rs` 在任何主机上对照两处数值。

/// "MSPS"，与 tip 自注入标记 0x4D53505x 一族相邻、互不重叠。
pub const PANEL_TEXT_SENDINPUT_EXTRA_INFO: usize = 0x4D53_5053;
