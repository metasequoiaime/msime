#pragma once

#include <string>

namespace msime::windows {
void configure_audio_mute_state_path(std::wstring path);
void mute_other_system_audio();
void restore_other_system_audio();
// 控制线程在静音期间每轮调用。录音中默认输出设备换了（插上耳机、切换播放设备）时，把新的默认设备上其他应用的声音也静音，并监听它上面新建的会话；之前设备上已静音的会话照常记录，restore 时一并恢复。对应 macOS VoiceAudioMuter 跟随默认输出设备的行为。没有变化时只读一个原子标记。
void follow_default_system_audio_output();
} // namespace msime::windows
