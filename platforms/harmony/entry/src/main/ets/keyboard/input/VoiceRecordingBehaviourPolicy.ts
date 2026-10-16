/**
 * 录音行为的几个开关在某一刻是否生效。
 *
 * 提示音只认总开关 `sound_enabled`（设置页的「提示音」）：开始、结束提示音都跟着它。存下的 `start_sound`、`end_sound` 不再读，设置页也不再给这两个开关（`voice_cue_switches` 为 false），与 macOS、Windows 一致；以前单独关掉其中一个的人会重新听到它，这是有意的，不留一个看不见却仍然生效的值。
 *
 * 提示音默认开，静音其他声音默认关，与共享 schema 一致。字段缺失说明是旧文档而不是用户的选择，所以取默认值，而不是看起来最安全的值。
 */
import { HttpAsrConfigurationPolicy } from "./HttpAsrConfigurationPolicy";
import { VoiceInputConfiguration } from "./VoiceInputConfiguration";

export class VoiceRecordingBehaviourPolicy {
  static playsStartTone(voice: VoiceInputConfiguration): boolean {
    return VoiceRecordingBehaviourPolicy.sounds(voice);
  }

  static playsEndTone(voice: VoiceInputConfiguration): boolean {
    return VoiceRecordingBehaviourPolicy.sounds(voice);
  }

  /**
   * Whether a partial result should reach the panel before the recognizer is finished.
   *
   * Off unless asked for, which is the shared default. A stream of interim guesses rewriting itself
   * is useful when you want to see the recognizer working and distracting when you do not, so the
   * panel stays empty until the final result unless the user said otherwise.
   */
  static showsInterimResults(voice: VoiceInputConfiguration): boolean {
    return voice.stream_inline_preedit === true;
  }

  /**
   * 服务商是否在用户还在说话时就返回文字，正是这一点让停顿能自行结束录音。兼容 OpenAI 的上传式服务商只在录音停止后才应答，因此需要显式的停止控件；端侧模型、豆包流式和系统识别器都是流式的，与 Android 键盘内的 `LOCAL` 和 `STREAMING` 引擎一致。
   */
  static streamsPartialResults(voice: VoiceInputConfiguration): boolean {
    return !HttpAsrConfigurationPolicy.supported(voice.asr_provider);
  }

  /** Off unless asked for: taking the audio session away from whatever is playing is intrusive. */
  static quietensOthers(voice: VoiceInputConfiguration): boolean {
    return voice.mute_system_audio === true;
  }

  private static sounds(voice: VoiceInputConfiguration): boolean {
    return voice.sound_enabled !== false;
  }
}
